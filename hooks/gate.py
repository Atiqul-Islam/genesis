#!/usr/bin/env python3
"""GATE hook (PreToolUse: Write|Edit) — deterministic enforcement + just-in-time rule SURFACING.

Two jobs, both at the moment a file is about to be written:

  1. BLOCK a Write/Edit whose content violates a checkable house rule (banned phrase / credential value /
     line budget). A hook is the ONLY way to make a rule deterministic — prompt text is hope, a gate is a
     guarantee (§16). It cannot judge semantics (that is the reviewer); it enforces only what a regex/count
     proves, over the tool input, before the write lands.

  2. SURFACE the governing rules for the artifact being written — re-asserting the top rules at the TAIL of
     the context, right before the consequential action, so the agent writes with them fresh. This is the
     expertise-application tail-re-assertion lever (ea-4 / ea-9): injection at SessionStart decays within
     ~8 turns (attention decay); a PreToolUse surface puts the rules back at the point of use, cache-safe
     (it rides in the tool-call turn, not the cached prefix). NON-BLOCKING — it informs, it does not refuse.

Wire in settings.json (assemble.py generates this with the absolute interpreter + quoted paths, cross-OS):
  "PreToolUse": [{ "matcher": "Write|Edit",
                   "hooks": [{ "type": "command", "command": "<python> <this>", "timeout": 15 }] }]

Reads the PreToolUse event JSON on stdin. On a violation → `deny` (Claude Code mechanically refuses the call).
Otherwise → `allow`, carrying `additionalContext` with the surfaced rules when the write is a substantial
authoring artifact. Degrades gracefully: if a Claude Code build ignores PreToolUse `additionalContext`, the
write still proceeds — the surface is a best-effort reinforcement, never a gate.
"""
import datetime, json, os, re, sys

# ---------- Layer 1: blocking checks (checkable over file content) ----------
BANNED_PHRASE = re.compile(r"chain[\s\-]?of[\s\-]?thought", re.I)
CRED_PATTERNS = [
    (re.compile(r"AKIA[0-9A-Z]{16}"), "AWS access key id"),
    (re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"), "private key block"),
    (re.compile(r"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['\"]?[^\s'\"]{6,}"),
     "credential value"),
]
BUDGETED = re.compile(r"(persona|behavior)\.md$|(^|/)CLAUDE\.md$", re.I)
LINE_BUDGET = 200

# ---------- Layer 2: rule surfacing ----------
HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
MANIFEST_DIR = os.path.join(HOOK_DIR, "..", "expertise", "manifests")
SURFACE_MIN = 300   # only surface for a substantial authoring write; skip tiny edits (noise + token cost)
SURFACE_N = 5       # top-N governing rules to re-assert
# artifact path → the expertise whose top rules to re-assert
AUTHORING = re.compile(r"(persona|behavior)\.md$|(^|/)CLAUDE\.md$|\.claude/agents/.*\.md$", re.I)
PROMPTISH = re.compile(r"(prompt|tool)[\w\-]*\.(md|jsonc?|txt)$|\.prompt$", re.I)
# Runtime dir: the genesis home when it IS a `.genesis/` workspace (bootstrapped repo), else home/.genesis
# (central install) — avoids a nested .genesis/.genesis/ in a bootstrapped repo.
_HOME = os.path.dirname(HOOK_DIR)
_RUNTIME = _HOME if os.path.basename(_HOME) == ".genesis" else os.path.join(_HOME, ".genesis")
LOG = os.path.join(_RUNTIME, "hook-decisions.log")


def log(decision, path, rule="", surfaced=""):
    """One timestamped PreToolUse record, feeding the adherence harness (ea-10). `rule` = which check fired
    on a deny; `surfaced` = the expertise whose rules were re-asserted on an allow."""
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        rec = {"ts": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
               "hook": "gate", "decision": decision, "path": path, "rule": rule, "surfaced": surfaced}
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec) + "\n")
    except Exception:
        pass


CAP = 9500  # additionalContext (and all hook output strings) is capped at 10,000 chars by Claude Code


def deny(reason, path="", rule=""):
    log("deny", path, rule=rule)
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny",
        "permissionDecisionReason": reason}}))
    sys.exit(0)


def proceed(context="", path="", surfaced=""):
    """No violation. We do NOT emit permissionDecision:'allow' — that would auto-approve every write and
    override the user's permission settings. Instead we stay out of the permission decision (normal flow
    applies) and, for a substantial authoring write, attach the surfaced rules via additionalContext
    (verified supported on PreToolUse: inserted 'next to the tool result', 10,000-char cap)."""
    if context:
        log("allow", path, surfaced=surfaced)
        print(json.dumps({"hookSpecificOutput": {
            "hookEventName": "PreToolUse", "additionalContext": context[:CAP]}}))
    sys.exit(0)  # nothing to surface → silent; let normal permissions decide


def target_manifest(path):
    if AUTHORING.search(path or ""):
        return "persona-creation"
    if PROMPTISH.search(path or ""):
        return "prompt-engineering"
    return None


def top_rules(name, n=SURFACE_N):
    """First n CHECKABLE rules from the manifest, each as 'id: first-sentence' (manifests are ordered
    foundational-first, so the head is the highest-leverage set)."""
    try:
        d = json.load(open(os.path.join(MANIFEST_DIR, f"{name}.json"), encoding="utf-8"))
    except Exception:
        return []
    out = []
    for r in d.get("rules", []):
        if r.get("type") != "checkable":
            continue
        first = re.split(r"(?<=[.:])\s", (r.get("text") or "").strip(), 1)[0]
        out.append(f'{r.get("id")}: {first[:170].rstrip()}')
        if len(out) >= n:
            break
    return out


def surface(path, content):
    """The tail re-assertion string for this write, or '' if it isn't a substantial authoring artifact."""
    if len((content or "")) < SURFACE_MIN:
        return ""
    name = target_manifest(path)
    if not name:
        return ""
    rules = top_rules(name)
    if not rules:
        return ""
    return (f"Governing rules for this artifact — re-asserted before you write ({name}; full manifest at "
            f"expertise/manifests/{name}.json). Apply them and cite the ones you use in your "
            f"APPLIED-EXPERTISE lines:\n- " + "\n- ".join(rules))


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        sys.exit(0)  # not our event / no input → allow, fail-open on parse
    ti = ev.get("tool_input", {}) or {}
    path = ti.get("file_path", "") or ""
    content = ti.get("content") or ti.get("new_string") or ""

    if BANNED_PHRASE.search(content):
        deny('Accuracy rule: do not write "chain-of-thought" — use "structured reasoning" / '
             '"step-by-step reasoning". Reword and retry.', path, "banned-phrase")

    for rx, what in CRED_PATTERNS:
        if rx.search(content):
            deny(f'Security rule: this looks like a committed {what}. Never write a credential value. '
                 f'Reference it as "credential present at <path>" instead.', path, "credential")

    if BUDGETED.search(path):
        n = content.count("\n") + 1
        if n > LINE_BUDGET:
            deny(f"Budget rule: {path} is {n} lines (>{LINE_BUDGET}). Keep persona/behavior/rules lean so "
                 f"adherence doesn't decay — trim to the smallest high-signal set, then retry.", path, "line-budget")

    # No checkable violation → proceed (defer to normal permissions), surfacing the governing rules for a
    # substantial authoring write.
    ctx = surface(path, content)
    proceed(ctx, path, surfaced=(target_manifest(path) or "") if ctx else "")


if __name__ == "__main__":
    main()
