#!/usr/bin/env python3
"""Portability tests for assemble.py frontmatter generation (§21 — Windows/POSIX).

Proves the generated hook `command:` lines are cross-platform: absolute interpreter (not bare 'python3',
which isn't on PATH on Windows), paths double-quoted so spaces survive, native separators preserved,
wrapped in a YAML single-quoted scalar so Windows backslashes round-trip intact.

Run:  python3 install/test_portability.py
"""
import os, shlex, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble  # noqa: E402


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

    # A genesis_home WITH A SPACE — the classic Windows "C:\Users\Jane Doe\..." breakage.
    gh = os.path.join(os.sep + "tmp", "gen esis home")
    fm = assemble.frontmatter("method", {"description": "d", "tools": ["Read", "Write"]}, gh, [])
    cmds = command_lines(fm)
    check("four hook commands generated (inject/gate/validate/review)", len(cmds) == 4)

    # Every command is a YAML single-quoted scalar (keeps inner double-quotes + backslashes literal).
    check("all commands are YAML single-quoted", all(c.startswith("'") and c.endswith("'") for c in cmds))

    # Interpreter is the absolute sys.executable, never bare 'python3'.
    shells = [unyaml_single(c) for c in cmds]
    check("uses sys.executable, not bare python3",
          all(sys.executable in s for s in shells) and not any(s.startswith("python3 ") for s in shells))

    # The space-containing path survives as ONE shell token (proves the double-quoting works).
    inject_tokens = shlex.split(shells[0])  # posix tokenization
    exp_dir = os.path.join(gh, "expertise")
    hook_script = os.path.join(gh, "hooks", "inject.py")
    check("interpreter is token[0]", inject_tokens[0] == sys.executable)
    check("inject.py path is one intact token", hook_script in inject_tokens)
    check("expertise dir with space is one intact token", exp_dir in inject_tokens)
    check("agent name is the trailing token", inject_tokens[-1] == "method")

    # Stop event has TWO hooks: validate then review; both carry the '.' root + agent name.
    stop_tokens = shlex.split(shells[2])
    check("stop cmd = python validate.py . method",
          stop_tokens[0] == sys.executable and stop_tokens[1].endswith("validate.py")
          and stop_tokens[-2] == "." and stop_tokens[-1] == "method")
    review_tokens = shlex.split(shells[3])
    check("review cmd = python review.py . method",
          review_tokens[0] == sys.executable and review_tokens[1].endswith("review.py")
          and review_tokens[-2] == "." and review_tokens[-1] == "method")

    # Windows backslash paths round-trip through the YAML single-quoted scalar unchanged.
    win = assemble._yaml_cmd('"C:\\Users\\Jane Doe\\genesis\\hooks\\gate.py"')
    scalar = win.strip()[len("command:"):].strip()
    inner = unyaml_single(scalar)
    check("windows backslashes preserved literally", inner == '"C:\\Users\\Jane Doe\\genesis\\hooks\\gate.py"')

    # A single quote in a path (e.g. user "O'Brien") is YAML-escaped by doubling.
    q = assemble._yaml_cmd('''"/home/O'Brien/hooks/gate.py"''')
    check("single quote in path is YAML-doubled", "''" in q and unyaml_single(q.strip()[len("command:"):].strip()) == '''"/home/O'Brien/hooks/gate.py"''')

    # The whole frontmatter parses as YAML when a parser is available (best-effort; skipped if no pyyaml).
    try:
        import yaml
        block = fm.split("---", 2)[1]
        doc = yaml.safe_load(block)
        ok = doc.get("name") == "method" and "SessionStart" in doc["hooks"] and "Stop" in doc["hooks"]
        check("frontmatter parses as valid YAML", ok)
    except ImportError:
        print("  SKIP  YAML parse (pyyaml not installed)")

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
