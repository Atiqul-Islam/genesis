---
description: Consolidate this repo's scattered Genesis memory into its canonical .genesis/ store — lossless, deterministic, idempotent. Strays are read-only; the only write is the repo's memory JSONL.
argument-hint: (no arguments) — add --archive to also keep a copy of each stray
---

You are running **`/genesis:fix`** — consolidate any scattered memory into this repo's canonical store. It
reads every stray memory database READ-ONLY and folds their memories, together with the repo's existing
store, into `<repo>/.genesis/memory/memory.jsonl` via a **lossless UNION** (dedupe by `(agent_id, text)` —
nothing is overwritten or dropped). The local `memory.db` catches up from that JSONL on the next server
start. See the **memory-sync** skill for the full model.

**Do this now:**

1. (Recommended) Diagnose first so the user sees what will change: run `/genesis:doctor`, or
   `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli doctor --root "${CLAUDE_PROJECT_DIR}"`.

2. Consolidate via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli fix --into "${CLAUDE_PROJECT_DIR}"
   ```
   Add `--archive` to also COPY (never move) each stray into `.genesis/memory/archived-strays/`. The command
   NEVER writes outside the target repo, and is idempotent — re-running with the strays still present adds
   nothing.

3. Report `records_before` → `records_after`, how many strays were consolidated, and the paths. Remind the
   user the strays were only read (safe to delete manually), and that the repo must be reopened (or the
   memory server restarted) for `memory.db` to rebuild from the updated JSONL.
