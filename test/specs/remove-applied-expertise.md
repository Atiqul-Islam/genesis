# Feature: Remove the APPLIED-EXPERTISE declaration mechanism (issue #39)

## Source
- Issue #39 (developer-directed): the APPLIED-EXPERTISE declaration gate enforces CITATION not
  APPLICATION — high friction, no value. Remove it completely.
- Keep the deterministic HOUSE rules (≤20-word bullets, banned phrase, credentials, line budget, no-grep).

## Feature
Remove the per-turn APPLIED-EXPERTISE declaration requirement and everything that enforces/surfaces it,
while leaving the independent deterministic house-rule checks intact.

## Expected behavior
- The Stop validator **no longer** parses, counts, or verifies APPLIED-EXPERTISE declarations.
- A turn is **never blocked** for missing/invalid declarations.
- The `expertise-warn` skip-warning hook is removed from the wiring (no `expertise-warnings.md`).
- No agent persona/behavior instructs emitting APPLIED-EXPERTISE.
- `.genesis/applied-expertise.jsonl` is neither required nor written by the toolchain.
- The deterministic house rules still block exactly as before (no behavior change there).

## Acceptance criteria
- AC1: `validate` returns no "did not credibly declare" / declaration-floor / verbatim-quote reasons.
- AC2: the other house-rule checks (bullets, banned phrase, credentials, line budget) pass identically — drift test.
- AC3: `expertise-warn` is unwired; `main.rs` no longer dispatches it.
- AC4: generated agent persona/behavior contain no "emit APPLIED-EXPERTISE" directive.
- AC5: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release` all exit 0.
- AC6: tests updated — a test asserts the declaration gate is gone (fail-before/pass-after).

## Constraints
- Never weaken a house rule; keep the gate intact.
- Cross-platform; fail-open.
- Version bump in lockstep: `.claude-plugin/plugin.json` + `bin/genesis-memory.js` RELEASE_VERSION (som-9).

## Out of scope
- The LLM application-verification reviewer (the replacement path) — tracked separately (#30).

## References
- `hook/src/validate.rs`, `hook/src/expertise_warn.rs`, `hook/src/main.rs`, `hook/src/lib.rs`,
  `cli/src/render.rs`. Related: #29, #30.
