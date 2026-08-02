#!/usr/bin/env node
/* Tests for session_copy/capture.js (Phase 1). Faithful Node (CommonJS, stdlib-only) port of test_capture.py.
   Synthetic data matches the REAL on-disk schemas verified 2026-07-22. The load-bearing assertion: a planted
   credential value NEVER appears in the output — only the label survives, redacted. No network.
   Run:  node session_copy/test_capture.js

   NOTE ON SOURCE HYGIENE: the planted value + the AWS-key / private-key fixtures are assembled at runtime from
   parts, and labelled forms are built with concatenation (never a literal "<label> = <value>" adjacency), so
   this source file itself contains no credential-shaped literal (house rule). Runtime values are byte-identical
   to test_capture.py's fixtures.
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { DatabaseSync } = require("node:sqlite");
const C = require("./capture.js");

const _pv = "S3cr3tV@lue987654"; // planted value, bound indirectly to avoid a label=value adjacency in source
const SECRET = _pv; // distinctive; must never appear un-redacted in any output
const AKIA_FIXTURE = "AKIA" + "1234567890ABCDEF"; // synthetic AWS-key-shaped value, built at runtime
const PK_FIXTURE = "-----BEGIN " + "PRIVATE KEY-----\nMIIabc\n-----END " + "PRIVATE KEY-----"; // synthetic key block

function main() {
  let passed = 0;
  let failed = 0;
  function check(name, cond) {
    if (cond) {
      passed += 1;
    } else {
      failed += 1;
    }
    console.log(`  ${cond ? "PASS" : "FAIL"}  ${name}`);
  }

  // 1. scrub_text across forms
  const forms = [
    "password = " + SECRET,
    '"api_key": "' + SECRET + '"',
    "token: " + SECRET,
    AKIA_FIXTURE,
    "ghp_" + "a".repeat(30),
    "sk-" + "b".repeat(30),
    PK_FIXTURE,
  ];
  let all_scrubbed = true;
  for (const f of forms) {
    const [out, n] = C.scrub_text(f);
    if (n === 0 || out.includes(SECRET) || out.includes(AKIA_FIXTURE) || out.includes("ghp_" + "a".repeat(30)) || out.includes("sk-" + "b".repeat(30)) || out.includes("MIIabc")) {
      all_scrubbed = false;
    }
  }
  check("scrub_text redacts every secret form (value never survives)", all_scrubbed);
  check("scrub_text keeps the label", C.scrub_text("password = " + SECRET)[0].includes("password"));
  check("scrub_text no-op on clean text", C.scrub_text("hello world")[1] === 0);
  // Honest limitation made visible: a BARE unlabelled unknown value is NOT auto-detectable...
  const bare = "random note " + SECRET;
  check("HONEST LIMIT: bare unlabelled unknown value is not auto-caught", C.scrub_text(bare)[1] === 0);
  // ...but the exact-value denylist guarantees its removal.
  const [o, n0] = C.scrub_text(bare, [SECRET]);
  check("known-denylist guarantees removal of a bare value", n0 === 1 && !o.includes(SECRET));

  // 2. _text_from_content
  const t = C._text_from_content([
    { type: "text", text: "hi" },
    { type: "tool_use", name: "Bash", input: { command: "ls" } },
    { type: "tool_result", content: "output here" },
  ]);
  check("_text_from_content flattens blocks", t.includes("hi") && t.includes("Bash") && t.includes("output here"));
  check("_text_from_content handles a plain string", C._text_from_content("plain") === "plain");

  const home = fs.mkdtempSync(path.join(os.tmpdir(), "cc-home-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const SID = "test-session-uuid-0001";
  const proj = path.join(home, "projects", "-tmp-fake-repo");
  fs.mkdirSync(path.join(proj, "memory"), { recursive: true });

  // 3. transcript
  const tpath = path.join(proj, `${SID}.jsonl`);
  fs.writeFileSync(
    tpath,
    [
      JSON.stringify({ type: "ai-title", sessionId: SID }), // metadata (ignored)
      JSON.stringify({ type: "user", timestamp: "t1", message: { content: "my password = " + SECRET + " keep it" } }),
      JSON.stringify({ type: "assistant", timestamp: "t2", message: { content: [{ type: "text", text: "understood" }] } }),
      JSON.stringify({ type: "system", timestamp: "t3", content: "a system note" }),
      JSON.stringify({ type: "mode", "permission-mode": "auto" }), // metadata (ignored)
    ].join("\n") + "\n",
    "utf8"
  );
  let [recs, red] = C.extract_transcript(tpath);
  const kinds = new Set(recs.map((r) => r.kind));
  let blob = JSON.stringify(recs);
  check("transcript extracts only conversation turns", kinds.size === 3 && kinds.has("user") && kinds.has("assistant") && kinds.has("system") && recs.length === 3);
  check("transcript scrubs a planted secret", red >= 1 && !blob.includes(SECRET) && blob.includes("password"));

  // 4. auto-memory
  fs.writeFileSync(path.join(proj, "memory", "MEMORY.md"), "# index\n- a note\n", "utf8");
  fs.writeFileSync(path.join(proj, "memory", "topic.md"), "detail with token: " + SECRET + "\n", "utf8");
  [recs, red] = C.extract_auto_memory(proj);
  check("auto-memory reads MEMORY.md + topic files", recs.length === 2 && recs.some((r) => r.title === "MEMORY.md"));
  check("auto-memory scrubs secrets", red >= 1 && !JSON.stringify(recs).includes(SECRET));

  // 5. context-mode (real chunks schema)
  const cmdir = path.join(home, "context-mode", "content");
  fs.mkdirSync(cmdir, { recursive: true });
  const db = path.join(cmdir, "abc123.db");
  let con = new DatabaseSync(db);
  con.exec("CREATE TABLE chunks (title TEXT, content TEXT, source_id INT, content_type TEXT, source_category TEXT, session_id TEXT, event_id INT, timestamp TEXT)");
  let ins = con.prepare("INSERT INTO chunks (title,content,session_id,timestamp) VALUES (?,?,?,?)");
  ins.run("Doc A", "indexed text with token: " + SECRET, SID, "t9");
  ins.run("Other", "belongs to another session", "OTHER-SID", "t9");
  con.close();
  [recs, red] = C.extract_contextmode(SID);
  check("context-mode extracts THIS session's chunks only", recs.length === 1 && recs[0].title === "Doc A");
  check("context-mode scrubs secrets", red >= 1 && !JSON.stringify(recs).includes(SECRET));

  // 6. claude-mem observer jsonl
  const obsdir = path.join(home, "projects", "-home-x--claude-mem-observer-sessions");
  fs.mkdirSync(obsdir, { recursive: true });
  fs.writeFileSync(
    path.join(obsdir, "obs.jsonl"),
    [
      JSON.stringify({ type: "observation", operation: "add", timestamp: "t", sessionId: SID, content: "observed api_key=" + SECRET }),
      JSON.stringify({ type: "observation", operation: "add", timestamp: "t", sessionId: "OTHER", content: "other session obs" }),
    ].join("\n") + "\n",
    "utf8"
  );
  [recs, red] = C.extract_claude_mem(SID);
  check("claude-mem keeps ONLY session-matched observations", recs.length === 1 && recs[0].title === "session-match");
  check("claude-mem scrubs secrets", red >= 1 && !JSON.stringify(recs).includes(SECRET));
  // the fix: a session with no matches captures NOTHING (never dumps the whole corpus)
  check("claude-mem no-match → 0 records (no corpus dump)", C.extract_claude_mem("NOPE-SID")[0].length === 0);

  // 7. genesis memory
  const gdb = path.join(home, "gmem.db");
  con = new DatabaseSync(gdb);
  con.exec("CREATE TABLE memories (agent_id TEXT, text TEXT)");
  con.prepare("INSERT INTO memories VALUES (?,?)").run("mine", "a durable fact");
  con.prepare("INSERT INTO memories VALUES (?,?)").run("other", "not mine");
  con.close();
  [recs, red] = C.extract_genesis_memory(gdb, "mine");
  check("genesis-memory extracts this agent's rows", recs.length === 1 && recs[0].text.includes("durable fact"));

  // 8. user config
  fs.writeFileSync(path.join(home, "CLAUDE.md"), "# user rules\n", "utf8");
  const settingsObj = { env: {}, model: "opus" };
  settingsObj.env["API_KEY"] = SECRET; // bracket-assigned so the source has no label=value adjacency
  fs.writeFileSync(path.join(home, "settings.json"), JSON.stringify(settingsObj), "utf8");
  const skdir = path.join(home, "skills", "myskill");
  fs.mkdirSync(skdir, { recursive: true });
  fs.writeFileSync(path.join(skdir, "SKILL.md"), "---\nname: myskill\n---\nbody\n", "utf8");
  [recs, red] = C.extract_user_config();
  blob = JSON.stringify(recs);
  check("user-config captures CLAUDE.md + settings + skills", recs.length === 3);
  check("user-config scrubs settings secrets (value gone)", !blob.includes(SECRET) && blob.includes("API_KEY"));

  // 9. capture() end-to-end — THE portability + no-leak guarantee.
  //    Plant a BARE secret in the transcript too, and rely on the denylist to guarantee its removal.
  fs.appendFileSync(tpath, JSON.stringify({ type: "user", timestamp: "t4", message: { content: "here is a raw value " + SECRET + " with no label" } }) + "\n", "utf8");
  const out = fs.mkdtempSync(path.join(os.tmpdir(), "cc-cap-"));
  const m = C.capture(SID, "/tmp/fake/repo", out, gdb, "mine", true, [SECRET]);
  const recs_path = path.join(out, "records.jsonl");
  const all_text = fs.readFileSync(recs_path, "utf8");
  check("capture() wrote records.jsonl + manifest", fs.existsSync(recs_path) && fs.existsSync(path.join(out, "capture_manifest.json")));
  check("capture() total_records covers every source", m.total_records >= 8 && Object.keys(m.by_source).length === 6);
  check("capture() manifest counts redactions", m.total_redactions >= 5);
  check("★ NO planted credential value anywhere (incl. a bare one via denylist)", !all_text.includes(SECRET));
  check("capture() resolved the transcript by session id", m.transcript === tpath);

  fs.rmSync(home, { recursive: true, force: true });
  fs.rmSync(out, { recursive: true, force: true });
  delete process.env.CLAUDE_CONFIG_DIR;

  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
