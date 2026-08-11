---
name: memory-sync
description: The deterministic, lossless procedure for keeping a Genesis agent's memory in the repo and updating the remote with it. Use when the user says to update/push the remote repo (memory), or when memory seems missing, scattered, or out of sync. Usable by the Genesis build agents, any built agent, or the main Claude.
---

# Keeping memory in the repo and syncing it to the remote

Genesis memory obeys ONE invariant: **all of an agent's memory lives in its repo's `.genesis/` store** and
travels with the repo. Memory is committed as BOTH artifacts:

| Artifact | Path | Committed? | Role |
|---|---|---|---|
| Vector DB | `<repo>/.genesis/memory.db` | **Yes** | Ready-to-use store (rows + 384-dim embeddings). On sync, git takes the latest. |
| JSONL mirror | `<repo>/.genesis/memory/memory.jsonl` | **Yes** | Diff-friendly **merge substrate** — the safety net that guarantees no memory is lost. |

The memory server auto-exports the DB → JSONL after every `store`/`consolidate` and on shutdown, and on
startup it UNION-merges the JSONL back into the DB. So the `.db` self-heals from the JSONL: even if git takes
a `.db` that is behind, the next server start folds in everything the JSONL holds.

## Updating the remote (the "update remote repo" procedure)

This is the deterministic flow — run it via **`/genesis:sync`** (or the steps below). It commits BOTH
artifacts, reconciles with the remote losslessly, and pushes; it consults the user ONLY on a genuine git
conflict.

1. **Commit both** `.genesis/memory.db` and `.genesis/memory/memory.jsonl`.
2. **Fetch** the remote and read its JSONL (`git show <upstream>:.genesis/memory/memory.jsonl`).
3. **Reconcile** local vs incoming — using the JSONL to decide BEFORE trusting the `.db`:
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli reconcile --repo "${CLAUDE_PROJECT_DIR}" --incoming <remote.jsonl>
   ```
   - **add-only** (`local ⊆ incoming`) → remote only added memories; replacing the `.db` loses nothing.
   - **already-synced** → identical; nothing changes.
   - **merged** → local held memories the remote lacked; the two are UNION-merged so **local-only memories
     survive** (a plain replace would have dropped them).
   The reconcile always writes the lossless UNION to the canonical JSONL; the `.db` catches up on next start.
4. **Commit** the reconciled artifacts and **push**.
5. **Conflict → consult the user.** The reconcile itself never loses memory, so the ONLY case needing a human
   is a genuine git conflict (JSONL left with `<<<<<<<` markers, unrelated histories, or a push it can't
   resolve). Never force-push and never discard either side — stop and ask.

## The deterministic merge (what "lossless" means)

`reconcile` (and the server's startup sync, and `fix`) all use the same rule — a **UNION keyed by
`(agent_id, text)`**:

- **Dedupe by content.** The same memory is never stored twice.
- **Never overwrite, never drop.** Every distinct memory from every source is kept.
- **Idempotent + stable.** The result is sorted by `(agent_id, created_at, text)` and re-ided `1..N`, so the
  same content always yields byte-identical JSONL (clean diffs; re-running is a no-op).
- **`superseded_by` is cleared** on a cross-store union (ids are rewritten) — the memory text is always kept.

## Diagnose + repair scattered memory (older workspaces)

Before the anchoring fix, a mis-configured server could write a stray `genesis-memory.db` in the launch
directory instead of the repo. To find and fix that:

- **`/genesis:doctor`** (`--run-cli doctor --root <repo>`) — READ-ONLY: reports every memory DB (canonical +
  strays) with per-agent counts and a `healthy` flag.
- **`/genesis:fix`** (`--run-cli fix --into <repo>`) — reads strays READ-ONLY and UNION-merges them into the
  repo's canonical JSONL (idempotent; zero footprint outside the target repo; `--archive` copies strays for
  safekeeping). The `.db` catches up on the next server start.

New installs no longer scatter: the server defaults its DB to `<cwd>/.genesis/memory.db`, and the plugin /
bootstrap `.mcp.json` set `GENESIS_MEMORY_DB`/`GENESIS_MEMORY_EXPORT` to the repo's `.genesis/` paths.
