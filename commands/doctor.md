---
description: Diagnose where this repo's Genesis memory actually lives — finds stray memory databases outside .genesis/ and reports per-agent counts. Read-only; changes nothing.
argument-hint: (no arguments)
---

You are running **`/genesis:doctor`** — a READ-ONLY health check on this repo's Genesis memory. It scans the
project for every genesis memory database (the canonical `.genesis/memory.db` plus any **stray**
`genesis-memory.db` a mis-anchored server left in a launch directory) and reports, per agent, how many
memories each database holds. It flags any memory that is NOT in the repo's `.genesis/` store. It changes
nothing.

**Do this now:**

1. Run the diagnosis via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli doctor --root "${CLAUDE_PROJECT_DIR}"
   ```
   It prints JSON: the canonical store (path, whether it exists, per-agent counts), the count in the
   committed `memory.jsonl`, a list of `strays` (databases outside `.genesis/` with their memories), and a
   `healthy` flag.

2. Summarize for the user: is memory healthy (all in `.genesis/`), or is memory scattered? If `healthy` is
   false, tell them exactly how many stray databases and memories were found, and that **`/genesis:fix`**
   will consolidate them losslessly into the repo store. Do not run `fix` yourself unless the user asks.
