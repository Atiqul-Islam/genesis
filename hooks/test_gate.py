#!/usr/bin/env python3
"""Unit tests for gate.py — PreToolUse blocking + just-in-time rule surfacing (§19).

Runs gate.py as a subprocess with synthetic PreToolUse events. Pure determinism, no network.
Run:  python3 hooks/test_gate.py
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "gate.py")


def run(file_path, content, edit=False, agent_type="sensei"):
    key = "new_string" if edit else "content"
    ev = {"tool_input": {"file_path": file_path, key: content}}
    # gate.py is DORMANT unless a genesis agent is active (payload agent_type). These functional tests run
    # AS a genesis agent (default "sensei"); pass agent_type=None to exercise the dormant path.
    if agent_type:
        ev["agent_type"] = agent_type
    p = subprocess.run([sys.executable, GATE], input=json.dumps(ev),
                       capture_output=True, text=True, timeout=30)
    out = p.stdout.strip()
    if not out:
        return {}
    try:
        d = json.loads(out).get("hookSpecificOutput", {})
    except Exception:
        return {}
    return d


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        if cond:
            passed += 1; print(f"  PASS  {name}")
        else:
            failed += 1; print(f"  FAIL  {name}")

    big = "# Agent\n## Mission\n" + ("Ship correct releases. " * 30) + "\n## Boundaries\nNever push if CI is red.\n"

    # 1. banned phrase → deny
    d = run("release-manager/CLAUDE.md", "Use chain-of-thought reasoning.\n" + big)
    check("banned phrase denies", d.get("permissionDecision") == "deny" and "chain-of-thought" in d.get("permissionDecisionReason", ""))

    # 2. credential → deny
    d = run("agent/config.md", 'api_key = "sk-abcdef123456"\n' + big)
    check("credential value denies", d.get("permissionDecision") == "deny")

    # 3. oversize budgeted file → deny
    d = run("x/CLAUDE.md", "\n".join(f"line {i}" for i in range(250)))
    check("oversize CLAUDE.md denies", d.get("permissionDecision") == "deny" and "lines" in d.get("permissionDecisionReason", ""))

    # 4. substantial clean CLAUDE.md → proceed (no auto-approve) + surface persona-creation rules
    d = run("release-manager/CLAUDE.md", big)
    ctx = d.get("additionalContext", "")
    check("substantial CLAUDE.md surfaces persona rules, does not auto-approve",
          d.get("permissionDecision") != "deny" and "permissionDecision" not in d
          and "persona-creation" in ctx and "pc-" in ctx)

    # 5. tiny edit → proceed silently, NO surface (below SURFACE_MIN)
    d = run("release-manager/CLAUDE.md", "## Voice\nTerse.\n", edit=True)
    check("tiny edit proceeds without surfacing",
          d.get("permissionDecision") != "deny" and "additionalContext" not in d)

    # 6. non-authoring file (code) substantial → proceed silently, NO surface
    d = run("src/util.py", "def f():\n    return 1\n" + ("# pad line\n" * 40))
    check("non-authoring file proceeds without surfacing",
          d.get("permissionDecision") != "deny" and "additionalContext" not in d)

    # 7. prompt/tool artifact substantial → proceed + surface prompt-engineering rules
    d = run("prompts/tool_defs.json", '{"name":"x","description":"' + ("y" * 320) + '"}')
    ctx = d.get("additionalContext", "")
    check("prompt/tool artifact surfaces prompt-engineering rules",
          d.get("permissionDecision") != "deny" and "prompt-engineering" in ctx and "pe-" in ctx)

    # 8. DORMANCY: with NO genesis agent active, gate does NOTHING — even banned content is not denied.
    #    This is what keeps genesis from policing a normal user's writes in every repo.
    d = run("release-manager/CLAUDE.md", "Use chain-of-thought reasoning.\n" + big, agent_type=None)
    check("DORMANT without a genesis agent: banned content is NOT denied (no global policing)",
          d.get("permissionDecision") != "deny" and "permissionDecision" not in d)
    d = run("method|genesis:method-check/CLAUDE.md", "chain-of-thought\n" + big, agent_type="genesis:method")
    check("scoped agent_type genesis:method still enforces",
          d.get("permissionDecision") == "deny")

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
