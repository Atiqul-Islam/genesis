# Spec: bootstrap emits portable, machine-independent paths

## Type
Bug fix (production-readiness / portability).

## Bug
`genesis-cli bootstrap` canonicalizes the target repo to an **absolute** path
(`cli/src/bootstrap.rs` line 182, `std::fs::canonicalize`) and then writes that absolute
launcher path verbatim into two per-repo config files:

- `write_mcp` (bootstrap.rs ~L107): `.mcp.json` `mcpServers.genesis-memory.args[0]` and the
  `GENESIS_MEMORY_DB` / `GENESIS_MEMORY_EXPORT` env values become absolute
  (e.g. `/abs/path/to/repo/.genesis/bin/genesis-memory.js`).
- `write_promote_offer_hook` (bootstrap.rs ~L127): `.claude/settings.json` SessionStart
  promote-offer command becomes `node "/abs/.../.genesis/bin/genesis-memory.js" --run-hook promote-offer`.

An absolute path baked into a distributable config breaks the instant the repo is moved,
cloned onto another machine, or committed (it carries the building machine's path). The
sibling hook commands (`inject` / `gate` / `validate`, written by `assemble.rs`) already use
the portable `${CLAUDE_PROJECT_DIR}/.genesis/...` form; bootstrap must match that convention.

## Expected behavior
Every path bootstrap writes into `.mcp.json` and `.claude/settings.json` is expressed relative
to the Claude Code project root via the `${CLAUDE_PROJECT_DIR}` variable — never an absolute
filesystem path. `${CLAUDE_PROJECT_DIR}` is always the repo root at runtime, and `.genesis/` is
always at `<repo>/.genesis`, so the portable form is correct on every machine and survives a
clone/move/commit.

## Acceptance criteria
1. After `genesis-cli bootstrap <target> <home>`, `.mcp.json`
   `mcpServers.genesis-memory.args[0]` equals exactly
   `${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js`.
2. `.mcp.json` env `GENESIS_MEMORY_DB` equals `${CLAUDE_PROJECT_DIR}/.genesis/memory.db` and
   `GENESIS_MEMORY_EXPORT` equals `${CLAUDE_PROJECT_DIR}/.genesis/memory/memory.jsonl`.
3. `.claude/settings.json` promote-offer command equals exactly
   `node "${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js" --run-hook promote-offer`.
4. No string bootstrap writes into `.mcp.json` or `.claude/settings.json` contains the target
   repo's absolute path (its canonicalized prefix), on any platform.
5. Existing bootstrap behavior is otherwise unchanged (idempotent re-bootstrap, preserved user
   settings/servers, staged binaries) — proven by the pre-existing bootstrap tests staying green.

## Notes
- `${CLAUDE_PROJECT_DIR}` is expanded by Claude Code in both hook commands and `.mcp.json`
  server configs (the plugin's own committed `.mcp.json` already relies on this).
- Windows: the portable string uses forward slashes and the `${CLAUDE_PROJECT_DIR}` variable, so
  no drive-letter/backslash absolute path is emitted.
