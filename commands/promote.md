---
description: Promote a Genesis agent to be this folder's main Claude — persona + full enforcement, non-destructive. Sets the folder up first if the agent is a shipped core agent (sensei/method) not yet staged here.
argument-hint: <agent-name>
---

You are running **`/genesis:promote`** — make a Genesis agent the **main Claude** for this folder. Its
persona is merged into the folder's `CLAUDE.md` (a managed block) and its enforcement hooks are wired into
`.claude/settings.json` with `--main-agent`, non-destructively and idempotently. The agent also remains
available as a subagent.

**Agent to promote:** $ARGUMENTS

**Do this now:**

1. If `$ARGUMENTS` is empty, list this folder's built agents (`.claude/agents/*.md`), ask the user which
   one to promote, and stop until they answer.

2. **Check whether the agent is staged in this folder** — does `.claude/agents/$ARGUMENTS.md` exist?

   - **It exists** → skip to step 3 (promote directly; do NOT re-bootstrap).

   - **It does NOT exist, and `$ARGUMENTS` is `sensei` or `method`** (Genesis's shipped CORE agents) → the
     folder just hasn't been set up yet. **Bootstrap it first** — this stages the functional tree, the
     native `.genesis/bin`, the memory MCP server, and installs sensei+method, so the promoted agent is the
     genuine Sensei with its per-agent memory wired (able to carry on / resume Sensei's work here):
     ```
     node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli bootstrap . "${CLAUDE_PLUGIN_ROOT}"
     ```
     Then continue to step 3.

   - **It does NOT exist, and `$ARGUMENTS` is any other name** → there is nothing to promote; that agent
     was never built here. Tell the user to build it first with `/genesis:new` (Sensei builds+installs it),
     then re-run promote. Stop.

3. Promote via the plugin launcher (it resolves the native `genesis-cli`; no manual staging needed):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli promote "$ARGUMENTS"
   ```
   This reads `.claude/agents/$ARGUMENTS.md`, merges its persona into `<repo>/CLAUDE.md` (a
   `>>> genesis agent: $ARGUMENTS <<<` managed block), and wires the main-thread enforcement hooks —
   SessionStart inject / PreToolUse gate / Stop validate + review, each carrying `--main-agent $ARGUMENTS` —
   into `<repo>/.claude/settings.json`. Existing `CLAUDE.md` content and any other hooks are preserved.

4. Report the result: confirm the managed block and the `--main-agent $ARGUMENTS` hooks are present, and
   tell the user to reopen the folder for the main-thread persona + hooks to take effect. To undo, use
   `/genesis:demote`.
