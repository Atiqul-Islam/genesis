# Spec: --sync heals a stale .mcp.json (GitHub #26)

## Type
Bug fix (update propagation). Pairs with #25 (the generator fix in `portable-bootstrap-paths`).

## Bug
The launcher's `--sync` refreshes staged binaries, the managed `.gitignore` block (`sync-gitignore`),
`.claude/settings.json` (`sync-settings`), and the derived `expertise.db` (`migrate-expertise`) — but it
never regenerates `.mcp.json`. So the #25 fix (a repo-relative `.mcp.json`) would NOT reach an
already-bootstrapped repo through `/plugin update`; that repo would keep its broken
`${CLAUDE_PROJECT_DIR}`-based `.mcp.json` and its memory server would keep failing to launch.

## Expected behavior
- A new cli subcommand `sync-mcp <target>` regenerates ONLY the `genesis-memory` entry in `<target>/.mcp.json`
  to the current, correct form (repo-relative `args` + repo-relative memory env), single-sourced from the
  SAME generator bootstrap uses, preserving any other MCP servers already present.
- The launcher's `--sync` invokes `sync-mcp` (like it already invokes `sync-settings`), fail-open.

## Acceptance criteria
1. `genesis-cli sync-mcp <target>` on a repo whose `.mcp.json` `genesis-memory.args[0]` is
   `${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js` rewrites it to `.genesis/bin/genesis-memory.js` and
   removes every `${CLAUDE_PROJECT_DIR}` from the `genesis-memory` entry.
2. Any OTHER server already in `.mcp.json` (e.g. a user's own `foo` server) is preserved unchanged.
3. `sync-mcp` is idempotent: a second run produces a byte-identical `.mcp.json`.
4. The bootstrap generator and `sync-mcp` are single-sourced (one function) so they cannot drift.
5. The launcher's `syncRepo` runs `sync-mcp <repo-root>` during a version-stamped `--sync`, fail-open (a
   `sync-mcp` failure never aborts session start).

## Constraints
- Fail-open: a `.mcp.json` heal must never break session start.
- Preserve the user's other `.mcp.json` servers and any non-`genesis-memory` keys.
- Cross-platform; run the repo's own-OS staged binary (never stage a cross-OS binary during sync).
- `serde_json` `preserve_order` byte-parity for the emitted JSON.

## Notes
- Prior art: `sync-gitignore` (`sync-heals-gitignore` spec) and `sync-settings`
  (`update-propagation-settings` spec) added exactly this kind of heal for their files; `.mcp.json` gets the
  same treatment here.
