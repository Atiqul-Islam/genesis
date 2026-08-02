#!/usr/bin/env node
/* Bootstrap a SELF-CONTAINED, repo-level Genesis workspace at <target_repo>/.genesis/.

   Faithful Node (CommonJS, stdlib-only) port of bootstrap.py, updated for the npm-distributed memory server:
   the generated repo `.mcp.json` launches the server via `npx @atiqul/genesis-memory-server` (Node is the one
   prerequisite) instead of a committed Rust binary — so there is NO local Rust build and NO multi-GB binary +
   model copy. Node is the only runtime a user's machine needs.

   Genesis's expertise store, memory, hooks, and team live PER REPOSITORY (never global). When Sensei is asked
   to build an agent in a repo that has no `.genesis/` workspace yet, this creates one in a single deterministic
   call, so the repo becomes a standalone Genesis: its own store + hooks + memory DB + base modules + sensei/method.

   What it does (idempotent, cross-platform):
     1. Copies the functional Genesis tree into <repo>/.genesis/: expertise/ (+manifests/, required.json),
        hooks/, skills/ (the team skills build-agent/research-expertise + the spec-forge suite), team/
        (sensei+method personas/behaviors), install/ (assemble/install/bootstrap).
     2. Registers a repo-local `genesis-memory` MCP server in <repo>/.mcp.json (merged, not clobbered) that runs
        `npx -y @atiqul/genesis-memory-server`, pointing the memory DB + portable JSONL export at <repo>/.genesis/.
     3. Installs sensei + method into <repo>/.claude/agents/ via the assembler with genesis_home = <repo>/.genesis,
        so their hook commands reference <repo>/.genesis/hooks/*.js and the validate/review hooks resolve the
        store at <repo>/.genesis/expertise/ through their existing HOOK_DIR/../expertise relative logic.
   Re-running refreshes the copied tree + agents WITHOUT clobbering an existing memory DB.

   usage: bootstrap.js <target_repo> [genesis_home]     # genesis_home from $GENESIS_HOME, else this repo root
*/
"use strict";
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const TEAM = ["sensei", "method"];

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch (e) {
    return false;
  }
}

function fail(msg) {
  process.stderr.write(msg + "\n");
  process.exit(1);
}

function copyTree(src, dst) {
  // shutil.copytree(dirs_exist_ok=True) equivalent, skipping __pycache__ dirs and *.pyc files.
  fs.cpSync(src, dst, {
    recursive: true,
    filter: (s) => path.basename(s) !== "__pycache__" && !s.endsWith(".pyc"),
  });
}

// The managed .gitignore block. COMMIT the agent brain (expertise + hooks) and portable memory
// (.genesis/memory/*.jsonl); IGNORE machine-local junk (the DB, caches, logs, the absolute-path .mcp.json).
// Rewritten in place between the sentinels on every bootstrap.
const GITIGNORE_START = "# >>> genesis runtime (managed by bootstrap) >>>";
const GITIGNORE_END = "# <<< genesis runtime <<<";
const GITIGNORE_BLOCK = [
  GITIGNORE_START,
  "# Commit the agent brain (expertise + hooks) and portable memory (JSONL) so agents and their",
  "# learned memory travel with the repo across systems. Ignore machine-local / regenerable junk.",
  ".genesis/*",
  "!.genesis/expertise/",
  "!.genesis/hooks/",
  "!.genesis/memory/",
  ".genesis/**/__pycache__/",
  ".genesis/expertise/.genesis/",
  ".genesis/memory/*.tmp",
  "*.db",
  ".mcp.json",
  GITIGNORE_END,
  "",
].join("\n");

function mergeGitignore(target) {
  // Idempotently write the managed genesis block into <target>/.gitignore.
  //
  // Replaces an existing managed block (between sentinels) in place; otherwise appends. Never touches the
  // user's own lines. Returns a note if a conflicting blanket `.genesis/` ignore is found outside the managed
  // block (it would defeat the re-includes).
  const p = path.join(target, ".gitignore");
  let existing;
  try {
    existing = fs.readFileSync(p, { encoding: "utf-8" });
  } catch (e) {
    existing = "";
  }
  let merged;
  if (existing.includes(GITIGNORE_START) && existing.includes(GITIGNORE_END)) {
    const pre = existing.slice(0, existing.indexOf(GITIGNORE_START));
    const post = existing.slice(existing.indexOf(GITIGNORE_END) + GITIGNORE_END.length);
    merged = pre.replace(/\n+$/, "") + (pre.trim() ? "\n\n" : "") + GITIGNORE_BLOCK + post.replace(/^\n+/, "");
  } else {
    const sep = !existing || existing.endsWith("\n") ? "" : "\n";
    merged = existing + sep + (existing.trim() ? "\n" : "") + GITIGNORE_BLOCK;
  }
  fs.writeFileSync(p, merged, { encoding: "utf-8" });
  // Warn if a blanket `.genesis/` ignore lives OUTSIDE our block — it would win and defeat the re-includes.
  const outside =
    merged.slice(0, merged.indexOf(GITIGNORE_START)) +
    merged.slice(merged.indexOf(GITIGNORE_END) + GITIGNORE_END.length);
  const conflict = outside
    .split(/\r\n|\r|\n/)
    .some((ln) => ln.trim() === ".genesis" || ln.trim() === ".genesis/");
  return conflict ? "conflicting blanket '.genesis/' ignore present — remove it" : null;
}

function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    fail("usage: bootstrap.js <target_repo> [genesis_home]  (or set $GENESIS_HOME)");
  }
  const target = path.resolve(args[0]);
  const gh =
    args.length > 1
      ? path.resolve(args[1])
      : process.env.GENESIS_HOME
      ? path.resolve(process.env.GENESIS_HOME)
      : path.dirname(__dirname);
  if (!isDir(target)) {
    fail(`target repo not found: ${target}`);
  }
  if (path.resolve(gh) === path.resolve(path.join(target, ".genesis"))) {
    fail("refusing to bootstrap a genesis into itself.");
  }

  const dest = path.join(target, ".genesis");

  // 1. Copy the functional Genesis tree (self-contained). `skills/` holds the team skills (build-agent,
  //    research-expertise) + the spec-forge suite, so it travels too — the assembler wires sensei's skills
  //    from <dest>/skills/ (built-ins) via installNamedSkills.
  for (const sub of ["expertise", "hooks", "skills", "team", "install"]) {
    const s = path.join(gh, sub);
    if (!isDir(s)) {
      fail(`genesis home is missing ${sub}/ at ${gh} — is ${gh} a Genesis install?`);
    }
    copyTree(s, path.join(dest, sub));
  }

  // 2. Memory DB lives under .genesis/ (the npx-launched server creates the file on first use). Never clobber
  //    an existing one. The DB is machine-local + regenerable; the PORTABLE, COMMITTED form of memory is the
  //    JSONL export at .genesis/memory/memory.jsonl (see the .gitignore in step 4). On a fresh clone the DB is
  //    absent but the JSONL is present, so the server rebuilds (re-embeds) the memory on first run.
  const memDb = path.join(dest, "memory.db");
  const memExport = path.join(dest, "memory", "memory.jsonl");

  // 3. Register the repo-local memory MCP server in <repo>/.mcp.json (merge; preserve other servers). The
  //    server is delivered via npx (@atiqul/genesis-memory-server resolves the prebuilt native binary + the
  //    ONNX model package for this platform) — NO local Rust build, and the model dir is provided by the npm
  //    model package, so no GENESIS_MODEL_DIR is set here. The repo-local DB + portable export keep memory
  //    per-repo and travelling with the repo across systems.
  const mcpPath = path.join(target, ".mcp.json");
  let mcp;
  try {
    mcp = JSON.parse(fs.readFileSync(mcpPath, { encoding: "utf-8" }));
  } catch (e) {
    mcp = {};
  }
  if (mcp === null || typeof mcp !== "object" || Array.isArray(mcp)) {
    mcp = {};
  }
  if (!mcp.mcpServers || typeof mcp.mcpServers !== "object") {
    mcp.mcpServers = {};
  }
  mcp.mcpServers["genesis-memory"] = {
    command: "npx",
    args: ["-y", "@atiqul/genesis-memory-server"],
    env: {
      GENESIS_MEMORY_DB: memDb,
      // The committed, cross-system-portable mirror. The server snapshots to this after every
      // store/consolidate and rebuilds from it when the DB is empty (fresh clone).
      GENESIS_MEMORY_EXPORT: memExport,
    },
  };
  fs.writeFileSync(mcpPath, JSON.stringify(mcp, null, 2) + "\n", { encoding: "utf-8" });

  // 4. Ensure the repo's .gitignore commits the agent BRAIN (expertise + hooks) and PORTABLE MEMORY
  //    (.genesis/memory/*.jsonl) while ignoring machine-local junk (the DB, caches, logs, the absolute-path
  //    .mcp.json). Without this, agents' memory + corrections never travel across systems.
  const gitignoreNote = mergeGitignore(target);

  // 5. Install sensei + method into <repo>/.claude/agents/, wired to the REPO-LEVEL .genesis/ home.
  const assembler = path.join(dest, "install", "assemble.js");
  const installed = [];
  for (const name of TEAM) {
    const src = path.join(dest, "team", name);
    const r = spawnSync(process.execPath, [assembler, src, name, target, dest], { encoding: "utf-8" });
    if (r.status !== 0) {
      fail(`assemble ${name} failed: ${((r.stderr || "").trim()) || ((r.stdout || "").trim())}`);
    }
    installed.push(JSON.parse((r.stdout || "").trim()).agent);
  }

  console.log(
    JSON.stringify(
      {
        bootstrapped: dest,
        target_repo: target,
        agents_installed: installed,
        agents_dir: path.join(target, ".claude", "agents"),
        memory_db: memDb,
        memory_db_preexisting: fs.existsSync(memDb),
        memory_export: memExport,
        memory_export_preexisting: fs.existsSync(memExport),
        gitignore: path.join(target, ".gitignore"),
        gitignore_warning: gitignoreNote,
        mcp_json: mcpPath,
        mcp_server: "npx -y @atiqul/genesis-memory-server",
        next: "Open Claude Code in the repo; talk to Sensei to build agents (expertise via the research-expertise skill).",
      },
      null,
      2
    )
  );
}

if (require.main === module) {
  main();
}

module.exports = { main, mergeGitignore, GITIGNORE_BLOCK, GITIGNORE_START, GITIGNORE_END };
