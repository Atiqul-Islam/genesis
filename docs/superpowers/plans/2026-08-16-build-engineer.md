# /genesis:build-engineer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `/genesis:build-engineer` slash command that reproducibly builds a single "complete engineer" agent for any target folder, with two modes — deep-read (real repo) and grill (empty folder) — from one fixed recipe.

**Architecture:** Approach A. A thin `commands/build-engineer.md` invokes the `sensei` agent, which follows a new `build-engineer` skill (the mechanizable recipe). It reuses the existing `research-expertise` skill, Method authoring, and the `genesis-cli` `bootstrap`/`assemble`/`promote` subcommands. Two new plugin skills (`build-engineer`, `grill`) and five new generic expertise modules ship with the plugin.

**Tech Stack:** Markdown skills/commands (Claude Code plugin), JSON rule-manifests, Node.js (`node:assert`+`node:fs`) test scripts. No new Rust/CLI. Spec: `test/specs/build-engineer.md`.

## Global Constraints

- Persona/behavior/CLAUDE.md files ≤ 200 lines each (house rule).
- Never write the banned reasoning-trace phrase named in the house rules; use "structured reasoning" / "step-by-step reasoning".
- Never write a credential value; reference it as "credential present at <path>".
- Expertise manifests match the existing store schema exactly: top-level `{expertise, source, note, schema, rules[], sections_accounted}`; each rule `{id, section, text, type: checkable|judgment|principle}` with `predicate` (checkable) or `reviewer_criterion` (judgment); ids unique; `sections_accounted` covers every guide section.
- Generic modules are repo-agnostic (no Genesis-only APIs). Names: `git-mastery`, `engineering-leadership`, `test-driven-development`, `spec-driven-development`, `system-operation-maintenance`.
- The built engineer's `tools` never include `Grep` (capability removal enforces no-grep).
- TDD: every task writes the failing test first, sees it fail, implements, sees it pass, commits.

---

## File Structure (decomposition)

- `expertise/<name>.md` (×5) — generic guides. One responsibility: the durable knowledge for one topic.
- `expertise/manifests/<name>.json` (×5) — the enforceable rule index for each guide.
- `expertise/manifests/tests/_manifest_check.mjs` — ONE shared validator (DRY) that validates any `<name>` guide+manifest pair.
- `skills/grill/SKILL.md` — the exhaustive onboarding-interview skill (shipped into built engineers).
- `skills/build-engineer/SKILL.md` — the mechanizable build recipe Sensei follows.
- `commands/build-engineer.md` — the slash-command entry point.
- `test/tools/build-engineer/skill_structure.test.mjs` — structural tests for the two skills + command.
- `test/tools/build-engineer/fixtures/` — a small real-repo fixture and an empty-folder fixture.
- `test/tools/build-engineer/integration.test.mjs` — mode-detection + no-drift/idempotence checks.

---

## Phase 1 — Generic expertise modules

### Task 1: Shared manifest validator (DRY foundation)

**Files:**
- Create: `expertise/manifests/tests/_manifest_check.mjs`
- Test: itself (self-checks against an existing shipped module, e.g. `expertise-application`).

**Interfaces:**
- Produces: a CLI `node _manifest_check.mjs <expertise-name> [<expertise-root>]` that exits non-zero on any failure. Later module tasks call it.

- [ ] **Step 1: Write the validator (it IS the test harness)**

```js
// expertise/manifests/tests/_manifest_check.mjs
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const name = process.argv[2];
const root = process.argv[3] || path.resolve(path.dirname(new URL(import.meta.url).pathname), '../..');
if (!name) { console.error('usage: node _manifest_check.mjs <name> [root]'); process.exit(2); }

const guidePath = path.join(root, `${name}.md`);
const manPath = path.join(root, 'manifests', `${name}.json`);
assert.ok(fs.existsSync(guidePath), `guide missing: ${guidePath}`);
assert.ok(fs.existsSync(manPath), `manifest missing: ${manPath}`);

const guide = fs.readFileSync(guidePath, 'utf8');
const m = JSON.parse(fs.readFileSync(manPath, 'utf8'));
assert.equal(m.expertise, name, 'expertise field must equal name');
assert.equal(m.source, `expertise/${name}.md`, 'source must point at the guide');
assert.ok(Array.isArray(m.rules) && m.rules.length > 0, 'rules non-empty');
assert.ok(m.sections_accounted && typeof m.sections_accounted === 'object', 'sections_accounted object');

const ids = new Set();
for (const r of m.rules) {
  for (const f of ['id', 'section', 'text', 'type']) assert.ok(r[f] && String(r[f]).length, `rule missing ${f}`);
  assert.ok(!ids.has(r.id), `duplicate id ${r.id}`); ids.add(r.id);
  assert.ok(['checkable', 'judgment', 'principle'].includes(r.type), `bad type ${r.type}`);
  if (r.type === 'checkable') assert.ok(r.predicate && r.predicate.kind && r.predicate.spec, `${r.id} needs predicate{kind,spec}`);
  if (r.type === 'judgment') assert.ok(r.reviewer_criterion && r.reviewer_criterion.length, `${r.id} needs reviewer_criterion`);
  if (r.type === 'principle') assert.ok(!r.predicate && !r.reviewer_criterion, `${r.id} principle has neither`);
}
// forbidden phrase (write the check without writing the phrase itself)
const banned = ['chain', 'of', 'thought'].join('-');
assert.ok(!guide.includes(banned) && !JSON.stringify(m).includes(banned), 'banned reasoning-trace phrase present');

// every "## " / "§" guide header appears as a sections_accounted key (report gaps)
const headers = [...guide.matchAll(/^##+\s+(.+)$/gm)].map(x => x[1].trim());
const keys = Object.keys(m.sections_accounted);
const missing = headers.filter(h => !keys.some(k => k.includes(h) || h.includes(k.replace(/^§?\s*/, ''))));
assert.equal(missing.length, 0, `sections_accounted missing headers: ${missing.join(' | ')}`);

// faithfulness: each rule id appears in sections_accounted values (nothing dropped)
const acc = JSON.stringify(m.sections_accounted);
for (const r of m.rules) assert.ok(acc.includes(r.id), `${r.id} not referenced in sections_accounted`);

console.log(`OK ${name}: ${m.rules.length} rules, ${headers.length} headers accounted.`);
```

- [ ] **Step 2: Run it against an existing shipped module to prove it passes on good input**

Run: `node expertise/manifests/tests/_manifest_check.mjs expertise-application expertise`
Expected: `OK expertise-application: ...` and exit 0.

- [ ] **Step 3: Commit**

```bash
git add expertise/manifests/tests/_manifest_check.mjs
git commit -m "test(build-engineer): shared expertise-manifest validator"
```

### Tasks 2–6: Author the five generic modules (one task each)

For EACH `<name>` in this list, do the full research-and-author cycle below. Modules and their primary research anchors:

- **Task 2 `git-mastery`** — the full git model + workflows: objects/refs/HEAD, branching, merge vs rebase, interactive rebase, worktrees, bisect, reflog recovery, cherry-pick, stash, submodules, hooks, `.gitattributes`, signed commits, bisect, clean history/hygiene. Primary: the official Git book (git-scm.com/book) + `git help` pages.
- **Task 3 `engineering-leadership`** — senior-developer + project-management practice: work decomposition/estimation, risk, code review discipline, definition-of-done, changelog/semver ownership, incident basics, tech-debt management, mentoring/communication. Primary: reputable engineering-practice sources; label VERIFIED/INFERRED.
- **Task 4 `test-driven-development`** — generic (language-agnostic) red-green-refactor, the three laws, test doubles, coverage as a tool not a target, property/snapshot testing, determinism. Primary: Beck TDD + xUnit patterns.
- **Task 5 `spec-driven-development`** — generic plain-English spec → executable/BDD (Gherkin) → tests; specs as the human-review surface; keeping spec↔code↔tests in sync. Primary: Cucumber/Gherkin docs + Dan North BDD + Gojko Adzic specification-by-example.
- **Task 6 `system-operation-maintenance`** — generic ops: semver, changelog discipline, CI gating, release/rollback, artifact integrity, backups, safe irreversible-action confirmation. Primary: semver.org, Keep a Changelog, GitHub Actions docs.

**Files (per module):**
- Create: `expertise/<name>.md`, `expertise/manifests/<name>.json`
- Test: reuse `expertise/manifests/tests/_manifest_check.mjs <name> expertise`

**Interfaces:**
- Produces: a shipped module the `build-engineer` recipe lists in `required_expertise`.

- [ ] **Step 1: Deep-research the topic from the primary sources above; capture VERIFIED/INFERRED labels.** (Dispatch parallel researcher subagents per the `research-expertise` method; each reads sources fully.)
- [ ] **Step 2: Write the failing test invocation**

Run: `node expertise/manifests/tests/_manifest_check.mjs <name> expertise`
Expected: FAIL — guide/manifest missing.

- [ ] **Step 3: Author `expertise/<name>.md`** — a thorough, sectioned, labelled guide (mirror the rigor and structure of an existing shipped guide such as `expertise/memory-management.md`). Repo-agnostic. ≤ house limits do not apply to guides (they may be long).
- [ ] **Step 4: Author `expertise/manifests/<name>.json`** — matching the schema in Global Constraints; `id` prefix `<abbrev>-` (e.g. `git-`, `lead-`, `tdd-`, `sdd-`, `som-`); `sections_accounted` covers every guide header.
- [ ] **Step 5: Run the validator to green**

Run: `node expertise/manifests/tests/_manifest_check.mjs <name> expertise`
Expected: `OK <name>: ...`, exit 0. Fix until green.

- [ ] **Step 6: Commit**

```bash
git add expertise/<name>.md expertise/manifests/<name>.json
git commit -m "feat(expertise): generic <name> module for build-engineer recipe"
```

---

## Phase 2 — The `grill` skill

### Task 7: `skills/grill/SKILL.md`

**Files:**
- Create: `skills/grill/SKILL.md`
- Test: `test/tools/build-engineer/skill_structure.test.mjs` (grill assertions)

**Interfaces:**
- Produces: a skill named `grill` the built engineer carries and runs for empty-folder onboarding.

- [ ] **Step 1: Write the failing structural test** (see Task 10 for the full test file; add the grill block first and run it — it fails because the file is absent).

Run: `node test/tools/build-engineer/skill_structure.test.mjs`
Expected: FAIL — `skills/grill/SKILL.md` missing.

- [ ] **Step 2: Author `skills/grill/SKILL.md`.** Front-matter `name: grill`, `description:` (when to use: exhaustive onboarding when there is no code to read). Body MUST cover, each as a checkable heading/line:
  - Purpose: gain complete project expertise by interviewing the user when deep-read is impossible.
  - Topics to cover in full: goals & success criteria; intended stack + exact versions; architecture & components; conventions & coding standards; testing/CI approach; deployment/ops; done-criteria; escalation triggers.
  - One question at a time; never speculate; do not finish until every topic is answered ("ensure it gets all questions answered").
  - Capture: store answers to memory under the engineer's `agent_id`; author a project-knowledge module from them.
  - Output: a completeness checklist the engineer marks off before declaring onboarding done.
- [ ] **Step 3: Run the structural test to green.**

Run: `node test/tools/build-engineer/skill_structure.test.mjs`
Expected: grill assertions PASS.

- [ ] **Step 4: Commit**

```bash
git add skills/grill/SKILL.md test/tools/build-engineer/skill_structure.test.mjs
git commit -m "feat(skill): grill onboarding-interview skill"
```

---

## Phase 3 — The `build-engineer` skill

### Task 8: `skills/build-engineer/SKILL.md`

**Files:**
- Create: `skills/build-engineer/SKILL.md`
- Test: `test/tools/build-engineer/skill_structure.test.mjs` (build-engineer assertions)

**Interfaces:**
- Consumes: `research-expertise`, Method, `genesis-cli bootstrap/assemble/promote`, the Phase-1 modules, the `grill` skill.
- Produces: the recipe Sensei follows when `commands/build-engineer.md` runs.

- [ ] **Step 1: Add the failing build-engineer assertions to the structural test; run to fail.**
- [ ] **Step 2: Author `skills/build-engineer/SKILL.md`.** Front-matter `name: build-engineer`, `description:`. Body MUST contain, in order, these mechanizable steps (each a heading the test checks):
  1. **Detect mode** — `git rev-parse --is-inside-work-tree` succeeds OR the folder contains non-binary source ⇒ deep-read; empty/no-repo ⇒ grill; ambiguous ⇒ ask the user.
  2. **Minimal asks** — engineer name (default `<repo>-engineer`); confirm single-agent + subagent install. Nothing else.
  3. **Bootstrap** — ensure `.genesis/` exists (`genesis-cli bootstrap <target> <genesis_home>`); ship the generic recipe modules in.
  4. **Expertise** — deep-read branch: enumerate then fully read every non-binary file (no content-grep) → run `research-expertise` to author `<target>-stack-mastery` at pinned versions. grill branch: skip stack research; the engineer will run `grill` post-build.
  5. **Delegate to Method** — TASK-SPEC for the complete-engineer persona/behavior (parameterised from `team/genesis-engineer/`), `meta.json` (tools omit `Grep`; `required_expertise` = the recipe), `grow-safely` + `grill` skills, tests-first.
  6. **Assemble** — `genesis-cli assemble <src> <name> <target> <genesis_home>` (subagent).
  7. **Verify** — Method's tests pass; frontmatter valid; no `Grep`; expertise registered.
  8. **Offer promote** — ask once; on yes run `genesis-cli promote <name>`.
  - State both hard rules explicitly (never speculate; never shortcut) as governing the whole procedure.
- [ ] **Step 3: Run the structural test to green.**
- [ ] **Step 4: Commit**

```bash
git add skills/build-engineer/SKILL.md test/tools/build-engineer/skill_structure.test.mjs
git commit -m "feat(skill): build-engineer mechanizable build recipe"
```

---

## Phase 4 — The command

### Task 9: `commands/build-engineer.md`

**Files:**
- Create: `commands/build-engineer.md`
- Test: `test/tools/build-engineer/skill_structure.test.mjs` (command assertions)

**Interfaces:**
- Consumes: the `sensei` agent + the `build-engineer` skill.

- [ ] **Step 1: Add failing command assertions to the structural test; run to fail.**
- [ ] **Step 2: Author `commands/build-engineer.md`** mirroring `commands/new.md`: front-matter `description:` + `argument-hint: [target path (optional)]`; body instructs invoking the `sensei` agent (Agent tool) and following the `build-engineer` skill end-to-end with the detected mode and the fixed recipe; escalate every decision, build nothing on an unconfirmed assumption.
- [ ] **Step 3: Run the structural test to green. Step 4: Commit.**

```bash
git add commands/build-engineer.md test/tools/build-engineer/skill_structure.test.mjs
git commit -m "feat(command): /genesis:build-engineer entry point"
```

---

## Phase 5 — Structural tests, fixtures, integration

### Task 10: Structural test file (the harness Tasks 7–9 target)

**Files:**
- Create: `test/tools/build-engineer/skill_structure.test.mjs`

- [ ] **Step 1: Write the complete structural test** (Node `node:assert`+`node:fs`, exit non-zero on fail). It asserts, using front-matter + required-substring checks:
  - `commands/build-engineer.md` exists; front-matter has `description`; body mentions `sensei` and `build-engineer`.
  - `skills/build-engineer/SKILL.md` exists; front-matter `name`/`description`; body contains the 8 step markers (detect mode, minimal asks, bootstrap, expertise, Method, assemble, verify, promote) in order; contains both hard rules.
  - `skills/grill/SKILL.md` exists; front-matter `name`/`description`; body contains the topic checklist + "does not finish until answered" + memory capture.
  - No authored file under `commands/`/`skills/build-engineer`/`skills/grill` contains the banned reasoning-trace phrase (build the check with `['chain','of','thought'].join('-')`, never the literal).
- [ ] **Step 2: Run — expect FAIL (files not yet written); this is the RED that Tasks 7–9 turn green.**
- [ ] **Step 3: Commit the test first.**

```bash
git add test/tools/build-engineer/skill_structure.test.mjs
git commit -m "test(build-engineer): structural tests for command + skills"
```

### Task 11: Fixtures + integration (mode-detection + no-drift)

**Files:**
- Create: `test/tools/build-engineer/fixtures/real-repo/` (a tiny git repo: one source file + one dependency manifest), `test/tools/build-engineer/fixtures/empty-folder/.gitkeep`
- Create: `test/tools/build-engineer/integration.test.mjs`

- [ ] **Step 1: Write the failing integration test.** It asserts the mode-detection contract deterministically without running a full LLM build:
  - a helper `detectMode(dir)` (extract the rule the skill encodes into a tiny shared `test/tools/build-engineer/detect_mode.mjs`) returns `deep-read` for `fixtures/real-repo` and `grill` for `fixtures/empty-folder`.
  - no-drift: authoring the same fixture twice (call the pure scaffolding step) yields byte-identical output.
- [ ] **Step 2: Create fixtures + `detect_mode.mjs` (the single source of truth the skill references).**
- [ ] **Step 3: Run to green. Step 4: Commit.**

```bash
git add test/tools/build-engineer/
git commit -m "test(build-engineer): fixtures + mode-detection + no-drift integration"
```

---

## Self-Review

- **Spec coverage:** AC1→Task 9; AC2/AC3→Task 8 (+Task 10 assertions); AC4→Task 7; AC5→Tasks 1–6; AC6→validator (Task 1) + structural test (Task 10); AC7→Task 8 (meta.json spec) verified in Task 11; AC8/AC9→Task 11 (fixtures) — note: full end-to-end LLM build is exercised manually at first run, integration test covers the deterministic contract; AC10→Task 11 no-drift. Gap acknowledged: AC8/AC9's *full* agent build is not unit-testable; the integration test covers the deterministic mode/scaffold contract and the manual first-run is the acceptance. Recorded, not hidden.
- **Placeholder scan:** none — research tasks carry named sources + the shared validator as the concrete gate.
- **Type consistency:** validator CLI signature `_manifest_check.mjs <name> [root]` used identically in Tasks 1–6; `detect_mode.mjs` is the single mode-detection source referenced by both the skill (Task 8) and the test (Task 11).

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-08-16-build-engineer.md`. Two execution options:
1. **Subagent-Driven (recommended)** — a fresh subagent per task, two-stage review between tasks.
2. **Inline Execution** — batch execution in this session with checkpoints.
