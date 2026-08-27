---
description: Show whether a Genesis agent's APPLIED-EXPERTISE declarations are DISPLAYED (verbose on) or recorded quietly (off, the default). Read-only.
argument-hint: <agent-name | --all>
---

You are running **`/genesis:verbose_status`** — report the verbose (declarations-display) state of a Genesis
agent. Verbose ON = the agent prints its `APPLIED-EXPERTISE` lines in replies; OFF (default) = it records
them quietly to `.genesis/applied-expertise.jsonl`. This command only READS the state; it changes nothing.

**Agent:** $ARGUMENTS

**Do this now:**

1. Report the status via the plugin launcher (it resolves the native `genesis-cli`):
   - One agent: `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli verbose status "$ARGUMENTS"`
   - All agents with a flag: `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli verbose status --all`

2. Relay the result: `on` or `off` per agent (an agent with no flag file is OFF — the default).

3. To change it: `/genesis:verbose_activate <agent>` (on) or `/genesis:verbose_deactivate <agent>` (off).
