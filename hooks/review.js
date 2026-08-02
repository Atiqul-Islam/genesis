#!/usr/bin/env node
/* REVIEW hook (Stop / SubagentStop) — the INDEPENDENT semantic reviewer.

   Faithful Node (CommonJS, stdlib-only) port of review.py. Judges whether produced artifacts actually
   EMBODY the semantic rules of each required expertise, using an INDEPENDENT judge. Every verdict must
   survive a POSITION-SWAP; the judge can ONLY BLOCK. FAIL-CLOSED. Self-guards on the active agent.

   Config (env): GENESIS_REVIEW, GENESIS_REVIEWER_CMD, GENESIS_REVIEWER_MODEL, GENESIS_REVIEW_MAX,
   GENESIS_REVIEW_TIMEOUT — see review.py for semantics.

   Wire (assemble adds this as a second Stop hook, AFTER validate):  { validate . <agent> }, { review . <agent> }
*/
"use strict";
const fs = require("fs");
const path = require("path");
const cp = require("child_process");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

const MECHANICAL_KINDS = new Set(["regex", "line_count", "declaration"]);
const HOOK_DIR = __dirname;
// READ-ONLY reference data ships next to the hooks — hook-relative.
const REQUIRED_JSON = path.join(HOOK_DIR, "..", "expertise", "required.json");
const MANIFEST_DIR = path.join(HOOK_DIR, "..", "expertise", "manifests");
// WRITABLE runtime data -> the PROJECT's .genesis/ (cwd-derived).
const LOG = path.join(agent_ident.runtime_dir(), "review.log");
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
function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }
function readText(p) { return fs.readFileSync(p, "utf8"); }

function now() { return new Date().toISOString().slice(0, 19) + "+00:00"; }

function log(rows) {
  try {
    fs.mkdirSync(path.dirname(LOG), { recursive: true });
    let blob = "";
    for (const r of rows) blob += JSON.stringify(r) + "\n";
    fs.appendFileSync(LOG, blob, { encoding: "utf8" });
  } catch (e) { /* ignore */ }
}

// ---- POSIX-mode shlex.split emulation (throws on unbalanced quote) ----
function shlexSplit(s) {
  const tokens = [];
  let cur = "", inToken = false;
  let i = 0;
  const n = s.length;
  while (i < n) {
    const c = s[i];
    if (c === "\\") {
      inToken = true; i++;
      if (i < n) { cur += s[i]; i++; } else { cur += "\\"; }
      continue;
    }
    if (c === "'") {
      inToken = true; i++;
      const j = s.indexOf("'", i);
      if (j === -1) throw new Error("No closing quotation");
      cur += s.slice(i, j); i = j + 1; continue;
    }
    if (c === '"') {
      inToken = true; i++;
      let closed = false;
      while (i < n) {
        const d = s[i];
        if (d === "\\") {
          i++;
          if (i < n) {
            const e = s[i];
            if (e === '"' || e === "\\" || e === "$" || e === "`" || e === "\n") cur += e;
            else cur += "\\" + e;
            i++;
          } else { cur += "\\"; }
        } else if (d === '"') { closed = true; i++; break; }
        else { cur += d; i++; }
      }
      if (!closed) throw new Error("No closing quotation");
      continue;
    }
    if (/\s/.test(c)) {
      if (inToken) { tokens.push(cur); cur = ""; inToken = false; }
      i++; continue;
    }
    inToken = true; cur += c; i++;
  }
  if (inToken) tokens.push(cur);
  return tokens;
}

// ---- shutil.which emulation ----
function which(cmd) {
  const pathVar = process.env.PATH || "";
  const dirs = pathVar.split(path.delimiter).filter(Boolean);
  const isWin = process.platform === "win32";
  const exts = isWin ? (process.env.PATHEXT || ".COM;.EXE;.BAT;.CMD").split(";").filter(Boolean) : [""];
  for (const dir of dirs) {
    for (const ext of exts) {
      const full = path.join(dir, cmd + (isWin ? ext : ""));
      try {
        const st = fs.statSync(full);
        if (st.isFile() && (isWin || (st.mode & 0o111))) return full;
      } catch (e) { /* not here */ }
    }
  }
  return null;
}

// ---- recursive artifact glob (replicates glob.glob(join(root,"**",name), recursive=True)) ----
function segToRegex(seg) {
  let re = "";
  for (const ch of seg) {
    if (ch === "*") re += "[^/]*";
    else if (ch === "?") re += "[^/]";
    else re += ch.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }
  const guard = seg.startsWith(".") ? "" : "(?!\\.)";
  return new RegExp("^" + guard + re + "$");
}
function pyjoin(a, b) {
  // Faithful to Python os.path.join for glob inputs: does NOT collapse a leading "./" the way
  // Node's path.join does, so produced paths match Python glob output (e.g. "./persona.md").
  const sep = path.sep;
  if (a === "") return b;
  if (a.endsWith(sep) || a.endsWith("/")) return a + b;
  return a + sep + b;
}
function reachableDirs(root) {
  const dirs = [];
  (function walk(d) {
    dirs.push(d);
    let entries;
    try { entries = fs.readdirSync(d, { withFileTypes: true }); } catch (e) { return; }
    for (const e of entries) if (e.isDirectory() && !e.name.startsWith(".")) walk(pyjoin(d, e.name));
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
    if (last) { if (e.isFile()) out.push(pyjoin(dir, e.name)); }
    else if (e.isDirectory()) matchSegs(pyjoin(dir, e.name), segs, idx + 1, out);
  }
}
function globOne(root, name) {
  const out = [];
  const segs = name.split("/");
  for (const base of reachableDirs(root)) matchSegs(base, segs, 0, out);
  return out;
}

function required_for(agent) {
  if (!agent) return [];
  try {
    const data = JSON.parse(readText(REQUIRED_JSON));
    return (data && data[agent]) || [];
  } catch (e) { return []; }
}

function semantic_rules(name) {
  // (judgment ∪ semantic-checkable) rules as [[id, criterion], ...], judgment first. null on read error.
  let d;
  try { d = JSON.parse(readText(path.join(MANIFEST_DIR, name + ".json"))); }
  catch (e) { return null; }
  const judged = [], semi = [];
  for (const r of (d.rules || [])) {
    const t = r.type;
    if (t === "judgment") {
      judged.push([r.id, r.reviewer_criterion || r.text || ""]);
    } else if (t === "checkable") {
      const pk = (r.predicate || {}).kind;
      if (!MECHANICAL_KINDS.has(pk)) semi.push([r.id, (r.predicate || {}).spec || r.text || ""]);
    }
    // principle -> skip
  }
  return judged.concat(semi);
}

function produced_artifacts(root) {
  const out = [];
  const seen = new Set();
  for (const name of ARTIFACT_GLOBS) {
    for (const f of globOne(root, name)) {
      if (seen.has(f) || !isFile(f)) continue;
      seen.add(f);
      try { out.push([f, readText(f)]); } catch (e) { /* skip */ }
    }
  }
  return out;
}

function reviewer_cmd() {
  const raw = process.env.GENESIS_REVIEWER_CMD;
  if (raw) return shlexSplit(raw);
  if (which("claude")) return ["claude", "-p", "--model", process.env.GENESIS_REVIEWER_MODEL || "claude-sonnet-5"];
  return null;
}

function build_prompt(name, rules, artifacts_text, swap) {
  const order = swap ? rules.slice().reverse() : rules;
  const opts = swap ? "FAIL or PASS" : "PASS or FAIL";
  const crit = order.map(([rid, c]) => "- " + rid + ": " + c).join("\n");
  return (
    "You are an INDEPENDENT reviewer. You did NOT write the artifacts below. Judge whether they actually " +
    "EMBODY each rule criterion for the '" + name + "' expertise. Be skeptical: if an artifact does not clearly " +
    "satisfy a rule, mark it FAIL — do not give the benefit of the doubt.\n\n" +
    "RULES (criteria to check):\n" + crit + "\n\n" +
    "ARTIFACTS under review:\n<<<\n" + artifacts_text.slice(0, 12000) + "\n>>>\n\n" +
    "For EACH rule id above, decide " + opts + ". Return ONLY strict JSON, nothing else:\n" +
    '{"verdicts":[{"id":"<rule-id>","verdict":"PASS|FAIL","reason":"<one short line>"}]}\n'
  );
}

function run_reviewer(cmd, prompt, timeout) {
  let r;
  try {
    r = cp.spawnSync(cmd[0], cmd.slice(1), {
      input: prompt, encoding: "utf8", timeout: timeout * 1000, maxBuffer: 256 * 1024 * 1024,
    });
  } catch (e) { return null; }
  if (r.error) return null;
  if (r.status !== 0) return null;
  return r.stdout;
}

function extract_verdicts(text) {
  // Parse {'verdicts':[{id,verdict,reason}]} -> {id:[VERDICT,reason]} or null if nothing parseable.
  if (!text) return null;
  const t = text.replace(/```(?:json)?/g, "");
  const idx = t.indexOf('"verdicts"');
  if (idx === -1) return null;
  const start = t.slice(0, idx).lastIndexOf("{");
  if (start === -1) return null;
  let depth = 0, end = -1;
  for (let i = start; i < t.length; i++) {
    if (t[i] === "{") depth += 1;
    else if (t[i] === "}") { depth -= 1; if (depth === 0) { end = i + 1; break; } }
  }
  if (end === -1) return null;
  let obj;
  try { obj = JSON.parse(t.slice(start, end)); } catch (e) { return null; }
  const out = {};
  for (const v of (obj.verdicts || [])) {
    const rid = String(v.id == null ? "" : v.id).toLowerCase();
    const verdict = String(v.verdict == null ? "" : v.verdict).toUpperCase();
    if (rid && (verdict === "PASS" || verdict === "FAIL")) out[rid] = [verdict, String(v.reason == null ? "" : v.reason)];
  }
  return out;
}

function reconcile(v1, v2, rule_ids) {
  const out = {};
  for (const rid of rule_ids) {
    const a = v1[rid], b = v2[rid];
    if (!a || !b) out[rid] = ["FAIL", "reviewer omitted a verdict on one of the swapped runs (fail-closed)"];
    else if (a[0] === b[0]) out[rid] = a;
    else out[rid] = ["FAIL", "inconsistent across position-swap (" + a[0] + " vs " + b[0] + ") — fail-closed"];
  }
  return out;
}

function review(root, agent) {
  const mode = (process.env.GENESIS_REVIEW || "on").toLowerCase();
  if (mode === "off") return [[], []];
  const strict = mode === "strict";
  const required = required_for(agent);
  if (!required.length) return [[], []];
  const artifacts = produced_artifacts(root);
  if (!artifacts.length) return [[], []];
  const cmd = reviewer_cmd();
  if (!cmd) {
    if (strict) return [["Semantic review is strict but no reviewer is configured (set GENESIS_REVIEWER_CMD)."], []];
    log([{ ts: now(), hook: "review", agent: agent, reviewer: "(none)", note: "inactive-no-reviewer" }]);
    return [[], []];
  }

  const reviewer_name = process.env.GENESIS_REVIEWER_MODEL || (cmd.length ? cmd[0] : "reviewer");
  const timeout = parseInt(process.env.GENESIS_REVIEW_TIMEOUT || "120", 10);
  const max_rules = parseInt(process.env.GENESIS_REVIEW_MAX || "8", 10);
  const art_text = artifacts.map(([f, txt]) => "### FILE: " + f + "\n" + txt).join("\n\n");

  const block_reasons = [], rows = [];
  for (const name of required) {
    let rules = semantic_rules(name);
    if (rules === null) {
      block_reasons.push("Could not read the '" + name + "' manifest to review it — cannot certify (fail-closed).");
      continue;
    }
    if (!rules.length) continue;
    const total = rules.length;
    rules = rules.slice(0, max_rules);
    if (total > rules.length) {
      rows.push({ ts: now(), hook: "review", agent: agent, reviewer: reviewer_name,
                  note: "coverage: reviewed " + rules.length + "/" + total + " semantic rules of " + name + " (cap " + max_rules + ")" });
    }
    const rule_ids = rules.map(([rid]) => rid);
    const r1 = extract_verdicts(run_reviewer(cmd, build_prompt(name, rules, art_text, false), timeout));
    const r2 = extract_verdicts(run_reviewer(cmd, build_prompt(name, rules, art_text, true), timeout));
    if (r1 === null || r2 === null) {
      block_reasons.push("'" + name + "': the independent reviewer failed or returned unparseable output — " +
                         "cannot certify semantic application (fail-closed).");
      rows.push({ ts: now(), hook: "review", agent: agent, reviewer: reviewer_name,
                  note: "reviewer-error on " + name, verdict: "fail" });
      continue;
    }
    const verdicts = reconcile(r1, r2, rule_ids);
    for (const rid of Object.keys(verdicts)) {
      const [verdict, reason] = verdicts[rid];
      rows.push({ ts: now(), hook: "review", agent: agent, reviewer: reviewer_name,
                  id: rid, verdict: verdict.toLowerCase(), reason: reason });
      if (verdict === "FAIL") {
        block_reasons.push("'" + name + "#" + rid + "': reviewer found the artifact does not embody this rule — " + reason);
      }
    }
  }
  return [block_reasons, rows];
}

function main() {
  let ev;
  try { ev = JSON.parse(readStdin()); } catch (e) { ev = {}; }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};
  if (ev.stop_hook_active) process.exit(0); // loop guard: don't block forever

  const [argv_root, argv_agent, main_agent] = agent_ident.split_args(process.argv.slice(2));
  const root = argv_root !== null ? argv_root : process.cwd();
  const agent = agent_ident.resolve_agent(ev, argv_agent, main_agent);
  // DORMANCY GUARD: no genesis agent active -> no-op.
  if (!agent) process.exit(0);

  let reasons, rows;
  try {
    [reasons, rows] = review(root, agent);
  } catch (e) {
    const nm = (e && e.name) || "Error";
    const msg = (e && e.message) || String(e);
    reasons = ["Semantic reviewer raised " + nm + ": " + msg + " — cannot certify (fail-closed)."];
    rows = [];
  }
  if (rows.length) log(rows);
  if (reasons.length) {
    emit({ decision: "block",
           reason: "Semantic review found unmet rules:\n- " + reasons.slice(0, 20).join("\n- ") +
                   "\nAddress these (or GENESIS_REVIEW=off to disable the semantic layer), then stop again." });
    process.exit(0);
  }
  process.exit(0);
}

if (require.main === module) {
  main();
}

module.exports = { main, review, semantic_rules, produced_artifacts, reviewer_cmd,
                   build_prompt, run_reviewer, extract_verdicts, reconcile, required_for, which, shlexSplit };
