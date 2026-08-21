---
description: Create or upgrade a Genesis agent's guard — the per-agent invariants (self_protect + must_match / must_not_match) that block edits which would weaken that agent's boundaries. Coordinator-only.
argument-hint: <agent-name>
---

You are running **`/genesis:update_guard`** — create or upgrade the **guard** for a Genesis agent. A guard
is that agent's own `.genesis/team/<agent>/guard.json`: a `self_protect` list (files the agent may not
edit) plus `invariants` (regexes that MUST or MUST NOT appear in named files). The PreToolUse gate loads
the ACTIVE agent's guard and blocks any Write/Edit that would break it. This flow is how Sensei (the
coordinator) edits ANY agent's guard — agents cannot edit their own.

**Agent whose guard to update:** $ARGUMENTS

**Do this now:**

1. If `$ARGUMENTS` is empty, list this folder's agents (`.claude/agents/*.md`), ask which agent's guard
   to update, and stop until they answer.

2. Read the agent's current guard if it exists (`.genesis/team/$ARGUMENTS/guard.json`) so you upgrade it
   rather than clobber it. Confirm the intended change with the user — a guard BLOCKS the agent's edits,
   so its invariants are enforcement and must be reviewed before they go live.

3. Write the FULL proposed guard JSON to a scratch file (e.g. `.genesis/team/$ARGUMENTS/guard.candidate.json`),
   using the schema:
   ```json
   {
     "self_protect": [".genesis/team/$ARGUMENTS/guard.json"],
     "invariants": [
       {"id":"c1","files":["persona.md"],"must_match":"(?is)per-action\\s+approval","why":"why this must hold"}
     ]
   }
   ```

4. Validate and install it via the plugin launcher (it resolves the native `genesis-cli`):
   ```
   node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --run-cli update-guard "$ARGUMENTS" ".genesis/team/$ARGUMENTS/guard.candidate.json"
   ```
   `update-guard` VALIDATES the candidate (valid JSON; a `self_protect` array; every invariant has an
   `id`, a `files` array, at least one of `must_match`/`must_not_match`, and every regex compiles) and only
   then writes `.genesis/team/$ARGUMENTS/guard.json`. A malformed candidate is rejected (exit 2) and
   nothing is written.

5. On success, delete the scratch candidate file and report the installed guard path + invariant count.
   The guard takes effect immediately for that agent's subsequent Write/Edit calls.
