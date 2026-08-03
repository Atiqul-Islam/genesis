#!/usr/bin/env node
/* Bootstrap a SELF-CONTAINED, repo-level Genesis workspace at <target_repo>/.genesis/.

   Faithful Node (CommonJS, stdlib-only) port of bootstrap.py, updated for the GitHub-Releases distribution:
   the generated repo `.mcp.json` launches the server via a small Node launcher (`.genesis/bin/genesis-memory.js`)
   that downloads + SHA256-verifies the platform binary + model from the GitHub Release and caches them — so
   there is NO local Rust build and NO committed multi-GB binary/model. Node is the only runtime a user needs.

   Genesis's expertise store, memory, hooks, and team live PER REPOSITORY (never global). When Sensei is asked
   to build an agent in a repo that has no `.genesis/` workspace yet, this creates one in a single deterministic
   call, so the repo becomes a standalone Genesis: its own store + hooks + memory DB + base modules + sensei/method.

   What it does (idempotent, cross-platform):
     1. Copies the functional Genesis tree into <repo>/.genesis/: expertise/ (+manifests/, required.json),
        hooks/ (hooks.json + the run.js resolver), skills/ (team skills + the spec-forge suite), team/,
        install/, and bin/ (the launcher). Then stages the native genesis-hook binary into <repo>/.genesis/bin.
     2. Registers a repo-local `genesis-memory` MCP server in <repo>/.mcp.json (merged, not clobbered) that runs
        `node <repo>/.genesis/bin/genesis-memory.js`, pointing the memory DB + portable JSONL export at <repo>/.genesis/.
     3. Installs sensei + method into <repo>/.claude/agents/ via the assembler with genesis_home = <repo>/.genesis,
        so their hook commands invoke <repo>/.genesis/bin/genesis-hook directly and pass
        --expertise <repo>/.genesis/expertise.
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
  for (const sub of ["expertise", "hooks", "skills", "team", "install", "bin"]) {
    const s = path.join(gh, sub);
    if (!isDir(s)) {
      fail(`genesis home is missing ${sub}/ at ${gh} — is ${gh} a Genesis install?`);
    }
    copyTree(s, path.join(dest, sub));
  }

  // 1b. Stage the native `genesis-hook` binary into <dest>/bin so the enforcement hooks invoke it
  //     DIRECTLY (no Node in the hot path). The launcher (copied above into <dest>/bin) resolves it:
  //     GENESIS_HOOK_BIN for dev/CI, else download from the GitHub Release + SHA256-verify. Non-fatal +
  //     timeout-bounded: if staging fails, run.js + the agents fail-open, so the repo still bootstraps —
  //     the deterministic checks stay dormant until the binary is present. `<dest>/bin` is machine-local
  //     + regenerable (covered by the .gitignore).
  const binDir = path.join(dest, "bin");
  const launcher = path.join(binDir, "genesis-memory.js");
  const hookExe = process.platform === "win32" ? "genesis-hook.exe" : "genesis-hook";
  let hookBinStaged = false;
  try {
    fs.mkdirSync(binDir, { recursive: true });
    const r = spawnSync(process.execPath, [launcher, "--stage-hook", binDir], {
      encoding: "utf-8",
      timeout: 300000, // a cold cache may download the binary from the release
    });
    hookBinStaged = r.status === 0 && fs.existsSync(path.join(binDir, hookExe));
  } catch (e) {
    hookBinStaged = false;
  }

  // 2. Memory DB lives under .genesis/ (the launched server creates the file on first use). Never clobber
  //    an existing one. The DB is machine-local + regenerable; the PORTABLE, COMMITTED form of memory is the
  //    JSONL export at .genesis/memory/memory.jsonl (see the .gitignore in step 4). On a fresh clone the DB is
  //    absent but the JSONL is present, so the server rebuilds (re-embeds) the memory on first run.
  const memDb = path.join(dest, "memory.db");
  const memExport = path.join(dest, "memory", "memory.jsonl");

  // 3. Register the repo-local memory MCP server in <repo>/.mcp.json (merge; preserve other servers). The
  //    server is launched via the Node launcher staged in step 1 (.genesis/bin/genesis-memory.js), which
  //    downloads + SHA256-verifies the platform binary + model from the GitHub Release and caches them — NO
  //    local Rust build, and the launcher points the server at the cached model (no GENESIS_MODEL_DIR here).
  //    The repo-local DB + portable export keep memory per-repo and travelling with the repo across systems.
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
    command: "node",
    args: [launcher],
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
        hook_bin: hookBinStaged ? path.join(binDir, hookExe) : null,
        hook_bin_staged: hookBinStaged,
        memory_db: memDb,
        memory_db_preexisting: fs.existsSync(memDb),
        memory_export: memExport,
        memory_export_preexisting: fs.existsSync(memExport),
        gitignore: path.join(target, ".gitignore"),
        gitignore_warning: gitignoreNote,
        mcp_json: mcpPath,
        mcp_server: "node " + launcher,
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
