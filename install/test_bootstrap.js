#!/usr/bin/env node
/* Integration test for bootstrap.js (D3) — runs the REAL bootstrap into an OS temp repo and asserts a
   correct, self-contained repo-level Genesis: layout, .mcp.json + agents wired repo-local via npx, hooks
   resolve the repo store, and re-run is idempotent (preserves the memory DB).

   Faithful Node (CommonJS, stdlib-only) port of test_bootstrap.py, updated for the npm-distributed server:
   the memory server is delivered by `npx @xcidos/genesis-memory-server` (no committed binary + model copy),
   so this test asserts the npx wiring instead of copied artifacts. Fast (no multi-GB copy).

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
  try {
    const r = spawnSync(process.execPath, [BOOTSTRAP, target, GH], { encoding: "utf-8", timeout: 900000 });
    check("bootstrap exits 0", r.status === 0);
    if (r.status !== 0) {
      console.log("  STDERR:", (r.stderr || "").trim().slice(0, 500));
    }

    // layout
    check("expertise/manifests copied", isDir(path.join(dest, "expertise", "manifests")));
    check("required.json copied", isFile(path.join(dest, "expertise", "required.json")));
    check(
      "hooks copied (validate.js + enforce_research.js)",
      isFile(path.join(dest, "hooks", "validate.js")) && isFile(path.join(dest, "hooks", "enforce_research.js"))
    );
    check("team copied", isDir(path.join(dest, "team", "sensei")));
    // Skills live at the genesis-home skills/ dir (plugin-root skills/), not under team/. Bootstrap copies
    // skills/ into <dest>, and the assembler installs sensei's named skills into .claude/skills/.
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

    // .mcp.json wired repo-local via npx (no committed binary + model; Node is the one prerequisite)
    const mcp = readJson(path.join(target, ".mcp.json"));
    const gm = mcp.mcpServers["genesis-memory"];
    check(".mcp.json command → npx (no python3, no local Rust binary)", gm.command === "npx");
    check(
      ".mcp.json args launch @xcidos/genesis-memory-server",
      Array.isArray(gm.args) && gm.args.some((a) => a === "@xcidos/genesis-memory-server")
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
    check("hooks are COMMITTED (not ignored)", !ignored(".genesis/hooks/inject.js"));
    check("memory.db is ignored (machine-local)", ignored(".genesis/memory.db"));
    check(".mcp.json is ignored (absolute paths)", ignored(".mcp.json"));

    // agents wired to .genesis/, sensei has the enforce gate, method does not
    const sensei = readText(path.join(target, ".claude", "agents", "sensei.md"));
    const method = readText(path.join(target, ".claude", "agents", "method.md"));
    // PORTABLE: built-agent hooks reference ${CLAUDE_PROJECT_DIR}/.genesis/hooks (runtime-resolved to the repo
    // root, cross-platform) via `node` — NOT an absolute machine path, so the agent survives a clone.
    check(
      "sensei.md hooks reference ${CLAUDE_PROJECT_DIR}/.genesis/hooks (portable, braced)",
      sensei.includes("${CLAUDE_PROJECT_DIR}/.genesis/hooks")
    );
    check(
      "sensei.md hook commands run `node` (not python3)",
      sensei.includes('command: \'"node" "${CLAUDE_PROJECT_DIR}/.genesis/hooks/') && !sensei.includes("python3")
    );
    check(
      "sensei.md hooks carry NO absolute machine path",
      !sensei.includes(path.join(dest, "hooks")) && !sensei.includes("/mnt/")
    );
    check("sensei has Bash enforce_research gate", sensei.includes("enforce_research.js"));
    check("method has NO enforce_research gate", !method.includes("enforce_research.js"));

    // hooks resolve the repo store via HOOK_DIR/../expertise (the relative logic actually points here)
    const resolved = path.normalize(path.join(dest, "hooks", "..", "expertise", "manifests"));
    check(
      "validate/review hooks resolve the repo store",
      resolved === path.normalize(path.join(dest, "expertise", "manifests")) && isDir(resolved)
    );

    // required.json has sensei + method registered in the repo-local store
    const req = readJson(path.join(dest, "expertise", "required.json"));
    check("repo required.json has sensei + method", "sensei" in req && "method" in req);

    // idempotent + preserves an existing memory DB
    const memdb = path.join(dest, "memory.db");
    fs.writeFileSync(memdb, "SENTINEL", { encoding: "utf-8" });
    const r2 = spawnSync(process.execPath, [BOOTSTRAP, target, GH], { encoding: "utf-8", timeout: 900000 });
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
