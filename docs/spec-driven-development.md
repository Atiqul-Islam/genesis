# Spec-Driven Development

A lightweight, enforceable workflow for building features with plain English specifications, Gherkin acceptance tests, and double-loop TDD — no mocking in BDD tests.

## Philosophy

Specs are the source of truth, not the code. This approach draws from:

- **Thoughtworks' Spec-Driven Development** (2025 Technology Radar) — separating the "what" from the "how" by writing specifications before implementation
- **Uncle Bob's BDD methodology** (Clean Coders Episodes 35-37) — acceptance tests first, make them executable, then write code to make them pass
- **Specification by Example** — concrete examples in specs serve as both documentation and executable tests

The AI twist: Claude Code acts as the "compiler" between plain English and Gherkin, and is constrained by skills and rules to never skip steps.

This methodology is delivered through two modes, both of which enforce the same gates and thresholds:

- **Multi-Agent Mode** — a supervisor orchestrates five persistent specialist agents (spec, dev, verify, review, docs). The supervisor is the only user-facing voice; it audits drafts for hallucination markers and routes by deterministic responsibility lookup. See [`multi-agent-workflow.md`](multi-agent-workflow.md) for details and `/spec-build` to invoke.
- **Manual Mode** — the original slash commands (`/spec-create`, `/spec-compile`, `/spec-test`, `/spec-simplify`, `/spec-crap`, `/docs-update`). Power users drive each step themselves. The sections below describe this mode in detail; multi-agent mode inlines the same behavior.

### References

- [Thoughtworks: Spec-Driven Development (2025)](https://www.thoughtworks.com/en-us/insights/blog/agile-engineering-practices/spec-driven-development-unpacking-2025-new-engineering-practices)
- [Martin Fowler: Understanding SDD — Kiro, spec-kit, and Tessl](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html)
- [SDD Academic Paper (arXiv)](https://arxiv.org/html/2602.00180v1)
- [Uncle Bob: BDD — Clean Coders Episode 35](https://cleancoders.com/episode/clean-code-episode-35)
- [Uncle Bob: The Cycles of TDD](https://blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html)
- [Spec-Driven Development with Claude Code](https://alexop.dev/posts/spec-driven-development-claude-code-in-action/)
- [playwright-bdd Documentation](https://vitalets.github.io/playwright-bdd/)

---

## Double-Loop TDD

This workflow uses Uncle Bob's double-loop pattern: an **outer BDD loop** wraps an **inner unit TDD loop**.

```
OUTER LOOP (BDD — behavior)          INNER LOOP (unit — implementation)
┌─────────────────────────┐          ┌─────────────────────────┐
│ 1. Spec + Compile       │          │ For each unit test:     │
│ 2. RED  — /spec-test    │───────>  │   a. Red   (test fails) │
│    (BDD + stubs fail)   │          │   b. Green (min code)   │
│                         │  <───────│   c. Refactor           │
│ 4. GREEN — /spec-test   │          │   d. Next test          │
│    (BDD + units pass)   │          └─────────────────────────┘
│ 5. Simplify             │
│ 6. Verify               │
│ 7. Regression           │
└─────────────────────────┘
```

- **BDD tests** validate observable behavior — what the user sees. No mocks.
- **Unit tests** validate implementation mechanics — internal correctness. Mocks allowed.
- **Refactoring happens inside the inner loop** (after each green), not as a batch step at the end.
- **`/spec-simplify`** is a final polish after the outer loop passes.

### Uncle Bob's Three Laws of TDD (inner loop)

1. Do not write production code except to make a failing unit test pass
2. Do not write more of a unit test than is sufficient to fail
3. Do not write more production code than is sufficient to pass the currently failing test

---

## The Pipeline

Two test layers in a double loop:

```
test/specs/*.md  -->  Claude "compiles"  -->  test/features/*.feature  -->  BDD runner (outer loop)  -->  Pass/Fail
(plain English)      (skill enforces)        (Gherkin)                    (real app, no mocks)
                           |
                           +--->  test/unit/test_*.py  -->  pytest (inner TDD loop)  -->  Pass/Fail
                                  (stubs from specs)       (mocks allowed)
```

### Layer 1: Plain English Specs (human-authored)

Markdown files in `test/specs/` describe features in plain English with acceptance criteria.

```markdown
# test/specs/user-login.md

## Feature: User Login

Users should be able to log in with their credentials.

### Expected Behavior
- When a user enters valid credentials, they are redirected to the dashboard
- Invalid credentials show an error message without revealing which field is wrong
- After 5 failed attempts, the account is temporarily locked
- Locked accounts show a specific message with unlock instructions

### Acceptance Criteria
- Valid username + password → redirect to dashboard
- Invalid credentials → generic error message
- 5 failed attempts → account locked message
- Locked account login attempt → shows unlock instructions
```

### Layer 2: Gherkin Feature Files (Claude-generated from specs)

Claude reads the spec and produces `.feature` files with traceable links back to the source spec.

```gherkin
# test/features/user-login.feature
# Source: test/specs/user-login.md

Feature: User Login

  Scenario: Successful login with valid credentials
    Given I am on the login page
    When I enter valid credentials
    And I submit the login form
    Then I should be redirected to the dashboard

  Scenario: Failed login shows generic error
    Given I am on the login page
    When I enter invalid credentials
    And I submit the login form
    Then I should see a generic error message

  Scenario: Account locks after repeated failures
    Given I am on the login page
    When I enter invalid credentials 5 times
    Then I should see an account locked message
```

### Layer 3: Test Runner Executes Against Real App (no mocking in BDD)

The BDD framework converts `.feature` files into executable tests and runs them against the live application. Unit tests run separately with `pytest` and may use mocks for isolated verification.

**playwright-bdd** (TypeScript, browser-based):
```bash
npx playwright test --config test/playwright.config.ts
```

**pytest-bdd** (Python, API or browser-based):
```bash
pytest test/features/ -v
```

---

## Toolchain Options

| Component | playwright-bdd | pytest-bdd |
|---|---|---|
| Language | TypeScript | Python |
| Test runner | Playwright | pytest |
| Step definitions | `test/steps/*.ts` | `test/steps/*.py` |
| Best for | Browser UI testing | API testing, Python projects |
| Install | `npm i -D @playwright/test playwright-bdd` | `pip install pytest-bdd` |

### Why playwright-bdd

- Single npm package — not a separate framework
- Runs on the Playwright test runner — keeps parallelism, traces, screenshots, auto-wait
- TypeScript-native step definitions
- Gherkin `.feature` files are the test source
- Tag filtering for running subsets

### playwright-bdd Installation

```bash
npm i -D @playwright/test playwright-bdd @cucumber/cucumber
```

### playwright-bdd Configuration (test/playwright.config.ts)

```typescript
import { defineConfig } from '@playwright/test';
import { defineBddConfig } from 'playwright-bdd';

const testDir = defineBddConfig({
  paths: ['features/**/*.feature'],
  require: ['steps/**/*.ts'],
});

export default defineConfig({
  testDir,
  reporter: [['html', { outputFolder: '../playwright-report' }]],
  outputDir: '../test-results',
  use: {
    baseURL: 'http://localhost:3000',  // Adjust to your app's URL
  },
});
```

### Step Definition Example (playwright-bdd)

```typescript
// test/steps/common/navigation.steps.ts
import { createBdd } from 'playwright-bdd';

const { Given, When, Then } = createBdd();

Given('I am on the login page', async ({ page }) => {
  await page.goto('/login');
  await page.waitForSelector('form');
});

When('I enter valid credentials', async ({ page }) => {
  await page.fill('[name="username"]', 'testuser');
  await page.fill('[name="password"]', 'testpass');
});

When('I submit the login form', async ({ page }) => {
  await page.click('button[type="submit"]');
});

Then('I should be redirected to the dashboard', async ({ page }) => {
  await page.waitForURL('**/dashboard', { timeout: 10000 });
});

Then('I should see a generic error message', async ({ page }) => {
  await page.waitForSelector('.error-message', { timeout: 10000 });
});
```

---

## Directory Structure

```
project-root/
├── test/                               # All test infrastructure
│   ├── specs/                          # Plain English specifications (source of truth)
│   │   ├── user-login.md
│   │   ├── data-export.md
│   │   └── search-filtering.md
│   │
│   ├── features/                       # Gherkin feature files (compiled from specs)
│   │   ├── user-login.feature
│   │   ├── data-export.feature
│   │   └── search-filtering.feature
│   │
│   ├── steps/                          # Step definitions (reusable glue code)
│   │   ├── common/
│   │   │   ├── navigation.steps.ts     # Shared: login, navigate
│   │   │   ├── forms.steps.ts          # Shared: fill, submit
│   │   │   └── assertions.steps.ts     # Shared: see element, see error
│   │   ├── helpers.ts                  # Shared constants (timeouts, URLs)
│   │   ├── login.steps.ts             # Login-specific steps
│   │   └── export.steps.ts            # Export-specific steps
│   │
│   ├── unit/                           # Python unit tests (pytest)
│   │   └── test_user_login.py          # Generated stubs from Implementation Requirements
│   ├── data/                           # Test data (SQL scripts, fixtures, seed files)
│   └── playwright.config.ts            # Playwright + BDD configuration
│
└── package.json
```

### Key Principles

- One `.md` spec per feature, one `.feature` file per spec
- Mirror directory structure between `test/specs/` and `test/features/`
- Step definitions are reusable — most tests reuse the same 10-15 common steps

---

## Claude Code Enforcement

### Skills (Slash Commands)

Claude Code skills enforce the spec-driven workflow:

#### `/spec-create` — Create or update a plain English spec

- Claude helps draft the spec based on the feature request
- Human reviews and approves the spec before proceeding
- Output: `test/specs/<feature-name>.md`

#### `/spec-compile` — Compile spec to Gherkin + unit test stubs

- Reads a spec file from `test/specs/`
- Generates the corresponding `.feature` file in `test/features/`
- Maps acceptance criteria to Gherkin scenarios
- Generates any new step definitions needed in `test/steps/`
- If the spec has Implementation Requirements: generates `pytest` test stubs in `test/unit/test_<feature>.py`
- Adds source traceability comment: `# Source: test/specs/<feature-name>.md`

#### `/spec-test` — Run BDD tests and unit tests

- Executes BDD tests for a specific feature or the full suite
- Executes unit tests from `test/unit/` (if they exist)
- Reports pass/fail for both layers with details on failure
- Includes optional log validation
- Must pass before any commit

#### `/spec-simplify` — Simplify implementation code

- Runs the built-in `/simplify` skill on recently modified implementation files
- Focuses on source code and config files; never touches specs, features, or steps
- Runs after the outer loop passes (GREEN) — a final polish before verification

#### `/spec-crap` — Change-risk gate

Computes the **CRAP** (Change Risk Analyzer and Predictor) score for every function in `src/`:

```
CRAP(f) = CC(f)² × (1 − cov(f)/100)³ + CC(f)
```

- `CC` is cyclomatic complexity (from `radon cc --json`)
- `cov` is unit-test coverage % (from `coverage.py` ≥ 7.6 per-function JSON)
- At 100% coverage, CRAP collapses to CC. At 0% coverage, CRAP ≈ CC² + CC.

CRAP was invented by Alberto Savoia and Bob Evans at Agitar Software in 2007 (shipped as [`crap4j`](https://www.artima.com/weblogs/viewpost.jsp?thread=215899)). Uncle Bob Martin has since popularised it for AI-agent workflows via [`crap4clj`](https://github.com/unclebob/crap4clj), [`crap4java`](https://github.com/unclebob/crap4java), and the Constitution in [`swarm-forge`](https://github.com/unclebob/swarm-forge). The three thresholds below mirror his repos:

| Tier | CRAP | Meaning |
|------|------|---------|
| alert | > 30 | Conventional Savoia/Evans "crappy" line |
| **fail** | **> 8** | Must be addressed before commit (matches `crap4java` exit code) |
| target | ≤ 4 | Refactor goal (matches SwarmForge reviewer prompt) |

- Runs at **step 8 REVIEW**, immediately before `/code-review`, so the reviewer has change-risk data as an input
- Exits non-zero when any function is above the fail line — blocks commit
- Radon counts boolean operators toward CC, so its scores run slightly higher than JaCoCo/PMD equivalents; thresholds assume radon calibration
- Degrades gracefully when `radon` or `coverage.py` aren't installed (prints a skip message, exits 0)

### Official Anthropic Plugins (from `claude-plugins-official` marketplace)

Two official plugins reinforce the workflow without overriding it. They're enabled in `.claude/settings.json` via `enabledPlugins` and install automatically when a user trusts the project folder.

#### `/code-review` — Independent CLAUDE.md compliance audit (plugin: `code-review`)

- Launches 4 parallel agents: 2× CLAUDE.md compliance auditors, 1× bug detector on the diff, 1× git blame/history analyzer
- Scores each finding 0–100 confidence; surfaces only ≥80
- Runs at step 8 REVIEW, after REGRESSION and before commit
- Catches workflow cheating that tests cannot detect (e.g. `.feature` edited to pass, RED skipped, stub-only implementations)

#### `/revise-claude-md` — Session learning capture (plugin: `claude-md-management`)

- End-of-session capture of newly discovered commands, patterns, gotchas, and environment quirks
- Proposes targeted edits to `CLAUDE.md` — not wholesale rewrites
- Companion `claude-md-improver` skill audits CLAUDE.md quality on demand
- Runs at step 9 DOCS when a session surfaced insights worth encoding into the project contract

#### Not included: `hookify`

The `hookify` plugin was considered but excluded from the template because its shipped hooks hardcode `python3` as the interpreter. On Windows, Python installs as `python` by default, so every hook invocation fails with `command not found` — including on every prompt submit. Until the upstream plugin fixes this (or Windows installs start shipping `python3`), hookify breaks cross-platform portability. Add new enforcement rules by editing `.claude/settings.json` directly instead.

### CLAUDE.md Rules (The Real Guardrail)

The project's `CLAUDE.md` constrains Claude Code with mandatory workflow rules:

```markdown
## Development Workflow (MANDATORY)

### Adding Features — Double-Loop TDD
Claude Code MUST follow this process for ALL feature work:

1. SPEC: /spec-create + /spec-compile
2. RED: /spec-test — BDD + unit stubs MUST FAIL
3. TDD LOOP (inner loop — repeat for each unit test):
   a. Red: verify unit test fails
   b. Green: write minimum code to pass
   c. Refactor: clean up, re-verify
   d. Next test
4. GREEN: /spec-test — BDD + units MUST PASS (if BDD fails, back to 3)
5. SIMPLIFY: /spec-simplify
6. VERIFY: /spec-test — must still pass after simplification
7. REGRESSION: /spec-test all
8. REVIEW: /spec-crap (fails at CRAP > 8) → /code-review — independent audit of diff vs. CLAUDE.md (address ≥80 confidence findings)
9. DOCS: /docs-update (if structural changes); /revise-claude-md (if session surfaced new patterns)

### Rules
- NEVER write implementation code without a corresponding spec in test/specs/
- NEVER modify a .feature file to make a failing test pass (that is cheating)
- NEVER skip the RED step — if tests pass before implementation, the tests are wrong
- NEVER delete or weaken existing .feature scenarios
- NEVER modify generated unit test stubs to make them pass — implement the actual logic
- If a spec changes, update the .feature file FIRST, verify it fails, then update the code
- When fixing bugs, add a new scenario to reproduce the bug BEFORE fixing it
```

### Hook Enforcement

Claude Code hooks in `.claude/settings.json` provide deterministic enforcement:

- **Notification hook**: Re-injects workflow rules after context compaction so Claude doesn't lose track of the process
- **PostToolUse hook** (optional): Auto-formats Python files after edits (requires `ruff` or `black`)

---

## Regression Testing

Every spec file produces `.feature` scenarios (and optionally unit test stubs). Every scenario and unit test is executable. The full suite is the regression:

```
test/specs/                     test/features/                      BDD scenarios
  user-login.md           -->     user-login.feature                (3 scenarios)
  data-export.md          -->     data-export.feature               (5 scenarios)
  search-filtering.md     -->     search-filtering.feature          (4 scenarios)
                                                                    -----------
                                                                    12 BDD tests

                                test/unit/                          Unit tests
                          -->     test_user_login.py                (2 tests)
                          -->     test_data_export.py               (3 tests)
                                                                    -----------
                                                                    5 unit tests

                                                                    17 total regression tests
```

Running regression with `/spec-test all` runs both BDD and unit tests.

After regression passes, `/code-review` (from the `code-review` plugin) performs an independent multi-agent audit of the diff against CLAUDE.md rules. This catches process violations and latent bugs that behavioral tests cannot detect — it is the final gate before commit.

Manual regression (not recommended — use `/spec-test` for log validation):

```bash
# Full BDD suite (playwright-bdd)
npx playwright test --config test/playwright.config.ts

# Full unit test suite
pytest test/unit/ -v

# Single feature BDD
npx playwright test --config test/playwright.config.ts --grep "user-login"

# By tag (BDD only — tags don't apply to unit tests)
npx playwright test --config test/playwright.config.ts --tags "@smoke"
```

### Regression Guarantees

- New features add new scenarios (and unit test stubs) — the suite only grows
- Bug fixes add reproducer scenarios — preventing re-introduction
- Spec changes require `.feature` updates first — keeping docs and tests in sync
- The full suite runs in CI on every push — nothing silently breaks
- `/spec-test all` runs both BDD and unit tests in a single command

---

## Test Data and State Management

Since BDD tests run against the real application with no mocking:

### Environment

- Tests run against a dedicated test environment (or local dev with test database)
- Frontend, backend, and database are all real and running
- Only external services outside your control get stubbed (if any)

### State Patterns

- **Database seeding**: Restore a known state before each test suite run
- **API-based setup**: Use the application's own API in `Before` hooks to create test data
- **Unique identifiers**: Generate unique session names per test run to avoid cross-test contamination
- **Cleanup**: `After` hooks clean up created data, or full database reset between suites

### Async/WebSocket Handling

- Use Playwright's `waitForSelector` and `waitForResponse` instead of arbitrary timeouts
- For WebSocket-driven UIs, use `page.waitForEvent('websocket')` to detect connections
- Set generous timeouts for long-running responses (30s+)

---

## Workflow Summary

```
1.  /spec-create    --> test/specs/feature-name.md (human reviews)
        |
2.  /spec-compile   --> test/features/feature-name.feature + step defs
        |               + test/unit/test_feature_name.py (stubs)
        |
3.  /spec-test      --> BDD + unit stubs FAIL (RED)
        |
4.  TDD LOOP:       For each unit test:
        |             a. Red   — verify test fails
        |             b. Green — write minimum code
        |             c. Refactor — clean up, re-verify
        |
5.  /spec-test      --> BDD + unit tests PASS (GREEN)
        |               If BDD fails --> back to step 4
        |
6.  /spec-simplify  --> simplify implementation code
        |
7.  /spec-test      --> still passes after simplification (VERIFY)
        |
8.  /spec-test all  --> ALL tests pass (REGRESSION)
        |
9.  /spec-crap      --> per-function CRAP report (fails if any > 8)
        |
10. /code-review    --> independent multi-agent CLAUDE.md audit (REVIEW)
        |
11. /docs-update    --> update architecture docs (if structural changes)
    /revise-claude-md --> capture session learnings into CLAUDE.md (optional)
```

No steps skipped. No mocking in BDD. No cheating.
