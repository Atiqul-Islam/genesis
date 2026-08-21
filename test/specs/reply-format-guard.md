## Feature: reply-format guard — bullets ≤ 20 words (genesis-engineer)

genesis-engineer's replies to the user must be tables or bullet points, **≤ 20 words per point** (user
hard rule, 2026-08-21). The Stop/validate hook flags any bullet in the CURRENT turn's visible reply that
exceeds 20 words, for agents on a format list. It is **conservative**: only markdown bullets are checked;
code fences, `APPLIED-EXPERTISE` lines, table rows, headers, and prose are exempt — so it never false-blocks
on those. The existing `stop_hook_active` guard prevents any permanent block-loop.

### Expected Behavior

- For an agent on the format list (`genesis-engineer`), a visible bullet over 20 words blocks finishing,
  with a reason to split it into shorter points.
- Bullets ≤ 20 words, tables, fenced code, headers, and `APPLIED-EXPERTISE` lines never trigger it.
- Agents not on the list are unaffected.
- Only the current turn's assistant text is checked; fail-open if the transcript is unreadable.

### Acceptance Criteria

- **AC1** — `overlong_bullets` flags a 25-word bullet; a 20-word bullet is allowed.
- **AC2** — a bullet inside a ``` fenced code block is not flagged.
- **AC3** — an `APPLIED-EXPERTISE:` line and a table row (`| … |`) are not flagged.
- **AC4** — `format_reasons("genesis-engineer", …)` returns a reason for an over-long bullet;
  `format_reasons("method", …)` returns none.
- **AC5** — validate blocks `genesis-engineer` when the current turn's reply has an over-long bullet
  (spawned-binary integration test).

### Implementation Requirements

- `validate.rs`: `BULLET_FORMAT_AGENTS = ["genesis-engineer"]`; `overlong_bullets(text)` — structural,
  skips code fences + `APPLIED-EXPERTISE` lines + non-bullet lines; `format_reasons(active, text)`;
  `current_turn_visible_text(transcript)` reusing the turn-scoping (`turn_start`); wired into `run()`'s
  reasons before the block decision.
- Conservative scope: only markdown bullets (`- `/`* `/`+ `/`N. `/`N) `) are checked; exactly 20 words is
  allowed; tables/headers/prose are never flagged.
- Fail-open on an unreadable transcript. `stop_hook_active` already prevents a permanent block-loop.

### References

`hook/src/validate.rs` (`parse_declarations` turn-scoping, `run`), `memory/response-format-bullets-or-table.md`.
