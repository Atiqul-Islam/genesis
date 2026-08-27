---
description: Refresh this repo's staged Genesis binaries to the installed plugin version — restages .genesis/bin so a promoted-main repo picks up the latest enforcement hooks (verbose, guards, fixes).
---

You are running **`/genesis:update-repo`** — bring THIS repo's staged Genesis runtime up to date with the
installed plugin. A Genesis repo runs its own `.genesis/bin/genesis-hook`; `/plugin update` refreshes the
plugin cache but not a repo's staged binary. This restages it on demand (it also happens automatically on
session start for promoted-main repos, and on core-subagent start).

**Do this now:**

1. Run the launcher's version-sync (it restages `.genesis/bin` + heals the managed `.gitignore` when the
   staged version is behind; a no-op when already current):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --sync "${CLAUDE_PROJECT_DIR}/.genesis"
   ```

2. Refresh this repo's main-thread hook WIRING in `.claude/settings.json` (issue #12 — new hooks like
   `capture-session`/`precompact`/`--sync` reach an already-promoted repo ONLY if `settings.json` is
   re-merged, not just the binary). Unlike step 1, this is NOT version-gated, so it heals a repo whose
   binary is already current but whose `settings.json` is frozen from an older promote:
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli sync-settings "${CLAUDE_PROJECT_DIR}"
   ```
   (A no-op when no Genesis agent is promoted in this repo.)

3. Report the before/after staged version (see `${CLAUDE_PROJECT_DIR}/.genesis/bin/.staged-version`).

4. Tell the user the enforcement/display change takes effect on the **next** session — the current
   session's hooks were already loaded at its start.

Note: this can only update a repo that is opened in Claude Code; Genesis cannot reach a repo that is never
opened. Fail-open — a sync error never breaks the session.
