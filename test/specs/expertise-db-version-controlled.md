# Bug: bootstrap's managed .gitignore block ignores expertise.db (#23)

## Type
Bug fix (db-travels: the expertise store DB must be version-controlled, like memory.db).

## Bug
`bootstrap.rs::gitignore_block()` emits `*.db` (ignore every DB) and re-includes only `!.genesis/memory.db`.
`.genesis/expertise/expertise.db` is therefore ignored in every bootstrapped repo — so genesis-engineer's /
any built agent's expertise store (incl. learned rules) does NOT travel with the repo, violating the
db-travels rule (memory.db AND expertise.db must both be committed).

## Expected behavior
The managed `.gitignore` block re-includes `.genesis/expertise/expertise.db` (alongside `!.genesis/memory.db`),
so both store DBs are committed and travel with the repo. Machine-local junk (bin binaries, sessions, temp
exports, stray DBs elsewhere) stays ignored.

## Acceptance criteria
1. `gitignore_block()` output contains `!.genesis/expertise/expertise.db`.
2. After `merge_gitignore` / `sync-gitignore`, `.genesis/expertise/expertise.db` is NOT ignored by git, while
   `.genesis/bin/<binary>` and a stray `.db` elsewhere ARE still ignored.
3. `!.genesis/expertise/expertise.db` appears AFTER `*.db` in the block (last-match-wins re-include).
4. Idempotent: a second `sync-gitignore` is byte-identical (existing no-drift behavior preserved).

## Implementation Requirements
- Add the single re-include line to `gitignore_block()` after `!.genesis/memory.db`.
- Extend the existing `sync_gitignore_heals_stale_block...` integration test to assert the expertise.db
  re-include.

## Notes
- Pairs with the repo-level `.gitignore` fix already applied to the genesis repo this session; this fixes the
  SOURCE so every future bootstrapped repo inherits it.
