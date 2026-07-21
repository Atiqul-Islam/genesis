#!/usr/bin/env python3
"""ENFORCE-RESEARCH hook (PreToolUse: Bash) — Sensei-scoped (§16).

The build-agent skill requires Sensei to run the `research-expertise` skill (select + user-confirm + research
an agent's expertise) BEFORE assembling that agent. Prompt text is hope; this hook makes it a guarantee: it
BLOCKS the `assemble.py` Bash call that builds a NON-builtin agent unless the session transcript shows the
`research-expertise` Skill was actually invoked this session. Sensei cannot skip the process.

Wired ONLY into Sensei's frontmatter (only Sensei orchestrates/assembles). Built-ins (sensei/method) are the
builder team itself and are exempt — their expertise is fixed, not researched per build.

Reads the PreToolUse event JSON on stdin. Decision:
  * command is not an `assemble.py <src> <name> ...` invocation      -> allow (not our concern)
  * the assembled agent name is a built-in (sensei/method)           -> allow
  * transcript shows a `research-expertise` Skill tool_use           -> allow
  * otherwise (incl. transcript missing/unreadable)                  -> DENY (fail-closed)

Fail-closed: if it IS a build of a non-builtin agent and we cannot confirm the skill ran, block.
"""
import datetime, json, os, re, shlex, sys

BUILTINS = {"sensei", "method"}
SKILL_NAME = "research-expertise"
HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
# Runtime dir: the genesis home when it IS a `.genesis/` workspace (bootstrapped repo), else home/.genesis
# (central install) — avoids a nested .genesis/.genesis/ in a bootstrapped repo.
_HOME = os.path.dirname(HOOK_DIR)
_RUNTIME = _HOME if os.path.basename(_HOME) == ".genesis" else os.path.join(_HOME, ".genesis")
LOG = os.path.join(_RUNTIME, "hook-decisions.log")


def log(decision, agent, reason=""):
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        rec = {"ts": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
               "hook": "enforce_research", "decision": decision, "agent": agent, "reason": reason}
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec) + "\n")
    except Exception:
        pass


def deny(agent, reason):
    log("deny", agent, reason)
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny",
        "permissionDecisionReason": reason}}))
    sys.exit(0)


def allow(agent="", note=""):
    if agent:
        log("allow", agent, note)
    sys.exit(0)  # stay silent; let normal permissions decide


def assembled_agent_name(command):
    """If `command` is an `assemble.py <src> <name> <target> <gh>` invocation, return <name>, else None."""
    try:
        toks = shlex.split(command)
    except Exception:
        toks = command.split()
    for i, t in enumerate(toks):
        if t.replace("\\", "/").endswith("assemble.py"):
            # positional args after the script: src(i+1), name(i+2), target(i+3), gh(i+4)
            if i + 2 < len(toks):
                return toks[i + 2]
            return ""  # malformed assemble call — treat as unknown (fail-closed below)
    return None


def research_skill_used(transcript_path):
    """True if the transcript shows a Skill tool_use naming research-expertise. None if the transcript path
    was given but unreadable (a real error → fail closed)."""
    if not transcript_path or not os.path.isfile(transcript_path):
        return False  # not provided / absent → cannot confirm → treated as not-used (deny for a built agent)
    try:
        for line in open(transcript_path, encoding="utf-8", errors="replace"):
            try:
                ev = json.loads(line)
            except Exception:
                continue
            if ev.get("type") != "assistant":
                continue
            content = ev.get("message", {}).get("content", "")
            blocks = content if isinstance(content, list) else []
            for b in blocks:
                if b.get("type") == "tool_use" and b.get("name") == "Skill":
                    inp = b.get("input", {}) or {}
                    if inp.get("skill") == SKILL_NAME or SKILL_NAME in json.dumps(inp):
                        return True
        return False
    except Exception:
        return None  # unreadable despite existing → fail closed


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        sys.exit(0)  # no/garbled input → allow, fail-open on parse (nothing to enforce)
    command = (ev.get("tool_input", {}) or {}).get("command", "") or ""
    name = assembled_agent_name(command)
    if name is None:
        allow()  # not an assemble.py invocation
    if name in BUILTINS:
        allow(name, "builtin-exempt")
    if not name:
        deny("(unknown)", "assemble.py invocation with no parseable agent name — cannot verify the "
                          "research-expertise skill ran; refusing to build (fail-closed).")

    used = research_skill_used(ev.get("transcript_path", ""))
    if used is True:
        allow(name, "research-expertise skill confirmed")
    # used is False or None -> fail closed
    deny(name, f"You must run the `research-expertise` skill to select and research '{name}'s expertise "
               f"(with the user) BEFORE assembling it. Invoke the research-expertise skill, complete the "
               f"process, then assemble again.")


if __name__ == "__main__":
    main()
