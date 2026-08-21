## Bug: `/genesis:promote sensei` fails in a folder that has never been set up

`promote <name>` reads `<repo>/.claude/agents/<name>.md` and errors if it is absent (`promote.rs`),
telling the user to *"Build '<name>' first (/genesis:new)"*. But **sensei** and **method** are Genesis
CORE agents shipped inside the plugin — they are never "built"; they are staged into a repo by
**bootstrap** (which copies the functional tree, stages the native binaries, registers the memory MCP
server, and installs sensei+method). So `/genesis:promote sensei` in a fresh folder dead-ends with
misleading guidance, even though Genesis ships a real Sensei. Promoting Sensei should just work — the
promoted agent is the genuine Sensei (same `agent_id`, memory tools wired), so it can carry on / resume
Sensei's work in that repo.

### Expected Behavior

- `/genesis:promote sensei` (or `method`) in a folder where it is not yet staged sets the folder up first
  (bootstrap: functional tree + `.genesis/bin` + memory MCP + sensei/method), then promotes it to the
  folder's main Claude — one command, no manual bootstrap step.
- If the agent is ALREADY staged (`.claude/agents/<name>.md` exists), promote just promotes it (no
  re-bootstrap, non-destructive).
- Promoting a NON-core name that was never built still tells the user to build it first (`/genesis:new`) —
  core agents are provisioned by bootstrap, custom agents are built.
- When `genesis-cli promote <name>` is run directly and the agent is absent, its error names the correct
  fix: bootstrap for a core agent, build-first for a custom one.

### Acceptance Criteria

- **AC1** — `genesis-cli promote sensei` against a repo with no `sensei.md` fails with a message that names
  **bootstrap** (not "/genesis:new") and identifies sensei as a core agent.
- **AC2** — `genesis-cli promote custombot` against a repo with no `custombot.md` fails with the
  build-first ("/genesis:new") message and does NOT mention bootstrap.
- **AC3** — The `/genesis:promote` command, given a core agent that is not yet staged, bootstraps the
  folder (with the plugin as genesis-home) and then promotes; given an already-staged agent, it only
  promotes.

### Implementation Requirements

- `promote.rs`: extract the missing-agent error into `missing_agent_message(name, agent_md)`; when
  `render::builtin_meta(name).is_some()` (sensei/method) it returns the bootstrap-first guidance, else the
  existing build-first guidance. `run()` calls it. No change to the happy path.
- `commands/promote.md`: before promoting, if `.claude/agents/$ARGUMENTS.md` is absent, branch — a core
  agent (sensei/method) → run `--run-cli bootstrap "<repo>" "${CLAUDE_PLUGIN_ROOT}"` first (idempotent
  setup that stages the agent), then promote; a custom agent → tell the user to build it with
  `/genesis:new`. If the agent is already staged, promote directly.
- The genesis-home passed to bootstrap is `${CLAUDE_PLUGIN_ROOT}` (the installed plugin carries
  `team/ expertise/ hooks/ skills/ bin/`), matching how the build-agent skill invokes the CLI from a bare
  plugin install.
