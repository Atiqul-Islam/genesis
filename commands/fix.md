---
description: Consolidate this repo's scattered Genesis memory into its canonical .genesis/ store — scans your home dir, pulls in THIS repo's agents from wherever they scattered, losslessly. Strays are read-only.
argument-hint: (no arguments) — --archive to keep a copy of each stray; --all-agents to take every agent
---

You are running **`/genesis:fix`** — consolidate scattered memory into this repo's canonical store. Scatter
lands in a **sibling** of the repo, so this scans your **home directory** and pulls in the memories that
belong to **this repo's own agents** (plus anything in a stray physically inside the repo), UNION-merging
them into `<repo>/.genesis/memory/memory.jsonl` — nothing is overwritten or dropped. Foreign repos' memory
(their `sensei`/`method`, other custom agents) is left untouched. The local `memory.db` catches up from the
JSONL on the next server start. See the **memory-sync** skill for the model.

**Do this now:**

1. (Recommended) Diagnose first so the user sees what will move: run `/genesis:doctor`.

2. Consolidate via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli fix --into "${CLAUDE_PROJECT_DIR}"
   ```
   The scan defaults to your home dir (add `--root "<dir>"` to narrow/speed it up). Add `--archive` to COPY
   (never move) each contributing stray into `.genesis/memory/archived-strays/`, `--agent <name>` to target
   one agent, or `--all-agents` to take every agent found. The command NEVER writes outside the target repo
   and is idempotent.

3. Report `records_before` → `records_after`, how many strays contributed, which agents/paths, and the
   `scan_root`. Remind the user the strays were only read (safe to delete), and that the repo must be reopened
   (or the memory server restarted) for `memory.db` to rebuild from the updated JSONL.
