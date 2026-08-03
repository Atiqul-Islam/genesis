#!/usr/bin/env node
/* Tests for bin/genesis-memory.js — the GitHub-Releases launcher (no npm, no token).

   Node (CommonJS, stdlib-only), NO NETWORK. The launcher downloads the platform binaries + model from
   this repo's GitHub Release, SHA256-verifies, caches, and execs the server (default) or stages the
   genesis-hook binary (--stage-hook). The download path needs a real published release, so it isn't
   exercised here; these tests drive the REAL launcher via the dev OVERRIDES (GENESIS_MEMORY_BIN /
   GENESIS_HOOK_BIN / GENESIS_MODEL_DIR), which is exactly the offline dev/CI path:
     * transparent exec: argv + GENESIS_MODEL_DIR forwarded, stdin/stdout pass through, STDOUT pristine,
       child exit status propagated;
     * --stage-hook copies the resolved hook binary into <dest>;
     * fail-closed: a missing override exits non-zero with a clear stderr message.

   Run:  node test/launcher.test.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const HERE = __dirname;
const REPO = path.dirname(HERE);
const LAUNCHER = path.join(REPO, "bin", "genesis-memory.js");
const NODE = process.execPath;

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

// Fake "server": prints MODEL=<GENESIS_MODEL_DIR> then echoes the first stdin line.
const FAKE_SERVER = [
  "'use strict';",
  "process.stdout.write('MODEL=' + (process.env.GENESIS_MODEL_DIR || '') + '\\n');",
  "const chunks = [];",
  "process.stdin.on('data', (c) => {",
  "  chunks.push(c);",
  "  const buf = Buffer.concat(chunks);",
  "  const nl = buf.indexOf(0x0a);",
  "  if (nl !== -1) { process.stdout.write('ECHO:' + buf.toString('utf8').slice(0, nl + 1)); process.exit(0); }",
  "});",
  "process.stdin.on('end', () => process.exit(0));",
  "",
].join("\n");

const FAKE_EXIT3 = ["'use strict';", "process.stdout.write('bye\\n');", "process.exit(3);", ""].join("\n");

function withTempDir(fn) {
  const td = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-launcher-"));
  try {
    return fn(td);
  } finally {
    fs.rmSync(td, { recursive: true, force: true });
  }
}

function makeModelDir(td) {
  const modelDir = path.join(td, "model");
  fs.mkdirSync(path.join(modelDir, "onnx"), { recursive: true });
  fs.writeFileSync(path.join(modelDir, "onnx", "model.onnx"), "m");
  fs.writeFileSync(path.join(modelDir, "tokenizer.json"), "t");
  return modelDir;
}

// baseEnv: process.env minus every launcher override, so a case controls exactly what is set (and no
// stray override or cache dir leaks in — keeps every case fully offline).
function baseEnv(extra) {
  const env = Object.assign({}, process.env);
  delete env.GENESIS_MEMORY_BIN;
  delete env.GENESIS_HOOK_BIN;
  delete env.GENESIS_MODEL_DIR;
  delete env.GENESIS_CACHE_DIR;
  return Object.assign(env, extra || {});
}

// ── transparent exec via GENESIS_MEMORY_BIN + GENESIS_MODEL_DIR overrides (offline) ─────
function testTransparentExec() {
  withTempDir((td) => {
    const fakeServer = path.join(td, "fake_server.js");
    fs.writeFileSync(fakeServer, FAKE_SERVER, { encoding: "utf-8" });
    const modelDir = makeModelDir(td);
    const proc = spawnSync(NODE, [LAUNCHER, fakeServer], {
      input: "ping-through-stdio\n",
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_MEMORY_BIN: NODE, GENESIS_MODEL_DIR: modelDir }),
    });
    const out = proc.stdout || "";
    check("exec: return code 0", proc.status === 0);
    check("exec: GENESIS_MODEL_DIR forwarded to server", out.includes("MODEL=" + modelDir));
    check("exec: stdin passed through to server stdout", out.includes("ECHO:ping-through-stdio"));
    const stdoutLines = out.split(/\r?\n/).filter((ln) => ln);
    check(
      "exec: STDOUT carries ONLY the server's two lines (launcher silent on success)",
      stdoutLines.length === 2 && stdoutLines[0].startsWith("MODEL=") && stdoutLines[1].startsWith("ECHO:")
    );
    check("exec: no launcher noise on STDOUT", !out.includes("[genesis]"));
  });
}

function testExitPropagation() {
  withTempDir((td) => {
    const fakeServer = path.join(td, "exit3.js");
    fs.writeFileSync(fakeServer, FAKE_EXIT3, { encoding: "utf-8" });
    const modelDir = makeModelDir(td);
    const proc = spawnSync(NODE, [LAUNCHER, fakeServer], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_MEMORY_BIN: NODE, GENESIS_MODEL_DIR: modelDir }),
    });
    check("exit: child's non-zero exit code is propagated (3)", proc.status === 3);
  });
}

function testMissingServerOverride() {
  withTempDir((td) => {
    const missing = path.join(td, "nope-not-a-file");
    const proc = spawnSync(NODE, [LAUNCHER], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_MEMORY_BIN: missing, GENESIS_MODEL_DIR: makeModelDir(td) }),
    });
    check("missing GENESIS_MEMORY_BIN → exit 1", proc.status === 1);
    check(
      "missing GENESIS_MEMORY_BIN → clear stderr (fail-closed, no download attempted)",
      (proc.stderr || "").includes("GENESIS_MEMORY_BIN is set but is not a file")
    );
  });
}

// ── --stage-hook copies the resolved hook binary into <dest> (via GENESIS_HOOK_BIN, offline) ──
function testStageHook() {
  withTempDir((td) => {
    const fakeHook = path.join(td, "genesis-hook-src");
    fs.writeFileSync(fakeHook, "hook-binary-bytes");
    const dest = path.join(td, "bin");
    const proc = spawnSync(NODE, [LAUNCHER, "--stage-hook", dest], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_HOOK_BIN: fakeHook }),
    });
    check("--stage-hook exits 0 (GENESIS_HOOK_BIN override)", proc.status === 0);
    const staged = path.join(dest, process.platform === "win32" ? "genesis-hook.exe" : "genesis-hook");
    check(
      "--stage-hook copies the hook binary into <dest>",
      fs.existsSync(staged) && fs.readFileSync(staged, "utf8") === "hook-binary-bytes"
    );
    const p2 = spawnSync(NODE, [LAUNCHER, "--stage-hook", dest], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_HOOK_BIN: path.join(td, "missing") }),
    });
    check("--stage-hook with a missing GENESIS_HOOK_BIN → exit 1 (fail-closed)", p2.status === 1);
  });
}

function main() {
  check("launcher file exists", fs.existsSync(LAUNCHER));
  testTransparentExec();
  testExitPropagation();
  testMissingServerOverride();
  testStageHook();
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
