# Bug: genesis-spawned processes flash a console window on Windows (should be hidden)

## Type
Bug fix. Windows-visible; no-op on macOS/Linux. Cross-platform-safe.

## Bug
Genesis spawns child processes without the Windows "no console window" flag, so a console window (often a
`node` window) appears/flashes on Windows during normal operation. Verified in code:
- `bin/genesis-memory.js`: every `childProcess.spawn` / `spawnSync` omits `windowsHide: true`
  (`execPassthrough`, `execServer`, and the four `--sync` `spawnSync` calls).
- `hook/src/expertise_warn.rs` `Server::spawn`: spawns `node <launcher>` (the embedding server, once per
  Stop) with no `CREATE_NO_WINDOW` creation flag — while the sibling `spawn_detached` in the same file
  already sets it.

## Expected behavior
No genesis-spawned process shows a console window on Windows. On macOS/Linux the change is a no-op.

## Acceptance criteria
- AC1: every `childProcess.spawn`/`spawnSync` in `bin/genesis-memory.js` passes `windowsHide: true`
  (except the Linux-only `ldd` probe).
- AC2: `hook/src/expertise_warn.rs` `Server::spawn` sets `CREATE_NO_WINDOW` on Windows
  (`#[cfg(windows)]`), matching the existing `spawn_detached` pattern.
- AC3: behavior on macOS/Linux is unchanged (the flag/option is a no-op there); the full existing suites
  stay green (launcher.test.js, hook cargo tests, fmt/clippy).
- AC4: no new dependency; the launcher stays Node-built-ins-only (gsm-12).

## Constraints
- Do not alter stdio wiring, exit-code passthrough, or the #32 signal forwarding.
- `windowsHide`/`CREATE_NO_WINDOW` only affect Windows console visibility — no functional change elsewhere.

## Propagation
- Launcher fix ships in the plugin file; repos adopt it on `--sync` (which copies the launcher). The hook
  fix ships in the `genesis-hook` binary; repos adopt it on `--sync` (which re-stages the binary). fih picks
  both up on its next open.

## Notes
- Not covered here: Claude Code launching `node genesis-memory.js` for the MCP server / hook events — that
  spawn's window flag is the host's, outside the genesis repo. If a window remains after this fix, that is
  the cause and must be confirmed on Windows.
- Cannot be reproduced on Linux/WSL (no Windows console); verified as a code-fact fix + confirmed on Windows.
