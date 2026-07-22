#!/usr/bin/env python3
"""Tests for hooks/inject.py — house-rule/expertise delivery + the session-copy summary injection (§Phase 3).
Runs inject.py as a subprocess (as Claude Code does). Run: python3 hooks/test_inject.py"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
INJECT = os.path.join(HERE, "inject.py")


def run(exp_dir, agent, cwd=None):
    p = subprocess.run([sys.executable, INJECT, exp_dir, agent], input="{}", capture_output=True,
                       text=True, cwd=cwd or os.getcwd(), timeout=20)
    try:
        return json.loads(p.stdout.strip()).get("hookSpecificOutput", {}).get("additionalContext", "")
    except Exception:
        return ""


def main():
    passed = failed = 0
    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    home = tempfile.mkdtemp(prefix="gh-")
    exp = os.path.join(home, "expertise"); os.makedirs(os.path.join(exp, "manifests"))
    with open(os.path.join(exp, "persona-creation.md"), "w") as fh:
        fh.write("# guide\n")
    with open(os.path.join(exp, "required.json"), "w") as fh:
        json.dump({"method": ["persona-creation"]}, fh)

    # 1. always delivers house rules
    ctx = run(exp, "method")
    check("house rules always injected", "Genesis house rules" in ctx and "credential" in ctx)
    check("required expertise injected for a known agent", "APPLIED-EXPERTISE" in ctx)

    # 2. NO summary block when the agent has no session-copy bundle (existing behavior preserved)
    check("no summary block without a bundle", "carried-over session memory" not in ctx)

    # 3. summary IS injected when the agent has a bundle at <home>/agents/<name>/summary.md
    ag = os.path.join(home, "agents", "copybot"); os.makedirs(ag)
    with open(os.path.join(ag, "summary.md"), "w") as fh:
        fh.write("# digest\n- prior fact: the deploy gate requires green CI\n")
    ctx2 = run(exp, "copybot")
    check("summary injected for a session-copy agent",
          "carried-over session memory" in ctx2 and "deploy gate requires green CI" in ctx2)
    check("summary block tells the agent to recall the rest", "recall" in ctx2.lower())

    # 4. bundle discoverable via cwd/.genesis too (repo-local layout)
    repo = tempfile.mkdtemp(prefix="repo-")
    ag2 = os.path.join(repo, ".genesis", "agents", "localbot"); os.makedirs(ag2)
    with open(os.path.join(ag2, "summary.md"), "w") as fh:
        fh.write("# digest\n- local bundle fact ABC123\n")
    ctx3 = run(os.path.join(repo, ".genesis", "expertise"), "localbot", cwd=repo)
    check("summary discovered via cwd/.genesis/agents", "local bundle fact ABC123" in ctx3)

    # 5. output stays under the 10,000-char hook cap
    big = os.path.join(home, "agents", "bigbot"); os.makedirs(big)
    with open(os.path.join(big, "summary.md"), "w") as fh:
        fh.write("x" * 50000)
    ctx4 = run(exp, "bigbot")
    check("output respects the 10k-char cap", len(ctx4) <= 10000)

    import shutil
    for d in (home, repo):
        shutil.rmtree(d, ignore_errors=True)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
