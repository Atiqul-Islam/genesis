#!/usr/bin/env python3
"""VALIDATE hook (Stop / SubagentStop) — the loop-closer.

§16: gate prevents; validate refuses to FINISH while a checkable rule is still violated. The agent cannot
end its turn until the offending files are fixed. Semantic quality is NOT judged here (that is Method's
reviewer subagent) — only rules a regex/count can prove.

Wire in a member's frontmatter or settings.json:
  "Stop": [{ "hooks": [{ "type":"command", "command":"python3 <this> <glob_root>", "timeout":15 }] }]

Scans <glob_root> (default: cwd) for produced agent files and blocks the stop with a reason listing every
offender, so the agent knows exactly what to fix. Guards against an infinite block loop via a bounded set.
"""
import glob, json, os, re, sys

BANNED_PHRASE = re.compile(r"chain[\s\-]?of[\s\-]?thought", re.I)
CREDS = [re.compile(r"AKIA[0-9A-Z]{16}"),
         re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
         re.compile(r"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['\"]?[^\s'\"]{6,}")]
LINE_BUDGET = 200


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


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        ev = {}
    # Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
    if ev.get("stop_hook_active"):
        sys.exit(0)
    root = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
    bad = offenders(root)
    if bad:
        print(json.dumps({"decision": "block",
                          "reason": "Cannot finish — checkable rules still violated:\n- " +
                                    "\n- ".join(bad[:20]) + "\nFix these, then stop again."}))
        sys.exit(0)
    sys.exit(0)  # clean → allow the stop


if __name__ == "__main__":
    main()
