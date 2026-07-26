#!/usr/bin/env python3
"""Portability tests for assemble.py frontmatter generation (§21 — cross-platform BUILT agents).

Proves a BUILT agent's generated hook `command:` lines are portable across machines/OSes:
  * interpreter is `python3` (resolved via PATH at runtime), NOT this machine's absolute sys.executable;
  * hook scripts are referenced via `$CLAUDE_PROJECT_DIR/.genesis/...` (Claude Code substitutes the project
    root at runtime, Windows + macOS + Linux) — NEVER an absolute machine path like /mnt/c/... or C:\\...;
  * paths are double-quoted (so a project dir with spaces survives) inside a YAML single-quoted scalar.

Run:  python3 install/test_portability.py
"""
import os, shlex, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble  # noqa: E402

HOME = "$CLAUDE_PROJECT_DIR/.genesis"  # what main() computes for the normal (gh = <repo>/.genesis) case


def command_lines(fm):
    return [ln.strip()[len("command:"):].strip() for ln in fm.splitlines() if ln.strip().startswith("command:")]


def unyaml_single(scalar):
    """Undo a YAML single-quoted scalar: strip the wrapping quotes, undouble ''."""
    assert scalar.startswith("'") and scalar.endswith("'"), scalar
    return scalar[1:-1].replace("''", "'")


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    fm = assemble.frontmatter("method", {"description": "d", "tools": ["Read", "Write"]}, HOME, [])
    cmds = command_lines(fm)
    check("four hook commands generated (inject/gate/validate/review)", len(cmds) == 4)
    check("all commands are YAML single-quoted", all(c.startswith("'") and c.endswith("'") for c in cmds))

    shells = [unyaml_single(c) for c in cmds]
    # Interpreter is portable `python3`, never an absolute interpreter path.
    check("uses portable `python3` interpreter", all(s.startswith('"python3" ') for s in shells))
    # THE FIX: every hook path is $CLAUDE_PROJECT_DIR-relative — no absolute machine path anywhere.
    check("every command references $CLAUDE_PROJECT_DIR", all("$CLAUDE_PROJECT_DIR/.genesis/hooks/" in s for s in shells))
    check("NO absolute machine path leaked (no /mnt/c, no C:\\, no leading-slash abs path)",
          not any(("/mnt/" in s or "C:\\" in s or ' "/' in s or ':\\' in s) for s in shells))

    # The $CLAUDE_PROJECT_DIR path is one intact shell token (double-quoted → survives spaces after runtime expansion).
    inject_tokens = shlex.split(shells[0])
    check("interpreter is token[0] = python3", inject_tokens[0] == "python3")
    check("inject.py path is one intact token", "$CLAUDE_PROJECT_DIR/.genesis/hooks/inject.py" in inject_tokens)
    check("expertise dir is one intact token", "$CLAUDE_PROJECT_DIR/.genesis/expertise" in inject_tokens)
    check("agent name is the trailing token", inject_tokens[-1] == "method")

    stop_tokens = shlex.split(shells[2])
    check("stop cmd = python3 validate.py . method",
          stop_tokens[0] == "python3" and stop_tokens[1].endswith("validate.py")
          and stop_tokens[-2] == "." and stop_tokens[-1] == "method")
    review_tokens = shlex.split(shells[3])
    check("review cmd = python3 review.py . method",
          review_tokens[0] == "python3" and review_tokens[1].endswith("review.py")
          and review_tokens[-2] == "." and review_tokens[-1] == "method")

    # A quoted path round-trips through the YAML single-quoted scalar unchanged.
    win = assemble._yaml_cmd('"$CLAUDE_PROJECT_DIR/.genesis/hooks/gate.py"')
    inner = unyaml_single(win.strip()[len("command:"):].strip())
    check("quoted $CLAUDE_PROJECT_DIR path preserved literally", inner == '"$CLAUDE_PROJECT_DIR/.genesis/hooks/gate.py"')
    q = assemble._yaml_cmd('''"/home/O'Brien/hooks/gate.py"''')
    check("single quote in path is YAML-doubled", "''" in q)

    try:
        import yaml
        block = fm.split("---", 2)[1]
        doc = yaml.safe_load(block)
        ok = doc.get("name") == "method" and "SessionStart" in doc["hooks"] and "Stop" in doc["hooks"]
        check("frontmatter parses as valid YAML", ok)
    except ImportError:
        print("  SKIP  YAML parse (pyyaml not installed)")

    # Sensei gets an ADDITIONAL Sensei-only Bash gate (enforce_research); still portable.
    sensei_fm = assemble.frontmatter("sensei", {"description": "d", "tools": ["Read", "Bash", "Agent"]}, HOME, [])
    scmds = command_lines(sensei_fm)
    check("sensei has five hook commands (adds enforce_research)", len(scmds) == 5)
    check("sensei wires enforce_research under a Bash matcher (portable path)",
          'matcher: "Bash"' in sensei_fm
          and any("$CLAUDE_PROJECT_DIR/.genesis/hooks/enforce_research.py" in unyaml_single(c) for c in scmds))
    check("method (non-sensei) has NO enforce_research / Bash matcher",
          "enforce_research.py" not in fm and 'matcher: "Bash"' not in fm)
    try:
        import yaml
        sdoc = yaml.safe_load(sensei_fm.split("---", 2)[1])
        matchers = {blk.get("matcher") for blk in sdoc["hooks"]["PreToolUse"]}
        check("sensei PreToolUse has both Write|Edit and Bash matchers", {"Write|Edit", "Bash"} <= matchers)
    except ImportError:
        print("  SKIP  sensei YAML parse (pyyaml not installed)")

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
