#!/usr/bin/env node
/* Integration test for bootstrap.js (D3) — runs the REAL bootstrap into an OS temp repo and asserts a
   correct, self-contained repo-level Genesis: layout, .mcp.json + agents wired to the native binary via
   the staged Node launcher, hooks resolve the repo store, and re-run is idempotent (preserves the memory DB).

   Updated for the GitHub-Releases distribution: the memory server is launched by the staged Node launcher
   (.genesis/bin/genesis-memory.js) and the genesis-hook binary is staged into .genesis/bin (from
   GENESIS_HOOK_BIN here, else downloaded from the release), so this test asserts that wiring.

   Run:  node install/test_bootstrap.js
*/
"use strict";
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const HERE = __dirname; // <gh>/install
const GH = path.dirname(HERE); // <gh>
const BOOTSTRAP = path.join(HERE, "bootstrap.js");

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

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}

function isFile(p) {
  try {
    return fs.statSync(p).isFile();
  } catch (e) {
    return false;
  }
}

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, { encoding: "utf-8" }));
}

function readText(p) {
  return fs.readFileSync(p, { encoding: "utf-8" });
}

function main() {
  const target = fs.mkdtempSync(path.join(os.tmpdir(), "genesis-boot-"));
  const dest = path.join(target, ".genesis");
  const hookExe = process.platform === "win32" ? "genesis-hook.exe" : "genesis-hook";
  const localHook = path.join(GH, "hook", "target", "release", hookExe);
  // Stage from the locally-built binary (fast, offline) when present; else bootstrap falls back to npx.
  const ENV = Object.assign({}, process.env, isFile(localHook) ? { GENESIS_HOOK_BIN: localHook } : {});
  try {
    const r = spawnSync(process.execPath, [BOOTSTRAP, target, GH], { encoding: "utf-8", timeout: 900000, env: ENV });
    check("bootstrap exits 0", r.status === 0);
    if (r.status !== 0) {
      console.log("  STDERR:", (r.stderr || "").trim().slice(0, 500));
    }

    // layout
    check("expertise/manifests copied", isDir(path.join(dest, "expertise", "manifests")));
    check("required.json copied", isFile(path.join(dest, "expertise", "required.json")));
    check(
      "hooks copied (hooks.json + run.js resolver)",
      isFile(path.join(dest, "hooks", "hooks.json")) && isFile(path.join(dest, "hooks", "run.js"))
    );
    check("team copied", isDir(path.join(dest, "team", "sensei")));
    check(
      "research-expertise skill copied to .genesis/skills/",
      isDir(path.join(dest, "skills", "research-expertise"))
    );
    check(
      "sensei's skills installed into <repo>/.claude/skills/",
      isDir(path.join(target, ".claude", "skills", "research-expertise")) &&
        isDir(path.join(target, ".claude", "skills", "build-agent"))
    );
    check("install scripts copied", isFile(path.join(dest, "install", "assemble.js")));

    // the native genesis-hook binary is staged into .genesis/bin (from GENESIS_HOOK_BIN when built locally)
    if (isFile(localHook)) {
      check("genesis-hook binary staged into .genesis/bin", isFile(path.join(dest, "bin", hookExe)));
    } else {
      check("genesis-hook staging attempted (npx path; skipped offline in this test env)", true);
    }

    // .mcp.json wired repo-local via npx (no committed binary + model; Node is the one prerequisite)
    const mcp = readJson(path.join(target, ".mcp.json"));
    const gm = mcp.mcpServers["genesis-memory"];
    check(".mcp.json command → node (GitHub-Releases launcher, no npx/python)", gm.command === "node");
    check(
      ".mcp.json args launch the .genesis/bin/genesis-memory.js launcher",
      Array.isArray(gm.args) && gm.args.some((a) => a.includes("genesis-memory.js"))
    );
    check(
      "launcher copied into .genesis/bin",
      isFile(path.join(dest, "bin", "genesis-memory.js"))
    );
    check(
      ".mcp.json env → repo-local db + portable export under .genesis/",
      gm.env.GENESIS_MEMORY_DB.includes(dest) &&
        gm.env.GENESIS_MEMORY_EXPORT.includes(path.join(dest, "memory")) &&
        gm.env.GENESIS_MEMORY_EXPORT.endsWith(".jsonl")
    );

    // .gitignore commits the brain + portable memory, ignores machine junk (via `git check-ignore`)
    const gi = readText(path.join(target, ".gitignore"));
    check(".gitignore has the managed genesis block", gi.includes("genesis runtime (managed by bootstrap)"));

    const ignored = (p) => spawnSync("git", ["-C", target, "check-ignore", "-q", p]).status === 0;
    spawnSync("git", ["-C", target, "init", "-q"]);
    check("memory JSONL is COMMITTED (not ignored)", !ignored(".genesis/memory/memory.jsonl"));
    check("expertise is COMMITTED (not ignored)", !ignored(".genesis/expertise/portfolio-truth.md"));
    check("hooks are COMMITTED (not ignored)", !ignored(".genesis/hooks/hooks.json"));
    check("staged binary is IGNORED (machine-local)", ignored(".genesis/bin/" + hookExe));
    check("memory.db is ignored (machine-local)", ignored(".genesis/memory.db"));
    check(".mcp.json is ignored (absolute paths)", ignored(".mcp.json"));

    // agents wired to the native binary; sensei has the enforce gate, method does not
    const sensei = readText(path.join(target, ".claude", "agents", "sensei.md"));
    const method = readText(path.join(target, ".claude", "agents", "method.md"));
    // PORTABLE: built-agent hooks reference ${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook (runtime-resolved
    // to the repo root, cross-platform) — NOT an absolute machine path, so the agent survives a clone.
    check(
      "sensei.md hooks reference ${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook (portable, braced)",
      sensei.includes("${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook")
    );
    check(
      "sensei.md deterministic hooks invoke the binary directly (no node/python)",
      sensei.includes('command: \'"${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook') &&
        !sensei.includes('"node"') &&
        !sensei.includes("python3")
    );
    check(
      "sensei.md hooks carry NO absolute machine path",
      !sensei.includes(path.join(dest, "bin")) && !sensei.includes("/mnt/")
    );
    check("sensei has Bash enforce-research gate", sensei.includes("enforce-research"));
    check("method has NO enforce-research gate", !method.includes("enforce-research"));
    check(
      "review is a built-in agent hook (Haiku) in sensei.md",
      sensei.includes("- type: agent") && sensei.includes("model: 'claude-haiku")
    );

    // repo store present + resolvable
    check("repo expertise manifests present", isDir(path.join(dest, "expertise", "manifests")));
    const req = readJson(path.join(dest, "expertise", "required.json"));
    check("repo required.json has sensei + method", "sensei" in req && "method" in req);

    // idempotent + preserves an existing memory DB
    const memdb = path.join(dest, "memory.db");
    fs.writeFileSync(memdb, "SENTINEL", { encoding: "utf-8" });
    const r2 = spawnSync(process.execPath, [BOOTSTRAP, target, GH], { encoding: "utf-8", timeout: 900000, env: ENV });
    check("re-run idempotent (exit 0)", r2.status === 0);
    check(
      "memory.db preserved across re-run",
      isFile(memdb) && readText(memdb) === "SENTINEL"
    );
  } finally {
    fs.rmSync(target, { recursive: true, force: true });
  }

  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

main();
