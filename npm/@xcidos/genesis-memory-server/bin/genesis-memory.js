#!/usr/bin/env node
"use strict";

// Genesis memory server -- cross-platform npm launcher.
//
// This mirrors esbuild's `generateBinPath` model (also used by @napi-rs, biome, and sharp,
// and how Claude Code itself ships native binaries): at run time we compute the
// `${platform}-${arch}(-${libc})` key for the current machine, resolve the matching prebuilt
// native server that npm installed as an optional dependency, point the server at the ONNX
// model shipped by @xcidos/genesis-memory-model, and spawn it with the MCP stdio stream
// inherited untouched.
//
// The one prerequisite is Node.js -- the accepted baseline for the Claude Code + MCP
// ecosystem. There is no Python, no local Rust build, and no network access at run time.
//
// Referenced by .mcp.json as:
//   { "command": "npx", "args": ["-y", "@xcidos/genesis-memory-server"] }
//
// Environment overrides (all optional):
//   GENESIS_MEMORY_BIN   run this locally-built server binary directly (dev/CI override)
//   GENESIS_MODEL_DIR    use this model directory as-is (must hold onnx/model.onnx +
//                        tokenizer.json); otherwise the bundled model package is used

const path = require("path");
const fs = require("fs");
const childProcess = require("child_process");

const SERVER_PKG_PREFIX = "@xcidos/genesis-memory-server-";
const MODEL_PKG = "@xcidos/genesis-memory-model";
const BIN_STEM = "genesis-memory-server"; // Cargo [[bin]] name

/** A fail-closed launcher condition (unsupported platform, missing package, bad override). */
class LauncherError extends Error {}

/** Diagnostics MUST go to stderr -- stdout is the MCP channel and must stay pristine. */
function log(msg) {
  process.stderr.write("[genesis-memory] " + msg + "\n");
}

// -- musl detection (biome pattern) -----------------------------------------------------
// Only meaningful on Linux. Fast path: Node's process.report exposes the glibc runtime
// version on glibc systems, so its presence rules musl out. Backstop: parse `ldd --version`
// -- musl's ldd prints "musl" (usually to stderr, with a non-zero exit), glibc's prints
// "GLIBC"/"GNU libc". We read both streams in either case and look for "musl".
function isLinuxMusl() {
  try {
    const report =
      process.report && typeof process.report.getReport === "function"
        ? process.report.getReport()
        : null;
    if (report && report.header && report.header.glibcVersionRuntime) {
      return false; // glibc runtime present -> not musl
    }
  } catch (_e) {
    // fall through to the ldd backstop
  }
  try {
    const out = childProcess.execSync("ldd --version", {
      stdio: ["ignore", "pipe", "pipe"],
    });
    return out.toString().includes("musl");
  } catch (err) {
    const combined = String((err && err.stdout) || "") + String((err && err.stderr) || "");
    return combined.includes("musl");
  }
}

// -- platform key + package/subpath for the current machine -----------------------------
function platformKey() {
  const platform = process.platform; // 'darwin' | 'linux' | 'win32' | ...
  const arch = process.arch; // 'x64' | 'arm64' | ...
  if (platform === "linux") {
    return platform + "-" + arch + "-" + (isLinuxMusl() ? "musl" : "gnu");
  }
  return platform + "-" + arch;
}

function exeName() {
  return process.platform === "win32" ? BIN_STEM + ".exe" : BIN_STEM;
}

// -- resolve the native binary (esbuild generateBinPath pattern) ------------------------
function generateBinPath() {
  // Dev / CI override: run a locally-built binary directly.
  const override = process.env.GENESIS_MEMORY_BIN;
  if (override) {
    if (!fs.existsSync(override)) {
      throw new LauncherError("GENESIS_MEMORY_BIN is set but is not a file: " + override);
    }
    return override;
  }

  const key = platformKey();
  const pkg = SERVER_PKG_PREFIX + key;
  const subpath = exeName();
  try {
    // A scoped package + a plain file subpath (no `exports` field on the platform packages),
    // so this is a straight on-disk file resolution -- exactly esbuild's pattern.
    return require.resolve(pkg + "/" + subpath);
  } catch (e) {
    throw new LauncherError(
      "could not find the prebuilt genesis-memory-server for this platform (" +
        key +
        ").\n" +
        'Expected the optional dependency "' +
        pkg +
        '" to be installed, providing "' +
        subpath +
        '".\n\n' +
        "Likely causes:\n" +
        "  - This OS/arch is not one of the published targets.\n" +
        "  - Installed with --no-optional, or restored from a lockfile generated on a\n" +
        "    different OS/arch (a known npm optionalDependencies bug). Delete node_modules\n" +
        "    and the lockfile, then reinstall.\n" +
        "  - A private/offline registry that did not mirror the platform packages.\n\n" +
        "To run a locally-built server instead, build it and set GENESIS_MEMORY_BIN:\n" +
        "  cd server && cargo build --release\n" +
        "  export GENESIS_MEMORY_BIN=\"$PWD/target/release/" +
        subpath +
        '"\n\n' +
        "Underlying resolver error: " +
        (e && e.message ? e.message : String(e))
    );
  }
}

// -- resolve the model directory --------------------------------------------------------
// The native server locates its embedder via GENESIS_MODEL_DIR (a directory holding
// onnx/model.onnx + tokenizer.json). We honor an explicit override (dev, or a user who
// fetched the model elsewhere) -- matching the legacy launcher -- otherwise we point it at
// the bundled @xcidos/genesis-memory-model package.
function resolveModelDir() {
  const override = process.env.GENESIS_MODEL_DIR;
  if (override) return override;
  try {
    return path.dirname(require.resolve(MODEL_PKG + "/package.json"));
  } catch (e) {
    throw new LauncherError(
      'could not find the embedding model package "' +
        MODEL_PKG +
        '".\n' +
        "It is a hard dependency of the launcher and ships onnx/model.onnx + tokenizer.json.\n" +
        "Reinstall @xcidos/genesis-memory-server, or set GENESIS_MODEL_DIR to a directory that\n" +
        "contains onnx/model.onnx and tokenizer.json.\n\n" +
        "Underlying resolver error: " +
        (e && e.message ? e.message : String(e))
    );
  }
}

function main() {
  let binPath;
  let modelDir;
  try {
    binPath = generateBinPath();
    modelDir = resolveModelDir();
  } catch (e) {
    if (e instanceof LauncherError) {
      log("ERROR: " + e.message);
    } else {
      log("ERROR: " + (e && e.stack ? e.stack : String(e)));
    }
    process.exit(1);
    return;
  }

  const env = Object.assign({}, process.env, { GENESIS_MODEL_DIR: modelDir });
  const args = process.argv.slice(2);

  // stdio:'inherit' -> the child owns fd 0/1/2 directly, so the MCP byte stream flows
  // between the client and the server with the launcher touching neither channel.
  const child = childProcess.spawn(binPath, args, { stdio: "inherit", env: env });

  // Forward termination signals so the native server shuts down with us. Claude Code stops
  // an MCP server by signalling this process; without forwarding, the child would orphan.
  const signals = ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT", "SIGBREAK"];
  const forwarders = {};
  for (const sig of signals) {
    forwarders[sig] = function () {
      try {
        child.kill(sig);
      } catch (_e) {
        // child already gone
      }
    };
    try {
      process.on(sig, forwarders[sig]);
    } catch (_e) {
      // signal not supported on this OS -- ignore
    }
  }

  child.on("error", function (err) {
    log("ERROR: failed to launch " + binPath + ": " + err.message);
    process.exit(1);
  });

  child.on("exit", function (code, signal) {
    for (const sig of signals) {
      try {
        process.removeListener(sig, forwarders[sig]);
      } catch (_e) {
        // ignore
      }
    }
    if (signal) {
      // Re-raise the same signal so our own exit status faithfully reflects the child's death.
      try {
        process.kill(process.pid, signal);
        return;
      } catch (_e) {
        process.exit(1);
        return;
      }
    }
    process.exit(code === null ? 0 : code);
  });
}

main();
