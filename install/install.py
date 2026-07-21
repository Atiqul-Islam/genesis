#!/usr/bin/env python3
"""Install Genesis into a target repo — one command.

Assembles Genesis's own team (Sensei + Method) into <target_repo>/.claude/agents/, installs their skills
into <target_repo>/.claude/skills/, and reports the memory-server binary location. After this, opening
Claude Code in <target_repo> exposes `sensei` and `method`; the user talks to Sensei to build agents.

usage: install.py <target_repo>            # genesis_home from $GENESIS_HOME, else this file's repo root
       install.py <target_repo> <genesis_home>
"""
import json, os, subprocess, sys

TEAM = ["sensei", "method"]


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: install.py <target_repo> [genesis_home]  (or set $GENESIS_HOME)")
    target = os.path.abspath(sys.argv[1])
    # genesis_home: explicit arg > $GENESIS_HOME > inferred from this file's location. Never hardcoded —
    # portable across machines/OSes (the env var is the parametrization the requirement calls for).
    gh = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 \
        else os.path.abspath(os.environ["GENESIS_HOME"]) if os.environ.get("GENESIS_HOME") \
        else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    assembler = os.path.join(gh, "install", "assemble.py")
    if not os.path.exists(assembler):
        sys.exit(f"assembler not found at {assembler} — is {gh} the Genesis home?")
    if not os.path.isdir(target):
        sys.exit(f"target repo not found: {target}")

    installed = []
    for name in TEAM:
        src = os.path.join(gh, "team", name)
        r = subprocess.run([sys.executable, assembler, src, name, target, gh],
                           capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"assemble {name} failed: {r.stderr.strip() or r.stdout.strip()}")
        installed.append(json.loads(r.stdout.strip()))

    server = os.path.join(gh, "server")
    # binary name is .exe on Windows; report whichever exists so `memory_server_built` is correct cross-OS
    exe = ".exe" if os.name == "nt" else ""
    server_bin = os.path.join(server, "target", "release", "genesis-memory-server" + exe)
    if not os.path.exists(server_bin) and os.path.exists(server_bin + ".exe"):
        server_bin = server_bin + ".exe"  # WSL building a Windows target, etc.
    report = {
        "genesis_home": gh,
        "target_repo": target,
        "agents_installed": [i["agent"] for i in installed],
        "agents_dir": os.path.join(target, ".claude", "agents"),
        "skills_installed": sorted({s for i in installed for s in i.get("skills_installed", [])}),
        "memory_server_built": os.path.exists(server_bin),
        "memory_server_path": server_bin,
        "next": "Open Claude Code in the target repo; talk to Sensei: \"build me an agent that …\".",
    }
    print(json.dumps(report, indent=2))
    if not report["memory_server_built"]:
        print("\nNote: the Rust memory server is not built yet. Build it with: "
              f"cd {server} && cargo build --release", file=sys.stderr)


if __name__ == "__main__":
    main()
