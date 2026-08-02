#!/usr/bin/env node
/* Integration test for session_copy/embed.js (Phase 2b) — against the REAL Genesis memory server binary +
   ONNX model (no mocks). Faithful Node (CommonJS, stdlib-only) port of test_embed.py. Proves: captured records
   embed under the agent's id, then are SEMANTICALLY RECALLABLE, and are per-agent isolated. Skips (does not
   fail) if the binary/model aren't built.
   Run: node session_copy/test_embed.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const E = require("./embed.js");

const HERE = __dirname;
const GH = path.dirname(HERE);
const BIN = path.join(GH, "server", "target", "release", "genesis-memory-server");
const MODEL = path.join(GH, "server", "models");

const RECS = [
  { source: "transcript", kind: "assistant", title: "", text: "The release manager never pushes to main when CI is red." },
  { source: "auto-memory", kind: "memory-file", title: "MEMORY.md", text: "The user prefers concise bullet replies, about 20 words each." },
  { source: "context-mode", kind: "chunk", title: "Rust", text: "The memory server uses sqlite-vec for KNN over 384-dim embeddings." },
];

function _recall(agent_id, query, db, k) {
  if (k === undefined) {
    k = 2;
  }
  const env = Object.assign({}, process.env, { GENESIS_MODEL_DIR: MODEL, GENESIS_MEMORY_DB: db });
  const p = spawn(BIN, [], { stdio: ["pipe", "pipe", "inherit"], env });
  let buf = "";
  const lines = [];
  const waiters = [];
  p.stdout.setEncoding("utf8");
  p.stdout.on("data", (d) => {
    buf += d;
    let nl;
    while ((nl = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, nl);
      buf = buf.slice(nl + 1);
      if (waiters.length) {
        waiters.shift()(line);
      } else {
        lines.push(line);
      }
    }
  });
  const readLine = () => (lines.length ? Promise.resolve(lines.shift()) : new Promise((r) => waiters.push(r)));
  const rpc = async (o) => {
    p.stdin.write(JSON.stringify(o) + "\n");
    return JSON.parse(await readLine());
  };
  return (async () => {
    try {
      await rpc({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "t", version: "1" } } });
      p.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
      const r = await rpc({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "recall", arguments: { agent_id, query, k } } });
      return JSON.stringify(r.result || {});
    } finally {
      p.kill();
    }
  })();
}

async function main() {
  if (!(fs.existsSync(BIN) && fs.existsSync(MODEL) && fs.statSync(MODEL).isDirectory())) {
    console.log(`  SKIP  memory server binary/model not built (${BIN}) — build with: cd server && cargo build --release`);
    console.log("\n0 passed, 0 failed (skipped)");
    return;
  }

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

  const db = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "embed-")), "memory.sqlite");
  const res = await E.embed_records(RECS, "copybot", BIN, MODEL, db);
  check("all records embedded (none failed)", res.stored === RECS.length && res.failed === 0);

  // semantic recall — a paraphrase must surface the right record
  const hit = await _recall("copybot", "when is it not allowed to push code?", db);
  check("recall surfaces the CI-red rule (semantic paraphrase)", hit.includes("CI is red"));
  const hit2 = await _recall("copybot", "how does the vector search work?", db);
  check("recall surfaces the sqlite-vec chunk", hit2.includes("sqlite-vec"));

  // per-agent isolation — a different agent sees nothing
  const other = await _recall("someone-else", "when is it not allowed to push code?", db);
  check("per-agent isolation (other agent sees none)", !other.includes("CI is red"));

  fs.rmSync(path.dirname(db), { recursive: true, force: true });
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
