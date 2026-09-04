# Feature: vector-based completeness WARNING for expertise declarations

## Context / Problem

The validate (Stop) hook enforces that DECLARED rules are real and evidenced, but it cannot tell whether the
agent SKIPPED a rule its response actually engaged (that is a judgment; deterministic code can't infer it).
We want a cheap, deterministic, **warn-only** nudge — no LLM — that surfaces rules the response looks like it
used but did not declare, using the embedder Genesis already ships (the ONNX memory server).

Honest scope: this is a **heuristic assist, not a guarantee**. Vector similarity ≈ applicability only
loosely (`memory-management.md` mm-35: cosine is ≈ chance for this class of judgment), so it WARNS, never
blocks, and its thresholds are calibration items (mm-26), never blind constants.

## Ratified design (how it reaches the embedder without slowing Stop)

The spike (`scratchpad/spike.mjs`, recorded 2026-09-04) confirmed the memory server's cosine ranking puts a
response's used-topic rules among its nearest neighbours (TESTING→tdd 0.90–0.98, MEMORY→mm 1.13–1.18,
RELEASE→som 1.12–1.25). It also confirmed the embedder already runs **warm** as the session's MCP server, and
that a cold one-shot spawn is ~1.3s.

Therefore the Stop path does NOT embed inline (which would slow finish) and does NOT need a precomputed vector
sidecar. Instead:

- The Stop hook entry `genesis-hook expertise-warn` is a fast **spawner**: it reads the Stop event, launches a
  **detached background worker**, and exits 0 immediately (zero added Stop latency, never blocks).
- The **worker** (`genesis-hook expertise-warn --worker …`) spawns its OWN memory server via the Node launcher
  (`node bin/genesis-memory.js`) pointed at an isolated temp DB, stores each required rule's text, recalls the
  response, runs the deterministic top-k + margin scorer, and writes `<repo>/.genesis/expertise-warnings.md`.
- The next **SessionStart** (`genesis-hook inject`) surfaces that file's contents into context and DELETES it
  (surface-once), so the agent sees "rules you may have skipped last turn" and can cite them.

## Expected Behavior

1. After a turn, the background worker compares the agent's **response text** against each rule of the agent's
   required expertise, by embedding similarity (recall against the memory server).
2. Undeclared rules that are among the response's nearest neighbours AND within a margin of the closest are
   **candidates** (likely engaged but not declared).
3. Each candidate is listed in a **WARNING** file for the next SessionStart to surface.
4. The warning **never blocks** the turn — the spawner always exits 0; the worker only writes an advisory file.
5. The existing deterministic checks (real id, evidence-is-verbatim-quote, floor) are unchanged and still block.

## Acceptance Criteria

- AC1: Given response→rule distances where an undeclared rule is within top-k and margin of the nearest, the
  scorer names it as a warning. (unit: `skip_warnings`)
- AC2: A response whose near rules are all declared yields NO warning. (unit: `skip_warnings`)
- AC3: The warning never blocks — the spawner exits 0 and writes no `decision: block`, under any input.
- AC4: Embedder/launcher unavailable, empty response, or any embed error → no warning file written / a stale
  one is cleared, turn finishes (fail-open).
- AC5: The Stop path stays fast: the spawner DETACHES the worker and returns immediately; the heavy embed runs
  in the background, not on the blocking Stop path.
- AC6: The top-k and margin are surfaced as parameters (calibration items, mm-26), not hidden magic constants.
- AC7: `inject` (SessionStart) surfaces the warnings file into context and DELETES it (surface-once); a
  missing/empty file injects nothing.
- AC8: A promoted main's `.claude/settings.json` wires the `expertise-warn` Stop command (propagation), and
  `demote` removes it with the rest of the agent's main-thread hooks.

## Implementation Requirements

- **No inline embed on Stop.** The spawner reads the event, detaches the worker (new process group / detached
  process, null stdio), and exits 0. (ratified — Stop must stay fast; the embedder is reached in background.)
- **Worker uses the shipped launcher.** `node bin/genesis-memory.js` resolves the cached server binary + ONNX
  model; the worker points it at a temp `GENESIS_MEMORY_DB` so it never pollutes the real store.
- **Deterministic scorer.** `skip_warnings(distances, declared, k, margin)` is pure and unit-tested; same
  inputs → same warnings. No LLM call.
- **Thresholds are parameters** (`WARN_K`, `WARN_MARGIN`), documented as calibration items (mm-26); ship
  defaults confirmed reasonable by the spike, keep them tunable, never hardcode blindly as sacred.
- **Warn surface:** the worker writes `<repo>/.genesis/expertise-warnings.md`; `inject` surfaces it once and
  deletes it. The spawner MUST NOT set `decision: block`.
- **Fail-open everywhere:** any launcher/embed/IO error → no file (or clear stale), never block, never break
  Stop or SessionStart.
- **Propagation:** wired into `render.rs main_thread_hooks` so every promoted repo gets it on
  promote / update (sync-settings); removed by `demote_settings`.

## Risks / spike-first (resolved)

- Can the Stop path reach the embedder without slowing finish? — YES via background detach; the warm embedder
  already runs, and even a cold one-shot (~1.3s) is off the blocking path.
- Does ANY cut actually separate "engaged" from "not"? — YES; the spike showed used-topic rules are the
  response's nearest neighbours, so a top-k + margin cut over the recall distances separates skips from noise.
