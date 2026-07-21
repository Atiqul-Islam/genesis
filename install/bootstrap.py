#!/usr/bin/env python3
"""Bootstrap a SELF-CONTAINED, repo-level Genesis workspace at <target_repo>/.genesis/.

Genesis's expertise store, memory, hooks, and team live PER REPOSITORY (never global). When Sensei is asked
to build an agent in a repo that has no `.genesis/` workspace yet, this creates one in a single deterministic
call, so the repo becomes a standalone Genesis: its own store + hooks + memory DB + base modules + sensei/method.

What it does (idempotent, cross-platform):
  1. Copies the functional Genesis tree into <repo>/.genesis/: expertise/ (+manifests/, required.json),
     hooks/, team/ (sensei+method+their skills), install/ (assemble/install/bootstrap).
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

    # 1. Copy the functional Genesis tree (self-contained).
    for sub in ("expertise", "hooks", "team", "install"):
        s = os.path.join(gh, sub)
        if not os.path.isdir(s):
            sys.exit(f"genesis home is missing {sub}/ at {gh} — is {gh} a Genesis install?")
        _copy_tree(s, os.path.join(dest, sub))

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
    mem_db = os.path.join(dest, "memory.db")

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
                "GENESIS_MEMORY_DB": mem_db},
    }
    with open(mcp_path, "w", encoding="utf-8") as f:
        json.dump(mcp, f, indent=2)
        f.write("\n")

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
        "mcp_json": mcp_path,
        "next": "Open Claude Code in the repo; talk to Sensei to build agents (expertise via the research-expertise skill).",
    }, indent=2))


if __name__ == "__main__":
    main()
