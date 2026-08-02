#!/usr/bin/env node
/* Tests for session_copy/store.js (Phase 2, deterministic).
   Faithful Node (CommonJS, stdlib-only) port of test_store.py. Run: node session_copy/test_store.js */
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { DatabaseSync } = require("node:sqlite");
const S = require("./store.js");

const RECS = [
  { source: "auto-memory", kind: "memory-file", ts: "", title: "MEMORY.md", text: "# index\n- fact one\n- fact two\n" },
  { source: "transcript", kind: "user", ts: "t1", title: "", text: "please build the widget" },
  { source: "transcript", kind: "assistant", ts: "t2", title: "", text: "widget built and tested" },
  { source: "context-mode", kind: "chunk", ts: "t3", title: "Doc", text: "indexed doc text" },
];

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

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "store-"));
  const recs_path = path.join(tmp, "records.jsonl");
  fs.writeFileSync(recs_path, RECS.map((r) => JSON.stringify(r)).join("\n") + "\n", "utf8");

  const out = path.join(tmp, ".genesis", "agents", "widgetbot");
  const m = S.build_bundle(recs_path, out, "widgetbot");

  // history.sqlite
  const db = path.join(out, "history.sqlite");
  check("history.sqlite created", fs.existsSync(db) && fs.statSync(db).isFile());
  let con = new DatabaseSync(db, { readOnly: true });
  const n = con.prepare("SELECT COUNT(*) AS c FROM records").get().c;
  const srcs = new Set(con.prepare("SELECT DISTINCT source FROM records").all().map((r) => r.source));
  const txt = con.prepare("SELECT text FROM records WHERE source='transcript' AND kind='user'").get().text;
  con.close();
  check("all records stored", Number(n) === RECS.length);
  check("every source present in db", srcs.size === 3 && srcs.has("auto-memory") && srcs.has("transcript") && srcs.has("context-mode"));
  check("record text preserved", txt === "please build the widget");

  // summary.md
  const sp = path.join(out, "summary.md");
  const summary = fs.readFileSync(sp, "utf8");
  check("summary.md created", fs.existsSync(sp) && fs.statSync(sp).isFile());
  check("summary lists carried-over sources + counts", summary.includes("transcript: 2 records") && summary.includes("auto-memory: 1 records"));
  check("summary embeds prior MEMORY.md", summary.includes("fact one") && summary.includes("Standing memory"));
  check("summary shows recent turns", summary.includes("widget built and tested") && summary.includes("Most recent"));
  check("summary tells the agent history is recallable", summary.toLowerCase().includes("recall"));

  // manifest
  check("manifest counts records + sources", m.records === RECS.length && m.by_source.transcript === 2);

  // portability: the bundle is self-contained files under .genesis/agents/<name>/
  const files = new Set(fs.readdirSync(out));
  check("bundle is self-contained (db + summary + manifest)", ["history.sqlite", "summary.md", "store_manifest.json"].every((f) => files.has(f)));

  // re-run overwrites cleanly (idempotent history db)
  const m2 = S.build_bundle(recs_path, out, "widgetbot");
  con = new DatabaseSync(path.join(out, "history.sqlite"), { readOnly: true });
  const n2 = con.prepare("SELECT COUNT(*) AS c FROM records").get().c;
  con.close();
  check("re-run is idempotent (no duplicate rows)", Number(n2) === RECS.length);

  fs.rmSync(tmp, { recursive: true, force: true });
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
