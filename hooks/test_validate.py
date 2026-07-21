#!/usr/bin/env python3
"""Unit tests for validate.py — the evidence-carrying declaration gate (§18).

Proves the bare-token hole is closed: a credible declaration must cite REAL rule-ids with VERIFIABLE
evidence, and the produced artifact is spot-checked. Runs validate.py as a subprocess (as Claude Code does),
feeding a synthetic transcript + workspace. No network, no API — pure determinism.

Run:  python3 hooks/test_validate.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
VALIDATE = os.path.join(HERE, "validate.py")
QUOTE = "Turn a green main into a correct, tagged, changelogged npm release."  # distinctive artifact line


def run(root, agent, transcript_path, stop_active=False):
    """Invoke validate.py; return True if it BLOCKED, plus the reason text."""
    payload = {"transcript_path": transcript_path, "stop_hook_active": stop_active}
    p = subprocess.run([sys.executable, VALIDATE, root, agent],
                       input=json.dumps(payload), capture_output=True, text=True, timeout=30)
    out = p.stdout.strip()
    if not out:
        return False, ""
    try:
        d = json.loads(out)
    except Exception:
        return False, out
    return d.get("decision") == "block", d.get("reason", "")


def make_workspace():
    """A temp workspace with one valid produced artifact (a lean, clean CLAUDE.md)."""
    root = tempfile.mkdtemp(prefix="genesis-val-")
    agent_dir = os.path.join(root, "release-manager")
    os.makedirs(agent_dir)
    claude_md = os.path.join(agent_dir, "CLAUDE.md")
    with open(claude_md, "w", encoding="utf-8") as fh:
        fh.write("# Ferry — Release Manager\n## Mission\n" + QUOTE + "\n## Boundaries\nYou never push if CI is red.\n")
    return root, claude_md


def transcript(root, text):
    """Write a one-message assistant transcript carrying `text`; return its path."""
    path = os.path.join(root, "transcript.jsonl")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "assistant",
                             "message": {"content": [{"type": "text", "text": text}]}}) + "\n")
    return path


def valid_decl(claude_md):
    """A credible evidence-carrying declaration for 'method' (persona-creation + prompt-engineering +
    expertise-application), ≥3 real ids each, evidence = the real file or a real quote."""
    p = claude_md
    return "\n".join([
        f"APPLIED-EXPERTISE: persona-creation#pc-1 — {p}",
        f"APPLIED-EXPERTISE: persona-creation#pc-2 — {p} is 5 lines, under budget",
        f'APPLIED-EXPERTISE: persona-creation#pc-7 — "{QUOTE}"',
        f"APPLIED-EXPERTISE: prompt-engineering#pe-4 — {p}",
        f"APPLIED-EXPERTISE: prompt-engineering#pe-6 — {p}",
        f"APPLIED-EXPERTISE: prompt-engineering#pe-18 — {p}",
        f"APPLIED-EXPERTISE: expertise-application#ea-1 — {p}",
        f"APPLIED-EXPERTISE: expertise-application#ea-3 — {p}",
        f"APPLIED-EXPERTISE: expertise-application#ea-5 — {p}",
    ])


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        if cond:
            passed += 1; print(f"  PASS  {name}")
        else:
            failed += 1; print(f"  FAIL  {name}")

    root, claude_md = make_workspace()

    # 1. No declaration at all → block (method requires 3 expertise)
    blocked, _ = run(root, "method", transcript(root, "I finished the persona. Looks good."))
    check("no declaration blocks", blocked)

    # 2. Bare tokens only (the old gameable form) → block
    bare = "APPLIED-EXPERTISE: persona-creation\nAPPLIED-EXPERTISE: prompt-engineering\nAPPLIED-EXPERTISE: expertise-application"
    blocked, reason = run(root, "method", transcript(root, bare))
    check("bare-token declaration blocks", blocked and "bare" in reason.lower())

    # 3. Fabricated rule-id → block
    bad = valid_decl(claude_md).replace("persona-creation#pc-1 ", "persona-creation#pc-999 ")
    blocked, reason = run(root, "method", transcript(root, bad))
    check("fabricated rule-id blocks", blocked and "pc-999" in reason)

    # 4. Below the coverage floor (one expertise cited only once) → block
    thin = valid_decl(claude_md)
    thin = "\n".join(l for l in thin.splitlines() if not (l.count("prompt-engineering") and "pe-6" not in l))
    blocked, reason = run(root, "method", transcript(root, thin))
    check("below-floor coverage blocks", blocked and "at least" in reason.lower())

    # 5. Fabricated evidence FILE → block
    fake_file = valid_decl(claude_md).replace(f"persona-creation#pc-1 — {claude_md}",
                                              "persona-creation#pc-1 — ghost/NOWHERE.md")
    blocked, reason = run(root, "method", transcript(root, fake_file))
    check("fabricated evidence file blocks", blocked and "NOWHERE.md" in reason)

    # 6. Fabricated evidence QUOTE → block
    fake_q = valid_decl(claude_md).replace(f'"{QUOTE}"', '"a sentence that appears in no artifact whatsoever"')
    blocked, reason = run(root, "method", transcript(root, fake_q))
    check("fabricated evidence quote blocks", blocked and "does not appear" in reason)

    # 7. Fully valid evidence-carrying declaration → ALLOW
    blocked, reason = run(root, "method", transcript(root, valid_decl(claude_md)))
    check("valid evidence-carrying declaration passes", not blocked)

    # 8. Valid declaration BUT the artifact violates a checkable rule (banned phrase) → block (layer 1)
    with open(claude_md, "a", encoding="utf-8") as fh:
        fh.write("\nUse chain-of-thought prompting here.\n")
    blocked, reason = run(root, "method", transcript(root, valid_decl(claude_md)))
    check("artifact violation blocks despite valid declaration", blocked and "chain-of-thought" in reason)

    # 9. stop_hook_active short-circuits → allow (loop guard)
    blocked, _ = run(root, "method", transcript(root, "nothing"), stop_active=True)
    check("stop_hook_active short-circuits to allow", not blocked)

    # 10. Agent with NO required expertise, clean artifacts → allow
    #     (recreate a clean workspace since #8 dirtied this one)
    root2, _ = make_workspace()
    blocked, _ = run(root2, "", transcript(root2, "done"))
    check("no-required-expertise agent with clean artifacts allows", not blocked)

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
