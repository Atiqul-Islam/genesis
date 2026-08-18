# Spec: plugin update heals a stale managed .gitignore (so memory travels)

## Type
Feature (self-healing update) + bug fix (stale workspaces never commit `.genesis/memory.db`).

## Problem
The managed `.gitignore` block that `genesis-cli bootstrap` writes has evolved — it now re-includes
`!.genesis/memory.db` after `*.db` so the ready-to-use vector DB travels with the repo. But a repo
bootstrapped with an OLDER template keeps its stale block, and the update path never re-applies the
current one: the launcher's `--sync` (`bin/genesis-memory.js` `syncRepo`, run on SubagentStart / every
`/plugin update`) refreshes only the staged `.genesis/bin` binaries — it does not touch `.gitignore`.
Result: an already-bootstrapped repo (e.g. one whose block predates `!.genesis/memory.db`) silently
keeps ignoring `.genesis/memory.db`, so its memory DB is never committed and does not travel.

## Expected behavior
1. A focused, idempotent command regenerates ONLY the managed `.gitignore` block (between the genesis
   sentinels) to the current template, preserving everything outside the sentinels. The block's
   definition stays single-sourced in Rust (`bootstrap::gitignore_block`) — no duplicate in the launcher.
2. The launcher's `--sync` invokes that command on every update, so a plain `/plugin update` heals every
   already-bootstrapped repo automatically — after which `.genesis/memory.db` is no longer ignored.
3. The heal is fail-open (a heal error must never break session start) and a no-op when the block is
   already current.

## Acceptance criteria
1. `genesis-cli sync-gitignore <target>` rewrites the managed block to exactly `bootstrap::gitignore_block()`.
2. Given a repo whose managed block is stale (has the sentinels but lacks `!.genesis/memory.db`), after
   `sync-gitignore` the `.gitignore` contains `!.genesis/memory.db` and `.genesis/memory.db` is no longer
   matched as ignored.
3. The user's own `.gitignore` lines outside the sentinels are preserved unchanged.
4. `sync-gitignore` is idempotent — a second run produces byte-identical output.
5. `bin/genesis-memory.js` `syncRepo` invokes `genesis-cli sync-gitignore <repo>` (repo = parent of the
   `.genesis` home) after staging binaries, wrapped so any failure is swallowed (fail-open).

## Notes
- Scope is the managed `.gitignore` block ONLY. The heal must NOT re-copy expertise/hooks — those are
  committed and may be locally customized; overwriting them on update would be wrong.
- Fixing the `.gitignore` makes `memory.db` committable; the actual commit still happens through the
  normal memory-commit flow. This spec closes the mechanism gap, not the commit action.
- Propagation: this changes launcher behavior, so it takes effect once released and a repo updates to the
  fixed launcher (which `syncRepo` copies in), then heals on the next genesis subagent start.
