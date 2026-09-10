# Bug: the `--run-hook` shim orphans its hook child on interrupt — ESC doesn't cancel hooks (#32)

## Type
Bug fix. Cross-platform (macOS / Windows / Ubuntu).

## Bug
Since #24 (beta.15) promoted-main + subagent hooks run through the Node shim
`node "<home>/bin/genesis-memory.js" --run-hook <sub> …`. The shim's `execPassthrough`
(`bin/genesis-memory.js`) spawns the staged `genesis-hook` binary as a child but installs **no signal
forwarders**. When the process running a hook is interrupted/terminated (e.g. the user presses ESC and
Claude Code terminates the hook), the Node wrapper dies but its hook child is **orphaned and keeps
running**.

Reproduced against the real launcher: spawning `--run-hook <slow-child>`, then killing the wrapper with
SIGTERM, leaves the child alive to the end of its work (`CHILD_SURVIVED_FULL_SLEEP`). A repo that runs the
`genesis-hook` binary **directly** (no shim) does not have this — ESC kills the hook cleanly (observed in
the genesis repo itself, which still uses baked binary paths). The server path (`execServer`) already
forwards signals to its child; the hook/cli shim path does not — that asymmetry is the defect.

## Expected behavior
When the process running a hook via the shim is interrupted/terminated, the shim forwards the termination
to its child so the hook stops too — no orphan. ESC cancels a shim-run hook just as it cancels a
directly-run binary. Identical on macOS, Windows, and Ubuntu.

## Acceptance criteria
- AC1: the shim (`--run-hook` / `--run-cli`) forwards catchable termination signals
  (SIGINT/SIGTERM/SIGHUP/SIGQUIT/SIGBREAK) to its spawned child; when the wrapper is signalled, the child
  is killed — no orphan.
- AC2: a fail-before/pass-after regression test spawns `--run-hook` with a long-lived child, kills the
  wrapper, and asserts the child did NOT survive (it was terminated).
- AC3: normal exit-code passthrough is unchanged (child exit 2 → shim exit 2; fail-open exit 0 when the
  hook binary is unresolved; fail-loud exit 1 for `--run-cli` with a missing override).
- AC4: the server path (`execServer`) keeps its existing signal forwarding, now shared with the shim via
  one `forwardSignals` helper — no behavior change to the server path.
- AC5: cross-platform — the forwarder set includes SIGBREAK (Windows) and skips signals unsupported on the
  host; no native dependency (the launcher stays Node-built-ins-only, gsm-12).

## Implementation Requirements
- Extract a `forwardSignals(child)` helper in `bin/genesis-memory.js` (installs the listeners, returns a
  remover) used by BOTH `execServer` and `execPassthrough`.
- `execPassthrough` installs the forwarders and removes them on child exit, matching `execServer`.

## Constraints
- Fail-open preserved: a missing/broken hook binary still exits 0 (never breaks a session).
- No new dependency; Node built-ins only (gsm-12); STDOUT stays pristine (gsm-16).
- Windows: SIGTERM is not deliverable as a catchable signal in Node; forwarding uses the signals Node
  supports there (SIGINT/SIGBREAK) — best-effort, matching the server path which already ships this way.

## Propagation
- The fix is in `bin/genesis-memory.js` (the launcher). It ships in a release; every promoted repo picks it
  up on open via `--sync`, which copies the current launcher into `.genesis/bin/genesis-memory.js`. The fih
  repo adopts it on its next open on its own OS.

## Notes
- This fixes the interrupt/ESC-cancellation defect for the shimmed COMMAND hooks. The reviewer/Mneme
  `type: agent` hooks are cancelled by Claude Code natively (observed: ESC stops the Stop chain in the
  genesis repo); only the shimmed command hooks were orphaning.
- Regression source: #24 (beta.15) introduced the shim without porting the server path's signal forwarding.
