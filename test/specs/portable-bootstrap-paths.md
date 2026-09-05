# Spec: bootstrap emits portable, machine-independent paths

## Type
Bug fix (production-readiness / portability). Supersedes the original `${CLAUDE_PROJECT_DIR}`-in-`.mcp.json`
approach after evidence that the client does not expand it there (GitHub #25).

## Bug
`genesis-cli bootstrap` must write per-repo config that is portable (survives clone/move/commit) AND that
actually resolves at runtime. Two historical mistakes:

1. (fixed earlier) Absolute paths baked into `.mcp.json` / `.claude/settings.json` broke on clone/move.
2. (this change — #25) The portability fix wrote `${CLAUDE_PROJECT_DIR}/.genesis/...` into **`.mcp.json`**
   `args`/`env`. Claude Code does **not** expand `${CLAUDE_PROJECT_DIR}` in a project `.mcp.json` server
   config (it DOES expand it in `.claude/settings.json` hook commands, and it expands `${CLAUDE_PLUGIN_ROOT}`
   in the plugin's own `.mcp.json`). So the literal `${CLAUDE_PROJECT_DIR}` reached `node`:
   `Cannot find module '<repo>/${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js'` → `MODULE_NOT_FOUND`
   → the MCP server exited → `/mcp` reported `CONNECTION_CLOSED`.

## Expected behavior
- `.mcp.json` uses a **repo-root-relative** launcher path (no variable, no absolute path). Claude Code
  launches a project MCP server with its working directory at the project root (verified: the failing
  literal was resolved relative to the project root), so a relative `args[0]` resolves correctly and travels
  with the repo.
- The memory DB/export paths in `.mcp.json` are also repo-relative; the Node launcher resolves the memory
  paths against the project root — `CLAUDE_PROJECT_DIR` when set, else the process working directory (Claude
  Code launches a project MCP server with cwd = the project root) — expanding a still-literal
  `${CLAUDE_PROJECT_DIR}`, resolving a relative value, and leaving an already-absolute value (a dev override)
  untouched. So the server always opens the correct DB regardless of the client's variable handling.
- `.claude/settings.json` hook commands keep the `${CLAUDE_PROJECT_DIR}` form — the client DOES expand it
  there (hooks work), and it is portable.

## Acceptance criteria
1. After `genesis-cli bootstrap <target> <home>`, `.mcp.json` `mcpServers.genesis-memory.args[0]` equals
   exactly `.genesis/bin/genesis-memory.js`.
2. `.mcp.json` env `GENESIS_MEMORY_DB` equals `.genesis/memory.db` and `GENESIS_MEMORY_EXPORT` equals
   `.genesis/memory/memory.jsonl`.
3. No string bootstrap writes into `.mcp.json` contains `${CLAUDE_PROJECT_DIR}` (nor any `${...}`), nor the
   target repo's absolute path — on any platform.
4. `.claude/settings.json` promote-offer command still equals exactly
   `node "${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js" --run-hook promote-offer`.
5. The Node launcher, in the server path, resolves `GENESIS_MEMORY_DB` / `GENESIS_MEMORY_EXPORT` to an
   absolute path against the project root (`CLAUDE_PROJECT_DIR` or the process cwd): unset → `<proj>/.genesis/
   memory.db`; relative → resolved against `<proj>`; literal `${CLAUDE_PROJECT_DIR}/...` → expanded to
   `<proj>/...`; already-absolute (a dev override) → left untouched.
6. Existing bootstrap behavior is otherwise unchanged (idempotent re-bootstrap, preserved user
   settings/servers, staged binaries) — proven by the pre-existing bootstrap tests staying green.

## Notes
- `${CLAUDE_PROJECT_DIR}` is expanded by Claude Code in `.claude/settings.json` hook commands, and
  `${CLAUDE_PLUGIN_ROOT}` is expanded in the plugin's own `.mcp.json` — but a **project** `.mcp.json` does not
  get `${CLAUDE_PROJECT_DIR}` expansion. This is why the memory paths are made launcher-resolved rather than
  variable-dependent.
- Windows: the relative string uses forward slashes and no drive-letter/backslash absolute path is emitted.
- Deliberate deviation (sdd-11): the earlier ACs mandated the `${CLAUDE_PROJECT_DIR}` form in `.mcp.json`;
  they are replaced here because that form does not resolve. Portability is preserved (no absolute path).
