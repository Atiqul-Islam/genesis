---
description: Diagnose where this repo's Genesis memory actually lives — scans your home dir for stray memory databases (scatter lands outside the repo) and flags any that hold THIS repo's agents. Read-only.
argument-hint: (no arguments)
---

You are running **`/genesis:doctor`** — a READ-ONLY health check on this repo's Genesis memory. Scattered
memory lands in whatever folder Claude Code was launched from — a **sibling** of the repo, not inside it — so
this scans your **home directory**, not just the repo. It reports the repo's canonical store, then any stray
databases that hold **this repo's own agents** (the memory `/genesis:fix` would recover). It changes nothing.

**Do this now:**

1. Run the diagnosis via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli doctor --repo "${CLAUDE_PROJECT_DIR}"
   ```
   It prints JSON: `canonical` (this repo's store + per-agent counts), `custom_agents` (this repo's agents),
   `recoverable_strays` (databases OUTSIDE `.genesis/` holding this repo's memory + how much), `other_stores`
   (memory belonging to other repos/agents — informational), and a `healthy` flag. (The scan defaults to your
   home dir; add `--root "<dir>"` to narrow it if it's slow.)

2. Summarize for the user: is memory healthy (all in `.genesis/`), or are there `recoverable_strays`? If so,
   name how many memories and which stray files hold them, and that **`/genesis:fix`** consolidates them into
   this repo losslessly. Do not run `fix` yourself unless the user asks.
