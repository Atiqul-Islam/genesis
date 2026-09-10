# Spec-Driven Development: Building So a Human Can Easily Review It

**A distilled, primary-source-grounded engineering guide for `genesis-engineer`** — the single agent
that owns the whole Genesis repo (three Rust crates `server/`, `cli/`, `hook/`; the Claude Code plugin
under `.claude/` = agents/skills/commands/hooks/templates; and `docs/`). It develops by strict TDD and
**spec-driven development (SDD)** so that a human reviewer can read *plain English first*, see it compiled
to executable Gherkin/tests, and trust that spec ↔ code ↔ tests never silently diverge.

> **Labels.** [VERIFIED] = read this run from a cited primary source (URL) or a concrete repo path.
> [INFERRED] = a reasoned corollary, labelled as such. Every load-bearing claim carries one.
> **Grounding.** The canonical in-repo references are `docs/spec-driven-development.md`,
> `docs/multi-agent-workflow.md`, `docs/architecture.md`, the `spec-*`/`forge-*` skills under
> `.claude/skills/`, and the ratified exemplar `test/specs/genesis-memory-server.md`.

---

## §0. The thesis (one sentence)

The spec — not the code — is the surface a human reviews, so you write the behaviour in **plain English
first**, **compile** it deterministically to executable Gherkin scenarios + typed tests, and then make
those tests pass; every requirement is traceable from an English sentence to a red-then-green test, and
the three artifacts (spec ↔ code ↔ tests) are kept in lockstep by process gates rather than by hope.
[INFERRED from the sources below]

## §1. What spec-driven development is, and why Genesis uses it

**Definition (primary sources).** SDD "means writing a 'spec' before writing code with AI ('documentation
first'). The spec becomes the source of truth for the human and the AI." [VERIFIED]
([Fowler/Böckeler, *Understanding SDD: Kiro, spec-kit, and Tessl*](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html)).
Thoughtworks frames it as "a development paradigm that uses well-crafted software requirement
specifications as prompts, aided by AI coding agents, to generate executable code." [VERIFIED]
([Thoughtworks, *Spec-driven development*](https://www.thoughtworks.com/en-us/insights/blog/agile-engineering-practices/spec-driven-development-unpacking-2025-new-engineering-practices)).

**The three maturity levels (know which one you are at).** Fowler distinguishes: **spec-first** (a spec is
written first, then drives the task), **spec-anchored** (the spec is *kept* after the task, to evolve and
maintain the feature), and **spec-as-source** (only the spec is edited; the human never touches code).
[VERIFIED] (Fowler). **Genesis operates at spec-anchored:** the ratified spec is retained
(`test/specs/genesis-memory-server.md` is a permanent, versioned artifact with a changelog), but the Rust
code and tests remain first-class artifacts a human also reviews. This matches the honest Thoughtworks
stance that "executable code remains the source of truth you need to maintain" — the spec *drives* code
generation the way a test drives it in TDD, rather than replacing it. [VERIFIED] (Thoughtworks).

**Why Genesis chose this (the load-bearing reason): human reviewability.** The whole point is that the
reviewer reads intent in their own language before they read a diff. GitHub's spec-kit puts it sharply:
"maintaining software means evolving specifications … code is the last-mile approach." [VERIFIED]
([github/spec-kit](https://github.com/github/spec-kit)). For `genesis-engineer`, whose changes span Rust
crates, a plugin, and docs, the plain-English spec is the *only* review surface that a non-Rust stakeholder
(the repo owner) can audit line-by-line — which is exactly the design intent recorded in
`docs/spec-driven-development.md` ("Specs are the source of truth, not the code"). [VERIFIED, repo path].

**What SDD is NOT.** It is not "prompt the model and read the diff." A spec that merely restates the code
in prose adds no review value. The spec must describe **WHAT** the system does (observable behaviour), not
**HOW** it does it (implementation) — see §3. Anything HOW-shaped that is nonetheless required (a storage
backend, an algorithm) is quarantined into a clearly-labelled *Implementation Requirements* section so it
is reviewed as a decision, not smuggled in as behaviour. [VERIFIED, `spec-create/SKILL.md` lines 66-74].

## §2. The pipeline: plain English → Gherkin → executable tests

This repo runs a two-layer, double-loop pipeline. [VERIFIED, `docs/spec-driven-development.md` §Pipeline]:

```
test/specs/*.md   --Claude "compiles"-->  test/features/*.feature  --BDD runner (outer loop)--> Pass/Fail
(plain English)        (skill enforces)        (Gherkin)               (real system, NO mocks)
       |
       +--(Implementation Requirements)--> unit-test stubs  --unit runner (inner TDD loop)--> Pass/Fail
                                            (mocks allowed)
```

- **Layer 1 — plain-English spec** (`test/specs/<slug>.md`): human-authored/ratified. The source of truth
  for review. [VERIFIED].
- **Layer 2 — Gherkin feature** (`test/features/<slug>.feature`): Claude *compiles* the spec's acceptance
  criteria into `Given/When/Then` scenarios. Claude Code "acts as the 'compiler' between plain English and
  Gherkin, and is constrained by skills and rules to never skip steps." [VERIFIED, repo doc].
- **Layer 3 — executable tests**: for Genesis's `server/` crate the runner is **cucumber-rs** (§6); it
  executes the `.feature` file against the *real* system. Implementation Requirements additionally compile
  to typed unit-test stubs. [VERIFIED, `spec-compile/SKILL.md`].

**Traceability is mechanical, both directions.** Every compiled `.feature` file's first line is
`# Source: test/specs/<slug>.md`, and every generated unit-stub / step file carries a `// Source:` /
`# Source:` comment. [VERIFIED, `store.feature` line 1; `spec-compile/SKILL.md` lines 81-82, 176]. This is
what lets a reviewer jump from a failing scenario back to the exact English sentence it enforces. The
in-repo `store_steps.rs` header even names the criteria it covers ("acceptance criteria 3, 9, 10").
[VERIFIED, `server/tests/bdd/store_steps.rs`].

## §3. Spec anatomy and authoring conventions

A Genesis spec has a fixed shape (feature form). [VERIFIED, `spec-create/SKILL.md` lines 42-64,
`spec-agent/SKILL.md` lines 54-76]:

```markdown
# test/specs/<feature-name>.md
## Feature: <Feature Title>
<1-2 sentence description: what it does and why.>

### Expected Behavior          # WHAT the user/caller observes — one bullet per behaviour
### Acceptance Criteria        # concrete, testable conditions — these become Gherkin scenarios
### Implementation Requirements (optional)  # non-observable concerns — become unit tests, NOT Gherkin
```

- **Bug specs** use `## Bug:` with `Current Behavior`, `Expected Behavior`, `Reproduction Steps`,
  `Acceptance Criteria` (the reproducer scenario + regressions). [VERIFIED, `spec-create/SKILL.md` 77-98].
- **Expected Behavior describes WHAT the user sees, not HOW it's implemented.** If a line is not
  observable (a storage backend, a hashing algorithm, a DB table, a performance number), it belongs in
  *Implementation Requirements*, not *Expected Behavior*. The `/spec-compile` structure gate blocks
  compilation on non-UI-observable Expected-Behavior lines. [VERIFIED, `spec-compile/SKILL.md` 46-53].
- **Every Expected Behavior bullet MUST have a matching Acceptance Criterion** (1:1). Orphaned behaviours
  are flagged. [VERIFIED, `spec-create/SKILL.md` 69; `spec-agent/SKILL.md` 102].
- **Acceptance Criteria are concrete and testable** — they are the sentences that become Gherkin scenarios
  (Layer 2). Dan North's original insight: "A story's behaviour is simply its acceptance criteria: if the
  system fulfils all the acceptance criteria, it's behaving correctly." [VERIFIED]
  ([Dan North, *Introducing BDD*](https://dannorth.net/blog/introducing-bdd/)).
- **Implementation Requirements are atomic.** Split compound requirements — "Passwords are hashed with
  bcrypt **and** salted" becomes two independently-verifiable requirements. Each compiles to its own unit
  stub. [VERIFIED, `spec-create/SKILL.md` 74; `spec-compile/SKILL.md` 134-176].
- **Include a happy path, an edge case, and an error case — but only if the request supplies them.** The
  supervisor-mediated `spec-agent` adds edge/error cases "ONLY IF user_intent supplies them." [VERIFIED,
  `spec-agent/SKILL.md` 104]. Inventing a plausible-but-unrequested edge case is a hallucination (§4).
- **Use the requester's natural language; inject no technical jargon they did not use.** [VERIFIED,
  `spec-create/SKILL.md` 76]. This keeps the spec reviewable by a non-engineer.

## §4. Grounding: never invent — the hallucination audit

The single largest risk in AI-authored specs is fabricated detail that reads as fact. Genesis defends with
a deterministic **hallucination audit** run on every spec draft before it is accepted. Six marker types
[VERIFIED, `spec-build/SKILL.md` 164-175; `multi-agent-workflow.md` 104-114]:

| Marker | What it looks like |
|---|---|
| `invented_identifier` | file/module/function/service names not in the request and not in the codebase (verify via Glob/Grep) |
| `number_without_origin` | thresholds/limits/timeouts the requester never gave ("lock after 5 attempts", "200ms") |
| `implementation_specific` | tech choices not derivable from behaviour (`bcrypt`, `PostgreSQL`, `Redis`) |
| `unconfirmed_edge_case` | "what if X is empty/null/negative" the requester never raised |
| `external_dependency` | APIs/DBs/queues/third-party services the requester never named |
| `compound_requirement` | one requirement bundling 2+ atomic claims with AND |

- **Rule: write no spec claim you cannot source** from the request or from existing codebase facts; every
  unsourced claim is declared (the author self-reports them as `audit_targets` / `assumptions_made`), and
  a human ratifies or corrects each before it is accepted. [VERIFIED, `spec-agent/SKILL.md` 12, 29-35, 108-116].
- **Provenance labelling (the exemplar to imitate).** The ratified `genesis-memory-server.md` spec labels
  every claim inline as one of three kinds: **Sourced** (traceable to a doc §-ref or the scaffold),
  **(ratified choice — unsourced; rationale: …)** (no source; a human chose it), or **Bootstrap /
  calibration item** (a value that can only be measured once the artifact exists — a file digest, an
  empirical fixture — the spec states the *requirement* to pin it, and the literal value is captured at
  first run and committed as a constant). [VERIFIED, `test/specs/genesis-memory-server.md` "Provenance
  legend"]. Imitate this: a reviewer must never mistake a ratified choice for a sourced fact.
- **Deliberate deviations are written down, not absorbed.** When the build must diverge from a source
  (e.g. D8: dedup runs at `consolidate` time, not "on insert" as §2.4 phrased it), the spec records the
  deviation, its rationale, its consequence on the acceptance criteria, and the v2 option — rather than
  silently doing something the source did not say. [VERIFIED, same spec, "Deliberate deviations from source"].

## §5. Gherkin authoring conventions

Gherkin is the human-and-machine-readable medium: BDD's *Formulation* step deliberately uses "a medium
that can be read by both humans and computers" so the whole team can confirm shared understanding *and*
the examples can be automated. [VERIFIED]
([Cucumber, *BDD*](https://cucumber.io/docs/bdd/)). Author scenarios to these rules
([Cucumber, *Gherkin reference*](https://cucumber.io/docs/gherkin/reference/), all [VERIFIED]):

- **Keyword grammar.** Primary keywords: `Feature`, `Rule`, `Example` (synonym `Scenario`),
  `Given`/`When`/`Then`/`And`/`But` (or `*`) for steps, `Background`, `Scenario Outline`, `Examples`.
  Secondary: `"""` doc strings, `|` data tables, `@` tags, `#` comments.
- **The step triple has fixed meaning.** `Given` = the initial context / scene (something that happened in
  the *past*; put the system in a known state — do not describe user interaction here). `When` = the event
  / action. `Then` = the expected, observable outcome. Every scenario follows: context → event → outcome.
- **One behaviour per scenario; 3-5 steps.** Cucumber recommends 3-5 steps per example — "having too many
  steps will cause the example to lose its expressive power as a specification and documentation." Keep
  scenarios focused: one behaviour each. [VERIFIED, Gherkin ref; `spec-compile/SKILL.md` 87].
- **Write declaratively in domain language, not imperative UI mechanics.** "The language you choose for
  Gherkin should be the same language your users and domain experts use." [VERIFIED, Gherkin ref]. The
  in-repo scenarios read `Given a memory server with an empty database … When agent "alpha" recalls with
  the calibrated paraphrase … Then the recall result contains an entry whose text is the calibrated source
  text` — domain vocabulary, not button clicks. [VERIFIED, `store.feature`].
- **Factor repeated `Given`s into `Background`.** Repetition across a feature's scenarios signals
  incidental detail — move it to a `Background` block. [VERIFIED, Gherkin ref].
- **Reuse step definitions; parameterize with `{string}`, `{int}`.** Compilation first inventories existing
  steps (Glob `test/steps/**` / `tests/bdd/*_steps.rs`) and reuses them; most suites reuse the same 10-15
  common steps. [VERIFIED, `spec-compile/SKILL.md` 39-43, 85-89; `docs/spec-driven-development.md`
  §Key Principles].
- **As a whole, your scenarios ARE an executable specification of the system.** [VERIFIED, Gherkin ref] —
  which is why the `.feature` set doubles as living, always-checked documentation.

## §6. Executable tests in Rust: cucumber-rs (the `server/` crate)

Genesis pins **cucumber `0.23`** (with `features = ["macros"]`), resolving to `cucumber 0.23.0` /
`gherkin 0.16.0`. [VERIFIED, `server/Cargo.toml` line 55; `server/Cargo.lock` cucumber 0.23.0, gherkin
0.16.0]. Cucumber tests "are run along with other tests via `cargo test`, but rely on `.feature` files …
as well as a set of step matchers (described in code)." [VERIFIED]
([Cucumber Rust Book, *Quickstart*](https://cucumber-rs.github.io/cucumber/current/quickstart.html)).

The in-repo pattern for each feature [VERIFIED, `server/tests/bdd/store_steps.rs`]:

```rust
use cucumber::{given, then, when, World as _};

#[derive(Debug, Default, cucumber::World)]     // the scenario-scoped World holds all state
struct StoreWorld { /* db_dir: TempDir, embedder, vector_store, last_recall, ... */ }

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut StoreWorld) { /* real SQLite + sqlite-vec */ }
// #[when(regex = ...)] / #[then(regex = ...)] async fns take &mut World
```

Non-negotiable Rust wiring rules:

- **Each `tests/bdd/<slug>_steps.rs` needs a matching `[[test]]` block in `server/Cargo.toml`, generated
  *together with* the steps file:** [VERIFIED, `spec-compile/SKILL.md` 109-116; `server/Cargo.toml`
  100-118]
  ```toml
  [[test]]
  name = "bdd_<slug>"
  path = "tests/bdd/<slug>_steps.rs"
  harness = false                # lets Cucumber print output instead of libtest
  ```
  A `[[test]]` block whose `path` does not exist breaks `cargo build`/`cargo test`; a steps file with no
  block is never discovered. `harness = false` is required so Cucumber (not libtest) drives the binary.
  [VERIFIED, Cucumber Rust Book quickstart; `spec-scaffold/SKILL.md` 47-49].
- **RED stubs must fail at runtime, never pass.** Generated step bodies and unit stubs use
  `unimplemented!("Implement via TDD")` (Rust) / `pytest.fail(...)` (Python), which panics/fails → the
  test is RED. Rust stubs *compile* because the lint gate sets `unimplemented = "warn"` (not deny).
  [VERIFIED, `spec-compile/SKILL.md` 106, 173-174; `spec-scaffold/SKILL.md` 52-54].
- **No mocks in the outer (BDD) loop.** The `server/` suites run against real SQLite + real `sqlite-vec` +
  the real ONNX embedder + the real spawned stdio binary; hermeticity comes from a per-scenario
  `tempfile::TempDir` in the `World`, not from mocking. Only inner unit tests may mock. [VERIFIED,
  `docs/architecture.md` "No-mock BDD"; `store_steps.rs` header; `docs/spec-driven-development.md` §Layer 3].
- **`cargo test` runs it directly** — there is no code-generation (`bddgen`) step for Rust; cucumber-rs
  reads `test/features/<slug>.feature` at runtime. Run the BDD bins with
  `cargo test --release --test 'bdd_*'`; full suite with `cargo test --release`. [VERIFIED,
  `spec-test/SKILL.md` 49, 81-85].
- **Runner-surface caveat.** The exact cucumber-rs 0.23 runner/macro entry (`run_and_exit` vs `World::run`)
  is flagged [INFERRED] in `docs/SPEC_FORGE_RUST_UPDATE.md` §6.2 #4 — confirm it at first compile rather
  than assuming. [VERIFIED that it is flagged, repo doc; the resolution is INFERRED].

## §7. Double-loop TDD and the ordered 9-step workflow

SDD in Genesis is executed as **double-loop TDD**: an **outer BDD loop** (behaviour, no mocks) wraps an
**inner unit TDD loop** (implementation mechanics, mocks allowed). Refactoring happens *inside* the inner
loop after each green, not as a batch at the end. [VERIFIED, `docs/spec-driven-development.md`
§Double-Loop TDD]. The inner loop obeys Uncle Bob's Three Laws: write production code only to pass a
failing unit test; write no more of a test than suffices to fail; write no more code than suffices to pass.
[VERIFIED, same].

The mandatory ordered pipeline (manual mode; the multi-agent modes inline the identical gates)
[VERIFIED, `docs/spec-driven-development.md` §CLAUDE.md Rules; external template `CLAUDE.md` 34-46]:

```
1 SPEC     /spec-create + /spec-compile          6 VERIFY      /spec-test (still green post-simplify)
2 RED      /spec-test — BDD + stubs MUST FAIL     7 REGRESSION  /spec-test all
3 TDD LOOP red→green→refactor, per unit test      8 REVIEW      /spec-crap (fails >8) → /code-review
4 GREEN    /spec-test — BDD + units MUST PASS     9 DOCS        /docs-update (+ optional learnings capture)
5 SIMPLIFY /spec-simplify (impl only)
```

- **RED before GREEN is inviolable.** "NEVER skip the RED step — if tests pass before implementation, the
  tests are wrong." A premature pass at RED routes back to regenerate the stubs. [VERIFIED,
  `docs/spec-driven-development.md` 372; `spec-build/SKILL.md` 98-99].
- **GREEN is stricter than "tests pass."** For `server/`, GREEN =
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release`; a non-zero
  `fmt`/`clippy` exit is not GREEN. [VERIFIED, `docs/architecture.md` "Quality gates"; `spec-test/SKILL.md`
  85]. Lints forbid `unwrap`/`expect`/`panic`/`todo` in `src/` (tests exempt).
- **No step is skipped and their order is enforced** — the supervisor "enforces the order (you can't
  accidentally skip RED)." [VERIFIED, `multi-agent-workflow.md` 152-157].

## §8. Keeping spec ↔ code ↔ tests in sync (the anti-drift discipline)

Living documentation only stays true if edits flow in one disciplined direction. In BDD "the code reflects
the documentation, and the documentation reflects the team's shared understanding." [VERIFIED, Cucumber
BDD]. The enforced sync rules [VERIFIED, `docs/spec-driven-development.md` §Rules 368-376; external
template `CLAUDE.md` 83-90]:

- **Spec changes update the `.feature` FIRST, verify it fails, then change the code.** The test always
  moves before the implementation.
- **Never modify a `.feature` file to make a failing test pass — that is cheating.** Never delete or weaken
  existing scenarios. Never edit a generated unit stub to make it pass — implement the real logic.
- **Never write implementation code without a corresponding spec in `test/specs/`.** No spec, no code.
- **Bug fixes add a reproducer scenario BEFORE the fix** — the scenario that reproduces the bug is written
  and seen to fail first, then the fix makes it green, preventing re-introduction. [VERIFIED, repo doc 375].
- **The suite only grows.** New features add scenarios; bug fixes add reproducers; the full suite is the
  regression and runs in CI on every push. [VERIFIED, repo doc §Regression Guarantees].
- **Spec-anchored maintenance.** The spec is retained and versioned (see the `genesis-memory-server.md`
  changelog v2→v3): behaviour changes are edits to the spec, which then re-drive the feature file and code.
  [VERIFIED, ratified spec "Changelog"]. After structural changes, `/docs-update` re-syncs
  `docs/architecture.md`; the methodology doc `docs/spec-driven-development.md` is maintained by hand.
  [VERIFIED, external template `CLAUDE.md` 150-160].

## §9. Acceptance criteria as the definition of done

- **Done means every acceptance criterion is an executable test, and all of them are green under the full
  gate.** Dan North: "Acceptance criteria should be executable." [VERIFIED, Dan North]. Concretely, for
  `server/`: every criterion maps to a Gherkin scenario / unit test, and the change is done only when
  `cargo fmt --check && cargo clippy -- -D warnings && cargo test --release` all pass, regression is green,
  and the CRAP gate passes. [VERIFIED, §7; `docs/architecture.md`].
- **The CRAP change-risk gate.** `CRAP(f) = CC(f)² × (1 − cov(f)/100)³ + CC(f)`; **exit 2 (fail) iff any
  function > 8** (alert > 30, target ≤ 4). For Rust it is computed from `rust-code-analysis-cli`
  (complexity) + `cargo-llvm-cov` (coverage) via `test/tools/rust_crap_adapter.mjs` → `crap.py`. Never
  lower the threshold to pass — fix the code (lower complexity or raise coverage). [VERIFIED,
  `spec-crap/SKILL.md`; `docs/architecture.md` "CRAP gate"].
- **A self-declared "pass" is not proof — the deterministic gate is.** GREEN is proven by the exit codes of
  `fmt`/`clippy`/`test`/`crap`, not by an assistant asserting success. Prove success with the commands,
  then claim it. [INFERRED from §7 gate semantics; consistent with `expertise-application.md` C2].

## §10. Independent review before commit

- **Run change-risk + an independent audit before commit.** Step 8 REVIEW runs `/spec-crap` (change-risk
  data as input) then `/code-review`, a multi-agent audit of the diff against `CLAUDE.md`. It "catches
  workflow cheating that tests cannot detect (e.g. `.feature` edited to pass, RED skipped, stub-only
  implementations)." [VERIFIED, `docs/spec-driven-development.md` 326-331, 410]. In `/spec-forge` this is
  the local `requesting-code-review` subagent (no GitHub PR needed). [VERIFIED, `multi-agent-workflow.md`
  179]. The review is a *separate* judge and may only block — it does not certify success (tests do).

## §11. Scope: where each discipline applies across the Genesis repo

The SDD/BDD machinery is not uniform across the repo — apply the right layer to the right artifact.
[VERIFIED, repo enumeration this run]:

- **`server/` — full cucumber-rs BDD + unit TDD.** Four feature files (`store`, `recall`, `consolidate`,
  `server`), each with a `harness = false` `[[test]]` bin; 17 acceptance criteria; a golden embedding
  fixture; the CRAP gate. This is the crate the `/spec-forge` Rust flow was built for. [VERIFIED,
  `test/features/`, `docs/architecture.md`].
- **`cli/` and `hook/` — plain Rust integration tests, not Gherkin.** These crates carry
  `tests/*.rs` (`cli/tests/integration.rs`, `cli/tests/session_copy.rs`, `hook/tests/cli.rs`) run by the
  libtest harness; there are no `.feature` files under them. [VERIFIED, directory listing this run]. Apply
  the *same spec-first discipline* (spec the behaviour, RED before GREEN, keep tests as the review surface)
  but the executable layer is ordinary `#[test]` integration tests, not cucumber-rs. [INFERRED — the crates
  ship no cucumber setup].
- **The plugin (`.claude/` skills/commands/hooks/templates) and `docs/` — prose artifacts.** Changes here
  are reviewed as English against their own contracts (skill `SKILL.md`, routing tables, handoff schemas);
  the "spec" and the "review surface" are the same document. Keep them internally consistent (a skill's
  described behaviour must match what it does) rather than compiling them to Gherkin. [INFERRED].
- **Language detection is parameterized, not hardcoded.** `/spec-forge` Phase 0 resolves `state.json.language`
  (flag → `Cargo.toml` ⇒ `rust` → `pyproject.toml` ⇒ `python` → default); phase order, gate *semantics*,
  routing, handoff schema, and thresholds are all language-agnostic and unchanged. [VERIFIED,
  `spec-forge/language-detection.md` on `main`].

## §12. Anti-patterns (do not do these)

- **Editing a test to fit the code.** Modifying a `.feature` scenario or a unit stub so a red test turns
  green — the canonical cheat the review gate exists to catch. [VERIFIED, §8, §10].
- **Skipping RED.** Writing implementation, then tests that pass immediately — you have proven nothing.
  [VERIFIED, §7].
- **HOW in the WHAT.** Putting storage backends, algorithms, or magic numbers in *Expected Behavior*
  instead of *Implementation Requirements* — the structure gate blocks it. [VERIFIED, §3].
- **Inventing detail.** Adding an identifier, number, dependency, or edge case the requester never gave —
  a hallucination marker. Flag it; never bake it in silently. [VERIFIED, §4].
- **Compound acceptance criteria / requirements.** One sentence asserting several things at once — it can't
  map cleanly to one scenario or one unit test. Split it. [VERIFIED, §3].
- **Imperative, UI-mechanical Gherkin.** `Given I click the button with id #x` instead of domain language
  — brittle and unreadable as a spec. [VERIFIED, §5].
- **A dangling `[[test]]` block or an orphan steps file.** Either breaks `cargo build` or is never
  discovered — always generate the pair together. [VERIFIED, §6].
- **Claiming GREEN without running the gate.** "Should pass" is not "passes"; run `fmt`/`clippy`/`test`/
  `crap` and read the exit codes first. [VERIFIED, §9].

## §13. Honest limits (do not oversell)

- **Executable tests prove the criteria you encoded, not that the spec is complete or correct.** A green
  suite means the *written* acceptance criteria hold; whether those criteria capture the real intent is a
  human judgement made on the plain-English review surface — which is the whole reason the spec exists.
  [INFERRED; consistent with Thoughtworks' "executable code remains the source of truth" caution].
- **"Spec-as-source" is not where Genesis is.** Only edit-the-spec-never-the-code (Fowler level 3) would
  remove code review; Genesis is spec-anchored, so the Rust diff still gets reviewed. Do not treat the spec
  as if it fully replaced the code. [VERIFIED, Fowler; §1].
- **The Rust runner surface is confirmed at compile, not assumed.** cucumber-rs 0.23's exact entry point is
  flagged [INFERRED] in the repo's own plan; verify at first compile. [VERIFIED that it is flagged, §6].
- **CRAP thresholds are calibrated for radon; the Rust adapter is close but not bit-identical.** Keep the
  `> 8` fail line, but treat the absolute numbers as calibration items to re-validate once on the real
  crate — never move the threshold to pass. [VERIFIED, `spec-crap/SKILL.md` 44].
