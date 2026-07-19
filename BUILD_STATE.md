# Genesis — build state (resume point)

Genesis source lives here: `~/Downloads/genesis` (WSL: `/mnt/c/Users/iatiq/Downloads/genesis`).
It is installed + used in the repo `48hr-freelancing-sprint` (agents land in that repo's `.claude/agents/`).
Full design + decisions: project memory `genesis-design.md`.

## Built + TESTED ✅
- **Expertise store** (`expertise/`) — the 5 verified reports, decoupled + versioned.
- **Sensei** (`team/sensei/persona.md` + `behavior.md`) — coordinator persona. Tests: **7/7 + adversarial held**.
- **Method** (`team/method/persona.md` + `behavior.md`) — craftsman persona. Tests: **6/6 + adversarial held**.
  - One weakness caught by harsh grading (S3 memory-wiring ambiguity) → fixed in behavior.md step 8.
- **Deterministic hooks** (`hooks/`, Python, no jq):
  - `gate.py` (PreToolUse Write|Edit) — blocks banned phrase / credential / >200-line file. **7/7**.
  - `validate.py` (Stop) — refuses to finish while a checkable rule is violated. **tested (block/allow/loop-guard)**.
  - `inject.py` (SessionStart) — delivers house rules + expertise pointers, under 10k cap. **tested**.
- **Assembler** (`install/assemble.py`) — clean source → `.claude/agents/<name>.md`. Enforces boundaries at the
  TOOL layer (Method has no `Agent` tool) and wires the inject/gate/validate hooks into frontmatter (verified
  YAML shape). Both agents assembled + frontmatter parses as valid YAML.

## Orchestration ✅ VERIFIED (2026-07-19)
- `team/sensei/skills/build-agent/SKILL.md` — the mechanizable build procedure (interview→verify→plan→
  delegate→retry/escalate→assemble→wire→install→verify), with TASK-SPEC + RESULT schemas and the teams
  error/retry/escalation/budget policy. Installed to the repo's `.claude/skills/` + wired via frontmatter.
- Assembler upgraded: installs an agent's skills into `.claude/skills/` + lists them in frontmatter; and
  reads a built agent's own `meta.json` (description + tools) instead of hardcoded metadata.
- **END-TO-END TEST PASSED:** Sensei spawned Method → Method authored `echo` (persona/behavior/meta/tests,
  test-first) → **26/26 tests pass** (Sensei re-ran independently) → assembled + installed `.claude/agents/
  echo.md` (valid YAML, hooks wired, no tools). Native `sensei`/`method`/`echo` agents register after a
  short refresh (no manual restart needed).

## Also done ✅
- **Live-model test** of echo: 3/3 pass (verbatim echo, ignores injection, no destructive action, no tool).
- **Installer** (`install/install.py`) — one command installs Sensei+Method+skills into any repo. Tested
  into a throwaway repo: valid frontmatter, flags the unbuilt server.

## Spec-forge Rust workflow ✅ BUILT + VERIFIED (2026-07-19)
- Copied the full `/spec-forge` family + docs into `.claude/skills/` (20 skill dirs). §3.1 files verbatim.
- CRAP gate: `crap.py` copied VERBATIM (30/8/4, exit-2 intact) + `test/tools/rust_crap_adapter.py`
  (rust-code-analysis + cargo-llvm-cov JSON → radon-cc.json + coverage.json → crap.py). Adapter
  **smoke-tested**: closure-skip, method/free-fn keying, innermost-span join, exit-2 on CRAP>8 all pass.
- Server scaffold: `server/Cargo.toml` (§2.2 stack, parses, NO dangling [[test]]) + 5 flat `src/*.rs`
  stubs (`unimplemented!("Implement via TDD")`, only std/anyhow/tokio in bodies) + `tests/{bdd,golden}/`.
  New `spec-scaffold/SKILL.md` (skip-if-exists).
- Edits E-0..E-9 applied + verified (7-gate sweep): language detect (E-0); verify-agent Rust branch
  + strengthened GREEN fmt+clippy+test (E-1); forge-verify tools (E-2); spec-test (E-3); spec-crap
  swap (E-4); forge-review swap + Genesis accuracy check (E-5); spec-compile/create/agent Rust stubs
  + [[test]] harness=false wiring (E-6); dev-agent Rust idioms, size-limits kept (E-7); simplify globs
  (E-8); timeline forge actors (E-9). Routing-table + handoff-schema UNTOUCHED. Phase order 0→11 intact.

## Remaining ⬜
- **Two run preconditions:** (1) `git init` genesis (Phase 0a worktree / Phase 9 SHAs / Phase 11 need git);
  (2) `cargo install rust-code-analysis-cli` (CRAP CC input; cargo-llvm-cov already present).
- **Rust memory server** (`server/`) — scaffolded; now BUILD IT spec-driven by running `/spec-forge`.
  (Native per-subagent `memory` field is the interim store.)
- **Starter templates** (`templates/`) — coder / reviewer / researcher.
- **Cross-platform paths** — hook commands use absolute /mnt/c paths; parametrize `GENESIS_HOME` for Windows.

## Verified facts driving the build
- Subagents: own context window; own frontmatter hooks (all events); share the main cwd; resumable via SendMessage.
- Native per-subagent `memory` field exists (`.claude/agent-memory/<name>/`, MEMORY.md) — complements the Rust server.
- Foundation = subagents + SendMessage (Agent Teams is experimental/off-by-default).
