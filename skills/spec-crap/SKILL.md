---
name: spec-crap
description: Compute the CRAP (Change Risk Analyzer and Predictor) score per function, flag functions above threshold
context: full
allowed-tools: Bash(cargo *), Bash(rust-code-analysis-cli *), Bash(cargo-llvm-cov *), Bash(python *), Bash(ls *), Read, Glob
---

# Spec CRAP Skill

Compute the **CRAP** score for every function in `src/` and flag high-risk functions before `/code-review`.

CRAP combines cyclomatic complexity with unit-test coverage into a single change-risk score per function:

```
CRAP(f) = CC(f)² × (1 − cov(f)/100)³ + CC(f)
```

- At 100% coverage, CRAP collapses to CC.
- At 0% coverage, CRAP ≈ CC² + CC.

This skill runs as the first half of step 8 REVIEW. It produces the report; `/code-review` consumes it.

## Usage

```
/spec-crap              Full report over all of src/
/spec-crap --changed    Limit to files changed vs. the merge base
```

The argument is provided via `$ARGUMENTS`.

## Thresholds

Three tiers, layered from Uncle Bob's own repos:

| Tier | CRAP | Meaning |
|------|------|---------|
| alert | > 30 | Function is crappy by the conventional Savoia/Evans line |
| **fail** | **> 8** | Must be addressed before commit (matches `crap4java`) |
| target | ≤ 4 | Goal during refactor (matches SwarmForge reviewer) |

Exit code 2 is returned when any function has CRAP > 8.

`rust-code-analysis` computes cyclomatic complexity via tree-sitter; its count is close but **not bit-identical** to radon's, on which these threshold numbers were originally calibrated. Keep the `> 8` fail line exactly as-is — but re-validate its absolute feel once on the real Genesis crate before trusting the numbers in absolute terms. The threshold itself never moves; only your confidence in its calibration is what you check.

## Workflow

### Phase 1: Parse Arguments

1. **Parse `$ARGUMENTS`**:
   - `--changed` → scope to files changed vs. `master` (fallback to `main` if `master` is missing).
   - Any other value or empty → full `src/` scope.

### Phase 2: Preflight

2. **Check tools are installed** — run `rust-code-analysis-cli --version` and `cargo llvm-cov --version`. If either is missing, print:

   ```
   CRAP check skipped — <tool> not installed.
   Install: cargo install rust-code-analysis-cli cargo-llvm-cov
   ```

   Exit 0 and end the workflow. CRAP is advisory when tooling is absent.

3. **Check src/ exists.** If not, print `"CRAP check skipped — no src/ directory"` and exit 0.

### Phase 3: Produce Coverage JSON

4. **Reuse existing coverage data if fresh.** If `test-results/llvm-cov.json` exists and is less than 10 minutes old, skip the coverage run.

5. **Otherwise run coverage**:

   ```bash
   cargo llvm-cov --json --release --output-path test-results/llvm-cov.json
   ```

   If there are no tests yet, coverage comes back empty. Treat that as "0% coverage across all functions" — still run Phase 4 so complexity alone is surfaced.

### Phase 4: Produce Cyclomatic Complexity JSON

6. **Run rust-code-analysis over the scope from Phase 1**:

   ```bash
   mkdir -p test-results/rca && rust-code-analysis-cli -m -p src/ -O json -o test-results/rca/
   ```

   Where the path after `-p` is `src/` for a full report, or the changed files/dirs for `--changed`. Note: the `-o` output directory **MUST pre-exist** (hence the `mkdir -p`), and per-file results land as `*.rs.json` under it — the extension is appended and the input tree mirrored.

### Phase 5: Compute and Report

7. **Invoke the adapter**:

   ```bash
   node test/tools/rust_crap_adapter.mjs
   ```

   The adapter converts the rust-code-analysis dump (`test-results/rca/*.rs.json`) and the coverage JSON (`test-results/llvm-cov.json`) into the `test-results/radon-cc.json` + `test-results/coverage.json` that `crap.py` consumes, then calls the **unchanged** `test/tools/crap.py` itself. `crap.py` reads both JSON files, joins per-function, computes CRAP, prints a table sorted descending by score plus a summary line, and propagates its exit code (including exit 2).

8. **Propagate the script's exit code**. Exit 2 means at least one function has CRAP > 8 and the workflow should stop here.

### Phase 6: Report Results

9. **Present results** to the user:

   **If all functions pass (exit 0):**
   ```
   CRAP Report: PASSED
   - Functions analyzed: N
   - Max CRAP: X (function: <name> in <file>)
   - Above target (>4): K
   - Above alert (>30): 0
   - Above fail (>8): 0

   Ready for /code-review.
   ```

   **If any function fails (exit 2):**
   ```
   CRAP Report: FAILED
   - Functions above fail line (>8): M
   - Functions above alert line (>30): N

   Top offenders:
     <file>:<name> — CC=<cc>, cov=<cov>%, CRAP=<score>
     ...

   Fix by lowering cyclomatic complexity or raising unit-test coverage for each offender, then re-run /spec-crap.
   ```

## Rules

- NEVER lower a threshold to make the report pass. Fix the code.
- NEVER delete or weaken unit tests to simplify coverage attribution.
- Coverage and rust-code-analysis data must be from the current state of `src/` — never carry forward stale JSON across implementation changes. The 10-minute reuse window exists only for back-to-back runs.

## WORKFLOW COMPLETE

After reporting results, this skill workflow is FINISHED. Return to normal conversation mode. If exit code was 0, the next step is `/code-review`.
