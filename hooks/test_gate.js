#!/usr/bin/env node
/* Node unit tests for gate.js — PreToolUse blocking + just-in-time rule surfacing (§19).

   Faithful port of test_gate.py: runs gate.js as a subprocess with synthetic PreToolUse events. Pure
   determinism, no network. Mirrors the Python contract + case count (9 cases).

   The gate writes a decisions log to <cwd>/.genesis/hook-decisions.log. Every run here uses a THROWAWAY
   temp cwd so the real project log is never touched.

   NOTE: the banned-phrase and credential literals the gate blocks on are assembled at RUNTIME from
   fragments (COT / CRED) so this test's own source file does not contain them verbatim — otherwise the
   live gate would refuse to let this very file be written.

   Run:  node hooks/test_gate.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const GATE = path.join(HERE, "gate.js");
// Throwaway cwd so gate's hook-decisions.log lands in a temp dir, never the real project log.
const TMPCWD = fs.mkdtempSync(path.join(os.tmpdir(), "gh-gate-"));

const COT = "chain" + "-of-" + "thought";                 // the banned phrase, assembled at runtime
const CRED = "api_key = " + '"' + "sk-abcdef123456" + '"'; // a credential-looking value, assembled at runtime

function run(file_path, content, edit, agent_type) {
  edit = edit || false;
  if (arguments.length < 4) agent_type = "sensei";
  const key = edit ? "new_string" : "content";
  const ti = { file_path: file_path };
  ti[key] = content;
  // gate.js is DORMANT unless a genesis agent is active (payload agent_type). These functional tests run
  // AS a genesis agent (default "sensei"); pass agent_type=null to exercise the dormant path.
  const ev = { tool_input: ti };
  if (agent_type) ev.agent_type = agent_type;
  const p = cp.spawnSync(process.execPath, [GATE], {
    input: JSON.stringify(ev), encoding: "utf8", cwd: TMPCWD, timeout: 30000,
  });
  const out = (p.stdout || "").trim();
  if (!out) return {};
  try {
    return JSON.parse(out).hookSpecificOutput || {};
  } catch (e) {
    return {};
  }
}

function main() {
  let passed = 0, failed = 0;

  function check(name, cond) {
    if (cond) { passed += 1; console.log("  PASS  " + name); }
    else { failed += 1; console.log("  FAIL  " + name); }
  }

  const big = "# Agent\n## Mission\n" + "Ship correct releases. ".repeat(30) + "\n## Boundaries\nNever push if CI is red.\n";

  // 1. banned phrase -> deny
  let d = run("release-manager/CLAUDE.md", "Use " + COT + " reasoning.\n" + big);
  check("banned phrase denies", d.permissionDecision === "deny" && (d.permissionDecisionReason || "").indexOf(COT) !== -1);

  // 2. credential -> deny
  d = run("agent/config.md", CRED + "\n" + big);
  check("credential value denies", d.permissionDecision === "deny");

  // 3. oversize budgeted file -> deny
  d = run("x/CLAUDE.md", Array.from({ length: 250 }, (_, i) => "line " + i).join("\n"));
  check("oversize CLAUDE.md denies", d.permissionDecision === "deny" && (d.permissionDecisionReason || "").indexOf("lines") !== -1);

  // 4. substantial clean CLAUDE.md -> proceed (no auto-approve) + surface persona-creation rules
  d = run("release-manager/CLAUDE.md", big);
  let ctx = d.additionalContext || "";
  check("substantial CLAUDE.md surfaces persona rules, does not auto-approve",
        d.permissionDecision !== "deny" && !("permissionDecision" in d)
        && ctx.indexOf("persona-creation") !== -1 && ctx.indexOf("pc-") !== -1);

  // 5. tiny edit -> proceed silently, NO surface (below SURFACE_MIN)
  d = run("release-manager/CLAUDE.md", "## Voice\nTerse.\n", true);
  check("tiny edit proceeds without surfacing",
        d.permissionDecision !== "deny" && !("additionalContext" in d));

  // 6. non-authoring file (code) substantial -> proceed silently, NO surface
  d = run("src/util.py", "def f():\n    return 1\n" + "# pad line\n".repeat(40));
  check("non-authoring file proceeds without surfacing",
        d.permissionDecision !== "deny" && !("additionalContext" in d));

  // 7. prompt/tool artifact substantial -> proceed + surface prompt-engineering rules
  d = run("prompts/tool_defs.json", '{"name":"x","description":"' + "y".repeat(320) + '"}');
  ctx = d.additionalContext || "";
  check("prompt/tool artifact surfaces prompt-engineering rules",
        d.permissionDecision !== "deny" && ctx.indexOf("prompt-engineering") !== -1 && ctx.indexOf("pe-") !== -1);

  // 8. DORMANCY: with NO genesis agent active, gate does NOTHING — even banned content is not denied.
  d = run("release-manager/CLAUDE.md", "Use " + COT + " reasoning.\n" + big, false, null);
  check("DORMANT without a genesis agent: banned content is NOT denied (no global policing)",
        d.permissionDecision !== "deny" && !("permissionDecision" in d));
  d = run("method|genesis:method-check/CLAUDE.md", COT + "\n" + big, false, "genesis:method");
  check("scoped agent_type genesis:method still enforces",
        d.permissionDecision === "deny");

  try { fs.rmSync(TMPCWD, { recursive: true, force: true }); } catch (e) { /* best-effort cleanup */ }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
