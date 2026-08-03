#!/usr/bin/env node
"use strict";

// Genesis hook launcher — resolves the staged `genesis-hook` binary and execs it.
//
// Used by the PLUGIN's static hooks.json (genesis:sensei / genesis:method), where a single
// cross-platform command string can't name a per-OS binary (`genesis-hook` vs `genesis-hook.exe`).
// ASSEMBLED agents do NOT use this shim — assemble.js bakes the direct binary path
// `${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook[.exe]` into their frontmatter (no Node in the hot
// path). This shim's only job is per-OS resolution, so it stays tiny.
//
// Resolution order:
//   1. GENESIS_HOOK_BIN                                        explicit path (dev/CI)
//   2. <project>/.genesis/bin/genesis-hook[.exe]              staged by bootstrap (from the GitHub Release)
//
// stdin (the hook event JSON) and stdout (the decision JSON) are inherited untouched; the child's exit
// code becomes ours. On resolution failure we exit 0 (fail-open: never break a session — the deterministic
// checks simply don't run until bootstrap has staged the binary).

const path = require("path");
const fs = require("fs");
const childProcess = require("child_process");

function exeName() {
  return process.platform === "win32" ? "genesis-hook.exe" : "genesis-hook";
}

function resolveBin() {
  const override = process.env.GENESIS_HOOK_BIN;
  if (override && fs.existsSync(override)) {
    return override;
  }
  const proj = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const staged = path.join(proj, ".genesis", "bin", exeName());
  return fs.existsSync(staged) ? staged : null;
}

function main() {
  const bin = resolveBin();
  if (!bin) {
    process.exit(0); // fail-open: a missing hook binary must never break the session
  }
  const child = childProcess.spawn(bin, process.argv.slice(2), { stdio: "inherit" });
  child.on("error", function () {
    process.exit(0); // spawn failure -> fail-open
  });
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

main();
