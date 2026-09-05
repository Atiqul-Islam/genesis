# Feature/Bug: promoted-main + subagent hooks route via the cross-platform `--run-hook` shim (issue #24)

## Problem
`settings.json` (promoted main) and subagent frontmatter bake a per-OS binary path
(`genesis-hook` vs `genesis-hook.exe`, chosen at compile time by `hook_bin()`'s `cfg!(windows)`). Those
files are committed and travel across systems, but the suffix is OS-specific — so a repo shared across OSes
gets wrong-OS hook paths → hooks fail (`Exec format error`, fail-open → enforcement silently off).

## Expected behavior
Hook commands invoke the launcher's existing `--run-hook` shim, which resolves the correct per-OS
`genesis-hook[.exe]` at RUNTIME. The generated `settings.json` / frontmatter become OS-agnostic — one
committed file works on Windows, macOS, and Linux.

## Acceptance criteria
- AC1: `main_thread_hooks` command hooks are `node "<home>/bin/genesis-memory.js" --run-hook <sub> …` —
  NO literal `genesis-hook`/`genesis-hook.exe` binary path in any command.
- AC2: `frontmatter` (subagent) command hooks likewise route via `--run-hook`; no baked binary path.
- AC3: every routed command still carries its subcommand + args verbatim (inject/gate/validate/
  enforce-research/capture-session/expertise-warn/precompact/reflect-surface) and the `--main-agent <name>`
  marker (so `main_settings` replace + `demote` remove stay idempotent).
- AC4: the `type: agent` hooks (reviewer, Mneme reflection) are unchanged (they run no binary).
- AC5: the SessionStart `--sync` command (already `node … --sync`) is unchanged.
- AC6: existing promoted repos auto-migrate: `sync_settings` replaces old baked-path commands with the
  shim form (idempotently; the `--main-agent` marker matches both), and `demote` still removes them.
- AC7: the same committed `settings.json` resolves the right binary on a `.exe` and a no-ext platform
  (the shim does OS resolution) — verified by an e2e on the real `sync-settings` output.

## Implementation
- New helper `run_hook(home, rest) -> "node <q(launcher)> --run-hook <rest>"` in `render.rs`.
- `frontmatter` + `main_thread_hooks`: build every genesis-hook command via `run_hook`; drop `hook_bin()`.
- Keep `--main-agent <name>` inside `rest` for the promoted-main commands.
- Launcher `--run-hook` already resolves the staged binary per-OS (`bin/genesis-memory.js`); no launcher change.

## Constraints
- Fail-open preserved (`--run-hook` exits 0 if the staged binary is unresolved).
- `node` is already required; no new dependency.
- Cross-platform mandatory; do not break `main_settings` idempotence or `demote`.
