## Feature: cross-system conversation resume (issue #9)

The live Claude Code transcript is machine-local (`~/.claude/projects/<encoded-cwd>/<id>.jsonl`) and does
not travel with the repo, so `claude -c` / `claude --resume` finds nothing after a clone/pull on another
system (incl. WSL↔Windows). This carries the transcript IN the repo and, on the target, drops it into the
place Claude Code already looks — so native resume works.

### Expected Behavior

- The current session's transcript is captured into the repo (`.genesis/sessions/<id>.jsonl`), committed,
  and travels with git.
- On the target system, at session start, the committed transcript(s) are restored into that machine's
  `~/.claude/projects/<encoded-cwd>/`, and the user is notified with the exact `claude --resume <id>`.
- After restore, native `claude -c` / `claude --resume <id>` continues the conversation.
- Dormant for non-genesis sessions; fail-open (a capture/restore error never breaks a session).

### Acceptance Criteria

- **AC1** — `encode_project_dir("/mnt/c/Users/x/proj")` == `-mnt-c-Users-x-proj` (each `/`→`-`; `\`→`-` too).
- **AC2** — `genesis-hook capture-session` copies the event's `transcript_path` to
  `<repo>/.genesis/sessions/<basename>` (dormant if no genesis agent; fail-open if the file is missing).
- **AC3** — restore copies every `<repo>/.genesis/sessions/*.jsonl` into `<home>/.claude/projects/<encoded-cwd>/`,
  skipping ones already present, and returns the restored session ids.
- **AC4** — the SessionStart inject output includes a resume notice naming `claude --resume <id>` when a
  transcript was restored; nothing when there are none.
- **AC5** — `.gitignore` managed block allowlists `.genesis/sessions/` so transcripts are committed.

### Implementation Requirements

- New `hook/src/session_transfer.rs`: `encode_project_dir(path)`; `capture(repo, transcript_path)`;
  `restore(repo, home, cwd) -> Vec<String>` (restored ids); `resume_notice(ids) -> String`. Pure + unit-tested.
- New `genesis-hook capture-session [--main-agent <name>]` subcommand: resolve the active agent (dormant if
  none); read `transcript_path`; `capture` it into `<cwd>/.genesis/sessions/`. Fail-open. Wired in main.rs.
- `hook/src/inject.rs`: on SessionStart `source` startup|resume, run `restore` for the repo and append the
  resume notice to `additionalContext` (within the CTX cap). Never on other sources.
- `cli/src/bootstrap.rs` gitignore block: add `!.genesis/sessions/` so the committed transcripts travel
  (rides the existing `/genesis:sync` commit+push; no new command). Propagates on-open per the feature standard.
- Wiring: this targets the MAIN conversation of a promoted-main repo, so it lives in
  `cli/src/render.rs::main_thread_hooks` only — a Stop hook running `capture-session --main-agent <name>`
  (restore is `inject` on SessionStart). The plugin `hooks.json` stays dormant (core subagents are
  transient; not resume material). Restore rides the on-open binary `--sync`; capture (a settings.json Stop
  entry) reaches an already-promoted repo on its next re-promote.
- Honest limit: whether the SAME `claude -c` auto-continues depends on undocumented hook-vs-select timing;
  the notice always gives the user the exact `claude --resume <id>`, so resume works regardless.

### References

`hook/src/inject.rs`, `hook/src/precompact.rs`, `hook/src/agent.rs`, `hook/src/io.rs`,
`cli/src/bootstrap.rs` (gitignore), `cli/src/render.rs` (`main_thread_hooks`), `hooks/hooks.json`;
verified: transcripts at `~/.claude/projects/<encoded-cwd>/<id>.jsonl`, encoding `/`→`-`, `.jsonl` alone resumes.
