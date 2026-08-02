#!/usr/bin/env node
/* End-to-end + PORTABILITY integration test for Session-Copy (Phases 1+2) against the REAL memory server.
   Faithful Node (CommonJS, stdlib-only) port of test_pipeline.py. Chains capture → store → embed on a synthetic
   session, then COPIES the agent bundle to a fresh location (simulating a `git clone` on another machine) and
   proves the copied agent still recalls its history. Skips if the server binary/model aren't built.
   Run: node session_copy/test_pipeline.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const CAP = require("./capture.js");
const STORE = require("./store.js");
const EMB = require("./embed.js");

const HERE = __dirname;
const GH = path.dirname(HERE);
const BIN = path.join(GH, "server", "target", "release", "genesis-memory-server");
const MODEL = path.join(GH, "server", "models");
const SID = "pipeline-session-0001";

function _recall(agent_id, query, db, k) {
  if (k === undefined) {
    k = 3;
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
    console.log(`  SKIP  memory server not built (${BIN})`);
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

  // ---- synthetic session on a fake ~/.claude ----
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "pipe-home-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const proj = path.join(home, "projects", "-fake-repo");
  fs.mkdirSync(path.join(proj, "memory"), { recursive: true });
  fs.writeFileSync(
    path.join(proj, `${SID}.jsonl`),
    [
      JSON.stringify({ type: "user", timestamp: "t1", message: { content: "We agreed the deploy script must never run on a red build." } }),
      JSON.stringify({ type: "assistant", timestamp: "t2", message: { content: [{ type: "text", text: "Understood — I gate deploy on green CI only." }] } }),
    ].join("\n") + "\n",
    "utf8"
  );
  fs.writeFileSync(path.join(proj, "memory", "MEMORY.md"), "# index\n- The API base url is configured per environment, never hardcoded.\n", "utf8");

  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "pipe-repo-"));
  const agent_dir = path.join(repo, ".genesis", "agents", "deploybot");

  // ---- Phase 1: capture ----
  const cap = CAP.capture(SID, "/fake/repo", agent_dir, null, null, false);
  check("capture produced records (transcript + memory)", cap.total_records >= 3);

  // ---- Phase 2: store + embed ----
  const st = STORE.build_bundle(path.join(agent_dir, "records.jsonl"), agent_dir, "deploybot");
  check("store built history.sqlite + summary.md", fs.existsSync(path.join(agent_dir, "summary.md")));
  check("summary embeds standing memory", fs.readFileSync(path.join(agent_dir, "summary.md"), "utf8").includes("API base url"));

  const mem_db = path.join(agent_dir, "memory.sqlite");
  const em = await EMB.embed_records(EMB._load_from_db(path.join(agent_dir, "history.sqlite")), "deploybot", BIN, MODEL, mem_db);
  check("embed stored all records", em.failed === 0 && em.stored >= 3);

  // recall works on the freshly built agent
  const hit = await _recall("deploybot", "can we deploy when the build is failing?", mem_db);
  check("agent recalls its history semantically", hit.includes("red build") || hit.includes("green CI"));

  // ---- PORTABILITY: copy the bundle to a fresh location (simulate a git clone on machine B) ----
  const machineB = fs.mkdtempSync(path.join(os.tmpdir(), "pipe-cloneB-"));
  const b_agent = path.join(machineB, ".genesis", "agents", "deploybot");
  fs.cpSync(agent_dir, b_agent, { recursive: true });
  const b_mem = path.join(b_agent, "memory.sqlite");
  check(
    "bundle files travelled (memory.sqlite + summary + history)",
    ["memory.sqlite", "summary.md", "history.sqlite"].every((f) => fs.existsSync(path.join(b_agent, f)))
  );
  // recall from the COPIED db — no ~/.claude dependency, no re-embed
  const b_hit = await _recall("deploybot", "can we deploy when the build is failing?", b_mem);
  check("★ copied agent recalls history on 'machine B' (portable)", b_hit.includes("red build") || b_hit.includes("green CI"));

  for (const d of [home, repo, machineB]) {
    fs.rmSync(d, { recursive: true, force: true });
  }
  delete process.env.CLAUDE_CONFIG_DIR;
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
