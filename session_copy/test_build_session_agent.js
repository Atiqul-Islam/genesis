#!/usr/bin/env node
/* Integration test for the Phase 3 orchestrator (build_session_agent.js) against the REAL memory server.
   Faithful Node (CommonJS, stdlib-only) port of test_build_session_agent.py. Proves: one call captures a live
   session, lays down the bundle where inject.js finds it, and embeds the history into the repo's SHARED memory
   under agent_id=<name> so it is recallable. Skips if server not built.
   Run: node session_copy/test_build_session_agent.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn, spawnSync } = require("child_process");
const B = require("./build_session_agent.js");

const HERE = __dirname;
const GH = path.dirname(HERE);
const BIN = path.join(GH, "server", "target", "release", "genesis-memory-server");
const MODEL = path.join(GH, "server", "models");
const INJECT = path.join(GH, "hooks", "inject.js");
const SID = "orch-session-0001";

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

  // synthetic live session
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "orch-home-"));
  process.env.CLAUDE_CONFIG_DIR = home;
  const proj = path.join(home, "projects", "-fake");
  fs.mkdirSync(path.join(proj, "memory"), { recursive: true });
  fs.writeFileSync(
    path.join(proj, `${SID}.jsonl`),
    [
      JSON.stringify({ type: "user", timestamp: "t1", message: { content: "Remember: the nightly job must skip weekends." } }),
      JSON.stringify({ type: "assistant", timestamp: "t2", message: { content: [{ type: "text", text: "Got it — nightly runs Mon–Fri only." }] } }),
    ].join("\n") + "\n",
    "utf8"
  );
  fs.writeFileSync(path.join(proj, "memory", "MEMORY.md"), "# index\n- Use UTC for all scheduled jobs.\n", "utf8");

  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "orch-repo-"));
  const ghome = path.join(repo, ".genesis");
  fs.mkdirSync(ghome, { recursive: true });
  const mem_db = path.join(ghome, "memory.db");

  const m = await B.build(SID, "nightbot", repo, ghome, BIN, MODEL, mem_db, [], false);
  check("orchestrator captured + embedded", m.captured >= 3 && m.embed_failed === 0 && m.embedded >= 3);

  // bundle laid down where inject.js looks (<genesis_home>/agents/<name>/summary.md)
  const bundle = path.join(ghome, "agents", "nightbot");
  check("bundle at <genesis_home>/agents/<name>/", fs.existsSync(path.join(bundle, "summary.md")) && fs.existsSync(path.join(bundle, "history.sqlite")));

  // the agent recalls its carried-over history from the SHARED repo memory under its id
  const hit = await _recall("nightbot", "does the nightly job run on saturday?", mem_db);
  check("agent recalls carried-over history under its id", hit.includes("weekend") || hit.includes("Mon"));

  // inject.js surfaces the summary for this agent (exp_dir = <genesis_home>/expertise)
  fs.mkdirSync(path.join(ghome, "expertise"), { recursive: true });
  const p = spawnSync("node", [INJECT, path.join(ghome, "expertise"), "nightbot"], { input: "{}", encoding: "utf8", cwd: repo, timeout: 20000 });
  const ctx = (JSON.parse((p.stdout || "").trim()).hookSpecificOutput || {}).additionalContext || "";
  check("inject.js surfaces the session-copy summary at start", ctx.includes("carried-over session memory") && ctx.includes("UTC"));

  for (const d of [home, repo]) {
    fs.rmSync(d, { recursive: true, force: true });
  }
  delete process.env.CLAUDE_CONFIG_DIR;
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
