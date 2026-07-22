#!/usr/bin/env python3
"""ADHERENCE harness (§20) — measure expertise application over time; stop ASSUMING it (ea-10).

The gate/validate hooks append one structured, timestamped record per decision to .genesis/hook-decisions.log
(and the LLM reviewer, review.py/§22, appends its verdicts to .genesis/review.log). This harness reads those
and reports, per agent and per rule:

  • turn-completion: how often an agent could finish vs. was blocked by the validator (block-rate);
  • deterministic violations (bucket-a, ea-10): gate denies by rule + validator block reasons, categorized;
  • per-rule application rate: how often each governing rule-id was actually CITED with evidence on a
    clean finish — the deterministic proxy for "this rule is being applied", per expertise;
  • semantic review (bucket-b, ea-10): the independent LLM-reviewer's pass/block rate, if review.log exists;
  • a multi-turn-stability slice: block-rate by day, to surface drift (adherence decaying over time).

It is a *standing* report (run it whenever), not a one-off — which is the ea-10 requirement. It measures;
it does not enforce. Deterministic checks can only prove the mechanical subset + citation integrity; the
semantic remainder is the reviewer's rate — reported separately and never conflated (ea-8/ea-11).

Usage:
  python3 hooks/adherence.py [--log <path>] [--review <path>] [--agent <name>] [--json]
"""
import argparse, collections, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import agent_ident  # noqa: E402  (sibling helper; travels with hooks/ in every install)

HOOK_DIR = os.path.dirname(os.path.abspath(__file__))
# The adherence report reads the SAME writable runtime logs the hooks write — under the PROJECT's .genesis/
# (cwd-derived). Run it from the repo root; override with --log/--review for a bootstrapped .genesis elsewhere.
_RUNTIME = agent_ident.runtime_dir()
DEFAULT_LOG = os.path.join(_RUNTIME, "hook-decisions.log")
DEFAULT_REVIEW = os.path.join(_RUNTIME, "review.log")

# Map a validator block-reason string to a category (first match wins).
REASON_CATS = [
    ("fabricated-rule-id", ("not in the manifest",)),
    ("below-floor-coverage", ("cite at least",)),
    ("fabricated-evidence", ("does not appear", "no such produced file", "points to")),
    ("artifact-violation", ('"chain-of-thought"', "credential value", "lines (>")),
    ("missing-declaration", ("did not credibly declare",)),
    ("unreadable", ("could not read",)),
]


def categorize(reason):
    low = reason.lower()
    for cat, needles in REASON_CATS:
        if any(n.lower() in low for n in needles):
            return cat
    return "other"


def read_jsonl(path):
    if not path or not os.path.isfile(path):
        return []
    out = []
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except Exception:
            continue
    return out


def aggregate(records, review, only_agent=None):
    a = {
        "window": {"first": None, "last": None},
        "counts": {"total": len(records)},
        "validate": collections.defaultdict(lambda: {"allow": 0, "block": 0}),   # per agent
        "gate_denies": collections.Counter(),                                    # per rule
        "block_reasons": collections.Counter(),                                  # per category
        "cited": collections.defaultdict(collections.Counter),                   # expertise -> rule-id -> n
        "by_day": collections.defaultdict(lambda: {"allow": 0, "block": 0}),     # validate stops per day
        "review": {"pass": 0, "block": 0, "by_reviewer": collections.defaultdict(lambda: {"pass": 0, "block": 0})},
    }
    for r in records:
        ts = r.get("ts")
        if ts:
            a["window"]["first"] = min(a["window"]["first"] or ts, ts)
            a["window"]["last"] = max(a["window"]["last"] or ts, ts)
        hook = r.get("hook")
        if hook == "validate":
            agent = r.get("agent") or "(unnamed)"
            if only_agent and agent != only_agent:
                continue
            dec = "block" if r.get("decision") == "block" else "allow"
            a["validate"][agent][dec] += 1
            if ts:
                a["by_day"][ts[:10]][dec] += 1
            if dec == "block":
                for reason in r.get("reasons", []):
                    a["block_reasons"][categorize(reason)] += 1
            else:
                # a clean finish → the citations it declared are genuine applications
                for name, ids in (r.get("cited") or {}).items():
                    for rid in ids:
                        a["cited"][name][rid] += 1
        elif hook == "gate":
            if r.get("decision") == "deny":
                a["gate_denies"][r.get("rule") or "(unknown)"] += 1
    for r in review:
        if only_agent and r.get("agent") not in (None, only_agent):
            continue
        v = "block" if r.get("decision") == "block" or r.get("verdict") in ("block", "fail") else "pass"
        a["review"][v] += 1
        a["review"]["by_reviewer"][r.get("reviewer") or "(reviewer)"][v] += 1
    return a


def rate(d):
    tot = d["allow"] + d["block"]
    return (d["block"] / tot) if tot else 0.0, tot


def render(a):
    L = []
    w = a["window"]
    L.append("Genesis expertise-adherence report")
    L.append(f"  window: {w['first'] or '—'} → {w['last'] or '—'}   records: {a['counts']['total']}")

    L.append("\nTurn completion (validate Stop hook) — block-rate = how often finishing was refused:")
    if not a["validate"]:
        L.append("  (no validate records yet)")
    for agent in sorted(a["validate"]):
        br, tot = rate(a["validate"][agent])
        L.append(f"  {agent:<16} stops={tot:<4} blocked={a['validate'][agent]['block']:<4} "
                 f"clean={a['validate'][agent]['allow']:<4} block-rate={br:5.1%}")

    L.append("\nDeterministic violations (bucket-a):")
    if a["gate_denies"]:
        L.append("  gate PreToolUse denies by rule:")
        for rule, n in a["gate_denies"].most_common():
            L.append(f"    {rule:<16} {n}")
    if a["block_reasons"]:
        L.append("  validate Stop block reasons by category:")
        for cat, n in a["block_reasons"].most_common():
            L.append(f"    {cat:<22} {n}")
    if not a["gate_denies"] and not a["block_reasons"]:
        L.append("  (none recorded)")

    L.append("\nPer-rule application rate (cited-with-evidence on a clean finish), per expertise:")
    if not a["cited"]:
        L.append("  (no clean finishes with citations yet)")
    for name in sorted(a["cited"]):
        items = a["cited"][name].most_common()
        shown = ", ".join(f"{rid}×{n}" for rid, n in items[:12])
        L.append(f"  {name}: {len(items)} distinct rules cited — {shown}" + (" …" if len(items) > 12 else ""))

    rv = a["review"]
    if rv["pass"] or rv["block"]:
        tot = rv["pass"] + rv["block"]
        L.append(f"\nSemantic review (bucket-b, independent LLM-reviewer): "
                 f"{rv['pass']}/{tot} passed, block-rate={rv['block']/tot:5.1%}")
        for who, d in sorted(rv["by_reviewer"].items()):
            t = d["pass"] + d["block"]
            L.append(f"  {who:<16} pass={d['pass']} block={d['block']} block-rate={(d['block']/t if t else 0):5.1%}")
    else:
        L.append("\nSemantic review (bucket-b): no review.log yet — run the LLM-reviewer (§22) to populate.")

    L.append("\nStability slice — validate block-rate by day (watch for an upward drift):")
    if not a["by_day"]:
        L.append("  (no dated records)")
    for day in sorted(a["by_day"]):
        br, tot = rate(a["by_day"][day])
        bar = "█" * int(round(br * 20))
        L.append(f"  {day}  n={tot:<4} block-rate={br:5.1%} {bar}")
    return "\n".join(L)


def to_plain(a):
    """Make the defaultdicts/Counters JSON-serializable."""
    return {
        "window": a["window"], "counts": a["counts"],
        "validate": {k: dict(v) for k, v in a["validate"].items()},
        "gate_denies": dict(a["gate_denies"]),
        "block_reasons": dict(a["block_reasons"]),
        "cited": {k: dict(v) for k, v in a["cited"].items()},
        "by_day": {k: dict(v) for k, v in a["by_day"].items()},
        "review": {"pass": a["review"]["pass"], "block": a["review"]["block"],
                   "by_reviewer": {k: dict(v) for k, v in a["review"]["by_reviewer"].items()}},
    }


def main():
    ap = argparse.ArgumentParser(description="Genesis expertise-adherence report (ea-10).")
    ap.add_argument("--log", default=DEFAULT_LOG)
    ap.add_argument("--review", default=DEFAULT_REVIEW)
    ap.add_argument("--agent", default=None, help="filter to one agent")
    ap.add_argument("--json", action="store_true", help="emit the aggregated data as JSON")
    args = ap.parse_args()

    records = read_jsonl(args.log)
    review = read_jsonl(args.review)
    if not records and not review:
        print(f"No adherence records found at {args.log}.\n"
              "Run some agent turns first — the gate/validate hooks populate it.")
        return
    a = aggregate(records, review, args.agent)
    print(json.dumps(to_plain(a), indent=2) if args.json else render(a))


if __name__ == "__main__":
    main()
