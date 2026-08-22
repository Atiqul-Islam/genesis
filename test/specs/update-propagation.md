## Bug: update propagation doesn't reach promoted-main repos (issue #3)

A Genesis repo runs a per-repo `.genesis/bin/genesis-hook`. `/plugin update` refreshes the plugin cache +
launcher `RELEASE_VERSION` but does not restage a repo's binary. `--sync` (which restages) is wired only on
`SubagentStart` for core subagents (`hooks/hooks.json`), so a promoted-main repo that never spawns a core
subagent runs a stale binary indefinitely — new features (verbose, guards, …) never take effect there.

### Expected Behavior

- A promoted-main repo restages its `.genesis/bin` to the current plugin version on session start — no core
  subagent required — so any managed repo that is opened self-updates.
- A `/genesis:update-repo` command restages on demand.
- Restaging is idempotent (gated by `.staged-version`); re-promote does not duplicate the sync hook.

### Acceptance Criteria

- **AC1** — `main_thread_hooks(name, home, …)` includes a `SessionStart` hook that runs the launcher with
  `--sync "<home>"` (via `node <home>/bin/genesis-memory.js`).
- **AC2** — `main_settings` is idempotent: after two runs there is exactly ONE sync hook and one inject for
  the agent (the sync sits in the same SessionStart block as inject, which carries `--main-agent <name>`).
- **AC3** — `commands/update-repo.md` exists and runs the launcher `--sync "${CLAUDE_PROJECT_DIR}/.genesis"`.
- **AC4** — demote still removes the SessionStart block (it carries `--main-agent` via inject), leaving no
  orphan sync hook.

### Implementation Requirements

- `cli/src/render.rs::main_thread_hooks`: prepend a `{type:command, command: node "<home>/bin/genesis-memory.js"
  --sync "<home>"}` hook to the existing `SessionStart` block (same block as inject, so the block still
  carries `--main-agent <name>` and `main_settings::is_this_agent` matches it for idempotent replace).
- New `commands/update-repo.md`: `node "${CLAUDE_PLUGIN_ROOT}/bin/genesis-memory.js" --sync
  "${CLAUDE_PROJECT_DIR}/.genesis"`, plus a note that the change lands on the next session.
- No new `genesis-cli` subcommand — `--sync` already exists in the launcher (`bin/genesis-memory.js`).
- Honest limit (documented, not worked around): hooks only run in a repo that is OPENED in Claude Code;
  Genesis cannot reach a repo that is never opened.

### References

`hooks/hooks.json`, `cli/src/render.rs` (`main_thread_hooks`, `main_settings`, `is_genesis_main_block`),
`bin/genesis-memory.js` (`syncRepo`/`--sync`), `commands/promote.md`.
