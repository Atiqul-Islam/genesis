#!/usr/bin/env python3
"""INJECT hook (SessionStart) — deterministic DELIVERY of the house rules + expertise pointers.

§16: injection guarantees the expertise reaches the context window; it does NOT guarantee obedience — the
gate/validate hooks do that for the checkable rules. So this stays small (the full 100-200KB reports are
NOT injected — that would blow the 10,000-char cap and the attention budget). It delivers:
  1. the checkable house rules (also enforced by gate.py / validate.py), and
  2. pointers to the decoupled expertise store, which the agent READS on demand for deep work.

Wire in a member's frontmatter or settings.json:
  "SessionStart": [{ "matcher":"startup|resume|compact",
                     "hooks":[{ "type":"command","command":"<python> <this> <expertise_dir>","timeout":10 }] }]
  (assemble.py fills <python> with the absolute interpreter + quotes paths, so it works on Windows and POSIX.)

Written as factual statements (not out-of-band commands) so it isn't flagged as prompt-injection (§16).
"""
import json, os, sys

RULES = """Genesis house rules (enforced by hooks — the gate blocks, the validator refuses to finish):
- Never write "chain-of-thought"; use "structured reasoning" / "step-by-step reasoning".
- Never write a credential value; reference it as "credential present at <path>".
- Keep persona.md / behavior.md / CLAUDE.md at or under 200 lines each.
- These are checkable and enforced deterministically; do not rely on memory to honor them."""

SUMMARY_CAP = 7000  # chars of the session-copy digest to inject; the rest is recall-only


def find_summary(exp_dir, agent):
    """A session-copy agent has a carried-over digest at <genesis_home>/agents/<name>/summary.md (derivable
    from exp_dir = <genesis_home>/expertise) or <cwd>/.genesis/agents/<name>/summary.md. First hit wins."""
    if not agent:
        return None
    cands = []
    if exp_dir:
        cands.append(os.path.join(os.path.dirname(os.path.abspath(exp_dir)), "agents", agent, "summary.md"))
    cands.append(os.path.join(os.getcwd(), ".genesis", "agents", agent, "summary.md"))
    for c in cands:
        if os.path.isfile(c):
            return c
    return None


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
                        + ", ".join(req) + ". Each has a rule manifest at expertise/manifests/<name>.json "
                        "(stable rule-ids + predicates). Before finishing, declare the governing rules you "
                        "actually applied — ONE LINE PER RULE, carrying evidence:\n"
                        "  APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>\n"
                        "where <evidence> is the file the rule is embodied in (e.g. release-manager/CLAUDE.md) "
                        "or a short verbatim quote from your output. The Stop hook (validate) verifies each "
                        "citation: a bare `APPLIED-EXPERTISE: <name>` with no rule-id, a rule-id not in the "
                        "manifest, or evidence pointing to a nonexistent file / a quote absent from your work "
                        "all BLOCK finishing. Cite at least the rules you truly used (a token gesture fails).")

    # Session-copy agents: surface the carried-over memory digest at start (D5). Recall the rest on demand.
    summary_block = ""
    sp = find_summary(exp_dir, agent)
    if sp:
        try:
            body = open(sp, encoding="utf-8", errors="replace").read().strip()
        except Exception:
            body = ""
        if body:
            if len(body) > SUMMARY_CAP:
                body = body[:SUMMARY_CAP] + "\n…(digest truncated — recall the rest via your memory tools)"
            summary_block = ("\n\n## Your carried-over session memory (you were created by copying a prior "
                             "Claude Code session)\n" + body +
                             "\nThe full prior conversation history + memory is stored under your agent id — "
                             "recall any specific detail with your memory tools (recall).")

    ctx = RULES + required + pointers + summary_block
    if len(ctx) > 9500:                       # stay under the 10,000-char hook-output cap
        ctx = ctx[:9500] + "\n…(truncated)"
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "SessionStart", "additionalContext": ctx}}))


if __name__ == "__main__":
    main()
