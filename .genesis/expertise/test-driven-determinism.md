# Test-Driven Determinism — Proving genesis-engineer's Work Correct and Reproducible

> **Purpose.** This is the definitive, evidence-backed practitioner guide to **test-driven, determinism-first
> engineering** for **genesis-engineer** — the single Claude agent that owns the whole Genesis repository:
> three Rust crates (`server/`, `cli/`, `hook/`), the Claude Code plugin (agents / skills / commands / hooks /
> templates), and the docs. genesis-engineer develops strictly by **TDD**, works **spec-first**, deploys and
> operates the project, **reads every non-binary file in full** (never grep-to-skip a content read), and must be
> able to **prove its work is deterministic**. Every rule below is scoped to that agent and grounded in this
> repository. It is written to be **executed by a tool** (a test, a hook, a CI gate), not merely read.
>
> **The thesis, in one line.** *Determinism is proven, not asserted.* Convert every claim about the system into a
> fresh command whose **exit code and output are the evidence**; and for the part that genuinely cannot be made
> deterministic — LLM / agent behavior — pin it with a **deterministic harness** built around it, never by
> trusting the forward pass. This is the same operating principle Genesis already encodes for expertise
> enforcement: *"determinism lives outside the forward pass"* (`expertise-application.md` §0/§2, rule `ea-0`).
> **[VERIFIED, repo `/mnt/c/Users/iatiq/Documents/genesis/.genesis/expertise/expertise-application.md`]**
>
> **Evidence discipline.** **[VERIFIED]** = read from a cited primary source (a URL or a repo path).
> **[INFERRED]** = a reasoned corollary this guide draws over the sources. Every load-bearing claim carries one.
>
> **Every actionable rule has a stable id (`tdd-N`).** The companion manifest
> `manifests/test-driven-determinism.json` indexes each, typed `checkable | judgment | principle`, with a
> `predicate` (checkable) or `reviewer_criterion` (judgment).
>
> **House rules honored throughout.** This guide never writes a credential value (a secret is referenced only as
> "credential present at `<path>`"); and it never writes the banned reasoning-trace phrase that the gate denies —
> it says **structured reasoning** / **step-by-step reasoning** instead.
>
> Status: **v1 — 2026-08-15. Grounded in the pinned repo tooling (`rust-toolchain.toml` 1.93.0; `cucumber 0.23`,
> `assert_cmd 2.2`, `approx 0.5`, `insta 1`, `tempfile 3`) and primary TDD / coverage / reliability sources. No
> section dropped — `sections_accounted` maps every header to its rules.**

---

## 0. Thesis

**tdd-0 (principle).** *Determinism is proven, not asserted.* Turn every completion claim into a **fresh,
full command** whose **exit code and output are the evidence**; make the maximum fraction of each requirement a
**mechanically-checkable** property a test or hook verifies; and wrap the irreducibly non-deterministic part —
Mneme's LLM structuring, an agent's behavior — in a **deterministic harness** that asserts store-level or
output-level invariants, never trusting the model's forward pass. This mirrors `ea-0`: *determinism lives
outside the forward pass*. Everything below is the mechanism behind that one sentence. **[INFERRED from
`expertise-application.md` §0; grounded in `server/tests/eval.rs`.]**

---

## 1. The three laws of TDD and the red-green-refactor cycle

TDD is not "write tests." It is a **tight loop with a mandatory order**, and the order is the whole point.

**tdd-1 (checkable).** Obey the **Three Laws of TDD** *(Robert C. Martin, primary)* for **every** feature, bug
fix, refactor, and behavior change in `server/`, `cli/`, and `hook/`: **(1)** write **no** production code except
to make a **failing** test pass; **(2)** write **no more of a test** than is sufficient to fail — *and a
compilation failure counts as a failure*; **(3)** write **no more production code** than is sufficient to pass
the one failing test. If code was written before its test, **delete it and start over from the test** — do not
keep it as reference, do not adapt it. **[VERIFIED, http://butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd]**

**tdd-2 (checkable).** **Verify RED before you write code.** Run the new test and confirm it **fails for the
intended reason** — the feature is missing — not because of a typo or a compile error. *If you did not watch the
test fail, you do not know it tests the right thing.* A test that **passes on its first run** against absent code
proves nothing and must be fixed until it fails correctly. **[VERIFIED, `test-driven-development` SKILL.md:
"Verify RED — Watch It Fail … MANDATORY. Never skip."]**

**tdd-3 (checkable).** Run the loop as **Red → verify-red → Green (minimal code) → verify-green → Refactor →
stay-green → next**. In the GREEN step write the **simplest** code that passes (no options, no YAGNI features);
refactor **only after green**, and add **no new behavior** during a refactor — the tests must stay green across
it. **[VERIFIED, `test-driven-development` SKILL.md, "Red-Green-Refactor".]** For genesis-engineer this loop is
externalized by `/spec-forge`: the RED gate requires every stubbed test to fail (`unimplemented!("Implement via
TDD")` panics at runtime → healthy RED), then the TDD inner loop turns each red test green one at a time.
**[VERIFIED, `verify-agent` SKILL.md Phase 5; `spec-test` SKILL.md.]**

---

## 2. Read the whole file before you test it

TDD quality collapses if you test code you have not read. This is where genesis-engineer's "read every non-binary
file fully" rule and TDD meet.

**tdd-4 (judgment).** Before writing or changing a test for any file, **read that file — and its reference
implementation — in full**; never grep-to-skip the content read. Partial understanding is exactly what produces
the classic failures: **testing a mock instead of the real behavior**, an **incomplete mock** that omits a field
downstream code consumes, or an assertion aimed at the wrong layer. When implementing a pattern, read the
reference implementation **completely — every line** — before applying it. **[VERIFIED, `systematic-debugging`
SKILL.md Phase 2 "read reference implementation COMPLETELY"; `testing-anti-patterns.md`.]** This is also the
first move of debugging: *read the error message and the stack trace completely; they often contain the exact
solution.* **[VERIFIED, `systematic-debugging` SKILL.md Phase 1.]**

---

## 3. The Rust test surface — unit, integration, doctest, BDD

genesis-engineer must place each test at the layer Cargo expects, because the layer changes what the test can
see and how it is compiled and run.

**tdd-5 (checkable).** Use the **four Rust test layers** at their canonical locations:
- **Unit tests** — in a `#[cfg(test)] mod tests` **inside the `src` file under test**; `#[cfg(test)]` compiles
  them only under `cargo test`, and they can reach private items. **[VERIFIED, Rust Book ch11-03.]**
- **Integration / end-to-end tests** — one file per crate under `<crate>/tests/*.rs`; **each file is compiled as
  its own separate crate** and sees only the crate's public API, so bring it into scope with `use`. This is
  exactly `server/tests/eval.rs`, `cli/tests/integration.rs`, and `hook/tests/cli.rs`. **[VERIFIED, Rust Book
  ch11-03; repo test tree.]**
- **Doctests** — runnable ` ``` ` examples on public items; they run under `cargo test` and double as living
  documentation. **[VERIFIED, Rust Book ch11.]**
- **BDD acceptance** — Gherkin in `test/features/*.feature` with step definitions in `server/tests/bdd/*_steps.rs`,
  wired as `harness = false` `[[test]]` bins (`bdd_store`, `bdd_recall`, `bdd_consolidate`, `bdd_server`) using
  the pinned `cucumber 0.23`. cucumber-rs has **no code-gen step** — it runs the `.feature` directly at runtime.
  **[VERIFIED, `server/Cargo.toml`; `spec-test` SKILL.md; https://cucumber-rs.github.io/cucumber/main/]**

**tdd-6 (checkable).** Drive acceptance **spec-first**: **every acceptance criterion in the spec maps to one
Gherkin scenario** (`Given/When/Then`) that cucumber-rs runs via the `bdd_*` bins with
`cargo test --release --test 'bdd_*'`. The **features are the outer loop; the unit tests are the inner loop** —
a feature is not done until its scenario is green **and** the units beneath it are green. Assert the step
definitions exist (`server/tests/bdd/*_steps.rs`) before claiming a feature is testable. **[VERIFIED,
`test/features/recall.feature` (criteria 4–16); `verify-agent` SKILL.md Phase 2/4.]**

**tdd-7 (judgment).** **Test at the altitude that matches the claim.** A pure function → a unit test; a CLI or
hook **contract** (event JSON in → decision JSON out) → an integration test against the **real spawned binary**;
an acceptance criterion → a **BDD scenario**; a store-level property (superseded-serve, contradiction rate) → an
**eval-harness invariant**. Proving a binary's contract with a library-level unit test, or a store invariant with
a single scenario, tests the wrong altitude and lets real regressions through. **[INFERRED from the repo's own
split: `hook/tests/cli.rs` spawns the binary; `server/tests/eval.rs` asserts store invariants; `recall.feature`
covers acceptance.]**

---

## 4. GREEN is stricter than green tests — fmt, clippy, the lint gate

For genesis-engineer, "the tests pass" is **necessary but not sufficient** to call a Rust change GREEN.

**tdd-8 (checkable).** **GREEN for any Rust change =** `cargo fmt --check && cargo clippy --all-targets --
-D warnings && cargo test --release` — **all three exit 0**. A non-zero `cargo fmt --check` or `cargo clippy`
exit is **not GREEN** and must fail the step exactly as a failing test would. This strengthens the gate; it never
weakens it. **[VERIFIED, `spec-test` SKILL.md GREEN step; `verify-agent` SKILL.md Phase 4 "GREEN definition
(Rust)".]**

**tdd-9 (checkable).** **Keep the house lint gate intact** across all three crates: `unsafe_code = "deny"` (with
the **single** sanctioned FFI exception — registering the `sqlite-vec` extension, carrying a scoped
`#[allow(unsafe_code)]` on `VectorStore::open`), plus `unwrap_used`, `expect_used`, `panic`, and `todo` all
`= "deny"`, and clippy `pedantic = "warn"`. **Never weaken a lint, widen an `#[allow]`, or lower a threshold to
make a run pass** — fix the code. Test code may `#![allow(clippy::unwrap_used, clippy::expect_used,
clippy::panic)]` because a panic **is** the failure signal there; production code may not. **[VERIFIED,
`server/Cargo.toml` / `cli/Cargo.toml` / `hook/Cargo.toml` `[lints]`; `hook/tests/cli.rs` head allow;
`spec-crap` SKILL.md "NEVER lower a threshold to make the report pass. Fix the code."]**

---

## 5. Real dependencies, honest mocks

Determinism does not mean fake. genesis-engineer's tests run against the **real** SQLite, the **real** ONNX
embedder, and the **real** compiled binaries — and they are still deterministic.

**tdd-10 (judgment).** **Test real behavior against real dependencies; mock only when unavoidable, and never
assert on a mock.** The memory tests drive the real library against **real SQLite + real ONNX** (`Embedder::load`,
`VectorStore::open`); the hook tests **spawn the real `genesis-hook` binary** and assert its stdout. When a mock
is truly unavoidable, **mirror the complete real data structure** (a partial mock hides the fields downstream
code consumes) and **never** write an assertion that only proves the mock exists. If mock setup exceeds the test
logic, prefer an integration test with the real component. **[VERIFIED, `server/tests/eval.rs`; `hook/tests/cli.rs`;
`testing-anti-patterns.md` (anti-patterns 1, 3, 4).]**

**tdd-11 (checkable).** **Fail loud, never skip, when a required real dependency is missing.** A test that needs
the ONNX model asserts its presence and **aborts** if absent (`assert!(m.exists(), "model missing: run
node scripts/fetch-model.mjs")`) — *an eval that silently skips is worse than none.* No test may `return`/skip
quietly when a real dependency is unavailable; the absence must surface as a failure. **[VERIFIED,
`server/tests/eval.rs` `setup()`.]**

---

## 6. Coverage and CRAP change-risk gating

Coverage tells genesis-engineer **where the tests do not look**; CRAP turns coverage + complexity into a single
**change-risk** number that gates the commit.

**tdd-12 (checkable).** **Measure coverage with `cargo-llvm-cov`** — LLVM source-based coverage
(`-C instrument-coverage`, needs the `llvm-tools-preview` component) — via
`cargo llvm-cov --json --release --output-path test-results/llvm-cov.json` (`--lcov` for CI upload; `--doctests`
is unstable). **Regenerate coverage from the current `src/`; never carry a stale JSON across an implementation
change** (the 10-minute reuse window is only for back-to-back runs). **[VERIFIED,
https://github.com/taiki-e/cargo-llvm-cov; `spec-crap` SKILL.md Phase 3; `rust_crap_adapter.mjs` header.]**

**tdd-13 (checkable).** **Gate change-risk with CRAP per function.** CRAP (**Change Risk Analysis and
Predictions**, Savoia & Evans) is `CRAP(f) = CC(f)² × (1 − cov(f)/100)³ + CC(f)`, where `CC` is cyclomatic
complexity and `cov` is automated-test coverage. In this repo it is computed by
`rust-code-analysis-cli` (CC) + `cargo llvm-cov` (coverage) → `rust_crap_adapter.mjs` → `crap.mjs`, with
tiers **alert > 30**, **fail > 8**, **target ≤ 4**; the tool **exits 2 if any function has CRAP > 8**. Fix an
offender by **lowering complexity or raising coverage — never by lowering the threshold or deleting tests**.
**[VERIFIED, https://www.artima.com/weblogs/viewpost.jsp?thread=210575; `test/tools/crap.mjs`; `spec-crap`
SKILL.md "Thresholds" + "Rules".]**

**tdd-14 (principle).** **At 100% coverage CRAP reduces to CC.** Coverage cannot buy complexity risk down to
zero — a fully-covered but convoluted function still carries change risk *(the formula's designers argue exactly
this)*. So genesis-engineer keeps functions **simple *and* covered**, not merely covered; high coverage over high
complexity is still risky code. **[VERIFIED, https://www.artima.com/weblogs/viewpost.jsp?thread=210575: "As the
code coverage approaches 100%, the formula reduces to CRAP(m) = comp(m)."]**

---

## 7. Exactness — snapshots, byte-identical output, and float tolerance

genesis-engineer generates artifacts other tools consume byte-for-byte (agent `.md`, `settings.json`, decision
JSON). Those need **exact** assertions; embeddings and scores need **tolerant** ones. Both are deterministic.

**tdd-15 (checkable).** For generated artifacts and machine-read output, assert **exact / snapshot** equality, not
"looks right". Use **`insta`** (pinned `insta 1`) for snapshot tests, and rely on `serde_json`'s `preserve_order`
so emitted JSON is **byte-identical** (Rust↔Node parity), not merely semantically equal. This is why the hook
decisions are asserted field-by-field and why the plugin agents carry a **no-drift** test (§10). **[VERIFIED,
`server/Cargo.toml` dev-dep `insta = "1"`; `hook/Cargo.toml` "preserve_order so emitted decision JSON is
byte-identical"; https://insta.rs/docs/.]**

**tdd-16 (checkable).** **Compare floating-point values with `approx`, never `==`.** Embedding components,
cosine/similarity scores, and composite ranks are `f32`/`f64`; assert them with `approx` (pinned `approx 0.5`)
macros — `assert_relative_eq!`, `abs_diff_eq!`, or `assert_ulps_eq!` with an explicit epsilon/ULPs — instead of
`assert_eq!`. Exact scalar equality (`assert_eq!(retired, 1)`, ordering, ids) stays exact; only the floats get a
tolerance. **[VERIFIED, `server/Cargo.toml` dev-dep `approx = "0.5"`; https://docs.rs/approx/latest/approx/.]**

---

## 8. Proving the enforcement layer deterministic

The Genesis hooks/gates **are** the deterministic layer that makes probabilistic agents safe. Their determinism
is not assumed — it is **tested by spawning the real binary**.

**tdd-17 (principle).** **Determinism lives outside the forward pass, and the `hook` crate is where it lives.** A
gate/validate/inject decision must be a **pure function of its input**: event JSON on stdin → decision JSON on
stdout, with no dependence on the model's reasoning. genesis-engineer proves this by treating the hook as a
black box and asserting the mapping, exactly as `expertise-application.md` `ea-0` prescribes. **[VERIFIED,
`hook/tests/cli.rs` header "spawn it with a real event JSON on stdin and assert the decision JSON on stdout";
`expertise-application.md` §2.]**

**tdd-18 (checkable).** **Test every enforcement branch by spawning the real `genesis-hook`** with a crafted
event on stdin and asserting the **exact** decision JSON: `gate` **denies** a banned reasoning-trace phrase, a
credential-shaped assignment, and an over-budget write; `gate` **surfaces** rules advisorily (no
`permissionDecision`) on an authoring write; `gate` is a **dormant no-op** with no `agent_type`; `--main-agent`
makes a promoted main fire; `validate` **blocks** on an offender artifact or an undeclared required expertise;
`inject` delivers the house rules. Each branch is one assertion over the spawned binary's output. **[VERIFIED,
`hook/tests/cli.rs` — every `#[test]` there.]**

**tdd-19 (checkable).** **Enforcement code is deterministic and side-effect-free per call, and both fail
directions are tested.** Same input ⇒ same output; **no clock, RNG, network, or ambient state** in the decision
path. Gates **fail-closed** (deny) so a bug cannot silently permit a violation; a path **fails-open** only where a
miss must never break a session (e.g. `--run-hook` when the binary is unresolved), and that fail-open is tested
just as explicitly as the fail-closed. **[VERIFIED, `test/launcher.test.js` `--run-hook` fail-open vs `--run-cli`
fail-loud; `hook/tests/cli.rs` `gate_is_dormant_without_a_genesis_agent`.]**

---

## 9. A deterministic harness around non-deterministic behavior

The one thing genesis-engineer cannot make deterministic is an LLM. The move is not to test the LLM — it is to
**test the deterministic machinery the LLM drives**, with the LLM removed from the loop.

**tdd-20 (checkable).** **Wrap irreducibly non-deterministic behavior in a deterministic harness.** Where the real
system uses an LLM (Mneme's write-time structuring), the test **simulates the LLM step by calling the
deterministic primitive directly** — `structure_memory(agent, id, "semantic", subject, relation, object)` — so
there is **no LLM in the loop** and the run is reproducible. The harness then asserts the store-level
consequences the LLM's output was supposed to produce (keyed supersession, active-set filtering). **[VERIFIED,
`server/tests/eval.rs` header: "Mneme's structuring is simulated by calling `structure_memory` directly, so there
is no LLM in the loop … It is deterministic."]**

**tdd-21 (checkable).** **Assert the load-bearing invariants as hard equalities with a target of exactly 0 — and
assert, do not merely print.** The eval harness both **prints** a metrics report and **`assert_eq!`s** the
invariants: **superseded-serve-rate == 0** (a superseded fact must never be served), **active-contradiction-rate
== 0** (every `(subject, relation)` resolves to exactly one current value), and **no false supersession from
similarity** (near-identical embeddings with different subjects never supersede — deterministic keying, not
cosine). A report that only prints metrics without an assertion is **not a test**; it cannot fail CI.
**[VERIFIED, `server/tests/eval.rs` — `assert_eq!(superseded_hits, 0, …)`, `assert_eq!(contradictions, 0, …)`,
`assert_eq!(retired, 0, …)`.]**

---

## 10. Idempotence and no-drift as determinism invariants

An installer/generator that yields a different result on the second run is non-deterministic by definition.
genesis-engineer tests both properties directly.

**tdd-22 (checkable).** **Assert idempotence for every generator and installer:** a second run produces
**byte-identical** output. `assemble --main` run twice must leave `CLAUDE.md` and `settings.json` unchanged
(`assert_eq!(md1, read(&claude_md))`); a re-`bootstrap` must not duplicate the promote-offer hook
(`matches("--run-hook promote-offer").count() == 1`); `--sync` must be a no-op when the version stamp is current.
**[VERIFIED, `cli/tests/integration.rs` `assemble_main_..._idempotently`, `bootstrap_builds_self_contained_workspace`;
`test/launcher.test.js` `testSync`.]**

**tdd-23 (checkable).** **Guard committed generated artifacts with a no-drift test.** Regenerate the artifact into
a temp tree and assert **equality with the committed copy** — `build-plugin-agents` regenerated
`agents/{sensei,method,mneme}.md` must equal the committed files, or the suite fails with "run
`genesis-cli build-plugin-agents`". Drift between a source of truth and its committed derivative is a
determinism defect. **[VERIFIED, `cli/tests/integration.rs` `build_plugin_agents_matches_committed_no_drift`.]**

---

## 11. Systematic debugging and regression tests

A bug is a missing test. genesis-engineer never patches a symptom, and never fixes a bug without a test that
reproduces it.

**tdd-24 (checkable).** **Find root cause before any fix, then encode it as a red-green regression test.** Follow
the four phases — root-cause investigation (read the full error/stack, reproduce consistently, check recent
changes, trace the bad value back to its source), pattern analysis, single-hypothesis testing, implementation —
and write a **failing test that reproduces the bug first**. Prove the test is real with the **red-green-revert
cycle**: write it, see it fail, **revert the fix and confirm it fails, restore the fix and confirm it passes**;
then fix the **root cause, not the symptom**, one change at a time. **[VERIFIED, `systematic-debugging` SKILL.md
(four phases; "NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST"); `verification-before-completion` SKILL.md
"Regression tests (TDD Red-Green): Write → Run (pass) → Revert fix → Run (MUST FAIL) → Restore → Run (pass)".]**

**tdd-25 (principle).** **After three failed fixes, stop and question the architecture.** 3+ failed attempts —
each revealing a new problem elsewhere, or each demanding "massive refactoring" — signal a **wrong design, not a
wrong hypothesis**. Do not attempt fix #4; escalate the architectural question rather than thrash. **[VERIFIED,
`systematic-debugging` SKILL.md Phase 4.5 "If 3+ Fixes Failed: Question Architecture".]**

---

## 12. Verification before completion

The single most common failure mode is claiming done without evidence. For genesis-engineer this is a hard gate,
enforced by a hook.

**tdd-26 (checkable).** **No completion claim without fresh verification evidence in the same turn.** Before
saying done / fixed / passing (or any synonym or implication of success): identify the command that proves it,
**run the FULL command fresh**, read the **exit code and failure counts**, and only then state the claim **with**
that evidence. "Should pass", "I'm confident", a previous run, or an agent's self-reported success are **not**
evidence — when a delegated agent reports success, **verify independently via the VCS diff**. **[VERIFIED,
`verification-before-completion` SKILL.md "The Iron Law … NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION
EVIDENCE"; `verify-agent` SKILL.md "Never mark a verdict GREEN if the log validator returns ISSUES".]**

**tdd-27 (checkable).** **Declare the expertise you applied before finishing.** Emit
`APPLIED-EXPERTISE: test-driven-determinism#<rule-ids>` naming the **concrete rule ids** you applied — not just
the expertise name — turning "I followed the rules" into a checkable artifact the Stop hook can enforce. This is
the same cite-the-rule-ID forcing that `expertise-application.md` `ea-3` prescribes and the `validate` hook
blocks on when absent. **[VERIFIED, `expertise-application.json` `ea-3`; `hook/tests/cli.rs`
`validate_blocks_when_required_expertise_undeclared`.]**

---

## 13. Reliability metrics — state-based success and pass^k

A single green run is proof for a **deterministic** path and only a **sample** for a variable one. genesis-engineer
measures the difference.

**tdd-28 (checkable).** **Prove reliability with state-based success and `pass^k`.** Judge success by comparing the
**end state to the annotated goal state** — not by matching surface text — exactly as the eval harness compares
the resolved `active_object` to the expected current value and the recall pipeline's returned ids to the current
fact. For any path whose behavior can **vary** across runs, run **k independent trials** and report **`pass^k`**
(the fraction where **all k** trials succeed), never a single lucky pass. **[VERIFIED,
https://arxiv.org/abs/2406.12045 (τ-bench: state-based evaluation + the `pass^k` reliability metric);
`server/tests/eval.rs` (`active_object` vs expected; ids vs `new_ids`).]**

**tdd-29 (principle).** **One green run is necessary, not sufficient, for a variable path.** Inconsistency hides
behind a single pass — even strong agents are unreliable across trials (τ-bench reports `pass^8 < 25%` in
retail). So a **deterministic** path proven green once holds by construction; a **probabilistic** path must be
**measured over trials**, and reported as `pass^k`, before it can be called reliable. **[VERIFIED,
https://arxiv.org/abs/2406.12045.]**

---

## 14. Honest limits

genesis-engineer never oversells its own gates. Three limits are stated plainly so no metric is mistaken for a
proof it is not.

**tdd-30 (principle).** **Coverage measures execution, not correctness; and CC is calibrated, not physics.** A
line executed by a weak assertion is still "covered", so coverage bounds where tests look but not whether they
check the right thing. And this repo's cyclomatic complexity comes from `rust-code-analysis` (tree-sitter), whose
count is **close but not bit-identical** to radon's, on which the CRAP thresholds were calibrated — so treat the
`> 8` fail line as a **calibrated gate to re-validate on the real crate**, not an absolute physical constant. A
passing suite proves only **what the tests assert**. **[VERIFIED, `spec-crap` SKILL.md ("not bit-identical to
radon's … re-validate its absolute feel"); CRAP formula source.]**

**tdd-31 (principle).** **Gates and tests verify form and asserted behavior, not intent — the semantic remainder
needs an independent, spec-grounded reviewer.** A green suite and a passing gate cannot prove a requirement was
*met*, only that the checks it encodes hold; a schema can force an `APPLIED-EXPERTISE` token without proving the
rule was truly applied (`ea-11`). So genesis-engineer **re-reads the spec line-by-line** against the work
(requirements are met by a checklist, not by "tests pass"), and routes the irreducibly-semantic judgment to an
**independent reviewer grounded on the spec** — never self-certifying "green" as done. **[VERIFIED,
`verification-before-completion` SKILL.md ("Requirements met → line-by-line checklist"); `expertise-application.md`
`ea-8`/`ea-11`.]**

---

## Commands and thresholds table (the deterministic toolbox — pinned to this repo)

- **Toolchain:** `rust-toolchain.toml` channel **1.93.0**, components `rustfmt` + `clippy`. **[VERIFIED]**
- **Full test run:** `cargo test --release` (compiles + runs lib/unit and the `bdd_*` harness bins together —
  split the counts by bin when reporting). **[VERIFIED, `spec-test` SKILL.md.]**
- **BDD only:** `cargo test --release --test 'bdd_*'` (cucumber-rs `0.23`, no bddgen). **[VERIFIED]**
- **GREEN gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release`.
  **[VERIFIED]**
- **Coverage:** `cargo llvm-cov --json --release --output-path test-results/llvm-cov.json` (`--lcov`/`--codecov`
  for CI; `--fail-under-lines <MIN>` / `--fail-under-functions <MIN>` to gate a floor; `--doctests` unstable).
  **[VERIFIED, cargo-llvm-cov docs.]**
- **CRAP gate:** `node test/tools/rust_crap_adapter.mjs` (→ `crap.mjs`), formula `CC²·(1−cov/100)³ + CC`, tiers
  **alert > 30 · fail > 8 · target ≤ 4**, **exit 2 if any function > 8**. **[VERIFIED]**
- **Test crates (pinned dev-deps):** `cucumber 0.23` (BDD) · `assert_cmd 2.2` (CLI E2E: `Command::cargo_bin(..)
  .assert().success()`) · `approx 0.5` (float assertions) · `insta 1` (snapshots) · `tempfile 3` (isolated temp
  dirs — never write into the repo tree). **[VERIFIED, the three `Cargo.toml`.]**
- **Invariant targets:** superseded-serve-rate **= 0** · active-contradiction-rate **= 0** · reliability reported
  as **`pass^k`** over k trials. **[VERIFIED, `server/tests/eval.rs`; τ-bench.]**

---

## Source ledger

**Primary external sources (read in full, 2026-08-15):**
Three Laws of TDD — Robert C. Martin, http://butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd ·
CRAP metric — Alberto Savoia & Bob Evans, "Pardon My French, But This Code Is C.R.A.P. (2)",
https://www.artima.com/weblogs/viewpost.jsp?thread=210575 (formula + `crap4j`) ·
Rust Book testing — https://doc.rust-lang.org/book/ch11-01-writing-tests.html and
https://doc.rust-lang.org/book/ch11-03-test-organization.html (unit `#[cfg(test)]`, integration `tests/` as
separate crates, doctests, `#[should_panic]`) · `cargo-llvm-cov` — https://github.com/taiki-e/cargo-llvm-cov
(`-C instrument-coverage`, `llvm-tools-preview`, `--json`/`--lcov`, `--fail-under-*`, `--doctests`) ·
`cucumber-rs` — https://cucumber-rs.github.io/cucumber/main/ (`Given/When/Then`, `#[derive(World)]`) ·
`insta` — https://insta.rs/docs/ (Rust snapshot testing) · `assert_cmd` 2.2.2 —
https://docs.rs/assert_cmd/latest/assert_cmd/ (`Command::cargo_bin`, explicit `.success()`) · `approx` 0.5.1 —
https://docs.rs/approx/latest/approx/ (`assert_relative_eq!`, `abs_diff_eq!`, `assert_ulps_eq!`) ·
`pass^k` + state-based evaluation — τ-bench, https://arxiv.org/abs/2406.12045.

**Primary repo sources (read in full):** `rust-toolchain.toml` · `server/Cargo.toml`, `cli/Cargo.toml`,
`hook/Cargo.toml` (pinned dev-deps + `[lints]`) · `server/tests/eval.rs` (deterministic eval harness) ·
`hook/tests/cli.rs` (spawn-the-binary enforcement tests) · `cli/tests/integration.rs` (idempotence + no-drift) ·
`test/launcher.test.js` (fail-open/fail-loud, Node test style) · `test/features/recall.feature` (acceptance
criteria) · `test/tools/crap.mjs` + `test/tools/rust_crap_adapter.mjs` (CRAP formula/thresholds) · skills
`spec-test`, `spec-crap`, `test-driven-development` (+ `testing-anti-patterns.md`), `verification-before-completion`,
`systematic-debugging`, `verify-agent` / `forge-verify-agent` · `expertise-application.md` / `.json`
(`ea-0`, `ea-3`, `ea-8`, `ea-11`).

*Colophon: v1, 2026-08-15. Authored with zero shortcuts from the primary TDD/coverage/reliability sources and the
pinned Genesis repo tooling; 32 rules (`tdd-0`…`tdd-31`); every claim labelled [VERIFIED] or [INFERRED]; house
rules (no credential value, no banned reasoning-trace phrase) honored throughout.*
