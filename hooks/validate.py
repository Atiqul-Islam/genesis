#!/usr/bin/env python3
"""VALIDATE hook (Stop / SubagentStop) — the loop-closer.

§16: gate prevents; validate refuses to FINISH while a checkable rule is still violated, OR while the agent
never *credibly* declared it applied its required expertise. The agent cannot end its turn until both hold.
Deep semantic quality is NOT judged here (that is the independent LLM-reviewer, review.py / §22, and Method's
reviewer subagent) — only what a regex/count can prove, plus the INTEGRITY of the expertise declaration.

Wire in a member's frontmatter or settings.json (argv[2] = the agent name, so the hook knows what to require).
assemble.py generates this line with the absolute interpreter (sys.executable) + quoted paths for Windows/POSIX:
  "Stop": [{ "hooks": [{ "type":"command", "command":"<python> <this> <glob_root> <agent_name>", "timeout":15 }] }]

Three enforcement layers:
  1. Checkable-rule offenders in produced files (banned phrase / credential / line budget).
  2. Expertise declaration is EVIDENCE-CARRYING, not a bare token. For every expertise required of it
     (expertise/required.json) the agent must emit, one line per governing rule:
         APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>
     and the validator checks the declaration is CREDIBLE:
       (a) the <rule-id> actually exists in that expertise's manifest (expertise/manifests/<name>.json) —
           a fabricated / hallucinated id blocks (arXiv:2606.04923-style declaration gaming);
       (b) at least a floor of distinct real rules is cited per expertise (a name-only or single-token
           declaration is insufficient — the exact bare-token hole this replaces);
       (c) the <evidence> is SPOT-CHECKED against the produced artifacts — a file pointer must resolve to a
           real file, and a quoted string must actually appear in an artifact. Fabricated evidence blocks.
  3. Artifact spot-check: the mechanically-checkable rules are verified against the produced files directly
     (layer 1), so a *cited* checkable rule that the artifact violates is caught — "verify the artifact
     embodies the rule, not just that the token exists" (expertise-application ea-11).

Honest limit (ea-11/ea-12): the deterministic layer proves citation INTEGRITY + the mechanically-checkable
subset; it cannot prove a *judgment* rule was truly applied. That residue is the independent LLM-reviewer's
job (§22). This hook makes the declaration non-trivial to fake, not impossible — defense in depth, not a wall.

Fail-closed: if a check can run and fails, block. Guards an infinite block loop via `stop_hook_active`.
"""
import datetime, glob, json, os, re, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import agent_ident  # noqa: E402  (sibling helper; travels with hooks/ in every install)

BANNED_PHRASE = re.compile(r"chain[\s\-]?of[\s\-]?thought", re.I)
CREDS = [re.compile(r"AKIA[0-9A-Z]{16}"),
         re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
         re.compile(r"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['\"]?[^\s'\"]{6,}")]
LINE_BUDGET = 200
BUDGETED = re.compile(r"(persona|behavior)\.md$|(^|/)CLAUDE\.md$", re.I)

# Evidence-carrying declaration: `APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>` (one rule per line).
DECL = re.compile(r"APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*#\s*([a-z]+-\d+)\b[ \t]*(?:[—:\-]+[ \t]*(.*?))?[ \t]*$",
                  re.I | re.M)
# Bare `APPLIED-EXPERTISE: <name>` with no `#<rule-id>` — the gameable form we now reject.
BARE = re.compile(r"APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*(?:$|[^#\w])", re.I | re.M)
# A path-like token inside evidence (must resolve to a real file).
PATHISH = re.compile(r"[\w./\\\-]+\.(?:md|jsonc?|toml|ya?ml|txt|py|js|ts|rs|cfg|ini)", re.I)
# A quoted span inside evidence (must appear in a produced artifact).
QUOTED = re.compile(r"\"([^\"]{8,})\"|`([^`]{8,})`|'([^']{8,})'")

FLOOR = 3  # min distinct real rule-ids per required expertise (or all checkable, if fewer)

HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
# READ-ONLY reference data (manifests + required.json) ships NEXT TO the hooks — resolve it hook-relative
# (plugin: ${CLAUDE_PLUGIN_ROOT}/expertise; bootstrap: <repo>/.genesis/expertise). It is never written.
REQUIRED_JSON = os.path.join(HOOK_DIR, "..", "expertise", "required.json")
MANIFEST_DIR = os.path.join(HOOK_DIR, "..", "expertise", "manifests")
# WRITABLE runtime data goes under the PROJECT's .genesis/ (cwd-derived) — NEVER the plugin dir (read-only +
# version-churned) and NEVER hook-dir-relative. See agent_ident.runtime_dir.
LOG = os.path.join(agent_ident.runtime_dir(), "hook-decisions.log")

ARTIFACT_GLOBS = ("*persona.md", "*behavior.md", "CLAUDE.md", ".claude/agents/*.md",
                  "*persona.spec.json", "*.tests.json")


def log(agent, decision, reasons, cited=None, session=""):
    """Append one structured, timestamped adherence record. The adherence harness (adherence.py) reads
    these to compute per-rule citation + violation rates over time (ea-10) — so `cited` (the parsed
    declaration) is logged on ALLOW too, not just failures."""
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        rec = {"ts": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
               "hook": "validate", "agent": agent, "session": session,
               "decision": decision, "reasons": reasons, "cited": cited or {}}
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec) + "\n")
    except Exception:
        pass  # logging must never itself break the gate


def produced_files(root):
    """{path: text} for every artifact an agent plausibly authored under root (deduped)."""
    out, seen = {}, set()
    for name in ARTIFACT_GLOBS:
        for f in glob.glob(os.path.join(root, "**", name), recursive=True):
            if f in seen or not os.path.isfile(f):
                continue
            seen.add(f)
            try:
                out[f] = open(f, encoding="utf-8", errors="replace").read()
            except Exception:
                pass
    return out


def offenders(files):
    """Layer-1: mechanically-checkable rule violations in the produced artifacts."""
    out = []
    for f, txt in files.items():
        if BANNED_PHRASE.search(txt):
            out.append(f'{f}: contains "chain-of-thought" — use "structured reasoning".')
        if any(rx.search(txt) for rx in CREDS):
            out.append(f"{f}: looks like a committed credential value — remove it.")
        if BUDGETED.search(f):
            n = txt.count("\n") + 1
            if n > LINE_BUDGET:
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


def load_manifest(name):
    """(all_rule_ids, checkable_rule_ids) for an expertise, or (None, None) if the manifest can't be read
    (→ fail closed: we cannot verify citations, so we must not certify)."""
    try:
        d = json.load(open(os.path.join(MANIFEST_DIR, f"{name}.json"), encoding="utf-8"))
    except Exception:
        return None, None
    ids, checkable = set(), set()
    for r in d.get("rules", []):
        rid = str(r.get("id", "")).lower()
        if not rid:
            continue
        ids.add(rid)
        if r.get("type") == "checkable":
            checkable.add(rid)
    return ids, checkable


def parse_declarations(path):
    """Parse the transcript's assistant text.
    Returns (decls, bare_names) or (None, None) if the transcript existed but was unreadable (fail closed).
      decls      = { expertise_name: [ (rule_id, evidence_str), ... ] }
      bare_names = { expertise_name that appeared WITHOUT any #rule-id }  (the gameable form)"""
    if not path or not os.path.isfile(path):
        return {}, set()  # not provided → treated as nothing declared (blocks below if expertise is required)
    try:
        decls, bare = {}, set()
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
                if b.get("type") != "text":
                    continue
                text = b.get("text", "")
                for m in DECL.finditer(text):
                    name, rid, evid = m.group(1).lower(), m.group(2).lower(), (m.group(3) or "").strip()
                    decls.setdefault(name, []).append((rid, evid))
                for m in BARE.finditer(text):
                    bare.add(m.group(1).lower())
        return decls, bare
    except Exception:
        return None, None  # unreadable despite existing → fail closed (block)


def _quote_norm(s):
    return re.sub(r"\s+", " ", s).strip().lower()


def check_evidence(evid, files_blob):
    """Spot-check one evidence string against the produced artifacts.
    Returns a violation string if the evidence is verifiably fabricated, else None (unverifiable → accept;
    the LLM-reviewer covers the semantic residue)."""
    pm = PATHISH.search(evid or "")
    if pm:
        tok = pm.group(0).replace("\\", "/")
        base = os.path.basename(tok)
        # resolve as-is, relative to root, or by basename anywhere under root
        if not (os.path.isfile(tok) or any(k.replace("\\", "/").endswith(tok) or
                                           os.path.basename(k) == base for k in files_blob)):
            return f'evidence points to "{tok}" but no such produced file exists'
        return None
    qm = QUOTED.search(evid or "")
    if qm:
        q = _quote_norm(next(g for g in qm.groups() if g))
        blob = _quote_norm("\n".join(files_blob.values()))
        if q and q not in blob:
            return f'quoted evidence "{q[:40]}…" does not appear in any produced artifact'
    return None


def verify_declaration(required, decls, bare, files):
    """Layers 2+3. Returns a list of block reasons (empty = declaration is credible)."""
    reasons = []
    files_blob = files  # {path: text}
    for name in required:
        all_ids, checkable = load_manifest(name)
        if all_ids is None:
            reasons.append(f"Could not read the manifest for '{name}' to verify your citations — cannot finish.")
            continue
        entries = decls.get(name, [])
        if not entries:
            hint = (" You wrote a bare `APPLIED-EXPERTISE: %s` with no rule-ids — that no longer counts." % name
                    ) if name in bare else ""
            reasons.append(
                f"You did not credibly declare applying '{name}'.{hint} Re-read expertise/{name}.md, then emit "
                f"one line per governing rule you applied: `APPLIED-EXPERTISE: {name}#<rule-id> — <evidence>` "
                f"(evidence = the file it's embodied in, or a short quote from it).")
            continue
        # (a) citation integrity — every cited id must be real
        bad = sorted({rid for rid, _ in entries if rid not in all_ids})
        if bad:
            reasons.append(f"'{name}': these cited rule-ids are not in the manifest (fabricated?): "
                           + ", ".join(bad) + f". Cite real ids from expertise/manifests/{name}.json.")
        # (b) coverage floor
        good = {rid for rid, _ in entries if rid in all_ids}
        floor = min(FLOOR, len(checkable) or FLOOR)
        if len(good) < floor:
            reasons.append(f"'{name}': only {len(good)} valid rule(s) cited; cite at least {floor} of the "
                           f"governing rules you actually applied (not a token gesture).")
        # (c) evidence spot-check
        for rid, evid in entries:
            if rid not in all_ids:
                continue
            v = check_evidence(evid, files_blob)
            if v:
                reasons.append(f"'{name}#{rid}': {v}.")
    return reasons


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        ev = {}
    # Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
    if ev.get("stop_hook_active"):
        sys.exit(0)
    # Agent identity: explicit argv positional (built-agent frontmatter path + tests) > payload
    # `agent_type` (plugin session-level path) > --main-agent fallback (main thread = sensei).
    argv_root, argv_agent, main_agent = agent_ident.split_args(sys.argv[1:])
    root = argv_root if argv_root is not None else os.getcwd()
    agent = agent_ident.resolve_agent(ev, argv_agent, main_agent)
    session = ev.get("session_id", "") or ""

    files = produced_files(root)
    reasons = offenders(files)

    cited = {}
    required = required_for(agent)
    if required:
        decls, bare = parse_declarations(ev.get("transcript_path", ""))
        if decls is None:  # transcript existed but unreadable → fail closed
            reasons.append("Could not read the transcript to verify the expertise declaration — cannot finish.")
        else:
            # record only citations to REAL rule-ids (for the adherence report), per required expertise
            for name in required:
                all_ids, _ = load_manifest(name)
                if all_ids:
                    ids = sorted({rid for rid, _ in decls.get(name, []) if rid in all_ids})
                    if ids:
                        cited[name] = ids
            reasons += verify_declaration(required, decls, bare, files)

    if reasons:
        log(agent, "block", reasons, cited, session)
        print(json.dumps({"decision": "block",
                          "reason": "Cannot finish:\n- " + "\n- ".join(reasons[:20]) +
                                    "\nFix these, then stop again."}))
        sys.exit(0)
    log(agent, "allow", [], cited, session)
    sys.exit(0)  # clean → allow the stop


if __name__ == "__main__":
    main()
