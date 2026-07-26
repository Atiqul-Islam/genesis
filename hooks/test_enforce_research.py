#!/usr/bin/env python3
"""Unit tests for enforce_research.py — the Sensei-scoped PreToolUse gate that blocks assembling a built
agent unless the research-expertise skill ran this session (§16 / D5). Runs the hook as a subprocess with
synthetic PreToolUse events. Pure determinism, no network.

Run:  python3 hooks/test_enforce_research.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ENFORCE = os.path.join(HERE, "enforce_research.py")
GEN = "/x/.genesis"


def run(command, transcript_path, agent="sensei"):
    # `agent` populates the payload's agent_type. enforce_research now has a DORMANCY GUARD: it no-ops
    # unless a genesis agent is active. Sensei is the only agent that assembles, so enforcement runs with
    # agent="sensei"; pass agent=None to simulate a normal (non-genesis) session.
    ev = {"tool_input": {"command": command}, "transcript_path": transcript_path}
    if agent:
        ev["agent_type"] = agent
    p = subprocess.run([sys.executable, ENFORCE], input=json.dumps(ev),
                       capture_output=True, text=True, timeout=30)
    out = p.stdout.strip()
    if not out:
        return False, ""  # silent → allow
    try:
        d = json.loads(out).get("hookSpecificOutput", {})
    except Exception:
        return False, out
    return d.get("permissionDecision") == "deny", d.get("permissionDecisionReason", "")


def transcript(root, used):
    path = os.path.join(root, f"t_{'yes' if used else 'no'}.jsonl")
    blocks = [{"type": "text", "text": "working on the build"}]
    if used:
        blocks.append({"type": "tool_use", "name": "Skill", "input": {"skill": "research-expertise"}})
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "assistant", "message": {"content": blocks}}) + "\n")
    return path


def assemble_cmd(name):
    return f'{sys.executable} {GEN}/install/assemble.py {GEN}/team/{name} "{name}" /x {GEN}'


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    root = tempfile.mkdtemp(prefix="genesis-enf-")
    with_skill = transcript(root, True)
    without_skill = transcript(root, False)

    # 1. built agent, skill NOT used → deny
    blocked, reason = run(assemble_cmd("acme-bot"), without_skill)
    check("built agent without research-expertise skill → DENY", blocked and "research-expertise" in reason)

    # 2. built agent, skill used → allow
    blocked, _ = run(assemble_cmd("acme-bot"), with_skill)
    check("built agent WITH research-expertise skill → allow", not blocked)

    # 3. builtin sensei → allow regardless of transcript
    blocked, _ = run(assemble_cmd("sensei"), without_skill)
    check("builtin sensei → allow (exempt)", not blocked)

    # 4. builtin method → allow regardless
    blocked, _ = run(assemble_cmd("method"), without_skill)
    check("builtin method → allow (exempt)", not blocked)

    # 5. non-assemble Bash command → allow (not our concern)
    blocked, _ = run(f"{sys.executable} -c 'print(1)' && ls -la", without_skill)
    check("non-assemble command → allow", not blocked)

    # 6. built agent, transcript MISSING → deny (fail-closed)
    blocked, _ = run(assemble_cmd("acme-bot"), os.path.join(root, "nope.jsonl"))
    check("built agent + missing transcript → DENY (fail-closed)", blocked)

    # 7. malformed assemble (no name arg) → deny (fail-closed)
    blocked, _ = run(f"{sys.executable} {GEN}/install/assemble.py", without_skill)
    check("assemble with no agent name → DENY (fail-closed)", blocked)

    # 8. DORMANCY: no genesis agent active (normal session) → no-op even for an assemble command.
    # This is the bug that blocked a plain `grep install/assemble.py` in a normal session.
    blocked, _ = run(assemble_cmd("acme-bot"), without_skill, agent=None)
    check("normal session (no genesis agent) → allow even an assemble cmd (DORMANT)", not blocked)

    import shutil
    shutil.rmtree(root, ignore_errors=True)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
