---
description: Promote an existing Genesis-built agent to be this folder's main Claude — persona + full enforcement, non-destructive.
argument-hint: <agent-name>
---

You are running **`/genesis:promote`** — turn an existing Genesis-built agent into the **main Claude** for this folder. Its persona is merged into the folder's `CLAUDE.md` (as a managed block) and its enforcement hooks are wired into `.claude/settings.json` with `--main-agent`, non-destructively and idempotently. The agent also remains available as a subagent.

**Agent to promote:** $ARGUMENTS

**Do this now:**

1. If `$ARGUMENTS` is empty, list the built agents (`.claude/agents/*.md`), ask the user which one to promote, and stop until they answer. Only one agent is the main Claude at a time — promoting a different agent just replaces the managed block, so there is no separate "demote".

2. Promote it via the plugin launcher (it resolves the native `genesis-cli`; no manual staging needed):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli promote "$ARGUMENTS"
   ```
   This reads `.claude/agents/$ARGUMENTS.md`, merges its persona into `<repo>/CLAUDE.md` (a `>>> genesis agent: $ARGUMENTS <<<` managed block), and wires the main-thread enforcement hooks — SessionStart inject / PreToolUse gate / Stop validate + review, each carrying `--main-agent $ARGUMENTS` — into `<repo>/.claude/settings.json`. Existing `CLAUDE.md` content and any other hooks are preserved.

3. Report the result: confirm the managed block and the `--main-agent $ARGUMENTS` hooks are present, and tell the user to reopen the folder for the main-thread persona + hooks to take effect. To undo, delete that managed block from `CLAUDE.md` and the `--main-agent` hook entries from `.claude/settings.json`.
