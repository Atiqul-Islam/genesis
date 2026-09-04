# Feature 2 — Memory & Expertise redesign (SPEC — locked, phased build)

> Promoted from DRAFT to SPEC after the design audit (workflow wf_e7db9df9-90c: 4 audits + 3 designs).
> Locked decisions come from the user; architecture choices below are labelled where they are the
> engineer's call (sdd-9/sdd-10). Built in phases; each phase ships spec → red → green before the next.

## Problem
Expertise today = plain `.md` guides + JSON manifests read by the hooks (validate/gate/inject). There is
no mechanism for the agent to LEARN durable rules from conversation. The user wants: expertise held in
SQLite, learnable (Mneme proposes after every turn), user-approved, enforced thereafter, propagated to all
repos, and a one-time retro-learn sweep across existing conversations/facts for every agent in every repo.

## Locked decisions (user)
- Two memory kinds: raw **conversation history** (plain files, as-is) + **expertise** (= facts, one bucket).
- Expertise lives in **SQLite**; **hooks read SQLite** (user chose full migration over additive).
- Learned rules are **ENFORCED** (can block), via the existing declare-and-quote mechanism.
- **Global by default**; task/feature-scoped only on user request or via contradiction→specialize.
- After every user input **Mneme** analyses the new turn (with prior context), identifies candidate
  expertise, and **asks the user first** (autonomous); **user-directed "memorize"** is written directly.
- **Contradiction** → show both rules in plain English + a 1-line conflict note → specialize or replace.
- Mneme keeps its **own plain-file log** of what it processed.
- Propagate to **all repos + agents** on update; run a **retro-learn** sweep across the system.

## Locked architecture (engineer's calls, labelled)
- **[choice] `expertise.db` = the runtime store the hooks read**, built (migrated) from a committed
  substrate: the existing `manifests/*.json` + `required.json` + `*.md` guides, plus a new committed
  `learned.jsonl` (learned rules). Mirrors `memory.db`(queried) + `memory.jsonl`(portable committed source).
  Rationale: honours "hooks read SQLite" AND keeps a portable, drift-checked, mergeable substrate.
- **[choice] Fail-open, DB-first / file-fallback readers** (strangler shim): every reader keeps its exact
  signature; tries `expertise.db`, else runs today's file body. A missing/broken DB never bricks the Stop
  hook. Rationale: reversibility guards the guard stack (this feature rewrites the enforcer of every agent).
- **[choice] `expertise.db` is gitignored + regenerable**; `learned.jsonl` is committed. Tests compare a
  canonical LOGICAL dump, never raw `.db` bytes (SQLite bytes are non-deterministic).
- Migration is idempotent (`source_sha` no-op) + reversible (`--export` regenerates the committed text
  byte-for-byte, so existing drift tests stay green).
- Propagation rides the EXISTING launcher `--sync` channel (add a `migrate-expertise` step in `syncRepo`);
  no new settings.json hook → no `main_settings`/`demote`/`sync_settings` churn.
- Mneme's reflection loop triggers on **Stop / SubagentStop** ("after every user input" = the Stop of the
  turn that answered it); it is non-blocking, fail-open, redacted, idempotent; proposals surface to the
  user via the main thread (Mneme has no SendMessage); approved writes go through a CLI (single-writer),
  never a hook. Enforced only after the DB refresh.
- Retro-learn is an on-demand skill (report-first, per-item approval, per-repo, bi-temporal, redacted);
  a learned rule is proposed only to its origin repo unless the user explicitly propagates it.

## Honest scope limits (state up front)
- "All repos in the system" = `.genesis` repos discoverable on THIS machine within a user-chosen scan
  scope. Cannot reach a repo never opened here, or one only used on another machine.
- Learns only from CAPTURED conversation (`.genesis/sessions/`, `resume-state.md`) or transcripts still
  present in `~/.claude/projects` (Claude Code may have rotated older ones).
- The candidate-JUDGMENT step is non-deterministic (one LLM call); determinism/idempotence are enforced at
  the WRITE layer (content_id + user approval), not at detection.

## Phased build (each phase: spec → red → green → gate; guard stack never left broken)
- **Phase A — expertise→SQLite foundation.** `cli` migrate/export + schema; `hook/expertise_db.rs`
  read-only queries + rusqlite (bundled, =0.39.0, same pin as cli/server) + `expertise-db` cargo feature;
  DB-first/file-fallback shims in validate/gate/inject; bootstrap + `syncRepo` propagation; release-time
  plugin-root DB. Spec: `test/specs/expertise-sqlite-migration.md`.
- **Phase B — Mneme reflection loop.** `hook reflect`/`reflect-surface`; `.genesis/mneme/` log+state+queue;
  `cli expertise-learn` (write/approve/retire learned.jsonl → re-migrate); Mneme agent wiring; contradiction
  flow. Spec: `test/specs/mneme-reflection-loop.md`.
- **Phase C — system-wide retro-learn.** `/genesis:retro-learn` skill: bounded repo enumeration, per-repo
  read, report-first HTML, per-item approval, bi-temporal apply. Spec: `test/specs/retro-learn-sweep.md`.

## Acceptance criteria (top-level; per-phase specs refine these)
- AC1: After migration, validate/gate/inject enforce EXACTLY the migrated rules (DB path == file path).
- AC2: A missing/corrupt `expertise.db` → readers fall back to files → enforcement unchanged (no brick).
- AC3: `migrate-expertise --export` regenerates required.json + manifests byte-identically (no-drift).
- AC4: Re-running migration is a no-op (source_sha); DB canonical logical dump is byte-identical.
- AC5: A `learned.jsonl` active rule becomes an enforced rule; a `proposed` one does NOT enforce.
- AC6: `/plugin update` migrates every promoted repo via `--sync` with no settings.json change.
- AC7: Mneme, on Stop, proposes a durable rule; user-directed "memorize" writes directly; autonomous asks.
- AC8: A contradiction surfaces both rules + a 1-line conflict; specialize/replace both bi-temporal.
- AC9: Retro-learn is read-only until per-item approval; nothing written/committed without the user.

## Open items resolved by default (not stopping; labelled)
- Learned rule-id format: per-bucket sequential `[a-z]+-[0-9]+`, monotonic, never reused (matches the
  declaration regex). A new logical rule gets the next id; an edit keeps its id (supersede).
- `expertise.db` file: sibling of `memory.db` under `.genesis/expertise/expertise.db` (gitignored).
- "Enforced" for a natural-language learned rule = the declare-and-quote guard (soft; honest per ea-11),
  NOT a mechanical predicate — same enforcement current judgment rules get.
