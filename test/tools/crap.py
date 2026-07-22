"""Compute per-function CRAP scores and report offenders.

Reads:
  test-results/radon-cc.json   (radon cc -s --json output)
  test-results/coverage.json   (coverage.py JSON report, coverage.py >= 7.6)

Writes a table to stdout, sorted by CRAP desc. Exits 2 if any function has
CRAP > FAIL_THRESHOLD.

Formula: CRAP(f) = CC(f)^2 * (1 - cov(f)/100)^3 + CC(f)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ALERT_THRESHOLD = 30.0
FAIL_THRESHOLD = 8.0
TARGET_THRESHOLD = 4.0

RADON_JSON = Path("test-results/radon-cc.json")
COVERAGE_JSON = Path("test-results/coverage.json")


def crap(cc: float, cov_pct: float) -> float:
    gap = 1.0 - (cov_pct / 100.0)
    return cc * cc * (gap ** 3) + cc


def normalize(path: str) -> str:
    return path.replace("\\", "/").lstrip("./")


def coverage_key(entry: dict) -> str:
    if entry.get("type") == "method" and entry.get("classname"):
        return f"{entry['classname']}.{entry['name']}"
    return entry["name"]


def load_coverage(path: Path) -> dict[str, dict[str, float]]:
    """Return {normalized_file_path: {function_key: percent_covered}}."""
    if not path.exists():
        return {}
    data = json.loads(path.read_text())
    out: dict[str, dict[str, float]] = {}
    for file_path, file_data in data.get("files", {}).items():
        funcs = {}
        for name, fn in file_data.get("functions", {}).items():
            funcs[name] = fn.get("summary", {}).get("percent_covered", 0.0)
        out[normalize(file_path)] = funcs
    return out


def collect_rows(radon_data: dict, coverage_map: dict) -> list[dict]:
    rows = []
    for file_path, entries in radon_data.items():
        norm = normalize(file_path)
        cov_funcs = coverage_map.get(norm, {})
        for entry in entries:
            if entry.get("type") == "class":
                continue
            key = coverage_key(entry)
            cov_pct = cov_funcs.get(key, 0.0)
            cc = entry.get("complexity", 0)
            rows.append({
                "file": norm,
                "name": key,
                "cc": cc,
                "cov": cov_pct,
                "crap": crap(cc, cov_pct),
            })
    rows.sort(key=lambda r: r["crap"], reverse=True)
    return rows


def print_report(rows: list[dict]) -> None:
    if not rows:
        print("CRAP Report: no functions analyzed")
        return
    print(f"{'CRAP':>7}  {'CC':>4}  {'Cov%':>6}  {'Function':<40}  File")
    print("-" * 100)
    for r in rows:
        print(f"{r['crap']:>7.1f}  {r['cc']:>4}  {r['cov']:>6.1f}  "
              f"{r['name']:<40}  {r['file']}")


def main() -> int:
    if not RADON_JSON.exists():
        print(f"CRAP check skipped — {RADON_JSON} not found")
        return 0
    radon_data = json.loads(RADON_JSON.read_text())
    coverage_map = load_coverage(COVERAGE_JSON)
    rows = collect_rows(radon_data, coverage_map)
    print_report(rows)

    above_target = sum(1 for r in rows if r["crap"] > TARGET_THRESHOLD)
    above_fail = sum(1 for r in rows if r["crap"] > FAIL_THRESHOLD)
    above_alert = sum(1 for r in rows if r["crap"] > ALERT_THRESHOLD)
    print()
    print(f"Summary: {len(rows)} functions analyzed")
    print(f"  above target (>{TARGET_THRESHOLD}): {above_target}")
    print(f"  above fail   (>{FAIL_THRESHOLD}): {above_fail}")
    print(f"  above alert  (>{ALERT_THRESHOLD}): {above_alert}")
    return 2 if above_fail else 0


if __name__ == "__main__":
    sys.exit(main())
