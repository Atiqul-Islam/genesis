#!/usr/bin/env python3
"""REVIEW hook (Stop / SubagentStop) — the INDEPENDENT semantic reviewer (§22; ea-5 / ea-8).

The gate/validate hooks prove the MECHANICAL subset (banned phrase / creds / line budget) + the INTEGRITY
of the expertise declaration. They cannot judge whether an artifact actually EMBODIES a rule whose predicate
is semantic (a persona's boundaries are load-bearing; a team's decomposition is sound). This hook closes that
loop with an INDEPENDENT judge, grounded on the manifest's judgment criteria + the produced artifacts.

It applies the anti-gaming controls from the rubric-reward-hacking literature (arXiv:2605.12474; MT-Bench
arXiv:2306.05685): the judge is a DIFFERENT model/instance from the author (never self-grades — self-
enhancement bias), and every verdict must survive a POSITION-SWAP (rules + option order reversed on a second
run); a verdict that flips across the swap is not trusted. Per ea-8 the judge can ONLY BLOCK: a PASS never
certifies success, it merely fails to find a violation. FAIL-CLOSED — reviewer error / timeout / unparseable
output / swap-inconsistency all resolve to a block when review is active.

Honest limit (ea-11/ea-12; arXiv:2605.12474): even a strong, swapped, cross-family verifier does not
eliminate gaming — the "wall" is structural. This raises the cost of faking application; it is not a proof.

Which rules it judges: type=="judgment" (reviewer_criterion) PLUS type=="checkable" whose predicate.kind is
semantic (structure/wiring/process/artifact) rather than mechanically executable (regex/line_count/declaration,
already enforced by the hooks). Principles are rationale, not per-artifact checkable — skipped (ea-0).

Config (env):
  GENESIS_REVIEW         "off" disables the semantic layer; "strict" blocks even when no reviewer is available;
                         default = on (review when a reviewer exists, else skip and log).
  GENESIS_REVIEWER_CMD   shell command for the judge; receives the review prompt on STDIN, prints verdict text
                         on STDOUT. Default: `claude -p --model <GENESIS_REVIEWER_MODEL>` (a different model /
                         instance from the author → independent). Set a non-Claude command for a TRUE
                         cross-family judge (the ea-8 ideal).
  GENESIS_REVIEWER_MODEL default judge model (default: claude-sonnet-5 — not the Opus author).
  GENESIS_REVIEW_MAX     max semantic rules judged per expertise per stop (default 8; coverage is LOGGED, never
                         silently truncated).
  GENESIS_REVIEW_TIMEOUT per-reviewer-call timeout seconds (default 120).

Wire (assemble.py adds this as a second Stop hook, AFTER validate.py):
  "Stop": [ { validate.py . <agent> }, { review.py . <agent> } ]
"""
import datetime, glob, json, os, re, shlex, shutil, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import agent_ident  # noqa: E402  (sibling helper; travels with hooks/ in every install)

MECHANICAL_KINDS = {"regex", "line_count", "declaration"}
HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
# READ-ONLY reference data ships next to the hooks (plugin: ${CLAUDE_PLUGIN_ROOT}/expertise) — hook-relative.
REQUIRED_JSON = os.path.join(HOOK_DIR, "..", "expertise", "required.json")
MANIFEST_DIR = os.path.join(HOOK_DIR, "..", "expertise", "manifests")
# WRITABLE runtime data → the PROJECT's .genesis/ (cwd-derived), never the plugin dir. See agent_ident.
LOG = os.path.join(agent_ident.runtime_dir(), "review.log")
ARTIFACT_GLOBS = ("*persona.md", "*behavior.md", "CLAUDE.md", ".claude/agents/*.md",
                  "*persona.spec.json", "*.tests.json")


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


def log(rows):
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        with open(LOG, "a", encoding="utf-8") as fh:
            for r in rows:
                fh.write(json.dumps(r) + "\n")
    except Exception:
        pass


def required_for(agent):
    if not agent:
        return []
    try:
        return json.load(open(REQUIRED_JSON, encoding="utf-8")).get(agent, [])
    except Exception:
        return []


def semantic_rules(name):
    """(judgment ∪ semantic-checkable) rules for an expertise as [(id, criterion)], judgment first.
    Returns None if the manifest can't be read (→ fail closed)."""
    try:
        d = json.load(open(os.path.join(MANIFEST_DIR, f"{name}.json"), encoding="utf-8"))
    except Exception:
        return None
    judged, semi = [], []
    for r in d.get("rules", []):
        t = r.get("type")
        if t == "judgment":
            judged.append((r["id"], r.get("reviewer_criterion") or r.get("text", "")))
        elif t == "checkable":
            pk = (r.get("predicate") or {}).get("kind")
            if pk not in MECHANICAL_KINDS:
                semi.append((r["id"], (r.get("predicate") or {}).get("spec") or r.get("text", "")))
        # principle → skip (rationale, not per-artifact checkable)
    return judged + semi


def produced_artifacts(root):
    out, seen = [], set()
    for name in ARTIFACT_GLOBS:
        for f in glob.glob(os.path.join(root, "**", name), recursive=True):
            if f in seen or not os.path.isfile(f):
                continue
            seen.add(f)
            try:
                out.append((f, open(f, encoding="utf-8", errors="replace").read()))
            except Exception:
                pass
    return out


def reviewer_cmd():
    raw = os.environ.get("GENESIS_REVIEWER_CMD")
    if raw:
        return shlex.split(raw)
    if shutil.which("claude"):
        return ["claude", "-p", "--model", os.environ.get("GENESIS_REVIEWER_MODEL", "claude-sonnet-5")]
    return None


def build_prompt(name, rules, artifacts_text, swap):
    order = list(reversed(rules)) if swap else rules
    opts = "FAIL or PASS" if swap else "PASS or FAIL"
    crit = "\n".join(f"- {rid}: {c}" for rid, c in order)
    return (
        f"You are an INDEPENDENT reviewer. You did NOT write the artifacts below. Judge whether they actually "
        f"EMBODY each rule criterion for the '{name}' expertise. Be skeptical: if an artifact does not clearly "
        f"satisfy a rule, mark it FAIL — do not give the benefit of the doubt.\n\n"
        f"RULES (criteria to check):\n{crit}\n\n"
        f"ARTIFACTS under review:\n<<<\n{artifacts_text[:12000]}\n>>>\n\n"
        f"For EACH rule id above, decide {opts}. Return ONLY strict JSON, nothing else:\n"
        '{"verdicts":[{"id":"<rule-id>","verdict":"PASS|FAIL","reason":"<one short line>"}]}\n')


def run_reviewer(cmd, prompt, timeout):
    try:
        p = subprocess.run(cmd, input=prompt, capture_output=True, text=True, timeout=timeout)
    except Exception:
        return None
    if p.returncode != 0:
        return None
    return p.stdout


def extract_verdicts(text):
    """Parse {'verdicts':[{id,verdict,reason}]} out of the judge's text. Returns {id:(VERDICT,reason)} or
    None if nothing parseable (→ fail closed)."""
    if not text:
        return None
    # strip code fences, then scan for the JSON object containing "verdicts"
    t = re.sub(r"```(?:json)?", "", text)
    idx = t.find('"verdicts"')
    if idx == -1:
        return None
    start = t.rfind("{", 0, idx)
    if start == -1:
        return None
    depth, end = 0, -1
    for i in range(start, len(t)):
        if t[i] == "{":
            depth += 1
        elif t[i] == "}":
            depth -= 1
            if depth == 0:
                end = i + 1
                break
    if end == -1:
        return None
    try:
        obj = json.loads(t[start:end])
    except Exception:
        return None
    out = {}
    for v in obj.get("verdicts", []):
        rid = str(v.get("id", "")).lower()
        verdict = str(v.get("verdict", "")).upper()
        if rid and verdict in ("PASS", "FAIL"):
            out[rid] = (verdict, str(v.get("reason", "")))
    return out


def reconcile(v1, v2, rule_ids):
    """Accept a verdict only if the two swapped runs AGREE; otherwise FAIL-closed."""
    out = {}
    for rid in rule_ids:
        a, b = v1.get(rid), v2.get(rid)
        if not a or not b:
            out[rid] = ("FAIL", "reviewer omitted a verdict on one of the swapped runs (fail-closed)")
        elif a[0] == b[0]:
            out[rid] = a
        else:
            out[rid] = ("FAIL", f"inconsistent across position-swap ({a[0]} vs {b[0]}) — fail-closed")
    return out


def review(root, agent):
    """Returns (block_reasons, log_rows). Empty block_reasons = nothing to block on."""
    mode = os.environ.get("GENESIS_REVIEW", "on").lower()
    if mode == "off":
        return [], []
    strict = mode == "strict"
    required = required_for(agent)
    if not required:
        return [], []
    artifacts = produced_artifacts(root)
    if not artifacts:
        return [], []  # nothing authored to judge
    cmd = reviewer_cmd()
    if not cmd:
        if strict:
            return ["Semantic review is strict but no reviewer is configured (set GENESIS_REVIEWER_CMD)."], []
        log([{"ts": now(), "hook": "review", "agent": agent, "reviewer": "(none)", "note": "inactive-no-reviewer"}])
        return [], []

    reviewer_name = os.environ.get("GENESIS_REVIEWER_MODEL") or (cmd[0] if cmd else "reviewer")
    timeout = int(os.environ.get("GENESIS_REVIEW_TIMEOUT", "120"))
    max_rules = int(os.environ.get("GENESIS_REVIEW_MAX", "8"))
    art_text = "\n\n".join(f"### FILE: {f}\n{txt}" for f, txt in artifacts)

    block_reasons, rows = [], []
    for name in required:
        rules = semantic_rules(name)
        if rules is None:
            block_reasons.append(f"Could not read the '{name}' manifest to review it — cannot certify (fail-closed).")
            continue
        if not rules:
            continue
        total = len(rules)
        rules = rules[:max_rules]
        if total > len(rules):
            rows.append({"ts": now(), "hook": "review", "agent": agent, "reviewer": reviewer_name,
                         "note": f"coverage: reviewed {len(rules)}/{total} semantic rules of {name} (cap {max_rules})"})
        rule_ids = [rid for rid, _ in rules]
        r1 = extract_verdicts(run_reviewer(cmd, build_prompt(name, rules, art_text, swap=False), timeout))
        r2 = extract_verdicts(run_reviewer(cmd, build_prompt(name, rules, art_text, swap=True), timeout))
        if r1 is None or r2 is None:  # judge failed/unparseable → fail closed
            block_reasons.append(f"'{name}': the independent reviewer failed or returned unparseable output — "
                                 f"cannot certify semantic application (fail-closed).")
            rows.append({"ts": now(), "hook": "review", "agent": agent, "reviewer": reviewer_name,
                         "note": f"reviewer-error on {name}", "verdict": "fail"})
            continue
        verdicts = reconcile(r1, r2, rule_ids)
        for rid, (verdict, reason) in verdicts.items():
            rows.append({"ts": now(), "hook": "review", "agent": agent, "reviewer": reviewer_name,
                         "id": rid, "verdict": verdict.lower(), "reason": reason})
            if verdict == "FAIL":
                block_reasons.append(f"'{name}#{rid}': reviewer found the artifact does not embody this rule — {reason}")
    return block_reasons, rows


def main():
    try:
        ev = json.load(sys.stdin)
    except Exception:
        ev = {}
    if ev.get("stop_hook_active"):   # loop guard: don't block forever
        sys.exit(0)
    # Agent identity: explicit argv positional > payload `agent_type` > --main-agent fallback (main = sensei).
    argv_root, argv_agent, main_agent = agent_ident.split_args(sys.argv[1:])
    root = argv_root if argv_root is not None else os.getcwd()
    agent = agent_ident.resolve_agent(ev, argv_agent, main_agent)
    try:
        reasons, rows = review(root, agent)
    except Exception as e:
        # a bug in the reviewer must not silently pass work; fail closed with the error surfaced
        reasons, rows = [f"Semantic reviewer raised {type(e).__name__}: {e} — cannot certify (fail-closed)."], []
    if rows:
        log(rows)
    if reasons:
        print(json.dumps({"decision": "block",
                          "reason": "Semantic review found unmet rules:\n- " + "\n- ".join(reasons[:20]) +
                                    "\nAddress these (or GENESIS_REVIEW=off to disable the semantic layer), then stop again."}))
        sys.exit(0)
    sys.exit(0)


if __name__ == "__main__":
    main()
