#!/usr/bin/env node
/* Tests for bin/genesis-memory.js — the GitHub-Releases launcher (no npm, no token).

   Node (CommonJS, stdlib-only), NO NETWORK. The launcher downloads the platform binaries + model from
   this repo's GitHub Release, SHA256-verifies, caches, and execs the server (default), stages a binary
   (--stage-hook / --stage-cli), or execs a resolved binary (--run-hook / --run-cli). The download path
   needs a real published release, so it isn't exercised here; these tests drive the REAL launcher via the
   dev OVERRIDES (GENESIS_MEMORY_BIN / GENESIS_HOOK_BIN / GENESIS_CLI_BIN / GENESIS_MODEL_DIR), the offline
   dev/CI path:
     * transparent exec: argv + GENESIS_MODEL_DIR forwarded, stdin/stdout pass through, STDOUT pristine,
       child exit status propagated;
     * --stage-hook / --stage-cli copy the resolved binary into <dest>;
     * --run-hook execs the staged hook, propagating exit code, FAIL-OPEN (exit 0) when unresolved;
     * --run-cli execs the resolved cli, propagating exit code, FAIL-LOUD (exit 1) when unresolved;
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

// Fake child "binary" for --run-hook / --run-cli: prints a marker to stdout, then exits 2. Run as the
// resolved binary's argument via GENESIS_*_BIN=NODE (offline, cross-platform — no shebang needed).
const FAKE_CHILD = ["'use strict';", "process.stdout.write('CHILDRAN\\n');", "process.exit(2);", ""].join("\n");

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

// ── --stage-cli copies the resolved cli binary into <dest> (via GENESIS_CLI_BIN, offline) ──
function testStageCli() {
  withTempDir((td) => {
    const fakeCli = path.join(td, "genesis-cli-src");
    fs.writeFileSync(fakeCli, "cli-binary-bytes");
    const dest = path.join(td, "bin");
    const proc = spawnSync(NODE, [LAUNCHER, "--stage-cli", dest], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_CLI_BIN: fakeCli }),
    });
    check("--stage-cli exits 0 (GENESIS_CLI_BIN override)", proc.status === 0);
    const staged = path.join(dest, process.platform === "win32" ? "genesis-cli.exe" : "genesis-cli");
    check(
      "--stage-cli copies the cli binary into <dest>",
      fs.existsSync(staged) && fs.readFileSync(staged, "utf8") === "cli-binary-bytes"
    );
    const p2 = spawnSync(NODE, [LAUNCHER, "--stage-cli", dest], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_CLI_BIN: path.join(td, "missing") }),
    });
    check("--stage-cli with a missing GENESIS_CLI_BIN → exit 1 (fail-closed)", p2.status === 1);
  });
}

// ── --run-hook: exec the resolved hook (exit-code passthrough) + FAIL-OPEN when unresolved ──
function testRunHook() {
  withTempDir((td) => {
    const child = path.join(td, "fake_hook_child.js");
    fs.writeFileSync(child, FAKE_CHILD, { encoding: "utf-8" });
    // GENESIS_HOOK_BIN=NODE, so `--run-hook <child.js>` execs `node <child.js>`.
    const proc = spawnSync(NODE, [LAUNCHER, "--run-hook", child], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_HOOK_BIN: NODE }),
    });
    check("--run-hook execs the resolved hook + propagates its exit code (2)", proc.status === 2);
    check("--run-hook passes the child's stdout through", (proc.stdout || "").includes("CHILDRAN"));

    // Unresolved (no override, empty project dir) → fail-OPEN (exit 0), never break a session.
    const p2 = spawnSync(NODE, [LAUNCHER, "--run-hook", "gate"], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ CLAUDE_PROJECT_DIR: td }),
    });
    check("--run-hook with no resolvable binary → exit 0 (fail-open)", p2.status === 0);
  });
}

// ── --run-cli: exec the resolved cli (exit-code passthrough) + FAIL-LOUD when unresolved ──
function testRunCli() {
  withTempDir((td) => {
    const child = path.join(td, "fake_cli_child.js");
    fs.writeFileSync(child, FAKE_CHILD, { encoding: "utf-8" });
    const proc = spawnSync(NODE, [LAUNCHER, "--run-cli", child], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_CLI_BIN: NODE }),
    });
    check("--run-cli execs the resolved cli + propagates its exit code (2)", proc.status === 2);
    check("--run-cli passes the child's stdout through", (proc.stdout || "").includes("CHILDRAN"));

    // Missing override → fail-LOUD (exit 1) with a clear message, no network attempted.
    const p2 = spawnSync(NODE, [LAUNCHER, "--run-cli", "assemble"], {
      encoding: "utf-8",
      timeout: 60000,
      env: baseEnv({ GENESIS_CLI_BIN: path.join(td, "missing") }),
    });
    check("--run-cli with a missing GENESIS_CLI_BIN → exit 1 (fail-loud)", p2.status === 1);
    check(
      "--run-cli fail-loud carries a clear stderr message",
      (p2.stderr || "").includes("GENESIS_CLI_BIN is set but is not a file")
    );
  });
}

// ── --sync refreshes an existing repo's staged binaries to this launcher's version (offline) ──
function testSync() {
  withTempDir((td) => {
    const fakeHook = path.join(td, "hook-src");
    fs.writeFileSync(fakeHook, "HOOK-NEW");
    const fakeCli = path.join(td, "cli-src");
    fs.writeFileSync(fakeCli, "CLI-NEW");
    const gh = path.join(td, "repo", ".genesis");
    fs.mkdirSync(gh, { recursive: true }); // an EXISTING workspace
    const env = baseEnv({ GENESIS_HOOK_BIN: fakeHook, GENESIS_CLI_BIN: fakeCli });
    const exe = process.platform === "win32" ? ".exe" : "";
    const hookExe = path.join(gh, "bin", "genesis-hook" + exe);
    const stamp = path.join(gh, "bin", ".staged-version");

    // 1. stale (no stamp) -> stages both binaries + writes the stamp
    const p1 = spawnSync(NODE, [LAUNCHER, "--sync", gh], { encoding: "utf-8", timeout: 60000, env });
    check("--sync exits 0", p1.status === 0);
    check(
      "--sync stages the hook binary",
      fs.existsSync(hookExe) && fs.readFileSync(hookExe, "utf8") === "HOOK-NEW"
    );
    check("--sync stages the cli binary", fs.existsSync(path.join(gh, "bin", "genesis-cli" + exe)));
    check("--sync writes the version stamp", fs.existsSync(stamp));

    // 2. current stamp -> no-op (prove by tampering the staged file; a no-op leaves it untouched)
    fs.writeFileSync(hookExe, "TAMPERED");
    const p2 = spawnSync(NODE, [LAUNCHER, "--sync", gh], { encoding: "utf-8", timeout: 60000, env });
    check("--sync is a no-op when the stamp is current (exit 0)", p2.status === 0);
    check("--sync does NOT re-stage when already current", fs.readFileSync(hookExe, "utf8") === "TAMPERED");

    // 3. non-existent workspace -> fail-open no-op (never break session start)
    const p3 = spawnSync(NODE, [LAUNCHER, "--sync", path.join(td, "nope", ".genesis")], {
      encoding: "utf-8",
      timeout: 60000,
      env,
    });
    check("--sync on a missing workspace is a no-op (exit 0)", p3.status === 0);
  });
}

function main() {
  check("launcher file exists", fs.existsSync(LAUNCHER));
  testTransparentExec();
  testExitPropagation();
  testMissingServerOverride();
  testStageHook();
  testStageCli();
  testRunHook();
  testRunCli();
  testSync();
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
