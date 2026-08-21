---
description: Show a Genesis agent's APPLIED-EXPERTISE declarations in its replies (verbose ON). Off by default; enforcement and logging are unaffected either way.
argument-hint: <agent-name>
---

You are running **`/genesis:verbose_activate`** — turn ON the visible display of a Genesis agent's
`APPLIED-EXPERTISE` declarations. By default those declarations are recorded quietly (to
`.genesis/applied-expertise.jsonl`) and NOT shown in the agent's replies; this makes them visible again.
It changes ONLY the display — the Stop-hook enforcement and the audit log are unchanged.

**Agent:** $ARGUMENTS

**Do this now:**

1. If `$ARGUMENTS` is empty, list this folder's agents (`.claude/agents/*.md`), ask which one to make
   verbose, and stop until they answer.

2. Set the flag via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli verbose on "$ARGUMENTS"
   ```
   This writes `<repo>/.genesis/verbose/$ARGUMENTS.json` = `{"verbose":true}`. The SessionStart inject
   hook reads it and, from the next session, instructs that agent to print its declarations in its
   replies instead of only recording them.

3. Report the result and tell the user the change takes effect on that agent's next session/turn. To turn
   it back off, run `/genesis:verbose_deactivate $ARGUMENTS`.
