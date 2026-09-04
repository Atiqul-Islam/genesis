# Phase C — system-wide retro-learn sweep

Part of Feature 2 ([[memory-expertise-redesign]]). Phases A+B make NEW turns learnable. Phase C backfills
the same analysis over history that predates the loop, across every reachable repo — on demand, report
first, nothing written without the user's per-item approval.

## Expected behavior
`/genesis:retro-learn [scope]` — the main agent (coordinator) drives it; Mneme analyzes; the user approves.
1. Enumerate `.genesis` repos within a user-chosen scan scope (default: user home; or whole system).
2. Per repo/agent, read the CAPTURED history (`.genesis/sessions/*.jsonl`, `resume-state.md`, the memory
   store, and this machine's `~/.claude/projects/<encode(cwd)>` transcripts).
3. Mneme proposes durable expertise rules (redacted); dedups against existing rules/facts.
4. Emit ONE report (grouped repo → agent → candidate → any contradiction) — nothing written yet.
5. The user approves per item / per repo; each approval applies via `genesis-cli expertise-learn`
   (`add --status active`), bi-temporal + reversible. A learned rule stays in its ORIGIN repo unless the
   user explicitly propagates it.

## Acceptance criteria
- C1: Read-only until approval — the sweep writes nothing to any store, and commits/pushes nothing.
- C2: Discovery is bounded to the chosen scan scope; repos outside it are not touched.
- C3: Each applied approval calls `expertise-learn` (so it is enforced in exactly that repo thereafter).
- C4: Credentials are never written (redaction on ingest); no cross-repo write without explicit approval.
- C5: Re-running is safe — candidates already learned dedup out (no duplicates).

## Honest scope limits (stated to the user up front)
- No global registry: reaches only `.genesis` repos discoverable ON THIS MACHINE within the scan scope.
- Learns only from CAPTURED conversation (`.genesis/sessions`, `resume-state.md`) or transcripts still in
  `~/.claude/projects` (Claude Code may have rotated older ones).
- `~/.claude/projects` transcripts are this-machine-only; a repo used mainly elsewhere yields little here.
- The candidate JUDGMENT is non-deterministic; determinism/idempotence hold at the WRITE layer
  (content dedup + user approval), not at detection.

## Implementation
- A slash command `commands/retro-learn.md` (procedure; reuses `expertise-learn` + the report pattern);
  no new enforcement code. Coordinator-driven (Mneme has no Agent/SendMessage — it proposes, the main
  agent drives the approved writes).
