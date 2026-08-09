---
description: Demote this folder's main Genesis Claude back to a normal folder — removes the managed persona + main-thread hooks. The agent stays a subagent.
argument-hint: (no arguments)
---

You are running **`/genesis:demote`** — the inverse of `/genesis:promote`. It removes the folder's main Genesis Claude: strips the managed persona block from `CLAUDE.md` and the `--main-agent` enforcement hooks from `.claude/settings.json`, non-destructively (everything else is preserved). No agent name is needed — only one agent is ever the main Claude. The agent stays available as a subagent.

**Do this now:**

1. Run the demotion via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli demote
   ```
   This removes the `>>> genesis agent <<<` managed block from `<repo>/CLAUDE.md` and the `--main-agent` main-thread hook entries from `<repo>/.claude/settings.json`, keeping all your other content and hooks. If no agent is currently promoted, it reports that and changes nothing.

2. Report the result (which agent was demoted, or that there was none), and tell the user to reopen the folder for the change to take effect.
