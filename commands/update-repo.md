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

2. Report the before/after staged version (see `${CLAUDE_PROJECT_DIR}/.genesis/bin/.staged-version`).

3. Tell the user the enforcement/display change takes effect on the **next** session — the current
   session's hooks were already loaded at its start.

Note: this can only update a repo that is opened in Claude Code; Genesis cannot reach a repo that is never
opened. Fail-open — a sync error never breaks the session.
