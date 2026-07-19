#!/usr/bin/env python3
"""GATE hook (PreToolUse: Write|Edit) — deterministic enforcement of the checkable house rules.

§16: a hook is the ONLY way to make a rule deterministic — prompt text is hope, a gate is a guarantee.
This BLOCKS a Write/Edit before it happens when the content violates a rule that can be checked over the
tool input. It cannot judge semantics (that's Method's reviewer); it enforces only what a regex/count proves.

Wire in settings.json:
  "PreToolUse": [{ "matcher": "Write|Edit",
                   "hooks": [{ "type": "command", "command": "python3 <this>", "timeout": 15 }] }]

Reads the PreToolUse event JSON on stdin. On a violation, prints a `deny` decision (exit 0) — Claude Code
mechanically refuses the tool call. Cannot be talked around.
"""
import json, re, sys

# Rules that are CHECKABLE over the file content. Each returns a reason string if violated, else None.
BANNED_PHRASE = re.compile(r"chain[\s\-]?of[\s\-]?thought", re.I)
CRED_PATTERNS = [
    (re.compile(r"AKIA[0-9A-Z]{16}"), "AWS access key id"),
    (re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"), "private key block"),
    (re.compile(r"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['\"]?[^\s'\"]{6,}"),
     "credential value"),
]
# Files whose length is budgeted (persona/behavior/rules kept lean so adherence doesn't decay).
BUDGETED = re.compile(r"(persona|behavior)\.md$|(^|/)CLAUDE\.md$", re.I)
LINE_BUDGET = 200


def deny(reason):
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny",
        "permissionDecisionReason": reason}}))
    sys.exit(0)


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        sys.exit(0)  # not our event / no input → allow, fail-open on parse
    ti = ev.get("tool_input", {}) or {}
    path = ti.get("file_path", "") or ""
    # Edit carries new_string; Write carries content.
    content = ti.get("content") or ti.get("new_string") or ""

    if BANNED_PHRASE.search(content):
        deny('Accuracy rule: do not write "chain-of-thought" — use "structured reasoning" / '
             '"step-by-step reasoning". Reword and retry.')

    for rx, what in CRED_PATTERNS:
        if rx.search(content):
            deny(f'Security rule: this looks like a committed {what}. Never write a credential value. '
                 f'Reference it as "credential present at <path>" instead.')

    if BUDGETED.search(path):
        n = content.count("\n") + 1
        if n > LINE_BUDGET:
            deny(f"Budget rule: {path} is {n} lines (>{LINE_BUDGET}). Keep persona/behavior/rules lean so "
                 f"adherence doesn't decay — trim to the smallest high-signal set, then retry.")

    # No checkable violation → allow (stay silent; let other hooks/permissions decide).
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "allow"}}))


if __name__ == "__main__":
    main()
