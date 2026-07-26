#!/usr/bin/env python3
"""Bootstrap a SELF-CONTAINED, repo-level Genesis workspace at <target_repo>/.genesis/.

Genesis's expertise store, memory, hooks, and team live PER REPOSITORY (never global). When Sensei is asked
to build an agent in a repo that has no `.genesis/` workspace yet, this creates one in a single deterministic
call, so the repo becomes a standalone Genesis: its own store + hooks + memory DB + base modules + sensei/method.

What it does (idempotent, cross-platform):
  1. Copies the functional Genesis tree into <repo>/.genesis/: expertise/ (+manifests/, required.json),
     hooks/, skills/ (the team skills build-agent/research-expertise + the spec-forge suite), team/
     (sensei+method personas/behaviors), install/ (assemble/install/bootstrap), and bin/ if present.
  2. Copies ONLY the memory-server binary + ONNX model (not the multi-GB Rust build tree) into
     <repo>/.genesis/server/ — so the workspace is fully self-contained (no dependency on the central Genesis).
  3. Registers a repo-local `genesis-memory` MCP server in <repo>/.mcp.json (merged, not clobbered), pointing
     the binary + model + SQLite DB at <repo>/.genesis/ paths.
  4. Installs sensei + method into <repo>/.claude/agents/ via the assembler with genesis_home = <repo>/.genesis,
     so their hook commands reference <repo>/.genesis/hooks/*.py and the validate/review hooks resolve the
     store at <repo>/.genesis/expertise/ through their existing HOOK_DIR/../expertise relative logic.
Re-running refreshes the copied tree + agents WITHOUT clobbering an existing memory DB.

usage: bootstrap.py <target_repo> [genesis_home]     # genesis_home from $GENESIS_HOME, else this repo root
"""
import json, os, shutil, subprocess, sys

TEAM = ["sensei", "method"]


def _ignore_junk(_dir, names):
    return {n for n in names if n == "__pycache__" or n.endswith(".pyc")}


def _copy_tree(src, dst):
    shutil.copytree(src, dst, dirs_exist_ok=True, ignore=_ignore_junk)


# The managed .gitignore block. COMMIT the agent brain (expertise + hooks) and portable memory
# (.genesis/memory/*.jsonl); IGNORE machine-local junk (the DB, server binary+model, caches, logs,
# the absolute-path .mcp.json). Rewritten in place between the sentinels on every bootstrap.
GITIGNORE_START = "# >>> genesis runtime (managed by bootstrap) >>>"
GITIGNORE_END = "# <<< genesis runtime <<<"
GITIGNORE_BLOCK = "\n".join([
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
])


def _merge_gitignore(target):
    """Idempotently write the managed genesis block into <target>/.gitignore.

    Replaces an existing managed block (between sentinels) in place; otherwise appends. Never
    touches the user's own lines. Returns a note if a conflicting blanket `.genesis/` ignore is
    found outside the managed block (it would defeat the re-includes).
    """
    path = os.path.join(target, ".gitignore")
    try:
        existing = open(path, encoding="utf-8").read()
    except Exception:
        existing = ""
    if GITIGNORE_START in existing and GITIGNORE_END in existing:
        pre = existing.split(GITIGNORE_START)[0]
        post = existing.split(GITIGNORE_END, 1)[1]
        merged = pre.rstrip("\n") + ("\n\n" if pre.strip() else "") + GITIGNORE_BLOCK + post.lstrip("\n")
    else:
        sep = "" if (not existing or existing.endswith("\n")) else "\n"
        merged = existing + sep + ("\n" if existing.strip() else "") + GITIGNORE_BLOCK
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(merged)
    # Warn if a blanket `.genesis/` ignore lives OUTSIDE our block — it would win and defeat the re-includes.
    outside = merged.split(GITIGNORE_START)[0] + merged.split(GITIGNORE_END, 1)[-1]
    conflict = any(ln.strip() in (".genesis", ".genesis/") for ln in outside.splitlines())
    return "conflicting blanket '.genesis/' ignore present — remove it" if conflict else None


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: bootstrap.py <target_repo> [genesis_home]  (or set $GENESIS_HOME)")
    target = os.path.abspath(sys.argv[1])
    gh = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 \
        else os.path.abspath(os.environ["GENESIS_HOME"]) if os.environ.get("GENESIS_HOME") \
        else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if not os.path.isdir(target):
        sys.exit(f"target repo not found: {target}")
    if os.path.abspath(gh) == os.path.abspath(os.path.join(target, ".genesis")):
        sys.exit("refusing to bootstrap a genesis into itself.")

    dest = os.path.join(target, ".genesis")

    # 1. Copy the functional Genesis tree (self-contained). `skills/` now holds the team skills (build-agent,
    #    research-expertise) + the spec-forge suite, so it travels too — the assembler wires sensei's skills
    #    from <dest>/skills/ (built-ins) via install_named_skills.
    for sub in ("expertise", "hooks", "skills", "team", "install"):
        s = os.path.join(gh, sub)
        if not os.path.isdir(s):
            sys.exit(f"genesis home is missing {sub}/ at {gh} — is {gh} a Genesis install?")
        _copy_tree(s, os.path.join(dest, sub))

    # 1b. Best-effort: carry the memory-server LAUNCHER (bin/) into <repo>/.genesis/bin/ if it exists (built
    #     by the plugin's binary-delivery worker). The compiled binary is also copied below (step 2), so a
    #     bootstrapped repo is self-contained regardless of whether the launcher is present yet.
    bin_src = os.path.join(gh, "bin")
    if os.path.isdir(bin_src):
        _copy_tree(bin_src, os.path.join(dest, "bin"))

    # 2. Copy ONLY the server binary + model (NOT server/target/, which is multi-GB build output).
    exe = ".exe" if os.name == "nt" else ""
    bin_src = os.path.join(gh, "server", "target", "release", "genesis-memory-server" + exe)
    if not os.path.exists(bin_src) and os.path.exists(bin_src + ".exe"):
        bin_src, exe = bin_src + ".exe", ".exe"
    if not os.path.exists(bin_src):
        sys.exit(f"memory-server binary not found at {bin_src} — build it first (cd server && cargo build --release).")
    bin_dst_dir = os.path.join(dest, "server", "target", "release")
    os.makedirs(bin_dst_dir, exist_ok=True)
    bin_dst = os.path.join(bin_dst_dir, "genesis-memory-server" + exe)
    shutil.copy2(bin_src, bin_dst)
    try:
        os.chmod(bin_dst, 0o755)
    except Exception:
        pass
    models_src = os.path.join(gh, "server", "models")
    if not os.path.isdir(models_src):
        sys.exit(f"model dir not found at {models_src}.")
    _copy_tree(models_src, os.path.join(dest, "server", "models"))

    # 3. Memory DB lives under .genesis/ (server creates the file on first use). Never clobber an existing one.
    #    The DB is machine-local + regenerable; the PORTABLE, COMMITTED form of memory is the JSONL export
    #    at .genesis/memory/memory.jsonl (see step 4b + the .gitignore in step 6). On a fresh clone the DB is
    #    absent but the JSONL is present, so the server rebuilds (re-embeds) the memory on first run.
    mem_db = os.path.join(dest, "memory.db")
    mem_export = os.path.join(dest, "memory", "memory.jsonl")

    # 4. Register the repo-local memory MCP server in <repo>/.mcp.json (merge; preserve other servers).
    mcp_path = os.path.join(target, ".mcp.json")
    try:
        mcp = json.load(open(mcp_path, encoding="utf-8"))
    except Exception:
        mcp = {}
    if not isinstance(mcp, dict):
        mcp = {}
    mcp.setdefault("mcpServers", {})
    mcp["mcpServers"]["genesis-memory"] = {
        "command": bin_dst,
        "args": [],
        "env": {"GENESIS_MODEL_DIR": os.path.join(dest, "server", "models"),
                "GENESIS_MEMORY_DB": mem_db,
                # The committed, cross-system-portable mirror. The server snapshots to this after every
                # store/consolidate and rebuilds from it when the DB is empty (fresh clone).
                "GENESIS_MEMORY_EXPORT": mem_export},
    }
    with open(mcp_path, "w", encoding="utf-8") as f:
        json.dump(mcp, f, indent=2)
        f.write("\n")

    # 6. Ensure the repo's .gitignore commits the agent BRAIN (expertise + hooks) and PORTABLE MEMORY
    #    (.genesis/memory/*.jsonl) while ignoring machine-local junk (the DB, the server binary+model,
    #    caches, logs, the absolute-path .mcp.json). Without this, agents' memory + corrections never
    #    travel across systems — the flaw this block fixes.
    gitignore_note = _merge_gitignore(target)

    # 5. Install sensei + method into <repo>/.claude/agents/, wired to the REPO-LEVEL .genesis/ home.
    assembler = os.path.join(dest, "install", "assemble.py")
    installed = []
    for name in TEAM:
        src = os.path.join(dest, "team", name)
        r = subprocess.run([sys.executable, assembler, src, name, target, dest],
                           capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"assemble {name} failed: {r.stderr.strip() or r.stdout.strip()}")
        installed.append(json.loads(r.stdout.strip())["agent"])

    print(json.dumps({
        "bootstrapped": dest,
        "target_repo": target,
        "agents_installed": installed,
        "agents_dir": os.path.join(target, ".claude", "agents"),
        "memory_server": bin_dst,
        "model_dir": os.path.join(dest, "server", "models"),
        "memory_db": mem_db,
        "memory_db_preexisting": os.path.exists(mem_db),
        "memory_export": mem_export,
        "memory_export_preexisting": os.path.exists(mem_export),
        "gitignore": os.path.join(target, ".gitignore"),
        "gitignore_warning": gitignore_note,
        "mcp_json": mcp_path,
        "next": "Open Claude Code in the repo; talk to Sensei to build agents (expertise via the research-expertise skill).",
    }, indent=2))


if __name__ == "__main__":
    main()
