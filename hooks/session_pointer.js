#!/usr/bin/env node
/* SESSION-POINTER hook (SessionStart | UserPromptSubmit | Stop) — records the LIVE session's id so
   "copy my current session into an agent" knows WHICH session to capture.

   Faithful Node (CommonJS, stdlib-only) port of session_pointer.py. Writes
   <cwd>/.genesis/current-session.json every turn. Fail-open + silent: a pointer-write failure must
   never disrupt the session.

   Wire in the repo's .claude/settings.json:
     "SessionStart":     [{ "hooks":[{ "type":"command","command":"node <this>" }] }]
     "UserPromptSubmit": [{ "hooks":[{ "type":"command","command":"node <this>" }] }]
*/
"use strict";
const fs = require("fs");
const path = require("path");

function readStdin() {
  try {
    return fs.readFileSync(0, "utf8");
  } catch (e) {
    if (e && e.code === "EAGAIN") {
      const buf = Buffer.alloc(65536);
      const chunks = [];
      for (;;) {
        let n;
        try {
          n = fs.readSync(0, buf, 0, buf.length, null);
        } catch (e2) {
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

function nowIso() {
  // Matches Python datetime.now(timezone.utc).isoformat(timespec="seconds") -> "...T..+00:00".
  return new Date().toISOString().slice(0, 19) + "+00:00";
}

function main() {
  let ev;
  try {
    ev = JSON.parse(readStdin());
  } catch (e) {
    ev = {};
  }
  if (ev === null || typeof ev !== "object" || Array.isArray(ev)) ev = {};
  const sid = ev.session_id || ev.sessionId || "";
  const tpath = ev.transcript_path || "";
  if (!sid) {
    process.exit(0); // nothing to record
  }
  try {
    const d = path.join(process.cwd(), ".genesis");
    fs.mkdirSync(d, { recursive: true });
    const rec = { session_id: sid, transcript_path: tpath, ts: nowIso() };
    fs.writeFileSync(path.join(d, "current-session.json"), JSON.stringify(rec), { encoding: "utf8" });
  } catch (e) {
    // fail-open: never disrupt the session
  }
  process.exit(0);
}

if (require.main === module) {
  main();
}

module.exports = { main };
