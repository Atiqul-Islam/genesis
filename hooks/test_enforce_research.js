#!/usr/bin/env node
/* Node unit tests for enforce_research.js — the Sensei-scoped PreToolUse gate that blocks assembling a
   built agent unless the research-expertise skill ran this session (§16 / D5).

   Faithful port of test_enforce_research.py: runs the hook as a subprocess with synthetic PreToolUse
   events. Pure determinism, no network. Mirrors the Python contract + case count (8 cases).

   The Node enforcer keys on the assembler basename "assemble.js" (the Node assembler), so the synthetic
   commands invoke assemble.js (the Python original keyed on "assemble.py"). enforce_research.js writes a
   decisions log to <cwd>/.genesis/hook-decisions.log — every run uses cwd = a throwaway temp dir so the
   real project log is never touched.

   Run:  node hooks/test_enforce_research.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const ENFORCE = path.join(HERE, "enforce_research.js");
const GEN = "/x/.genesis";
const ROOT = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-enf-")); // transcripts + throwaway cwd (log sink)

function run(command, transcript_path, agent) {
  // `agent` populates the payload's agent_type. enforce_research has a DORMANCY GUARD: it no-ops unless a
  // genesis agent is active. Sensei is the only agent that assembles, so enforcement runs with
  // agent="sensei"; pass agent=null to simulate a normal (non-genesis) session.
  if (arguments.length < 3) agent = "sensei";
  const ev = { tool_input: { command: command }, transcript_path: transcript_path };
  if (agent) ev.agent_type = agent;
  const p = cp.spawnSync(process.execPath, [ENFORCE], {
    input: JSON.stringify(ev), encoding: "utf8", timeout: 30000, cwd: ROOT,
  });
  const out = (p.stdout || "").trim();
  if (!out) return [false, ""]; // silent -> allow
  let d;
  try { d = JSON.parse(out).hookSpecificOutput || {}; } catch (e) { return [false, out]; }
  return [d.permissionDecision === "deny", d.permissionDecisionReason || ""];
}

function transcript(root, used) {
  const p = path.join(root, "t_" + (used ? "yes" : "no") + ".jsonl");
  const blocks = [{ type: "text", text: "working on the build" }];
  if (used) blocks.push({ type: "tool_use", name: "Skill", input: { skill: "research-expertise" } });
  fs.writeFileSync(p, JSON.stringify({ type: "assistant", message: { content: blocks } }) + "\n", { encoding: "utf8" });
  return p;
}

function assemble_cmd(name) {
  return process.execPath + " " + GEN + "/install/assemble.js " + GEN + "/team/" + name + ' "' + name + '" /x ' + GEN;
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const with_skill = transcript(ROOT, true);
  const without_skill = transcript(ROOT, false);

  let blocked, reason;

  // 1. built agent, skill NOT used -> deny
  [blocked, reason] = run(assemble_cmd("acme-bot"), without_skill);
  check("built agent without research-expertise skill -> DENY", blocked && reason.indexOf("research-expertise") !== -1);

  // 2. built agent, skill used -> allow
  [blocked] = run(assemble_cmd("acme-bot"), with_skill);
  check("built agent WITH research-expertise skill -> allow", !blocked);

  // 3. builtin sensei -> allow regardless of transcript
  [blocked] = run(assemble_cmd("sensei"), without_skill);
  check("builtin sensei -> allow (exempt)", !blocked);

  // 4. builtin method -> allow regardless
  [blocked] = run(assemble_cmd("method"), without_skill);
  check("builtin method -> allow (exempt)", !blocked);

  // 5. non-assemble Bash command -> allow (not our concern)
  [blocked] = run(process.execPath + " -e 'process.exit(0)' && ls -la", without_skill);
  check("non-assemble command -> allow", !blocked);

  // 6. built agent, transcript MISSING -> deny (fail-closed)
  [blocked] = run(assemble_cmd("acme-bot"), path.join(ROOT, "nope.jsonl"));
  check("built agent + missing transcript -> DENY (fail-closed)", blocked);

  // 7. malformed assemble (no name arg) -> deny (fail-closed)
  [blocked] = run(process.execPath + " " + GEN + "/install/assemble.js", without_skill);
  check("assemble with no agent name -> DENY (fail-closed)", blocked);

  // 8. DORMANCY: no genesis agent active (normal session) -> no-op even for an assemble command.
  [blocked] = run(assemble_cmd("acme-bot"), without_skill, null);
  check("normal session (no genesis agent) -> allow even an assemble cmd (DORMANT)", !blocked);

  try { fs.rmSync(ROOT, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
