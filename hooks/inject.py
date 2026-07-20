#!/usr/bin/env python3
"""INJECT hook (SessionStart) — deterministic DELIVERY of the house rules + expertise pointers.

§16: injection guarantees the expertise reaches the context window; it does NOT guarantee obedience — the
gate/validate hooks do that for the checkable rules. So this stays small (the full 100-200KB reports are
NOT injected — that would blow the 10,000-char cap and the attention budget). It delivers:
  1. the checkable house rules (also enforced by gate.py / validate.py), and
  2. pointers to the decoupled expertise store, which the agent READS on demand for deep work.

Wire in a member's frontmatter or settings.json:
  "SessionStart": [{ "matcher":"startup|resume|compact",
                     "hooks":[{ "type":"command","command":"python3 <this> <expertise_dir>","timeout":10 }] }]

Written as factual statements (not out-of-band commands) so it isn't flagged as prompt-injection (§16).
"""
import json, os, sys

RULES = """Genesis house rules (enforced by hooks — the gate blocks, the validator refuses to finish):
- Never write "chain-of-thought"; use "structured reasoning" / "step-by-step reasoning".
- Never write a credential value; reference it as "credential present at <path>".
- Keep persona.md / behavior.md / CLAUDE.md at or under 200 lines each.
- These are checkable and enforced deterministically; do not rely on memory to honor them."""


def main():
    exp_dir = sys.argv[1] if len(sys.argv) > 1 else ""
    agent = sys.argv[2] if len(sys.argv) > 2 else ""
    pointers = ""
    if exp_dir and os.path.isdir(exp_dir):
        files = sorted(f for f in os.listdir(exp_dir) if f.endswith(".md"))
        if files:
            lines = "\n".join(f"- {os.path.splitext(f)[0]}: {os.path.join(exp_dir, f)}" for f in files)
            pointers = ("\nYour expertise store (decoupled, authoritative — read the file your behavior "
                        "names, on demand, before deep work):\n" + lines)

    required = ""
    if agent and exp_dir:
        try:
            req = json.load(open(os.path.join(exp_dir, "required.json"), encoding="utf-8")).get(agent, [])
        except Exception:
            req = []
        if req:
            required = (f"\nYou are '{agent}'. Every task, load and apply these REQUIRED expertise: "
                        + ", ".join(req) + ". Before finishing, declare each on its own line — "
                        "`APPLIED-EXPERTISE: <name>#<rule-ids>`. The Stop hook (validate) blocks finishing "
                        "until all are declared; declaring is cheap, so do it.")

    ctx = RULES + required + pointers
    if len(ctx) > 9500:                       # stay under the 10,000-char hook-output cap
        ctx = ctx[:9500] + "\n…(truncated)"
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "SessionStart", "additionalContext": ctx}}))


if __name__ == "__main__":
    main()
