## Feature: Quiet expertise declarations (verbose off by default)

Genesis agents declare `APPLIED-EXPERTISE:` lines each turn so the Stop validator can verify
they applied their required expertise. Today those lines live in the agent's **visible chat
prose** — `validate.rs::parse_declarations` scans assistant `text` blocks for them — so every
turn ends with a wall of declaration lines the user must read. This feature moves the
declarations to a non-visible **record channel** by default, keeps the enforcement and the
logging fully active, and lets the maintainer turn the visible display back on **per agent**.

The functionality never changes — only whether the declarations are shown. Enforcement stays
on; every accepted declaration is still logged.

### Expected Behavior

- By default, an agent's `APPLIED-EXPERTISE:` declarations do NOT appear in its visible chat output.
- The Stop validator still enforces declarations exactly as before: an agent that fails to declare
  its required expertise (real rule-ids, the coverage floor, valid evidence) is still blocked.
- Every accepted declaration is recorded to a durable, per-repo audit log — displayed or not.
- `/genesis:verbose_activate <agent_name>` makes that agent's declarations appear in visible prose again.
- `/genesis:verbose_deactivate <agent_name>` returns that agent to quiet (no visible declarations).
- Verbose is OFF for every agent by default (no prior activation is needed to be quiet).
- Turning verbose on/off changes ONLY what is displayed; enforcement and audit logging are identical
  in both states.

### Acceptance Criteria

- **AC1** — With no verbose flag set, a turn that records its declarations only to the record channel
  (no `APPLIED-EXPERTISE:` prose) is ALLOWED to finish by validate.
- **AC2** — A turn that neither records nor prints the required declarations is still BLOCKED, with the
  same reason text as today ("You did not credibly declare applying '<name>'…").
- **AC3** — After an ALLOWED Stop, each accepted `<name>#<rule-id>` appears as one JSONL record in the
  audit log `<repo>/.genesis/applied-expertise.log.jsonl`.
- **AC4** — With verbose ON for agent X, the inject hook's instruction tells X to ALSO print its
  declarations in prose; with verbose OFF, the instruction tells X to record-only.
- **AC5** — `genesis-cli verbose on <agent>` creates the flag and `verbose off <agent>` removes it; each
  is idempotent (a second identical run is a byte-identical no-op).
- **AC6** — For the same declarations, a quiet agent and a verbose agent produce the SAME validate
  decision (allow or block) — display state never alters the enforcement result.

### Implementation Requirements

- *(ratified choice — unsourced; rationale: reuse the validator's existing, robust turn-scoping instead
  of inventing a second one)* Declarations are recorded by the agent writing them to
  `<repo>/.genesis/applied-expertise.jsonl` via a **Write/Edit** tool call. `validate.rs::parse_declarations`
  is extended to also extract `APPLIED-EXPERTISE:` lines from the CURRENT turn's Write/Edit `tool_use`
  input whose `file_path` ends with `applied-expertise.jsonl`, unioned with the assistant `text` blocks.
  The turn boundary (records after the last genuine human message) is UNCHANGED.
- *(ratified choice)* The per-agent verbose flag is the file `<repo>/.genesis/verbose/<agent>.json`
  (present with `{"verbose":true}` = on; absent = off). Default absent → quiet.
- *(ratified choice)* The audit log `<repo>/.genesis/applied-expertise.log.jsonl` is append-only JSONL,
  one record per accepted citation `{ts, agent, name, rule_id, evidence, session}`, written by
  `validate.rs` on an ALLOW decision via `io::append_log`.
- `inject.rs::build_required` reads the active agent's verbose flag and emits either the record-only
  instruction (default) or the also-print-in-prose instruction (verbose on). The record-only wording
  names the exact record path and format.
- New `genesis-cli verbose <on|off> <agent>` subcommand sets/clears the flag file deterministically and
  idempotently; wired into `cli/src/main.rs` dispatch.
- New plugin commands `commands/verbose_activate.md` and `commands/verbose_deactivate.md`, modeled on
  `commands/promote.md`, calling `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli verbose on|off "$ARGUMENTS"`.
- NO change to `FLOOR` (3), the manifest citation-integrity check, the coverage-floor check, or the
  evidence spot-check. Enforcement is byte-for-byte identical; only the SOURCE of the declaration lines
  and the DISPLAY instruction change.
