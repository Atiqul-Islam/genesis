## Feature: PreCompact resume — persist + restore session state across compaction (issue #1)

When a genesis session compacts, in-session state is lost. Verified Claude Code contract
(https://code.claude.com/docs/en/hooks.md): `PreCompact` fires before compaction with `transcript_path`
but its output is NOT injected afterward; the restore path is `SessionStart` with `source: "compact"` +
`additionalContext`, and all hook output is capped at 10,000 chars. So: CAPTURE to disk at PreCompact,
RESTORE (capped) via inject at the next compact/resume.

### Expected Behavior

- On compaction, a genesis session writes a resume snapshot of its recent state to disk.
- On the next session start with `source` compact or resume, that snapshot is injected back so the agent
  continues where it left off; the full snapshot remains on disk (a path pointer covers what exceeds the cap).
- Credential-shaped strings are redacted from the snapshot.
- Dormant for non-genesis sessions; fail-open (a hook error never breaks the session).

### Acceptance Criteria

- **AC1** — `genesis-hook precompact` for a genesis agent reads the transcript and writes
  `<repo>/.genesis/resume-state.md` containing the recent user/assistant conversation.
- **AC2** — a credential-shaped line in the transcript is redacted in the snapshot (no secret value written).
- **AC3** — `inject` on `source=compact` (or `resume`) includes the snapshot in `additionalContext` plus a
  pointer to `.genesis/resume-state.md`; on `source=startup` it does NOT (avoid stale re-injection).
- **AC4** — with no genesis agent active, `precompact` writes nothing and emits no decision (dormant); any
  error is swallowed (fail-open).
- **AC5** — total inject output stays within the existing CTX cap (≤ ~9500 chars); overflow is left on disk.

### Implementation Requirements

- New `hook/src/precompact.rs` (`genesis-hook precompact [--main-agent <name>]`): resolve the active agent
  (dormant if none); read `transcript_path`; extract the recent (role, text) turns up to a disk budget;
  redact credentials (same patterns as `gate`); write `agent::runtime_dir(cwd)/resume-state.md` with a
  header (ts, session, trigger). Fail-open. Register in `hook/src/main.rs` + `lib.rs`.
- `hook/src/inject.rs`: read the SessionStart `source`; when compact/resume and `resume-state.md` exists,
  append a resume block (capped, with the file path) to the injected context.
- `cli/src/render.rs::main_thread_hooks`: add a `PreCompact` hook running `genesis-hook precompact
  --main-agent <name>` (the promoted-main case, which is where compaction + the SessionStart restore live).
- Honest limit: additionalContext is capped at 10,000 chars — the full snapshot is on disk; only the
  recent slice is injected inline, with a path pointer to the rest.

### References

Claude Code hooks: https://code.claude.com/docs/en/hooks.md , https://code.claude.com/docs/en/hooks-guide.md ;
`hook/src/inject.rs`, `hook/src/gate.rs` (credential patterns), `hook/src/validate.rs` (transcript parsing),
`hook/src/agent.rs` (`runtime_dir`), `cli/src/render.rs` (`main_thread_hooks`), `hooks/hooks.json`.
