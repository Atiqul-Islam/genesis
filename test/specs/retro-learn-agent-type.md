# Bug: /genesis:retro-learn invokes an unresolvable agent type (#28)

## Type
Bug fix (a shipped plugin command references a non-existent subagent type).

## Bug
`commands/retro-learn.md` step 4 tells the coordinator to invoke the **`mneme`** agent (bare). That agent
type resolves nowhere: the plugin registers the agent as **`genesis:mneme`**, and `bootstrap` installs only
`sensei` + `method` as repo-local agents (`cli/src/bootstrap.rs:12`) — never a bare `mneme`. So retro-learn
dies with `Agent type 'mneme' not found` the moment it reaches step 4, in any repo.

## Expected behavior
retro-learn invokes the agent by its real, resolvable type `genesis:mneme` (the plugin core subagent), so
the analysis step runs. No plugin command instructs invoking a bare `mneme` agent (which is never installed
as a repo-local agent). `sensei`/`method` may still be referenced bare — bootstrap DOES install those as
repo-local agents, so they resolve.

## Acceptance criteria
1. `commands/retro-learn.md` invokes the agent as `genesis:mneme`, not bare `` `mneme` ``.
2. No file under `commands/` contains the bare agent token `` `mneme` `` (backtick-delimited) — only
   `` `genesis:mneme` `` is allowed (guards against the exact recurrence).
3. A CI-run node test enforces criteria 1-2 (fails before the fix, passes after).

## Implementation Requirements
- Edit only the invocation in `commands/retro-learn.md` (prose "Mneme"/"the memory agent" wording stays).
- Add `test/commands-agents.test.js` and wire it into ci.yml's node-test step.
- Bump the plugin version so `/plugin update` propagates the corrected command to consumers (som-21).

## Notes
- End-to-end verification: launch a `genesis:mneme` subagent and confirm it resolves (bare `mneme` errors —
  already evidenced by the runtime report). A live run is what this bug's shipping lacked.
- A fully general "every agent type a command names must exist" check is a good follow-up; this spec adds the
  deterministic regression guard for the mneme case.
