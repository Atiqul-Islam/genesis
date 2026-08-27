# Feature: settings.json hook wiring refreshes on update (not just binaries)

## Context / Problem

Genesis distributes prebuilt binaries via GitHub Releases. A repo's `.genesis/bin` is refreshed by the
launcher's `--sync <home>` step (`bin/genesis-memory.js` `syncRepo`), which stages the binaries + heals the
managed `.gitignore` — but **never rewrites `.claude/settings.json`**. The main-thread hook WIRING is written
only once, by `render::main_settings`, at **promote time**.

Consequence (issues #12 / #3 / #10): when a release adds a new main-thread hook (e.g. issue #9
`capture-session` on Stop, issue #1 `precompact` on PreCompact, or the `--sync` hook itself), that hook
never reaches an already-promoted repo on update — the repo's `settings.json` is frozen at the template it
was promoted with. Verified live: the `ifs-fiber-insight` repo (agent `fih-engineer`) and this genesis repo
both have `settings.json` with only inject/gate/validate (+ reviewer) and no capture-session/precompact/`--sync`.

## Expected Behavior

1. A new CLI subcommand `genesis-cli sync-settings <repo-root>` refreshes the promoted agent's main-thread
   hooks in `<repo-root>/.claude/settings.json` to the CURRENT template, idempotently.
2. It detects the promoted agent from the managed `CLAUDE.md` sentinel block
   (`# >>> genesis agent: <name>`). If there is no managed block, it makes NO change (no-op).
3. It resolves the agent's required expertise from `<repo-root>/.genesis/expertise/required.json` so the
   review hook is regenerated correctly (present iff the agent has required expertise).
4. It PRESERVES the user's own hooks and any OTHER agent's entries — only this agent's main-thread entries
   are replaced (same idempotent semantics as `main_settings`).
5. The launcher's `--sync` step, after staging binaries, invokes `genesis-cli sync-settings <repo-root>` so a
   version bump refreshes `settings.json` too. Fail-open: a settings refresh must never break session start.
6. The `update-repo` skill runs stage + `sync-settings`, giving an already-frozen repo (whose `settings.json`
   lacks the `--sync` hook, so it never self-heals) a manual escape hatch.

## Acceptance Criteria

- AC1: Given a repo promoted with the OLD hook set (SessionStart=inject only, PreToolUse=gate,
  Stop=validate+reviewer, no PreCompact), when `sync-settings <repo>` runs, then `settings.json` gains, for
  that agent: a Stop `capture-session` command, a `PreCompact` `precompact` command, and a SessionStart
  `--sync` command — matching the current `main_settings` template.
- AC2: Given the repo's `settings.json` also holds a user's own hook (e.g. a custom Notification hook) and
  another agent's main block, when `sync-settings` runs, then both are preserved unchanged.
- AC3: `sync-settings` is idempotent — running it twice produces byte-identical `settings.json` (no
  duplicated hook entries).
- AC4: Given `required.json` maps the agent to a non-empty expertise list, the refreshed Stop hooks include
  the `type:"agent"` reviewer; given an empty/missing list, the reviewer hook is absent.
- AC5: Given a repo with NO managed `CLAUDE.md` block (agent not promoted here), `sync-settings` makes no
  change and reports it did nothing (exit 0, settings untouched).
- AC6: Given a `settings.json` that already has the CURRENT full hook set, `sync-settings` leaves it
  byte-identical (idempotent on an up-to-date repo).

## Implementation Requirements

- `sync-settings` reuses `render::main_settings` (the single source of the hook template) — it must NOT
  re-implement the hook JSON, so the promote path and the update path can never drift.
- Agent detection reuses the same sentinel parsing as `render::demote_claude_md` (single-sourced).
- `required.json` read is tolerant: a missing file or missing agent key yields an empty expertise list
  (review hook omitted), never a hard error.
- The launcher change mirrors the existing `sync-gitignore` invocation in `syncRepo` (spawn the staged
  `genesis-cli`, `stdio: ignore`, wrapped so a failure is swallowed — fail-open).
- No behavior change to promote, demote, or the binary-staging path.
