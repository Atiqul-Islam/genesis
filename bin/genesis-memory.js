#!/usr/bin/env node
"use strict";

// Genesis launcher — GitHub Releases distribution (no npm, no registry token).
//
// It downloads the prebuilt native binaries + the ONNX model from this repo's GitHub Release for the
// pinned version, caches them per-user, verifies each with SHA256, and then does one of:
//   * (default)            execs the memory server over stdio for Claude Code's MCP channel;
//   * (--stage-hook <dir>) copies the genesis-hook binary into <dir> (used by bootstrap to populate a
//                          repo's .genesis/bin so the enforcement hooks run a DIRECT native binary);
//   * (--stage-cli <dir>)  copies the genesis-cli binary into <dir> (the installer/orchestrator);
//   * (--run-hook <sub>…)  resolves the STAGED genesis-hook binary and execs `<hook> <sub> …`, passing
//                          stdin/stdout through, fail-OPEN (exit 0 if unresolved). This is the plugin's
//                          per-OS hook shim — one Node file for the whole toolchain instead of a separate
//                          run.js. ASSEMBLED agents skip it (their frontmatter names the binary directly);
//   * (--run-cli <sub>…)   ensures the genesis-cli binary (download+cache) and execs `<cli> <sub> …` — the
//                          installer/orchestrator entry point (bootstrap/assemble/promote/…). Fail-LOUD
//                          (exit 1 if the binary can't be resolved) since the user asked for it explicitly;
//   * (--sync <dir>)       refresh an EXISTING repo workspace's `.genesis/bin` (binaries + launcher copy) to
//                          THIS launcher's RELEASE_VERSION when a version stamp shows they're stale — a
//                          one-file-read no-op otherwise. Fail-OPEN. This is how a `/plugin update` reaches
//                          already-bootstrapped repos: the plugin's SubagentStart hook runs it, so a repo's
//                          staged hook binary tracks the plugin with no manual staging.
//
// The release assets are PUBLIC, fetched over HTTPS — a consumer needs no credentials. Node.js is the
// only prerequisite (18+ for global fetch); no third-party runtime dependencies.
//
// Referenced by .mcp.json as: { "command": "node", "args": ["<path>/bin/genesis-memory.js"] }
//
// Environment overrides (all optional):
//   GENESIS_MEMORY_BIN   run this locally-built server binary directly (dev/CI — skips download)
//   GENESIS_HOOK_BIN     use this locally-built genesis-hook binary for --stage-hook / --run-hook (dev/CI)
//   GENESIS_CLI_BIN      use this locally-built genesis-cli binary for --stage-cli (dev/CI)
//   GENESIS_MODEL_DIR    use this model directory as-is (must hold onnx/model.onnx + tokenizer.json)
//   GENESIS_CACHE_DIR    override the per-user cache root

const fs = require("fs");
const os = require("os");
const path = require("path");
const crypto = require("crypto");
const childProcess = require("child_process");

// The GitHub Release tag (minus the leading "v") to fetch, and the repo. BUMP RELEASE_VERSION per
// release (same commit as the git tag). This is the single source of truth for which assets to pull.
const RELEASE_VERSION = "0.2.0-beta.6";
const REPO = "Atiqul-Islam/genesis";
const SERVER_STEM = "genesis-memory-server";
const HOOK_STEM = "genesis-hook";
const CLI_STEM = "genesis-cli";

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

async function ensureCliBin() {
  const override = process.env.GENESIS_CLI_BIN;
  if (override) {
    if (!fs.existsSync(override)) throw new LauncherError("GENESIS_CLI_BIN is set but is not a file: " + override);
    return override;
  }
  const asset = CLI_STEM + "-" + platformKey() + exeSuffix();
  return ensureAsset(asset, path.join(versionDir(), CLI_STEM + exeSuffix()), true);
}

async function ensureModelDir() {
  const override = process.env.GENESIS_MODEL_DIR;
  if (override) return override;
  const dir = path.join(versionDir(), "model");
  await ensureAsset("model.onnx", path.join(dir, "onnx", "model.onnx"), false);
  await ensureAsset("tokenizer.json", path.join(dir, "tokenizer.json"), false);
  return dir;
}

// Ensure <bin> (already resolved) is copied into <dest> as <stem>[.exe], executable. Shared by
// --stage-hook / --stage-cli.
function stageInto(bin, dest, stem) {
  fs.mkdirSync(dest, { recursive: true });
  const out = path.join(dest, stem + exeSuffix());
  fs.copyFileSync(bin, out);
  try {
    fs.chmodSync(out, 0o755);
  } catch (_e) {
    // non-POSIX filesystem — ignore
  }
  return out;
}

// --stage-hook <dest>: ensure the hook binary is cached, then copy it into <dest>.
async function stageHook(dest) {
  return stageInto(await ensureHookBin(), dest, HOOK_STEM);
}

// --stage-cli <dest>: ensure the genesis-cli binary is cached, then copy it into <dest>.
async function stageCli(dest) {
  return stageInto(await ensureCliBin(), dest, CLI_STEM);
}

// --run-hook <sub> [args…]: resolve the STAGED genesis-hook binary (GENESIS_HOOK_BIN, else
// <project>/.genesis/bin/genesis-hook[.exe]) and exec `<hook> <sub> …`, inheriting stdin/stdout so the
// hook event JSON and the decision JSON pass through untouched. FAIL-OPEN: a missing/unspawnable binary
// exits 0 so it can never break a session (the deterministic checks simply don't run until staged).
function resolveStagedHook() {
  const override = process.env.GENESIS_HOOK_BIN;
  if (override && fs.existsSync(override)) return override;
  const proj = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const staged = path.join(proj, ".genesis", "bin", HOOK_STEM + exeSuffix());
  return fs.existsSync(staged) ? staged : null;
}

// Spawn <bin> with <args>, inheriting stdio, forwarding the child's exit code / signal as ours.
function execPassthrough(bin, args, onSpawnError) {
  const child = childProcess.spawn(bin, args, { stdio: "inherit" });
  child.on("error", onSpawnError);
  child.on("exit", function (code, signal) {
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

function runHook(hookArgs) {
  const bin = resolveStagedHook();
  if (!bin) {
    process.exit(0); // fail-open: a missing hook binary must never break the session
    return;
  }
  // fail-OPEN on spawn error too — a broken hook binary must never break a session.
  execPassthrough(bin, hookArgs, function () {
    process.exit(0);
  });
}

// --run-cli <sub> [args…]: ensure the genesis-cli binary (download+cache, or GENESIS_CLI_BIN) and exec it.
// FAIL-LOUD: unlike the hook shim, the user asked for this explicitly, so a missing binary is exit 1.
function runCli(cliArgs) {
  ensureCliBin()
    .then(function (bin) {
      execPassthrough(bin, cliArgs, function (err) {
        log("ERROR: failed to launch genesis-cli: " + err.message);
        process.exit(1);
      });
    })
    .catch(function (e) {
      log("ERROR: " + (e instanceof LauncherError ? e.message : e && e.stack ? e.stack : String(e)));
      process.exit(1);
    });
}

// --sync <genesis_home>: keep an EXISTING repo workspace's staged binaries + launcher current with THIS
// launcher's RELEASE_VERSION. A version stamp makes the common case a single file read (no work), so it is
// safe to run at (sub)agent start. FAIL-OPEN: any error is logged to stderr and swallowed — a stale binary
// or an offline machine must never break session start.
async function syncRepo(genesisHome) {
  try {
    if (!fs.existsSync(genesisHome)) return; // not a workspace — bootstrap CREATES; --sync only REFRESHES
    const binDir = path.join(genesisHome, "bin");
    const stamp = path.join(binDir, ".staged-version");
    let current = "";
    try {
      current = fs.readFileSync(stamp, "utf8").trim();
    } catch (_e) {
      // no stamp yet (first sync, or pre-stamp install) -> fall through and stage
    }
    if (current === RELEASE_VERSION) return; // already current — nothing to do
    log("syncing " + genesisHome + " to v" + RELEASE_VERSION + " (was " + (current || "unstamped") + ") ...");
    await stageHook(binDir);
    await stageCli(binDir);
    // refresh the repo's own launcher copy so its RELEASE_VERSION matches (harmless self-copy if same file)
    try {
      const selfCopy = path.join(binDir, "genesis-memory.js");
      if (path.resolve(selfCopy) !== path.resolve(__filename)) fs.copyFileSync(__filename, selfCopy);
    } catch (_e) {
      // ignore — the binaries are what matter
    }
    // Heal the managed .gitignore block so a workspace bootstrapped with an OLDER template adopts the
    // current one (e.g. the `!.genesis/memory.db` re-include that lets the vector DB travel with the repo).
    // The block is single-sourced in the Rust cli; run it on the repo root (parent of the .genesis home).
    // Fail-open: a gitignore heal must never break session start.
    try {
      const cliBin = path.join(binDir, "genesis-cli" + (process.platform === "win32" ? ".exe" : ""));
      if (fs.existsSync(cliBin)) {
        childProcess.spawnSync(cliBin, ["sync-gitignore", path.dirname(genesisHome)], { stdio: "ignore" });
      }
    } catch (_e) {
      // ignore — never block on a gitignore heal
    }
    fs.writeFileSync(stamp, RELEASE_VERSION + "\n");
    log("synced " + genesisHome + " -> v" + RELEASE_VERSION);
  } catch (e) {
    log("sync skipped (non-fatal): " + (e && e.message ? e.message : String(e)));
  }
}

function execServer(binPath, modelDir) {
  // Only override GENESIS_MODEL_DIR when a model dir was actually resolved (the stdio server + `import`
  // need it); model-free one-shots (`structure`, `export`) pass the environment through untouched.
  const env = modelDir ? Object.assign({}, process.env, { GENESIS_MODEL_DIR: modelDir }) : process.env;
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

  // Hook shim: resolve the staged genesis-hook and exec it (fail-open). Must run BEFORE any download.
  if (argv[0] === "--run-hook") {
    runHook(argv.slice(1));
    return;
  }

  // Orchestrator: ensure the genesis-cli binary (download+cache) and exec it (fail-loud).
  if (argv[0] === "--run-cli") {
    runCli(argv.slice(1));
    return;
  }

  // Version-sync: refresh a repo workspace's staged binaries to this launcher's version (fail-open).
  if (argv[0] === "--sync") {
    const dest = argv[1];
    if (dest) await syncRepo(dest);
    process.exit(0);
    return;
  }

  // Staging modes: download/resolve a binary and copy it into <dest>, then exit.
  if (argv[0] === "--stage-hook" || argv[0] === "--stage-cli") {
    const mode = argv[0];
    const dest = argv[1];
    if (!dest) {
      log("ERROR: " + mode + " requires a destination directory");
      process.exit(1);
      return;
    }
    try {
      const out = mode === "--stage-cli" ? await stageCli(dest) : await stageHook(dest);
      log("staged " + (mode === "--stage-cli" ? CLI_STEM : HOOK_STEM) + " -> " + out);
      process.exit(0);
    } catch (e) {
      log("ERROR: " + (e instanceof LauncherError ? e.message : e && e.stack ? e.stack : String(e)));
      process.exit(1);
    }
    return;
  }

  // Model-free one-shot subcommands: resolve ONLY the server binary and exec it — skip the (potentially
  // large) ONNX model download the stdio server needs. `structure` is the PostToolUse hook's write-back
  // (Mneme adds structure + supersedes; never embeds); `export` mirrors the DB to JSONL; `unstructured`
  // lists memories awaiting structure (the migrate input). NOT `import`, which re-embeds and so falls
  // through to the default path that resolves the model.
  if (argv[0] === "structure" || argv[0] === "export" || argv[0] === "unstructured") {
    let sbin;
    try {
      sbin = await ensureServerBin();
    } catch (e) {
      log("ERROR: " + (e instanceof LauncherError ? e.message : e && e.stack ? e.stack : String(e)));
      process.exit(1);
      return;
    }
    execServer(sbin); // no model dir — pass the environment through untouched
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
