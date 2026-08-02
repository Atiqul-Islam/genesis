#!/usr/bin/env node
/* Node tests for hooks/inject.js — house-rule/expertise delivery + the session-copy summary injection.

   Faithful port of test_inject.py: runs inject.js as a subprocess (as Claude Code does). inject.js writes
   NO log, so no log-redirection is needed; all fixtures live under throwaway temp dirs. Mirrors the Python
   contract + case count (7 cases).

   Run: node hooks/test_inject.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const INJECT = path.join(HERE, "inject.js");

function run(exp_dir, agent, cwd) {
  const p = cp.spawnSync(process.execPath, [INJECT, exp_dir, agent], {
    input: "{}", encoding: "utf8", cwd: cwd || process.cwd(), timeout: 20000,
  });
  try {
    return JSON.parse((p.stdout || "").trim()).hookSpecificOutput.additionalContext || "";
  } catch (e) {
    return "";
  }
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const home = fs.mkdtempSync(path.join(os.tmpdir(), "gh-"));
  const exp = path.join(home, "expertise");
  fs.mkdirSync(path.join(exp, "manifests"), { recursive: true });
  fs.writeFileSync(path.join(exp, "persona-creation.md"), "# guide\n");
  fs.writeFileSync(path.join(exp, "required.json"), JSON.stringify({ method: ["persona-creation"] }));

  // 1. always delivers house rules
  const ctx = run(exp, "method");
  check("house rules always injected", ctx.indexOf("Genesis house rules") !== -1 && ctx.indexOf("credential") !== -1);
  check("required expertise injected for a known agent", ctx.indexOf("APPLIED-EXPERTISE") !== -1);

  // 2. NO summary block when the agent has no session-copy bundle (existing behavior preserved)
  check("no summary block without a bundle", ctx.indexOf("carried-over session memory") === -1);

  // 3. summary IS injected when the agent has a bundle at <home>/agents/<name>/summary.md
  const ag = path.join(home, "agents", "copybot");
  fs.mkdirSync(ag, { recursive: true });
  fs.writeFileSync(path.join(ag, "summary.md"), "# digest\n- prior fact: the deploy gate requires green CI\n");
  const ctx2 = run(exp, "copybot");
  check("summary injected for a session-copy agent",
        ctx2.indexOf("carried-over session memory") !== -1 && ctx2.indexOf("deploy gate requires green CI") !== -1);
  check("summary block tells the agent to recall the rest", ctx2.toLowerCase().indexOf("recall") !== -1);

  // 4. bundle discoverable via cwd/.genesis too (repo-local layout)
  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "repo-"));
  const ag2 = path.join(repo, ".genesis", "agents", "localbot");
  fs.mkdirSync(ag2, { recursive: true });
  fs.writeFileSync(path.join(ag2, "summary.md"), "# digest\n- local bundle fact ABC123\n");
  const ctx3 = run(path.join(repo, ".genesis", "expertise"), "localbot", repo);
  check("summary discovered via cwd/.genesis/agents", ctx3.indexOf("local bundle fact ABC123") !== -1);

  // 5. output stays under the 10,000-char hook cap
  const big = path.join(home, "agents", "bigbot");
  fs.mkdirSync(big, { recursive: true });
  fs.writeFileSync(path.join(big, "summary.md"), "x".repeat(50000));
  const ctx4 = run(exp, "bigbot");
  check("output respects the 10k-char cap", ctx4.length <= 10000);

  for (const d of [home, repo]) {
    try { fs.rmSync(d, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
