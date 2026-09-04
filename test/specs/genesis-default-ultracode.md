# Requirements capture (DRAFT — gathering, NOT a spec yet): Genesis agents default to ultracode

> Zero speculation. Below is the user's stated intent + open questions. Do not implement from this yet.

## Stated by the user (their words)
- ALL instances of Genesis start with **ultracode** by default.
- EXCEPT **Mneme**, which starts at **xhigh** effort.
- The default holds **until/unless the user says otherwise or changes it**.
- Implication: if a Genesis agent is a repo's **MAIN** agent, opening Claude Code should **always trigger
  ultracode**.

## OPEN QUESTIONS (must resolve — no guessing)
- Q1 — What does "ultracode" map to as a SETTING? The Workflow **ultracode MODE** (author/run workflows by
  default), a reasoning-**EFFORT** level, or **both**? (It's listed next to "xhigh", which is an effort level,
  so the two are being mixed.)
- Q2 — CAN "ultracode"/effort be set automatically at SESSION START (via a SessionStart hook or
  settings.json), or is it only an in-session toggle? This is a Claude Code CAPABILITY to VERIFY, not assume
  (same class as the earlier "can a hook hide a tool card" question).
- Q3 — Scope: every Genesis agent (sensei, method, built engineers) = ultracode, and ONLY mneme = xhigh?
- Q4 — Override lifetime: "until the user changes it" — per-session, or persisted across sessions?
- Q5 — Propagation: a system-wide default across ALL Genesis repos (rides the update path)?

## ANSWERED (user)
- Q3: YES — all Genesis agents = ultracode; ONLY Mneme = xhigh.
- Q4: Override lasts **per-session**.
- Q5: **System-wide** across all repos (rides the update path).

## Q1 + Q2 — engineer to VERIFY (Claude Code capability; not the user's job)
- Q1: precise meaning of "ultracode" (workflow mode vs reasoning-effort level vs both); the real effort
  levels; whether ultracode and effort are independent settings.
- Q2: can ultracode mode and/or a default reasoning effort be set at SESSION START (SessionStart hook /
  settings.json / config), or only via an in-session command? Exact keys/commands. Also: can a SUBAGENT's
  effort (mneme=xhigh) be pinned in its agent-definition frontmatter?
- STATUS: dispatched to the claude-code guide for authoritative answers.

### Q1 — VERIFIED (from Genesis research docs citing Anthropic)
- "ultracode" = **xhigh reasoning effort + standing permission to launch multi-agent workflows** (BOTH,
  bundled). Source: `docs/research/expertise-application-opus4x.md:131`.
- Reasoning-effort levels = **low | medium | high | xhigh | max**; default **high**; effort is a SEPARATE
  setting from ultracode. Source: `expertise/token-efficiency.md:219-220` (Workflow tool contract).

### Q1 + Q2 — RESOLVED (verified vs LIVE Claude Code docs, 2026-09-04)
- **`ultracode` is a settings.json boolean key** (docs: settings-reference#ultracode):
  `{"ultracode": true}` → sessions START at `xhigh` effort, with workflow-planning on when dynamic
  workflows are enabled + the model supports `xhigh`. Scope = "Any file" (so `.claude/settings.json` works).
  Claude Code READS but NEVER WRITES it, so a per-session `/effort` override survives (matches Q4).
- ultracode is NOT a value of `effortLevel`/`CLAUDE_CODE_EFFORT_LEVEL` (those reject it); it's its own key.
- Effort levels = low|medium|high|xhigh|max (docs: model-config#adjust-effort-level). ultracode == xhigh + workflows.
- **Subagent frontmatter supports `effort:`** (docs: sub-agents supported frontmatter fields) → mneme=xhigh
  via `effort: xhigh` in `.claude/agents/mneme.md`.

### BUILD MECHANISM (feasible — reuses the #12 settings path)
- `render::main_settings` writes `"ultracode": true` into a promoted repo's `.claude/settings.json`.
- `mneme.md` frontmatter gets `effort: xhigh`.
- Propagates to all repos via the sync-settings path built for #12.
- CAVEAT: ultracode's WORKFLOW planning needs dynamic workflows enabled + an xhigh-capable model + a recent
  Claude Code build; the `xhigh` effort itself applies regardless.

## Note
- Feasibility of Q2 gates the whole feature: if Claude Code cannot set the mode/effort at session start,
  "opening claude always triggers ultracode" may not be achievable as stated — verify before speccing.
