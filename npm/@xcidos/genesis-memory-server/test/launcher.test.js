#!/usr/bin/env node
/* Tests for bin/genesis-memory.js — the npm cross-platform memory-server launcher.

   Node (CommonJS, stdlib-only), NO NETWORK. This launcher is the successor to the retired pure-Python
   download-as-dependency launcher (install/test_launcher.py); npm now delivers the prebuilt native binary
   (optionalDependencies, os/cpu/libc-gated) + the ONNX model package, so the OLD launcher's download /
   checksum / cache / parse-checksums machinery no longer exists and is intentionally not tested here. What
   survived the rearchitecture — and is what these tests cover, driving the REAL launcher as a subprocess:
     * GENESIS_MEMORY_BIN dev override runs a locally-built server directly, forwarding argv + GENESIS_MODEL_DIR;
     * transparent exec: stdin/stdout pass through untouched, STDOUT stays pristine (MCP channel), the child's
       exit status is faithfully propagated;
     * fail-closed: a missing override, or no resolvable platform package, exits non-zero with a clear stderr
       message — the launcher never silently continues.

   Run:  node "npm/@xcidos/genesis-memory-server/test/launcher.test.js"
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const HERE = __dirname;
const LAUNCHER = path.join(HERE, "..", "bin", "genesis-memory.js");
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

// A fake "server": prints MODEL=<GENESIS_MODEL_DIR> then echoes the first stdin line as ECHO:<line>.
const FAKE_SERVER = [
  "'use strict';",
  "process.stdout.write('MODEL=' + (process.env.GENESIS_MODEL_DIR || '') + '\\n');",
  "const chunks = [];",
  "process.stdin.on('data', (c) => {",
  "  chunks.push(c);",
  "  const buf = Buffer.concat(chunks);",
  "  const nl = buf.indexOf(0x0a);",
  "  if (nl !== -1) {",
  "    process.stdout.write('ECHO:' + buf.toString('utf8').slice(0, nl + 1));",
  "    process.exit(0);",
  "  }",
  "});",
  "process.stdin.on('end', () => process.exit(0));",
  "",
].join("\n");

// A fake server that exits with a specific non-zero code (exit-status propagation test).
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

// baseEnv: process.env minus the launcher overrides, so a case controls exactly what is set.
function baseEnv(extra) {
  const env = Object.assign({}, process.env);
  delete env.GENESIS_MEMORY_BIN;
  delete env.GENESIS_MODEL_DIR;
  return Object.assign(env, extra || {});
}

// ── transparent exec via GENESIS_MEMORY_BIN dev override ────────────────────────────────
function testTransparentExec() {
  withTempDir((td) => {
    const fakeServer = path.join(td, "fake_server.js");
    fs.writeFileSync(fakeServer, FAKE_SERVER, { encoding: "utf-8" });
    const modelDir = makeModelDir(td);

    // GENESIS_MEMORY_BIN = the node interpreter; the launcher forwards its own argv (fake_server.js) as the
    // binary's args → it runs `node fake_server.js`, proving args + env + stdio pass through untouched.
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
    // STDOUT must be pristine — only the server wrote to it; the launcher is silent on success (it logs to
    // STDERR only on error). So stdout carries exactly the server's two lines and no launcher noise.
    const stdoutLines = out.split(/\r?\n/).filter((ln) => ln);
    check(
      "exec: STDOUT carries ONLY the server's two lines (launcher silent on success)",
      stdoutLines.length === 2 && stdoutLines[0].startsWith("MODEL=") && stdoutLines[1].startsWith("ECHO:")
    );
    check("exec: no launcher noise on STDOUT", !out.includes("[genesis-memory]"));
    check("exec: launcher is silent on STDERR on the happy path", !(proc.stderr || "").includes("[genesis-memory]"));
  });
}

// ── exit-status propagation ─────────────────────────────────────────────────────────────
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

// ── dev override that does NOT exist → fail closed ─────────────────────────────────────
function testMissingOverrideFailsClosed() {
  withTempDir((td) => {
    const missing = path.join(td, "nope-not-a-file");
    const modelDir = makeModelDir(td);
    const proc = spawnSync(NODE, [LAUNCHER], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_MEMORY_BIN: missing, GENESIS_MODEL_DIR: modelDir }),
    });
    check("missing GENESIS_MEMORY_BIN → exit 1", proc.status === 1);
    check(
      "missing GENESIS_MEMORY_BIN → clear stderr (fail-closed)",
      (proc.stderr || "").includes("[genesis-memory] ERROR") &&
        (proc.stderr || "").includes("GENESIS_MEMORY_BIN is set but is not a file")
    );
  });
}

// ── no override + no resolvable platform package → fail closed ──────────────────────────
function testUnresolvedPlatformFailsClosed() {
  // With no GENESIS_MEMORY_BIN and the platform packages not installed as node_modules, the launcher's
  // require.resolve of the prebuilt server fails → LauncherError → exit 1 with guidance. It never spawns
  // an unresolved binary.
  const proc = spawnSync(NODE, [LAUNCHER], {
    encoding: "utf-8",
    timeout: 60000,
    env: baseEnv({}),
  });
  check("no override + no installed platform pkg → exit 1", proc.status === 1);
  check(
    "no override + no installed platform pkg → clear stderr (fail-closed)",
    (proc.stderr || "").includes("could not find the prebuilt genesis-memory-server")
  );
}

function main() {
  check("launcher file exists", fs.existsSync(LAUNCHER));
  testTransparentExec();
  testExitPropagation();
  testMissingOverrideFailsClosed();
  testUnresolvedPlatformFailsClosed();
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
