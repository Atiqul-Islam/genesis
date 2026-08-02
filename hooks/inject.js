#!/usr/bin/env node
/* INJECT hook (SessionStart) — deterministic DELIVERY of the house rules + expertise pointers.

   Faithful Node (CommonJS, stdlib-only) port of inject.py. Delivers (1) the checkable house rules and
   (2) pointers to the decoupled expertise store; and, for a session-copy agent, its carried-over digest.
   Stays under the 10,000-char hook-output cap.

   Wire in a member's frontmatter or settings.json:
     "SessionStart": [{ "matcher":"startup|resume|compact",
                        "hooks":[{ "type":"command","command":"node <this> <expertise_dir>","timeout":10 }] }]
*/
"use strict";
const fs = require("fs");
const path = require("path");
const agent_ident = require("./agent_ident.js"); // sibling helper; travels with hooks/ in every install

const RULES =
  "Genesis house rules (enforced by hooks — the gate blocks, the validator refuses to finish):\n" +
  '- Never write "chain-of-thought"; use "structured reasoning" / "step-by-step reasoning".\n' +
  '- Never write a credential value; reference it as "credential present at <path>".\n' +
  "- Keep persona.md / behavior.md / CLAUDE.md at or under 200 lines each.\n" +
  "- These are checkable and enforced deterministically; do not rely on memory to honor them.";

const SUMMARY_CAP = 7000; // chars of the session-copy digest to inject; the rest is recall-only

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

function emit(obj) {
  fs.writeSync(1, JSON.stringify(obj) + "\n");
}

function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }
function isDir(p) { try { return fs.statSync(p).isDirectory(); } catch (e) { return false; } }

function readText(p) {
  // errors="replace" parity: Node's utf8 decoder substitutes U+FFFD for invalid bytes.
  return fs.readFileSync(p, "utf8");
}

function findSummary(exp_dir, agent) {
  // A session-copy agent has a carried-over digest at <genesis_home>/agents/<name>/summary.md
  // (derivable from exp_dir = <genesis_home>/expertise) or <cwd>/.genesis/agents/<name>/summary.md.
  if (!agent) return null;
  const cands = [];
  if (exp_dir) {
    cands.push(path.join(path.dirname(path.resolve(exp_dir)), "agents", agent, "summary.md"));
  }
  cands.push(path.join(process.cwd(), ".genesis", "agents", agent, "summary.md"));
  for (const c of cands) {
    if (isFile(c)) return c;
  }
  return null;
}

function main() {
  // argv: <expertise_dir> [<agent>] [--main-agent <name>].
  let main_agent = "";
  const pos = [];
  const args = process.argv.slice(2);
  let i = 0;
  while (i < args.length) {
    if (args[i] === "--main-agent" && i + 1 < args.length) {
      main_agent = args[i + 1];
      i += 2;
      continue;
    }
    pos.push(args[i]);
    i += 1;
  }
  const exp_dir = pos.length ? pos[0] : "";
  const argv_agent = pos.length > 1 ? pos[1] : "";

  let ev;
  try { ev = JSON.parse(readStdin()); } catch (e) { ev = {}; }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};

  const agent = agent_ident.resolve_agent(ev, argv_agent, main_agent);

  let pointers = "";
  if (exp_dir && isDir(exp_dir)) {
    let files = [];
    try { files = fs.readdirSync(exp_dir).filter((f) => f.endsWith(".md")).sort(); } catch (e) { files = []; }
    if (files.length) {
      const lines = files
        .map((f) => {
          const stem = f.slice(0, f.length - path.extname(f).length);
          return "- " + stem + ": " + path.join(exp_dir, f);
        })
        .join("\n");
      pointers =
        "\nYour expertise store (decoupled, authoritative — read the file your behavior " +
        "names, on demand, before deep work):\n" + lines;
    }
  }

  let required = "";
  if (agent && exp_dir) {
    let req = [];
    try {
      const data = JSON.parse(readText(path.join(exp_dir, "required.json")));
      req = (data && data[agent]) || [];
    } catch (e) {
      req = [];
    }
    if (req && req.length) {
      required =
        "\nYou are '" + agent + "'. Every task, load and apply these REQUIRED expertise: " +
        req.join(", ") + ". Each has a rule manifest at expertise/manifests/<name>.json " +
        "(stable rule-ids + predicates). Before finishing, declare the governing rules you " +
        "actually applied — ONE LINE PER RULE, carrying evidence:\n" +
        "  APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>\n" +
        "where <evidence> is the file the rule is embodied in (e.g. release-manager/CLAUDE.md) " +
        "or a short verbatim quote from your output. The Stop hook (validate) verifies each " +
        "citation: a bare `APPLIED-EXPERTISE: <name>` with no rule-id, a rule-id not in the " +
        "manifest, or evidence pointing to a nonexistent file / a quote absent from your work " +
        "all BLOCK finishing. Cite at least the rules you truly used (a token gesture fails).";
    }
  }

  // Session-copy agents: surface the carried-over memory digest at start (D5). Recall the rest on demand.
  let summary_block = "";
  const sp = findSummary(exp_dir, agent);
  if (sp) {
    let body = "";
    try { body = readText(sp).trim(); } catch (e) { body = ""; }
    if (body) {
      if (body.length > SUMMARY_CAP) {
        body = body.slice(0, SUMMARY_CAP) + "\n…(digest truncated — recall the rest via your memory tools)";
      }
      summary_block =
        "\n\n## Your carried-over session memory (you were created by copying a prior " +
        "Claude Code session)\n" + body +
        "\nThe full prior conversation history + memory is stored under your agent id — " +
        "recall any specific detail with your memory tools (recall).";
    }
  }

  let ctx = RULES + required + pointers + summary_block;
  if (ctx.length > 9500) {
    // stay under the 10,000-char hook-output cap
    ctx = ctx.slice(0, 9500) + "\n…(truncated)";
  }
  emit({ hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: ctx } });
}

if (require.main === module) {
  main();
}

module.exports = { main, findSummary };
