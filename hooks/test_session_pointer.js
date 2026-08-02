#!/usr/bin/env node
/* Node tests for hooks/session_pointer.js + the resolve_session('current') contract.

   Faithful port of test_session_pointer.py. session_pointer.js writes <cwd>/.genesis/current-session.json
   every turn; every run here uses cwd = a throwaway temp repo, so the real project pointer/log is never
   touched. Mirrors the Python contract + case count (7 cases).

   NOTE on parity: the Python test also exercised build_session_agent.resolve_session (Python). That reader
   is NOT part of the hook port and remains Python (session_copy/build_session_agent.py). Since the plugin's
   hooks may assume ONLY node at runtime, this test verifies the SAME observable contract with a Node
   `resolveSession` that mirrors build_session_agent.py's resolve_session exactly — proving the pointer the
   .js hook writes is consumable by that resolver and that the missing-pointer error path holds.

   Run: node hooks/test_session_pointer.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const HERE = __dirname;
const POINTER = path.join(HERE, "session_pointer.js");

function run(payload, cwd) {
  cp.spawnSync(process.execPath, [POINTER], {
    input: JSON.stringify(payload), encoding: "utf8", cwd: cwd, timeout: 15000,
  });
}

function isFile(p) { try { return fs.statSync(p).isFile(); } catch (e) { return false; } }

// Mirror of build_session_agent.py resolve_session (Python; not ported). Reads the pointer the hook writes.
function SystemExit(msg) { const e = new Error(msg); e.isSystemExit = true; return e; }
function resolveSession(session_id, repo) {
  if (session_id && session_id !== "current") return session_id;
  const ptr = path.join(repo, ".genesis", "current-session.json");
  let sid = "";
  try { sid = (JSON.parse(fs.readFileSync(ptr, "utf8")).session_id) || ""; } catch (e) { sid = ""; }
  if (!sid) {
    throw SystemExit("session '--current' requested but no current-session.json pointer found — is the "
      + "session_pointer hook wired into this repo? (or pass an explicit --session <id>)");
  }
  return sid;
}

function main() {
  let passed = 0, failed = 0;
  function check(name, cond) {
    if (cond) { passed += 1; } else { failed += 1; }
    console.log("  " + (cond ? "PASS" : "FAIL") + "  " + name);
  }

  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "ptr-"));
  const ptr = path.join(repo, ".genesis", "current-session.json");

  // 1. records the current session id
  run({ session_id: "SID-123", transcript_path: "/x/SID-123.jsonl" }, repo);
  check("pointer file written", isFile(ptr));
  const rec = JSON.parse(fs.readFileSync(ptr, "utf8"));
  check("pointer holds the session id + transcript", rec.session_id === "SID-123" && rec.transcript_path.endsWith("SID-123.jsonl"));

  // 2. accepts the camelCase sessionId variant too
  run({ sessionId: "SID-456" }, repo);
  check("pointer updates on a new turn (sessionId variant)", JSON.parse(fs.readFileSync(ptr, "utf8")).session_id === "SID-456");

  // 3. no id -> no crash, no bogus write
  fs.rmSync(ptr);
  run({ transcript_path: "/x/y.jsonl" }, repo);
  check("no session id -> no pointer written (fail-open)", !isFile(ptr));

  // 4. resolveSession('current') reads the pointer
  run({ session_id: "SID-789" }, repo);
  check("resolve_session('current') returns the pointer id", resolveSession("current", repo) === "SID-789");
  check("resolve_session passes through an explicit id", resolveSession("EXPLICIT", repo) === "EXPLICIT");

  // 5. missing pointer -> clear error
  const empty = fs.mkdtempSync(path.join(os.tmpdir(), "ptr-empty-"));
  let raised;
  try { resolveSession("current", empty); raised = false; }
  catch (e) { raised = !!e.isSystemExit; }
  check("missing pointer -> SystemExit with guidance", raised);

  for (const d of [repo, empty]) {
    try { fs.rmSync(d, { recursive: true, force: true }); } catch (e) { /* best-effort */ }
  }
  console.log("\n" + passed + " passed, " + failed + " failed");
  process.exit(failed ? 1 : 0);
}

if (require.main === module) {
  main();
}
