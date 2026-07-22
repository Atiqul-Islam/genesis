#!/usr/bin/env python3
"""Unit tests for review.py — the independent semantic reviewer (§22).

Drives review.py as a subprocess with a MOCK judge (GENESIS_REVIEWER_CMD), so every control path is proven
deterministically without a live model: pass, fail, position-swap inconsistency, reviewer error, unparseable
output, disabled, no-reviewer (lenient + strict), and the loop guard. No network.

Run:  python3 hooks/test_review.py
"""
import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REVIEW = os.path.join(HERE, "review.py")
# review.py now writes runtime logs under the PROJECT's .genesis/ (cwd-derived, item C), so the log lands in
# the workspace we run it from — not hook-dir-relative. main() computes it per-workspace after make_ws().

MOCK = r'''
import sys, os, re, json
mode = os.environ.get("GENESIS_MOCK_MODE", "pass")
prompt = sys.stdin.read()
if mode == "error":
    sys.exit(1)
if mode == "garbage":
    print("Looks fine to me. (no JSON)"); sys.exit(0)
m = re.search(r"RULES \(criteria to check\):\n(.*?)\n\nARTIFACTS", prompt, re.S)
ids = re.findall(r"^- (\S+):", m.group(1) if m else "", re.M)
def verdict():
    if mode == "fail":
        return "FAIL"
    if mode == "inconsistent":                 # flip based on the swapped option order → never agrees
        return "PASS" if "PASS or FAIL" in prompt else "FAIL"
    return "PASS"
print(json.dumps({"verdicts": [{"id": i, "verdict": verdict(), "reason": "mock"} for i in ids]}))
'''


def make_ws():
    root = tempfile.mkdtemp(prefix="genesis-rev-")
    d = os.path.join(root, "release-manager"); os.makedirs(d)
    with open(os.path.join(d, "CLAUDE.md"), "w", encoding="utf-8") as fh:
        fh.write("# Ferry — Release Manager\n## Mission\nShip correct releases.\n## Boundaries\nNever push if CI is red.\n")
    return root


def run(root, agent, mock_path, mode=None, review=None, no_reviewer=False, stop_active=False):
    env = os.environ.copy()
    env.pop("GENESIS_REVIEWER_CMD", None)
    if no_reviewer:
        env["PATH"] = ""                                   # hide `claude` so reviewer_cmd() → None
    else:
        env["GENESIS_REVIEWER_CMD"] = f"{sys.executable} {mock_path}"
    if mode:
        env["GENESIS_MOCK_MODE"] = mode
    if review:
        env["GENESIS_REVIEW"] = review
    payload = {"transcript_path": "", "stop_hook_active": stop_active}
    # cwd=root so review.py resolves its runtime .genesis/ inside THIS workspace (project-derived path, item C).
    p = subprocess.run([sys.executable, REVIEW, root, agent], input=json.dumps(payload),
                       capture_output=True, text=True, timeout=60, env=env, cwd=root)
    out = p.stdout.strip()
    if not out:
        return False, ""
    try:
        d = json.loads(out)
    except Exception:
        return False, out
    return d.get("decision") == "block", d.get("reason", "")


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0; failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    mf = tempfile.NamedTemporaryFile("w", suffix="_mock.py", delete=False)
    mf.write(MOCK); mf.close(); mock = mf.name
    root = make_ws()
    review_log = os.path.join(root, ".genesis", "review.log")   # cwd-derived project runtime path (item C)
    if os.path.isfile(review_log):
        os.remove(review_log)

    # 1. all PASS → allow
    blocked, _ = run(root, "method", mock, mode="pass")
    check("all-PASS judge allows the stop", not blocked)

    # 2. all FAIL → block, naming a real rule
    blocked, reason = run(root, "method", mock, mode="fail")
    check("a reviewer FAIL blocks", blocked and "does not embody" in reason and "#" in reason)

    # 3. inconsistent across position-swap → block (fail-closed)
    blocked, reason = run(root, "method", mock, mode="inconsistent")
    check("position-swap inconsistency blocks (fail-closed)", blocked and ("position-swap" in reason or "inconsistent" in reason))

    # 4. reviewer errors (exit 1) → block (fail-closed)
    blocked, reason = run(root, "method", mock, mode="error")
    check("reviewer error blocks (fail-closed)", blocked and "cannot certify" in reason.lower())

    # 5. unparseable output → block (fail-closed)
    blocked, reason = run(root, "method", mock, mode="garbage")
    check("unparseable reviewer output blocks (fail-closed)", blocked and "unparseable" in reason.lower())

    # 6. GENESIS_REVIEW=off → allow even though the judge would FAIL everything
    blocked, _ = run(root, "method", mock, mode="fail", review="off")
    check("GENESIS_REVIEW=off disables the semantic layer", not blocked)

    # 7. no reviewer available, default mode → allow (skip, logged)
    blocked, _ = run(root, "method", mock, no_reviewer=True)
    check("no reviewer + lenient → allow (skip)", not blocked)

    # 8. no reviewer available, strict → block
    blocked, reason = run(root, "method", mock, no_reviewer=True, review="strict")
    check("no reviewer + strict → block", blocked and "no reviewer" in reason.lower())

    # 9. stop_hook_active → allow (loop guard) even with a failing judge
    blocked, _ = run(root, "method", mock, mode="fail", stop_active=True)
    check("stop_hook_active short-circuits to allow", not blocked)

    # 10. review.log captured verdicts from the runs above
    logged = os.path.isfile(review_log) and any('"verdict"' in ln for ln in open(review_log, encoding="utf-8"))
    check("review.log captured verdicts (under the project .genesis/)", logged)

    # 11. an agent with no required expertise → allow (nothing to review)
    blocked, _ = run(root, "", mock, mode="fail")
    check("no-required-expertise agent allows", not blocked)

    if os.path.isfile(review_log):
        os.remove(review_log)
    os.remove(mock)
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
