## Feature: Agent-scoped guards + `/genesis:update_guard <agent_name>`

A **guard** is a per-agent set of invariants that protect that agent's own defining files from
drift — regexes that MUST match (or must NOT match) in named files — plus a **self-protect** list
of files the agent may not edit. Today Genesis enforces only GLOBAL house rules uniformly (`gate.rs`):
banned phrase, credential shape, and the line budget, identical for every agent. There is no
per-agent guard in shipped Genesis; that concept only exists as a bespoke, hand-written hook outside
Genesis. This feature **reuses the existing guard** that already protects the maintainer's agents — the
`protected_core.json` shape (a `self_protect` list + `must_match` / `must_not_match` invariants over
named files) — rather than inventing a new format. It only makes that same guard **agent-scoped** (one
per agent instead of one shared file) and adds a coordinator command that lets Sensei create or upgrade
**any** agent's guard through a reviewed, tested flow.

> Deliberate deviation (spec-driven sdd-11): the existing guard is enforced by a Python script
> (`protect.py`). Genesis deliberately ships NO runtime Python (a CI gate asserts it), so the identical
> guard RULES are enforced through Genesis's existing Rust PreToolUse gate (`gate.rs`) instead — same
> data, same behavior, no new Python. If the maintainer instead wants the Python `protect.py` shipped
> per-agent, that overrides this deviation.

> Before this ships: a guard BLOCKS file edits, so it is a real enforcement control. I will not turn the
> blocking on until the maintainer has approved the guard model below — that approval is the only gate.

### Expected Behavior

- Each agent may have its own guard, independent of every other agent's guard.
- When an agent is active and about to Write/Edit a file, a change that would VIOLATE that agent's
  guard is blocked with a clear, invariant-named reason; a change that SATISFIES the guard proceeds.
- An agent cannot edit its own guard file — only the coordinator flow can (self-protect).
- One agent's guard never constrains a different agent (strictly scoped to the active agent).
- `/genesis:update_guard <agent_name>` lets Sensei create or upgrade the named agent's guard.
- An agent with no guard behaves exactly as today — guards are additive; absence adds no constraint.

### Acceptance Criteria

- **AC1** — Agent A active + a Write to A's protected file that DROPS a `must_match` invariant → gate
  DENIES, and the reason names that invariant's id.
- **AC2** — Agent A active + a Write to A's protected file that KEEPS every invariant → gate ALLOWS.
- **AC3** — Agent A active + a Write/Edit to A's own `guard.json` → gate DENIES (self-protect).
- **AC4** — Agent B active + a Write that would violate A's guard → gate ALLOWS (guard is scoped to the
  active agent only).
- **AC5** — An agent with no guard file → gate produces the identical decision to the pre-feature gate
  (no new denials on any input).
- **AC6** — `genesis-cli update-guard <agent> …` writes/updates `<repo>/.genesis/team/<agent>/guard.json`
  ONLY when the result is well-formed (valid JSON; `self_protect` present; every invariant carries an id
  and at least one of `must_match`/`must_not_match`); a malformed result is REJECTED, not written.

### Implementation Requirements

- A guard is `<repo>/.genesis/team/<agent>/guard.json`, using the SAME schema as the existing
  `protected_core.json` guard (reused, not reinvented):
  `{ "self_protect": [<paths>], "invariants": [ {"id", "files":[<paths>], "must_match"?, "must_not_match"?, "why"} ] }`.
  Per-agent location is the only structural change from the existing single-file guard.
- `gate.rs` loads the ACTIVE agent's guard (via `agent::resolve_agent` + `agent::runtime_dir`). For a
  Write/Edit whose `file_path` matches an invariant's `files`, it evaluates the invariant against the
  PROPOSED post-write content in `tool_input` (`content`, else `new_string`) and DENIES on a failed
  `must_match` or a matched `must_not_match`. It DENIES any Write/Edit whose `file_path` is in
  `self_protect`. The existing global checks (banned phrase / credential / budget) are UNCHANGED and run
  FIRST.
- FAIL-OPEN on a missing/unreadable/malformed guard (parity with the hooks' fail-open posture): a broken
  guard never blocks a session. Well-formedness is enforced at write time by `genesis-cli update-guard`,
  never by the gate.
- *(ratified choice — exact edit interface to be pinned at first implementation)* New
  `genesis-cli update-guard <agent> …` subcommand: reads a guard spec, validates it (AC6), and writes
  `guard.json` atomically; wired into `cli/src/main.rs`.
- New plugin command `commands/update_guard.md`, Sensei-facing, calling
  `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli update-guard "$ARGUMENTS"`, modeled on
  `commands/promote.md`.
- Determinism guards: `update-guard` run twice with the same spec is byte-identical (idempotent), and a
  committed guard round-trips (no-drift) — asserted as tests per test-driven-determinism tdd-22/tdd-23.
