#!/usr/bin/env node
/* VALIDATE hook (Stop / SubagentStop) — the loop-closer.

   Faithful Node (CommonJS, stdlib-only) port of validate.py. Refuses to FINISH while a checkable rule
   is still violated, OR while the agent never *credibly* declared it applied its required expertise.
   Three enforcement layers: (1) checkable-rule offenders in produced files; (2) evidence-carrying
   declaration integrity (real rule-ids, coverage floor, evidence spot-check); (3) artifact spot-check.
   Fail-closed; guards an infinite block loop via `stop_hook_active`. Self-guards on the active agent.

   argv (historical): <glob_root> <agent_name>  (mixed with an optional --main-agent <name> flag).
*/
"use strict";
const fs = require("fs");
const path = require("path");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

const BANNED_PHRASE = /chain[\s\-]?of[\s\-]?thought/i;
const CREDS = [
  /AKIA[0-9A-Z]{16}/,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  /\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['"]?[^\s'"]{6,}/i,
];
const LINE_BUDGET = 200;
const BUDGETED = /(persona|behavior)\.md$|(^|\/)CLAUDE\.md$/i;

// Evidence-carrying declaration: `APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>` (one rule per line).
const DECL = /APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*#\s*([a-z]+-\d+)\b[ \t]*(?:[—:\-]+[ \t]*(.*?))?[ \t]*$/gim;
// Bare `APPLIED-EXPERTISE: <name>` with no `#<rule-id>` — the gameable form we now reject.
const BARE = /APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*(?:$|[^#\w])/gim;
// A path-like token inside evidence (must resolve to a real file).
const PATHISH = /[\w.\/\\-]+\.(?:md|jsonc?|toml|ya?ml|txt|py|js|ts|rs|cfg|ini)/i;
// A quoted span inside evidence (must appear in a produced artifact).
const QUOTED = /"([^"]{8,})"|`([^`]{8,})`|'([^']{8,})'/;

const FLOOR = 3; // min distinct real rule-ids per required expertise (or all checkable, if fewer)

const HOOK_DIR = __dirname;
// READ-ONLY reference data (manifests + required.json) ships NEXT TO the hooks — hook-relative.
const REQUIRED_JSON = path.join(HOOK_DIR, "..", "expertise", "required.json");
const MANIFEST_DIR = path.join(HOOK_DIR, "..", "expertise", "manifests");
// WRITABLE runtime data -> the PROJECT's .genesis/ (cwd-derived).
const LOG = path.join(agent_ident.runtime_dir(), "hook-decisions.log");

const ARTIFACT_GLOBS = ["*persona.md", "*behavior.md", "CLAUDE.md", ".claude/agents/*.md",
                        "*persona.spec.json", "*.tests.json"];

function readStdin() {
  try {
    return fs.readFileSync(0, "utf8");
  } catch (e) {
    if (e && e.code === "EAGAIN") {
      const buf = Buffer.alloc(65536);
      const chunks = [];
      for (;;) {
        let n;
        try { n = fs.readSync(0, buf, 0, buf.length, null); }
        catch (e2) {
          if (e2 && e2.code === "EAGAIN") continue;
          if (e2 && e2.code === "EOF") break;
          return "";
        }
        if (!n) break;
        chunks.push(Buffer.from(buf.slice(0, n)));
      }
      return Buffer.concat(chunks).toString("utf8");
    }
    return "";
  }
}

function emit(obj) { fs.writeSync(1, JSON.stringify(obj) + "\n"); }
function nowIso() { return new Date().toISOString().slice(0, 19) + "+00:00"; }
function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }
function readText(p) { return fs.readFileSync(p, "utf8"); }

function log(agent, decision, reasons, cited, session) {
  cited = cited || {};
  session = session || "";
  try {
    fs.mkdirSync(path.dirname(LOG), { recursive: true });
    const rec = { ts: nowIso(), hook: "validate", agent: agent, session: session,
                  decision: decision, reasons: reasons, cited: cited };
    fs.appendFileSync(LOG, JSON.stringify(rec) + "\n", { encoding: "utf8" });
  } catch (e) { /* logging must never break the gate */ }
}

// ---- recursive artifact glob (replicates glob.glob(join(root,"**",name), recursive=True)) ----
function segToRegex(seg) {
  let re = "";
  for (const ch of seg) {
    if (ch === "*") re += "[^/]*";
    else if (ch === "?") re += "[^/]";
    else re += ch.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }
  const guard = seg.startsWith(".") ? "" : "(?!\\.)"; // '*' does not match a leading dot (glob semantics)
  return new RegExp("^" + guard + re + "$");
}

function pyjoin(a, b) {
  // Faithful to Python os.path.join for glob inputs: does NOT collapse a leading "./" the way
  // Node's path.join does, so produced paths match Python glob output (e.g. "./CLAUDE.md", not
  // "CLAUDE.md"). Uses path.sep so the platform separator matches Python's os.sep on Win/POSIX.
  const sep = path.sep;
  if (a === "") return b;
  if (a.endsWith(sep) || a.endsWith("/")) return a + b;
  return a + sep + b;
}

function reachableDirs(root) {
  // every dir reachable from root by descending only through NON-hidden subdirs (incl. root itself),
  // replicating that Python's `**` does not descend into dot-directories.
  const dirs = [];
  (function walk(d) {
    dirs.push(d);
    let entries;
    try { entries = fs.readdirSync(d, { withFileTypes: true }); } catch (e) { return; }
    for (const e of entries) {
      if (e.isDirectory() && !e.name.startsWith(".")) walk(pyjoin(d, e.name));
    }
  })(root);
  return dirs;
}

function matchSegs(dir, segs, idx, out) {
  const re = segToRegex(segs[idx]);
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch (e) { return; }
  const last = idx === segs.length - 1;
  for (const e of entries) {
    if (!re.test(e.name)) continue;
    if (last) {
      if (e.isFile()) out.push(pyjoin(dir, e.name));
    } else if (e.isDirectory()) {
      matchSegs(pyjoin(dir, e.name), segs, idx + 1, out);
    }
  }
}

function globOne(root, name) {
  const out = [];
  const segs = name.split("/");
  for (const base of reachableDirs(root)) matchSegs(base, segs, 0, out);
  return out;
}

function produced_files(root) {
  // {path: text} for every artifact an agent plausibly authored under root (deduped).
  const out = {};
  const seen = new Set();
  for (const name of ARTIFACT_GLOBS) {
    for (const f of globOne(root, name)) {
      if (seen.has(f) || !isFile(f)) continue;
      seen.add(f);
      try { out[f] = readText(f); } catch (e) { /* skip unreadable */ }
    }
  }
  return out;
}

function offenders(files) {
  const out = [];
  for (const f of Object.keys(files)) {
    const txt = files[f];
    if (BANNED_PHRASE.test(txt)) {
      out.push(f + ': contains "chain-of-thought" — use "structured reasoning".');
    }
    if (CREDS.some((rx) => rx.test(txt))) {
      out.push(f + ": looks like a committed credential value — remove it.");
    }
    if (BUDGETED.test(f)) {
      const n = txt.split("\n").length; // == txt.count("\n") + 1
      if (n > LINE_BUDGET) out.push(f + ": " + n + " lines (>" + LINE_BUDGET + " budget) — trim.");
    }
  }
  return out;
}

function required_for(agent) {
  if (!agent) return [];
  try {
    const data = JSON.parse(readText(REQUIRED_JSON));
    return (data && data[agent]) || [];
  } catch (e) {
    return [];
  }
}

function load_manifest(name) {
  // [all_rule_ids(Set), checkable_rule_ids(Set)] or [null, null] if the manifest can't be read.
  let d;
  try { d = JSON.parse(readText(path.join(MANIFEST_DIR, name + ".json"))); }
  catch (e) { return [null, null]; }
  const ids = new Set(), checkable = new Set();
  for (const r of (d.rules || [])) {
    const rid = String(r.id == null ? "" : r.id).toLowerCase();
    if (!rid) continue;
    ids.add(rid);
    if (r.type === "checkable") checkable.add(rid);
  }
  return [ids, checkable];
}

function parse_declarations(p) {
  // [decls, bare_set] or [null, null] if the transcript existed but was unreadable (fail closed).
  if (!p || !isFile(p)) return [{}, new Set()];
  let text;
  try { text = readText(p); } catch (e) { return [null, null]; }
  try {
    const decls = {}, bare = new Set();
    const lines = text.split(/\r\n|\r|\n/);
    for (const line of lines) {
      let ev;
      try { ev = JSON.parse(line); } catch (e) { continue; }
      if (!ev || ev.type !== "assistant") continue;
      let content = (ev.message && ev.message.content);
      if (content === undefined || content === null) content = "";
      const blocks = Array.isArray(content) ? content : [{ type: "text", text: content }];
      for (const b of blocks) {
        if (!b || b.type !== "text") continue;
        const t = b.text || "";
        for (const m of t.matchAll(DECL)) {
          const name = m[1].toLowerCase(), rid = m[2].toLowerCase(), evid = (m[3] || "").trim();
          if (!decls[name]) decls[name] = [];
          decls[name].push([rid, evid]);
        }
        for (const m of t.matchAll(BARE)) bare.add(m[1].toLowerCase());
      }
    }
    return [decls, bare];
  } catch (e) {
    return [null, null]; // unreadable despite existing -> fail closed (block)
  }
}

function _quote_norm(s) { return String(s).replace(/\s+/g, " ").trim().toLowerCase(); }

function check_evidence(evid, files_blob) {
  // Spot-check one evidence string. Returns a violation string if verifiably fabricated, else null.
  const pm = (evid || "").match(PATHISH);
  if (pm) {
    const tok = pm[0].replace(/\\/g, "/");
    const base = path.basename(tok);
    const keys = Object.keys(files_blob);
    const ok = isFile(tok) || keys.some((k) => k.replace(/\\/g, "/").endsWith(tok) || path.basename(k) === base);
    if (!ok) return 'evidence points to "' + tok + '" but no such produced file exists';
    return null;
  }
  const qm = (evid || "").match(QUOTED);
  if (qm) {
    const g = qm.slice(1).find((x) => x);
    const q = _quote_norm(g);
    const blob = _quote_norm(Object.values(files_blob).join("\n"));
    if (q && !blob.includes(q)) {
      return 'quoted evidence "' + q.slice(0, 40) + '…" does not appear in any produced artifact';
    }
  }
  return null;
}

function sortedUnique(arr) { return Array.from(new Set(arr)).sort(); }

function verify_declaration(required, decls, bare, files) {
  const reasons = [];
  const files_blob = files;
  for (const name of required) {
    const [all_ids, checkable] = load_manifest(name);
    if (all_ids === null) {
      reasons.push("Could not read the manifest for '" + name + "' to verify your citations — cannot finish.");
      continue;
    }
    const entries = decls[name] || [];
    if (entries.length === 0) {
      const hint = bare.has(name)
        ? " You wrote a bare `APPLIED-EXPERTISE: " + name + "` with no rule-ids — that no longer counts."
        : "";
      reasons.push(
        "You did not credibly declare applying '" + name + "'." + hint + " Re-read expertise/" + name + ".md, then emit " +
        "one line per governing rule you applied: `APPLIED-EXPERTISE: " + name + "#<rule-id> — <evidence>` " +
        "(evidence = the file it's embodied in, or a short quote from it)."
      );
      continue;
    }
    // (a) citation integrity — every cited id must be real
    const bad = sortedUnique(entries.filter(([rid]) => !all_ids.has(rid)).map(([rid]) => rid));
    if (bad.length) {
      reasons.push("'" + name + "': these cited rule-ids are not in the manifest (fabricated?): " +
                   bad.join(", ") + ". Cite real ids from expertise/manifests/" + name + ".json.");
    }
    // (b) coverage floor
    const good = new Set(entries.filter(([rid]) => all_ids.has(rid)).map(([rid]) => rid));
    const floor = Math.min(FLOOR, checkable.size || FLOOR);
    if (good.size < floor) {
      reasons.push("'" + name + "': only " + good.size + " valid rule(s) cited; cite at least " + floor + " of the " +
                   "governing rules you actually applied (not a token gesture).");
    }
    // (c) evidence spot-check
    for (const [rid, evid] of entries) {
      if (!all_ids.has(rid)) continue;
      const v = check_evidence(evid, files_blob);
      if (v) reasons.push("'" + name + "#" + rid + "': " + v + ".");
    }
  }
  return reasons;
}

function main() {
  let ev;
  try { ev = JSON.parse(readStdin()); } catch (e) { ev = {}; }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};
  // Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
  if (ev.stop_hook_active) process.exit(0);

  const [argv_root, argv_agent, main_agent] = agent_ident.split_args(process.argv.slice(2));
  const root = argv_root !== null ? argv_root : process.cwd();
  const agent = agent_ident.resolve_agent(ev, argv_agent, main_agent);
  const session = ev.session_id || "";

  // DORMANCY GUARD: do nothing unless a genesis agent is active.
  if (!agent) process.exit(0);

  const files = produced_files(root);
  let reasons = offenders(files);

  const cited = {};
  const required = required_for(agent);
  if (required.length) {
    const [decls, bare] = parse_declarations(ev.transcript_path || "");
    if (decls === null) {
      reasons.push("Could not read the transcript to verify the expertise declaration — cannot finish.");
    } else {
      for (const name of required) {
        const [all_ids] = load_manifest(name);
        if (all_ids) {
          const ids = sortedUnique((decls[name] || []).filter(([rid]) => all_ids.has(rid)).map(([rid]) => rid));
          if (ids.length) cited[name] = ids;
        }
      }
      reasons = reasons.concat(verify_declaration(required, decls, bare, files));
    }
  }

  if (reasons.length) {
    log(agent, "block", reasons, cited, session);
    emit({ decision: "block",
           reason: "Cannot finish:\n- " + reasons.slice(0, 20).join("\n- ") + "\nFix these, then stop again." });
    process.exit(0);
  }
  log(agent, "allow", [], cited, session);
  process.exit(0); // clean -> allow the stop
}

if (require.main === module) {
  main();
}

module.exports = { main, produced_files, offenders, required_for, load_manifest,
                   parse_declarations, check_evidence, verify_declaration };
