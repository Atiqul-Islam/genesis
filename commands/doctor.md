---
description: Diagnose where this repo's Genesis memory actually lives — you choose the scan scope (your user directory, or the whole system), then it flags any stray memory databases holding THIS repo's agents. Read-only.
argument-hint: (no arguments) — you'll be asked for the scan scope
---

You are running **`/genesis:doctor`** — a READ-ONLY health check on this repo's Genesis memory. Scattered
memory lands in whatever directory Claude Code was launched from, which can be anywhere, so **you choose how
wide to look** — there is no guessed default. It reports the repo's canonical store, then any stray databases
that hold **this repo's own agents** (the memory `/genesis:fix` would recover). It changes nothing.

**Do this now:**

1. **Ask the user the scan scope** (do not guess) — present exactly two choices:
   - **User** — scan everything under their user/home directory (covers all their projects and launch dirs).
   - **System** — scan the entire machine (every drive). Thorough but slow.
   Wait for their answer.

2. Build the scope argument:
   - **User** → pass `--scope user`. The CLI resolves the OS user directory itself — and when running under
     WSL it also covers the Windows profile (`/mnt/c/Users/<name>`), where the repos actually live.
   - **System** → pass `--scope system` (the CLI enumerates every filesystem root itself).
   (You can still pass an explicit `--root "<dir>"` instead, e.g. if the user names a specific folder.)

3. Run the diagnosis via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli doctor --repo "${CLAUDE_PROJECT_DIR}" <scope-arg from step 2>
   ```
   It prints JSON: `scan_roots`, `canonical` (this repo's store + per-agent counts), `custom_agents`,
   `recoverable_strays` (databases OUTSIDE `.genesis/` holding this repo's memory + how much), `other_stores`
   (memory belonging to other repos/agents — informational), and a `healthy` flag.

4. Summarize: healthy (all in `.genesis/`), or are there `recoverable_strays`? If so, name how many memories
   and which files hold them, and that **`/genesis:fix`** (at the same scope) consolidates them into this repo
   losslessly. Do not run `fix` yourself unless the user asks.
