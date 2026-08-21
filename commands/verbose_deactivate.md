---
description: Stop showing a Genesis agent's APPLIED-EXPERTISE declarations in its replies (verbose OFF — the default). Enforcement and logging are unaffected.
argument-hint: <agent-name>
---

You are running **`/genesis:verbose_deactivate`** — turn OFF the visible display of a Genesis agent's
`APPLIED-EXPERTISE` declarations (the default state). The agent keeps declaring — the declarations are
still recorded to `.genesis/applied-expertise.jsonl`, still enforced by the Stop hook, and still written
to the audit log — they simply no longer appear in its replies.

**Agent:** $ARGUMENTS

**Do this now:**

1. If `$ARGUMENTS` is empty, list this folder's agents (`.claude/agents/*.md`), ask which one to quiet,
   and stop until they answer.

2. Clear the flag via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli verbose off "$ARGUMENTS"
   ```
   This removes `<repo>/.genesis/verbose/$ARGUMENTS.json`. The SessionStart inject hook then instructs
   that agent to record its declarations quietly rather than print them.

3. Report the result and tell the user the change takes effect on that agent's next session/turn. To turn
   it back on, run `/genesis:verbose_activate $ARGUMENTS`.
