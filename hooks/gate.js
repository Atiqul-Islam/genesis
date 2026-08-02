#!/usr/bin/env node
/* GATE hook (PreToolUse: Write|Edit) — deterministic enforcement + just-in-time rule SURFACING.

   Faithful Node (CommonJS, stdlib-only) port of gate.py. Two jobs at the moment a file is about to be
   written: (1) BLOCK a Write/Edit whose content violates a checkable house rule (banned phrase /
   credential value / line budget); (2) SURFACE the governing rules for a substantial authoring
   artifact (non-blocking). Self-guards: no-op unless a genesis agent is active (payload agent_type).

   Reads the PreToolUse event JSON on stdin. On a violation -> `deny`. Otherwise stays out of the
   permission decision and may attach `additionalContext` with the surfaced rules.
*/
"use strict";
const fs = require("fs");
const path = require("path");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

// ---------- Layer 1: blocking checks (checkable over file content) ----------
const BANNED_PHRASE = /chain[\s\-]?of[\s\-]?thought/i;
const CRED_PATTERNS = [
  [/AKIA[0-9A-Z]{16}/, "AWS access key id"],
  [/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/, "private key block"],
  [/\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['"]?[^\s'"]{6,}/i, "credential value"],
];
const BUDGETED = /(persona|behavior)\.md$|(^|\/)CLAUDE\.md$/i;
const LINE_BUDGET = 200;

// ---------- Layer 2: rule surfacing ----------
const HOOK_DIR = __dirname;
const MANIFEST_DIR = path.join(HOOK_DIR, "..", "expertise", "manifests");
const SURFACE_MIN = 300; // only surface for a substantial authoring write; skip tiny edits
const SURFACE_N = 5;     // top-N governing rules to re-assert
// artifact path -> the expertise whose top rules to re-assert
const AUTHORING = /(persona|behavior)\.md$|(^|\/)CLAUDE\.md$|\.claude\/agents\/.*\.md$/i;
const PROMPTISH = /(prompt|tool)[\w\-]*\.(md|jsonc?|txt)$|\.prompt$/i;
// WRITABLE runtime data -> the PROJECT's .genesis/ (cwd-derived). MANIFEST_DIR stays hook-relative.
const LOG = path.join(agent_ident.runtime_dir(), "hook-decisions.log");

const CAP = 9500; // additionalContext (and all hook output strings) is capped at 10,000 chars

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

function log(decision, p, rule, surfaced) {
  rule = rule || "";
  surfaced = surfaced || "";
  try {
    fs.mkdirSync(path.dirname(LOG), { recursive: true });
    const rec = { ts: nowIso(), hook: "gate", decision: decision, path: p, rule: rule, surfaced: surfaced };
    fs.appendFileSync(LOG, JSON.stringify(rec) + "\n", { encoding: "utf8" });
  } catch (e) { /* logging must never break the gate */ }
}

function deny(reason, p, rule) {
  p = p || "";
  rule = rule || "";
  log("deny", p, rule, "");
  emit({ hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: reason } });
  process.exit(0);
}

function proceed(context, p, surfaced) {
  context = context || "";
  p = p || "";
  surfaced = surfaced || "";
  if (context) {
    log("allow", p, "", surfaced);
    emit({ hookSpecificOutput: { hookEventName: "PreToolUse", additionalContext: context.slice(0, CAP) } });
  }
  process.exit(0); // nothing to surface -> silent; let normal permissions decide
}

function targetManifest(p) {
  if (AUTHORING.test(p || "")) return "persona-creation";
  if (PROMPTISH.test(p || "")) return "prompt-engineering";
  return null;
}

function firstSentence(text) {
  // Emulate re.split(r"(?<=[.:])\s", text, 1)[0]: text up to (not including) the first whitespace
  // that follows a '.' or ':'.
  const m = /(?<=[.:])\s/.exec(text);
  return m ? text.slice(0, m.index) : text;
}

function topRules(name, n) {
  n = n || SURFACE_N;
  let d;
  try {
    d = JSON.parse(fs.readFileSync(path.join(MANIFEST_DIR, name + ".json"), "utf8"));
  } catch (e) {
    return [];
  }
  const out = [];
  for (const r of (d.rules || [])) {
    if (r.type !== "checkable") continue;
    const first = firstSentence(String(r.text || "").trim());
    out.push(r.id + ": " + first.slice(0, 170).replace(/\s+$/, ""));
    if (out.length >= n) break;
  }
  return out;
}

function surface(p, content) {
  if ((content || "").length < SURFACE_MIN) return "";
  const name = targetManifest(p);
  if (!name) return "";
  const rules = topRules(name);
  if (!rules.length) return "";
  return (
    "Governing rules for this artifact — re-asserted before you write (" + name + "; full manifest at " +
    "expertise/manifests/" + name + ".json). Apply them and cite the ones you use in your " +
    "APPLIED-EXPERTISE lines:\n- " + rules.join("\n- ")
  );
}

function main() {
  let ev;
  try {
    ev = JSON.parse(readStdin());
  } catch (e) {
    process.exit(0); // not our event / no input -> allow, fail-open on parse
  }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};
  // DORMANCY GUARD: no-op unless a genesis agent is active (payload agent_type).
  if (!agent_ident.resolve_agent(ev)) process.exit(0);

  const ti = ev.tool_input || {};
  const p = ti.file_path || "";
  const content = ti.content || ti.new_string || "";

  if (BANNED_PHRASE.test(content)) {
    deny(
      'Accuracy rule: do not write "chain-of-thought" — use "structured reasoning" / ' +
        '"step-by-step reasoning". Reword and retry.',
      p, "banned-phrase"
    );
  }

  for (const [rx, what] of CRED_PATTERNS) {
    if (rx.test(content)) {
      deny(
        "Security rule: this looks like a committed " + what + ". Never write a credential value. " +
          'Reference it as "credential present at <path>" instead.',
        p, "credential"
      );
    }
  }

  if (BUDGETED.test(p)) {
    const n = content.split("\n").length; // == content.count("\n") + 1
    if (n > LINE_BUDGET) {
      deny(
        "Budget rule: " + p + " is " + n + " lines (>" + LINE_BUDGET + "). Keep persona/behavior/rules lean so " +
          "adherence doesn't decay — trim to the smallest high-signal set, then retry.",
        p, "line-budget"
      );
    }
  }

  const ctx = surface(p, content);
  proceed(ctx, p, ctx ? (targetManifest(p) || "") : "");
}

if (require.main === module) {
  main();
}

module.exports = { main, surface, topRules, targetManifest };
