# Phase B — Mneme reflection loop (learn → ask → approve → enforce)

Part of Feature 2 ([[memory-expertise-redesign]]). Phase A made `learned.jsonl` active rows enforce via
`expertise.db`. Phase B is how rules GET into `learned.jsonl`: after each turn Mneme proposes; the user
approves; an approved rule becomes enforced.

## Architecture (deterministic spine, one LLM judgment step)
- **`cli expertise-learn`** — the ONLY writer of `learned.jsonl`; deterministic; re-migrates after each
  write so the DB (and enforcement) reflects it. Never runs an LLM.
- **`hook reflect-surface`** (UserPromptSubmit, command) — if `.genesis/mneme/proposals/pending.jsonl` is
  non-empty, inject it so the MAIN agent presents the proposals to the user (Mneme has no SendMessage).
- **Mneme reflection** (Stop/SubagentStop, `type: agent`) — runs Mneme with a reflection prompt: read the
  turn, and if there is a durable lesson, either write it directly (`expertise-learn add --status active`)
  when the user explicitly said "memorize/remember", or queue a proposal (`--status proposed`) otherwise.
  Non-blocking, fail-open, redacted; the judgment is the one non-deterministic step.

## `expertise-learn` interface
```
genesis-cli expertise-learn <root> add --expertise <name> --text <t> [--id <id>] [--type <ty>]
                            [--status active|proposed] [--agents a,b] [--scope global|task:<slug>]
genesis-cli expertise-learn <root> set-status --expertise <name> --id <id> --status <active|rejected|retired>
```
- `add` appends a learned row to `<root>/learned.jsonl`; if `--id` is omitted, allocate the next id in the
  bucket (`<prefix>-<1+max numeric suffix over manifest + learned rows>`, matching the `[a-z]+-[0-9]+`
  declaration regex, never reused). Default `--type judgment`, `--status proposed` (autonomous), `--scope
  global`. Then re-migrate `expertise.db`.
- `set-status` flips an existing learned row's status (approve `proposed→active`, `reject`, `retire`), then
  re-migrates. A row is matched by `(expertise, id)`.

## Acceptance criteria
- B1: `add --status active` appends a row and, after re-migrate, the rule is in the DB active set (enforced).
- B2: `add --status proposed` appends a row that is NOT in the active set (not enforced until approved).
- B3: `add` without `--id` allocates a fresh, non-colliding id matching `[a-z]+-[0-9]+`, > every existing
  numeric suffix in the bucket (manifest + learned); re-running with the same text does not duplicate.
- B4: `set-status --status active` on a proposed row makes it enforced; `retire` removes it from active.
- B5: `--agents a,b` attaches the learned bucket to those agents' required set (so it is enforced for them).
- B6: `reflect-surface` injects pending proposals at UserPromptSubmit; empty/missing queue injects nothing.
- B7: every `expertise-learn` write is idempotent on `(expertise, id)` — a repeat updates in place, never
  double-appends; credentials are never written (reuse the redaction shape).

## Non-goals (Phase C / later)
- The Mneme LLM judgment quality (prompt-tuned, not unit-tested here).
- Cross-repo retro-learn sweep (Phase C).
- Contradiction auto-detection accuracy (surfaced to the user; the write mechanics are deterministic here).
