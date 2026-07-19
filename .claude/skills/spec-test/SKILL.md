---
name: spec-test
description: Run BDD tests and unit tests for a feature or full suite
context: full
allowed-tools: Bash(cd *), Bash(cmd *), Bash(npx *), Bash(curl *), Bash(sleep *), Bash(ls *), Bash(pytest *), Bash(cargo *), Read, Glob, Task
---

# Spec Test Skill

Run BDD tests and unit tests for a specific feature or the full test suite.

## Usage

```
/spec-test <feature-name>    Run tests for a specific feature
/spec-test all               Run the full test suite
/spec-test --tag @smoke      Run tests matching a tag
```

**Examples:**
```
/spec-test chart-filtering
/spec-test all
/spec-test --tag @critical
```

The arguments are provided via `$ARGUMENTS`.

## Workflow

### Phase 1: Parse Arguments

1. **Parse `$ARGUMENTS`**:
   - If a feature name is given, resolve to `test/features/<name>.feature`
   - If `all` or no arguments, run the full suite
   - If `--tag <tag>`, pass as tag filter
   - Verify the feature file exists if a specific one was requested

### Phase 2: Generate Tests from Features

2. **If the project uses playwright-bdd**, run `npx bddgen` from the project root with the appropriate config flag to generate Playwright test files from `.feature` files.

   - This converts Gherkin scenarios to executable Playwright tests
   - Must run before every test execution to pick up feature file changes
   - If bddgen fails, report the error (usually a step definition mismatch)

   If the project uses pytest-bdd, skip this step (pytest discovers features directly).

   If the project is Rust (cucumber-rs), skip this step — there is no code-generation step; cucumber-rs runs `test/features/<name>.feature` directly at runtime via its `harness=false` bins. Instead, verify the step definitions exist (Glob `tests/bdd/*_steps.rs`); if none exist, report the error.

### Phase 3: Start App (if needed)

If the tests require a running application (e.g., end-to-end UI tests):

3. **Stop any running instance** using the project's stop mechanism (e.g., stop script, docker-compose down, kill process).

4. **Start with test config** using the project's start mechanism (e.g., start script with test flag, docker-compose up).

5. **Poll for readiness** (up to 10 attempts, 2 seconds apart). Check the app's health endpoint or base URL for a successful response.

   If healthy after polling, proceed to tests. If not healthy after 10 attempts, report failure and **stop** (do not proceed to test execution).

If tests don't need a running app (e.g., unit-style BDD tests), skip Phase 3.

### Phase 4: Execute Tests

6. **Note the current time** before running tests. This timestamp is used in Phase 6 to filter log entries.

7. **Run the tests** with appropriate arguments:

   **playwright-bdd:**
   ```bash
   npx playwright test --config test/playwright.config.ts --reporter=list,html --grep "<feature-name>"
   ```

   **pytest-bdd:**
   ```bash
   pytest test/features/ -k "<feature-name>" -v
   ```

   **cucumber-rs (Rust):** no `bddgen` step — cucumber-rs runs the `.feature` directly via its `harness=false` bins:
   ```bash
   cargo test --release --test 'bdd_*'
   ```
   For a full regression run, drop the `--test 'bdd_*'` filter and run `cargo test --release`. At the GREEN step, GREEN is stricter than tests alone — `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release` (a non-zero `fmt`/`clippy` exit is not GREEN). This strengthens the gate; it never weakens it.

8. **Capture the output** — pass/fail status, test counts, error details.

### Phase 4.5: Execute Unit Tests

9. **Determine if unit tests exist for this feature:**
   - If a specific feature was requested: check for `test/unit/test_<feature_name>.py` (convert kebab-case to snake_case)
   - If `all` or no arguments: check if any test files exist in `test/unit/`

10. **If unit test files exist, run them:**

    **Specific feature:**
    ```bash
    pytest test/unit/test_<feature_name>.py -v
    ```

    **Full suite:**
    ```bash
    pytest test/unit/ -v
    ```

    **Rust (cargo):** unit tests live in `#[cfg(test)] mod tests` / `tests/*.rs`. Specific module: `cargo test <module>` (single test: `cargo test <module>::<test>`); full suite: `cargo test --release`. A single `cargo test` compiles and runs the lib/unit and integration (`bdd_*`) bins together, so split the counts by bin when reporting.

11. **If no unit test files found**, skip with note: "Unit tests: skipped (no unit test files found)"

12. **Capture unit test output** separately from BDD results.

### Phase 5: Report Results

13. **Present results** to the user in two sections:

**BDD Tests:**

*If all BDD tests pass (GREEN):*
```
BDD Tests: PASSED
- Features: N
- Scenarios: M
- Duration: Xs
```

*If BDD tests fail (RED):*
```
BDD Tests: FAILED (expected for RED step)
- Passed: X/Y scenarios
- Failed: Z scenarios

Failed scenarios:
  - <Scenario name>: <error message>
  - <Scenario name>: <error message>

Screenshots saved for failed tests (check test-results/).
```

**Unit Tests:**

*If all unit tests pass:*
```
Unit Tests: PASSED
- Tests: N passed
- Duration: Xs
```

*If unit tests fail:*
```
Unit Tests: FAILED
- Passed: X/Y tests
- Failed: Z tests

Failed tests:
  - <test name>: <error message>
```

*If skipped:*
```
Unit Tests: skipped (no unit test files found)
```

14. **Contextual guidance based on workflow stage:**
    - If this is the **RED** step (tests expected to fail): "Tests are RED as expected. Enter the TDD loop — pick the first failing unit test, write the minimum code to make it pass, refactor, then move to the next test. Use `pytest test/unit/test_<feature>.py -v` (Rust: `cargo test <module>`) for fast feedback during the loop."
    - If this is the **GREEN** step and BDD tests fail: "Unit tests pass but BDD tests still failing. Return to the TDD loop — the implementation doesn't yet satisfy the acceptance criteria."
    - If this is the **GREEN** step and all pass: "All tests GREEN. Proceed to `/spec-simplify`."
    - If this is the **VERIFY** step (after simplification): Report any failures introduced by simplification.
    - If this is **regression** (full suite): Flag any newly broken tests in either layer.

### Phase 6: Log Validation (optional)

If the project produces agent or application logs, validate behavior during the test run:

15. **Find the latest log file** via Glob in common locations (`logs/`, `test-results/`, project root). If no log files found, print `"Log validation: skipped (no log files found)"` and skip the rest of Phase 6.

16. **Delegate log review to a general-purpose subagent** using the Task tool (`subagent_type: "general-purpose"`). Pass the log path and the Phase 4 start timestamp. Use this prompt verbatim:

    > Read the log file at `<path>` and filter to entries at or after `<timestamp>`. Return exactly this format and nothing else:
    >
    > - Line 1: `entries_reviewed: <N>`
    > - Line 2: `verdict: CLEAN` OR `verdict: ISSUES`
    > - If `ISSUES`: up to 20 bullets of the form `- [<timestamp>] <short issue description>` covering application errors, failed tool calls, empty/missing responses, unexpected stops, and any other anomalies.
    >
    > Do not quote full log lines. Cap total output at 40 lines.

17. **Relay the subagent's verdict block verbatim** as the Phase 6 report. Do not quote raw log content in the main conversation.

    Example output on success:
    ```
    Log validation: CLEAN
    - Log file: <filename>
    - entries_reviewed: N
    - verdict: CLEAN
    ```

    Example output on issues:
    ```
    Log validation: ISSUES FOUND
    - Log file: <filename>
    - entries_reviewed: N
    - verdict: ISSUES
      - [timestamp] <short issue description>
    ```

### Phase 7: Shutdown (if app was started)

18. **Stop the app** so it's not left running with test config.

Report: "App stopped. Use your start script to restart with normal config."

## WORKFLOW COMPLETE

After reporting results, this skill workflow is FINISHED. Return to normal conversation mode.
