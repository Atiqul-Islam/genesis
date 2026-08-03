#!/usr/bin/env node
"use strict";

// Genesis launcher — GitHub Releases distribution (no npm, no registry token).
//
// It downloads the prebuilt native binaries + the ONNX model from this repo's GitHub Release for the
// pinned version, caches them per-user, verifies each with SHA256, and then either:
//   * (default)            execs the memory server over stdio for Claude Code's MCP channel; or
//   * (--stage-hook <dir>) copies the genesis-hook binary into <dir> (used by bootstrap to populate a
//                          repo's .genesis/bin so the enforcement hooks run a DIRECT native binary).
//
// The release assets are PUBLIC, fetched over HTTPS — a consumer needs no credentials. Node.js is the
// only prerequisite (18+ for global fetch); no third-party runtime dependencies.
//
// Referenced by .mcp.json as: { "command": "node", "args": ["<path>/bin/genesis-memory.js"] }
//
// Environment overrides (all optional):
//   GENESIS_MEMORY_BIN   run this locally-built server binary directly (dev/CI — skips download)
//   GENESIS_HOOK_BIN     use this locally-built genesis-hook binary for --stage-hook (dev/CI)
//   GENESIS_MODEL_DIR    use this model directory as-is (must hold onnx/model.onnx + tokenizer.json)
//   GENESIS_CACHE_DIR    override the per-user cache root

const fs = require("fs");
const os = require("os");
const path = require("path");
const crypto = require("crypto");
const childProcess = require("child_process");

// The GitHub Release tag (minus the leading "v") to fetch, and the repo. BUMP RELEASE_VERSION per
// release (same commit as the git tag). This is the single source of truth for which assets to pull.
const RELEASE_VERSION = "0.1.0-beta.1";
const REPO = "Atiqul-Islam/genesis";
const SERVER_STEM = "genesis-memory-server";
const HOOK_STEM = "genesis-hook";

/** A fail-closed launcher condition (unsupported platform, bad download/checksum, bad override). */
class LauncherError extends Error {}

/** Diagnostics MUST go to stderr — stdout is the MCP channel and must stay pristine. */
function log(msg) {
  process.stderr.write("[genesis] " + msg + "\n");
}

// -- musl detection (biome pattern) -----------------------------------------------------
function isLinuxMusl() {
  try {
    const report =
      process.report && typeof process.report.getReport === "function" ? process.report.getReport() : null;
    if (report && report.header && report.header.glibcVersionRuntime) {
      return false; // glibc runtime present -> not musl
    }
  } catch (_e) {
    // fall through to the ldd backstop
  }
  try {
    const out = childProcess.execSync("ldd --version", { stdio: ["ignore", "pipe", "pipe"] });
    return out.toString().includes("musl");
  } catch (err) {
    const combined = String((err && err.stdout) || "") + String((err && err.stderr) || "");
    return combined.includes("musl");
  }
}

// -- platform key (matches the release asset suffixes) ----------------------------------
function platformKey() {
  const platform = process.platform; // 'darwin' | 'linux' | 'win32'
  const arch = process.arch; // 'x64' | 'arm64'
  if (platform === "linux") {
    return platform + "-" + arch + "-" + (isLinuxMusl() ? "musl" : "gnu");
  }
  return platform + "-" + arch;
}

function exeSuffix() {
  return process.platform === "win32" ? ".exe" : "";
}

// -- per-user cache: <cache>/v<version>/{binaries, model/} ------------------------------
function cacheRoot() {
  if (process.env.GENESIS_CACHE_DIR) return process.env.GENESIS_CACHE_DIR;
  if (process.platform === "win32") {
    const base = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
    return path.join(base, "genesis", "cache");
  }
  const xdg = process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(xdg, "genesis");
}

function versionDir() {
  return path.join(cacheRoot(), "v" + RELEASE_VERSION);
}

function assetUrl(name) {
  return "https://github.com/" + REPO + "/releases/download/v" + RELEASE_VERSION + "/" + name;
}

async function fetchBuffer(url) {
  let res;
  try {
    res = await fetch(url, { redirect: "follow" });
  } catch (e) {
    throw new LauncherError("network error fetching " + url + ": " + (e && e.message ? e.message : String(e)));
  }
  if (!res.ok) {
    throw new LauncherError(
      "download failed (HTTP " + res.status + ") for " + url + ".\n" +
        "The release v" + RELEASE_VERSION + " or this platform's asset may not be published yet — " +
        "build from source and set GENESIS_MEMORY_BIN / GENESIS_HOOK_BIN / GENESIS_MODEL_DIR (see README)."
    );
  }
  return Buffer.from(await res.arrayBuffer());
}

// SHA256SUMS: standard `<hex>␠␠<filename>` lines. Cached in-process.
let _sums = null;
async function checksums() {
  if (_sums) return _sums;
  const txt = (await fetchBuffer(assetUrl("SHA256SUMS"))).toString("utf8");
  const map = {};
  for (const line of txt.split(/\r?\n/)) {
    const m = line.match(/^([0-9a-fA-F]{64})\s+\*?(.+)$/);
    if (m) map[m[2].trim()] = m[1].toLowerCase();
  }
  _sums = map;
  return map;
}

function sha256(buf) {
  return crypto.createHash("sha256").update(buf).digest("hex");
}

// Ensure <assetName> is present at <destPath> (cached). Download + verify + atomic-write if missing.
async function ensureAsset(assetName, destPath, executable) {
  if (fs.existsSync(destPath)) return destPath;
  fs.mkdirSync(path.dirname(destPath), { recursive: true });
  log("fetching " + assetName + " (v" + RELEASE_VERSION + ") ...");
  const buf = await fetchBuffer(assetUrl(assetName));
  const want = (await checksums())[assetName];
  if (!want) {
    throw new LauncherError("no checksum listed for " + assetName + " in SHA256SUMS (refusing unverified asset)");
  }
  if (sha256(buf) !== want) {
    throw new LauncherError("SHA256 mismatch for " + assetName + " (expected " + want + ")");
  }
  const tmp = destPath + ".tmp-" + process.pid;
  fs.writeFileSync(tmp, buf);
  if (executable) {
    try {
      fs.chmodSync(tmp, 0o755);
    } catch (_e) {
      // non-POSIX filesystem — ignore
    }
  }
  fs.renameSync(tmp, destPath); // atomic; safe if two launchers race
  return destPath;
}

async function ensureServerBin() {
  const override = process.env.GENESIS_MEMORY_BIN;
  if (override) {
    if (!fs.existsSync(override)) throw new LauncherError("GENESIS_MEMORY_BIN is set but is not a file: " + override);
    return override;
  }
  const asset = SERVER_STEM + "-" + platformKey() + exeSuffix();
  return ensureAsset(asset, path.join(versionDir(), SERVER_STEM + exeSuffix()), true);
}

async function ensureHookBin() {
  const override = process.env.GENESIS_HOOK_BIN;
  if (override) {
    if (!fs.existsSync(override)) throw new LauncherError("GENESIS_HOOK_BIN is set but is not a file: " + override);
    return override;
  }
  const asset = HOOK_STEM + "-" + platformKey() + exeSuffix();
  return ensureAsset(asset, path.join(versionDir(), HOOK_STEM + exeSuffix()), true);
}

async function ensureModelDir() {
  const override = process.env.GENESIS_MODEL_DIR;
  if (override) return override;
  const dir = path.join(versionDir(), "model");
  await ensureAsset("model.onnx", path.join(dir, "onnx", "model.onnx"), false);
  await ensureAsset("tokenizer.json", path.join(dir, "tokenizer.json"), false);
  return dir;
}

// --stage-hook <dest>: ensure the hook binary is cached, then copy it into <dest>.
async function stageHook(dest) {
  const bin = await ensureHookBin();
  fs.mkdirSync(dest, { recursive: true });
  const out = path.join(dest, HOOK_STEM + exeSuffix());
  fs.copyFileSync(bin, out);
  try {
    fs.chmodSync(out, 0o755);
  } catch (_e) {
    // non-POSIX filesystem — ignore
  }
  return out;
}

function execServer(binPath, modelDir) {
  const env = Object.assign({}, process.env, { GENESIS_MODEL_DIR: modelDir });
  const child = childProcess.spawn(binPath, process.argv.slice(2), { stdio: "inherit", env });

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
      // signal not supported on this OS
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

async function main() {
  const argv = process.argv.slice(2);

  // Staging mode: download/resolve the hook binary and copy it into <dest>, then exit.
  if (argv[0] === "--stage-hook") {
    const dest = argv[1];
    if (!dest) {
      log("ERROR: --stage-hook requires a destination directory");
      process.exit(1);
      return;
    }
    try {
      const out = await stageHook(dest);
      log("staged genesis-hook -> " + out);
      process.exit(0);
    } catch (e) {
      log("ERROR: " + (e instanceof LauncherError ? e.message : e && e.stack ? e.stack : String(e)));
      process.exit(1);
    }
    return;
  }

  // Default: resolve the server binary + model (download if needed), then exec the server.
  let binPath;
  let modelDir;
  try {
    binPath = await ensureServerBin();
    modelDir = await ensureModelDir();
  } catch (e) {
    log("ERROR: " + (e instanceof LauncherError ? e.message : e && e.stack ? e.stack : String(e)));
    process.exit(1);
    return;
  }
  execServer(binPath, modelDir);
}

main();
