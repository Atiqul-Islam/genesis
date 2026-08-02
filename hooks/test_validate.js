#!/usr/bin/env node
/* Node unit tests for validate.js — the evidence-carrying declaration gate (§18).

   Faithful port of test_validate.py. Proves the bare-token hole is closed: a credible declaration must
   cite REAL rule-ids with VERIFIABLE evidence, and the produced artifact is spot-checked. Runs validate.js
   as a subprocess (as Claude Code does), feeding a synthetic transcript + workspace. No network, no API —
   pure determinism. Mirrors the Python contract + case count (10 cases).

   validate.js writes a decisions log to <cwd>/.genesis/hook-decisions.log. Each run uses cwd = the
   throwaway temp WORKSPACE, so the log lands in a disposable dir and the real project log is never touched.
   (The banned-phrase literal is assembled at runtime via COT so the live gate lets this file be written.)

   Run:  node hooks/test_validate.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const VALIDATE = path.join(HERE, "validate.js");
const QUOTE = "Turn a green main into a correct, tagged, changelogged npm release."; // distinctive artifact line
const COT = "chain" + "-of-" + "thought"; // the banned phrase, assembled at runtime

function pyReplaceAll(s, oldSub, newSub) {
  // Python str.replace replaces ALL occurrences; JS String.replace(str,..) only the first. Emulate replace-all.
  return s.split(oldSub).join(newSub);
}

function run(root, agent, transcript_path, stop_active) {
  // Invoke validate.js; return [blocked, reason].
  const payload = { transcript_path: transcript_path, stop_hook_active: !!stop_active };
  const p = cp.spawnSync(process.execPath, [VALIDATE, root, agent], {
    input: JSON.stringify(payload), encoding: "utf8", timeout: 30000, cwd: root,
  });
  const out = (p.stdout || "").trim();
  if (!out) return [false, ""];
  let d;
  try { d = JSON.parse(out); } catch (e) { return [false, out]; }
  return [d.decision === "block", d.reason || ""];
}

function make_workspace() {
  // A temp workspace with one valid produced artifact (a lean, clean CLAUDE.md).
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-val-"));
  const agent_dir = path.join(root, "release-manager");
  fs.mkdirSync(agent_dir, { recursive: true });
  const claude_md = path.join(agent_dir, "CLAUDE.md");
  fs.writeFileSync(claude_md, "# Ferry — Release Manager\n## Mission\n" + QUOTE + "\n## Boundaries\nYou never push if CI is red.\n", { encoding: "utf8" });
  return [root, claude_md];
}

function transcript(root, text) {
  // Write a one-message assistant transcript carrying `text`; return its path.
  const p = path.join(root, "transcript.jsonl");
  fs.writeFileSync(p, JSON.stringify({ type: "assistant", message: { content: [{ type: "text", text: text }] } }) + "\n", { encoding: "utf8" });
  return p;
}

function valid_decl(claude_md) {
  // A credible evidence-carrying declaration for 'method' (persona-creation + prompt-engineering +
  // expertise-application), >=3 real ids each, evidence = the real file or a real quote.
  const p = claude_md;
  return [
    "APPLIED-EXPERTISE: persona-creation#pc-1 — " + p,
    "APPLIED-EXPERTISE: persona-creation#pc-2 — " + p + " is 5 lines, under budget",
    'APPLIED-EXPERTISE: persona-creation#pc-7 — "' + QUOTE + '"',
    "APPLIED-EXPERTISE: prompt-engineering#pe-4 — " + p,
    "APPLIED-EXPERTISE: prompt-engineering#pe-6 — " + p,
    "APPLIED-EXPERTISE: prompt-engineering#pe-18 — " + p,
    "APPLIED-EXPERTISE: expertise-application#ea-1 — " + p,
    "APPLIED-EXPERTISE: expertise-application#ea-3 — " + p,
    "APPLIED-EXPERTISE: expertise-application#ea-5 — " + p,
  ].join("\n");
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; console.log("  PASS  " + name); }
    else { failed += 1; console.log("  FAIL  " + name); }
  }

  const [root, claude_md] = make_workspace();

  // 1. No declaration at all -> block (method requires 3 expertise)
  let [blocked, reason] = run(root, "method", transcript(root, "I finished the persona. Looks good."));
  check("no declaration blocks", blocked);

  // 2. Bare tokens only (the old gameable form) -> block
  const bare = "APPLIED-EXPERTISE: persona-creation\nAPPLIED-EXPERTISE: prompt-engineering\nAPPLIED-EXPERTISE: expertise-application";
  [blocked, reason] = run(root, "method", transcript(root, bare));
  check("bare-token declaration blocks", blocked && reason.toLowerCase().indexOf("bare") !== -1);

  // 3. Fabricated rule-id -> block
  const bad = pyReplaceAll(valid_decl(claude_md), "persona-creation#pc-1 ", "persona-creation#pc-999 ");
  [blocked, reason] = run(root, "method", transcript(root, bad));
  check("fabricated rule-id blocks", blocked && reason.indexOf("pc-999") !== -1);

  // 4. Below the coverage floor (one expertise cited only once) -> block
  let thin = valid_decl(claude_md);
  thin = thin.split("\n").filter((l) => !(l.indexOf("prompt-engineering") !== -1 && l.indexOf("pe-6") === -1)).join("\n");
  [blocked, reason] = run(root, "method", transcript(root, thin));
  check("below-floor coverage blocks", blocked && reason.toLowerCase().indexOf("at least") !== -1);

  // 5. Fabricated evidence FILE -> block
  const fake_file = pyReplaceAll(valid_decl(claude_md), "persona-creation#pc-1 — " + claude_md, "persona-creation#pc-1 — ghost/NOWHERE.md");
  [blocked, reason] = run(root, "method", transcript(root, fake_file));
  check("fabricated evidence file blocks", blocked && reason.indexOf("NOWHERE.md") !== -1);

  // 6. Fabricated evidence QUOTE -> block
  const fake_q = pyReplaceAll(valid_decl(claude_md), '"' + QUOTE + '"', '"a sentence that appears in no artifact whatsoever"');
  [blocked, reason] = run(root, "method", transcript(root, fake_q));
  check("fabricated evidence quote blocks", blocked && reason.indexOf("does not appear") !== -1);

  // 7. Fully valid evidence-carrying declaration -> ALLOW
  [blocked, reason] = run(root, "method", transcript(root, valid_decl(claude_md)));
  check("valid evidence-carrying declaration passes", !blocked);

  // 8. Valid declaration BUT the artifact violates a checkable rule (banned phrase) -> block (layer 1)
  fs.appendFileSync(claude_md, "\nUse " + COT + " prompting here.\n", { encoding: "utf8" });
  [blocked, reason] = run(root, "method", transcript(root, valid_decl(claude_md)));
  check("artifact violation blocks despite valid declaration", blocked && reason.indexOf(COT) !== -1);

  // 9. stop_hook_active short-circuits -> allow (loop guard)
  [blocked, reason] = run(root, "method", transcript(root, "nothing"), true);
  check("stop_hook_active short-circuits to allow", !blocked);

  // 10. Agent with NO required expertise, clean artifacts -> allow
  //     (recreate a clean workspace since #8 dirtied this one)
  const [root2] = make_workspace();
  [blocked, reason] = run(root2, "", transcript(root2, "done"));
  check("no-required-expertise agent with clean artifacts allows", !blocked);

  for (const d of [root, root2]) {
    try { fs.rmSync(d, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
