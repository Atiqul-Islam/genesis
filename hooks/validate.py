#!/usr/bin/env python3
"""VALIDATE hook (Stop / SubagentStop) — the loop-closer.

§16: gate prevents; validate refuses to FINISH while a checkable rule is still violated, OR while the agent
never declared it applied its required expertise. The agent cannot end its turn until both hold. Semantic
quality is NOT judged here (that is Method's reviewer subagent) — only rules a regex/count can prove, plus
the presence of the expertise declaration.

Wire in a member's frontmatter or settings.json (argv[2] = the agent name, so the hook knows what to require):
  "Stop": [{ "hooks": [{ "type":"command", "command":"python3 <this> <glob_root> <agent_name>", "timeout":15 }] }]

Two enforcement layers:
  1. Checkable-rule offenders in produced files (banned phrase / credential / line budget).
  2. Expertise declaration: the agent must emit `APPLIED-EXPERTISE: <name>#<rule-ids>` for EVERY expertise
     required of it (expertise/required.json), proving it loaded + reasoned with each. Missing → block with a
     minimal-correction reason (re-read + declare), which is cheap to satisfy (no full rebuild).

Fail-closed: if a check can run and fails, block. Guards an infinite block loop via `stop_hook_active`.
"""
import glob, json, os, re, sys

BANNED_PHRASE = re.compile(r"chain[\s\-]?of[\s\-]?thought", re.I)
CREDS = [re.compile(r"AKIA[0-9A-Z]{16}"),
         re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
         re.compile(r"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['\"]?[^\s'\"]{6,}")]
LINE_BUDGET = 200
DECL = re.compile(r"APPLIED-EXPERTISE:\s*([a-z0-9\-]+)", re.I)  # names the expertise the agent applied
HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
REQUIRED_JSON = os.path.join(HOOK_DIR, "..", "expertise", "required.json")
LOG = os.path.join(HOOK_DIR, "..", ".genesis", "hook-decisions.log")


def log(agent, decision, reasons):
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write(json.dumps({"hook": "validate", "agent": agent,
                                 "decision": decision, "reasons": reasons}) + "\n")
    except Exception:
        pass  # logging must never itself break the gate


def offenders(root):
    out = []
    pats = [os.path.join(root, "**", n) for n in
            ("*persona.md", "*behavior.md", "CLAUDE.md", ".claude/agents/*.md")]
    seen = set()
    for pat in pats:
        for f in glob.glob(pat, recursive=True):
            if f in seen or not os.path.isfile(f):
                continue
            seen.add(f)
            try:
                txt = open(f, encoding="utf-8", errors="replace").read()
            except Exception:
                continue
            if BANNED_PHRASE.search(txt):
                out.append(f'{f}: contains "chain-of-thought" — use "structured reasoning".')
            if any(rx.search(txt) for rx in CREDS):
                out.append(f"{f}: looks like a committed credential value — remove it.")
            n = txt.count("\n") + 1
            if re.search(r"(persona|behavior)\.md$|CLAUDE\.md$", f) and n > LINE_BUDGET:
                out.append(f"{f}: {n} lines (>{LINE_BUDGET} budget) — trim.")
    return out


def required_for(agent):
    """Expertise names this agent must declare. Missing map/agent → no requirement (checkable rules still run)."""
    if not agent:
        return []
    try:
        return json.load(open(REQUIRED_JSON, encoding="utf-8")).get(agent, [])
    except Exception:
        return []


def declared_in_transcript(path):
    """Set of expertise names the agent declared via `APPLIED-EXPERTISE:` in its assistant messages.
    Returns None if the transcript path was given but could not be read (a real error → fail closed)."""
    if not path or not os.path.isfile(path):
        return set()  # not provided → declaration simply unverifiable here; treated as none-declared below
    try:
        names = set()
        for line in open(path, encoding="utf-8", errors="replace"):
            try:
                ev = json.loads(line)
            except Exception:
                continue
            if ev.get("type") != "assistant":
                continue
            content = ev.get("message", {}).get("content", "")
            blocks = content if isinstance(content, list) else [{"type": "text", "text": content}]
            for b in blocks:
                if b.get("type") == "text":
                    for m in DECL.finditer(b.get("text", "")):
                        names.add(m.group(1).lower())
        return names
    except Exception:
        return None  # unreadable despite existing → fail closed (block)


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        ev = {}
    # Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
    if ev.get("stop_hook_active"):
        sys.exit(0)
    root = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
    agent = sys.argv[2] if len(sys.argv) > 2 else ""

    reasons = offenders(root)

    # Expertise-declaration check (only when this agent has required expertise).
    required = required_for(agent)
    if required:
        declared = declared_in_transcript(ev.get("transcript_path", ""))
        if declared is None:  # transcript existed but unreadable → fail closed
            reasons.append("Could not read the transcript to verify the expertise declaration — cannot finish.")
        else:
            missing = [e for e in required if e.lower() not in declared]
            if missing:
                reasons.append(
                    "You never declared applying your required expertise: " + ", ".join(missing) +
                    ". Re-read each at expertise/<name>.md, apply its rules, then emit one line per "
                    "expertise: `APPLIED-EXPERTISE: <name>#<rule-ids>`. (If the work already follows them, "
                    "just add the declaration lines.)")

    if reasons:
        log(agent, "block", reasons)
        print(json.dumps({"decision": "block",
                          "reason": "Cannot finish:\n- " + "\n- ".join(reasons[:20]) +
                                    "\nFix these, then stop again."}))
        sys.exit(0)
    log(agent, "allow", [])
    sys.exit(0)  # clean → allow the stop


if __name__ == "__main__":
    main()
