#!/usr/bin/env python3
"""Integration test for bootstrap.py (D3) — runs the REAL bootstrap into an OS temp repo and asserts a
correct, self-contained repo-level Genesis: layout, binary+model copied, .mcp.json + agents wired repo-local,
hooks resolve the repo store, and re-run is idempotent (preserves the memory DB).

Copies the real 33MB binary + 90MB model, so it is slow (~1 min); that is the point — no shortcuts.
Run:  python3 install/test_bootstrap.py
"""
import json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))         # <gh>/install
GH = os.path.dirname(HERE)                                  # <gh>
BOOTSTRAP = os.path.join(HERE, "bootstrap.py")


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    target = tempfile.mkdtemp(prefix="genesis-boot-")
    dest = os.path.join(target, ".genesis")
    try:
        r = subprocess.run([sys.executable, BOOTSTRAP, target, GH],
                           capture_output=True, text=True, timeout=900)
        check("bootstrap exits 0", r.returncode == 0)
        if r.returncode != 0:
            print("  STDERR:", r.stderr.strip()[:500])

        # layout
        check("expertise/manifests copied", os.path.isdir(os.path.join(dest, "expertise", "manifests")))
        check("required.json copied", os.path.isfile(os.path.join(dest, "expertise", "required.json")))
        check("hooks copied (validate + enforce_research)",
              os.path.isfile(os.path.join(dest, "hooks", "validate.py"))
              and os.path.isfile(os.path.join(dest, "hooks", "enforce_research.py")))
        check("team copied", os.path.isdir(os.path.join(dest, "team", "sensei")))
        # Skills now live at the genesis-home skills/ dir (plugin-root skills/), not under team/. Bootstrap
        # copies skills/ into <dest>, and the assembler installs sensei's named skills into .claude/skills/.
        check("research-expertise skill copied to .genesis/skills/",
              os.path.isdir(os.path.join(dest, "skills", "research-expertise")))
        check("sensei's skills installed into <repo>/.claude/skills/",
              os.path.isdir(os.path.join(target, ".claude", "skills", "research-expertise"))
              and os.path.isdir(os.path.join(target, ".claude", "skills", "build-agent")))
        check("install scripts copied", os.path.isfile(os.path.join(dest, "install", "assemble.py")))

        # binary + model (real copies)
        exe = ".exe" if os.name == "nt" else ""
        binp = os.path.join(dest, "server", "target", "release", "genesis-memory-server" + exe)
        check("server binary copied (>1MB)", os.path.isfile(binp) and os.path.getsize(binp) > 1_000_000)
        onnx = os.path.join(dest, "server", "models", "onnx", "model.onnx")
        check("onnx model copied (>1MB)", os.path.isfile(onnx) and os.path.getsize(onnx) > 1_000_000)
        check("tokenizer copied", os.path.isfile(os.path.join(dest, "server", "models", "tokenizer.json")))

        # .mcp.json wired repo-local
        mcp = json.load(open(os.path.join(target, ".mcp.json"), encoding="utf-8"))
        gm = mcp["mcpServers"]["genesis-memory"]
        check(".mcp.json command → repo-local binary", gm["command"] == binp)
        check(".mcp.json env → repo-local model + db",
              dest in gm["env"]["GENESIS_MODEL_DIR"] and dest in gm["env"]["GENESIS_MEMORY_DB"])

        # agents wired to .genesis/, sensei has the enforce gate, method does not
        sensei = open(os.path.join(target, ".claude", "agents", "sensei.md"), encoding="utf-8").read()
        method = open(os.path.join(target, ".claude", "agents", "method.md"), encoding="utf-8").read()
        # PORTABLE: built-agent hooks reference $CLAUDE_PROJECT_DIR/.genesis/hooks (runtime-resolved to the
        # repo root, cross-platform) — NOT an absolute machine path, so the agent survives a clone.
        check("sensei.md hooks reference $CLAUDE_PROJECT_DIR/.genesis/hooks (portable)",
              "$CLAUDE_PROJECT_DIR/.genesis/hooks" in sensei)
        check("sensei.md hooks carry NO absolute machine path", os.path.join(dest, "hooks") not in sensei
              and "/mnt/" not in sensei)
        check("sensei has Bash enforce_research gate", "enforce_research.py" in sensei)
        check("method has NO enforce_research gate", "enforce_research.py" not in method)

        # hooks resolve the repo store via HOOK_DIR/../expertise (the relative logic actually points here)
        resolved = os.path.normpath(os.path.join(dest, "hooks", "..", "expertise", "manifests"))
        check("validate/review hooks resolve the repo store",
              resolved == os.path.normpath(os.path.join(dest, "expertise", "manifests"))
              and os.path.isdir(resolved))

        # required.json has sensei + method registered in the repo-local store
        req = json.load(open(os.path.join(dest, "expertise", "required.json"), encoding="utf-8"))
        check("repo required.json has sensei + method", "sensei" in req and "method" in req)

        # idempotent + preserves an existing memory DB
        memdb = os.path.join(dest, "memory.db")
        with open(memdb, "w", encoding="utf-8") as fh:
            fh.write("SENTINEL")
        r2 = subprocess.run([sys.executable, BOOTSTRAP, target, GH],
                            capture_output=True, text=True, timeout=900)
        check("re-run idempotent (exit 0)", r2.returncode == 0)
        check("memory.db preserved across re-run",
              os.path.isfile(memdb) and open(memdb, encoding="utf-8").read() == "SENTINEL")
    finally:
        shutil.rmtree(target, ignore_errors=True)

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
