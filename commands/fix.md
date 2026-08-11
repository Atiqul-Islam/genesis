---
description: Consolidate this repo's scattered Genesis memory into its canonical .genesis/ store — you choose the scan scope (your user directory, or the whole system); it pulls in THIS repo's agents losslessly. Strays are read-only.
argument-hint: (no arguments) — you'll be asked for the scan scope
---

You are running **`/genesis:fix`** — consolidate scattered memory into this repo's canonical store. Scattered
memory can be anywhere Claude Code was launched, so **you choose how wide to scan** (no guessed default). It
pulls in the memories that belong to **this repo's own agents** (plus anything in a stray physically inside
the repo), UNION-merging them into `<repo>/.genesis/memory/memory.jsonl` — nothing is overwritten or dropped.
Foreign repos' memory is left untouched. The local `memory.db` catches up from the JSONL on the next server
start. See the **memory-sync** skill for the model.

**Do this now:**

1. (Recommended) Run `/genesis:doctor` first so the user sees what will move.

2. **Ask the user the scan scope** (do not guess):
   - **User** — everything under their user/home directory.
   - **System** — the entire machine (every drive). Thorough but slow.

3. Build the scope argument (same as doctor):
   - **User** → resolve the OS home (`%USERPROFILE%` on Windows, `$HOME` on macOS/Linux) and pass
     `--root "<that home dir>"`.
   - **System** → pass `--scope system`.

4. Consolidate via the plugin launcher:
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli fix --into "${CLAUDE_PROJECT_DIR}" <scope-arg>
   ```
   Add `--archive` to COPY (never move) each contributing stray into `.genesis/memory/archived-strays/`,
   `--agent <name>` to target one agent, or `--all-agents` to take every agent found. The command NEVER writes
   outside the target repo and is idempotent.

5. Report `records_before` → `records_after`, how many strays contributed, which agents/paths, and the
   `scan_roots`. Remind the user the strays were only read (safe to delete), and that the repo must be
   reopened (or the memory server restarted) for `memory.db` to rebuild from the updated JSONL.
