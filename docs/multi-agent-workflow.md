# Multi-Agent Workflow (`/spec-build`)

A supervisor-led orchestration of the spec-driven development workflow. Same 9-step behavior as manual mode, restructured into five persistent specialist agents coordinated by a supervisor.

For the underlying methodology see [`spec-driven-development.md`](spec-driven-development.md). For the project contract see [`../CLAUDE.md`](../CLAUDE.md).

## When to Use Which Mode

| Mode | Command | When to use |
|---|---|---|
| **Multi-Agent — Lean** | `/spec-build <description>` | Default. Fewest plugin dependencies. Conversational, audited, observable. |
| **Multi-Agent — Forge** | `/spec-forge <description>` | Same orchestration + `superpowers` discipline at every step. Requires the superpowers plugin. |
| **Manual** | `/spec-create`, `/spec-compile`, `/spec-test`, `/spec-simplify`, `/spec-crap`, `/docs-update` | Power-user escape hatch. Same behavior; you drive each step. |

All three modes preserve identical gates and thresholds — no behavioral difference, only orchestration.

## Architecture

```
                    user
                     │
              /spec-build <description>
                     │
                     ▼
              ┌──────────────┐
              │  Supervisor  │  only voice the user hears
              │ (spec-build) │  conversational, audits drafts, routes by responsibility
              └──────┬───────┘
                     │ Task (spawn once) + SendMessage (subsequent directives)
       ┌─────────┬───┴────┬──────────┬──────────┐
       ▼         ▼        ▼          ▼          ▼
   spec-agent dev-agent verify-agent review-agent docs-agent
   (persistent for the whole workflow; silent workers)

   run state + audit log: .planning/builds/<run-id>/
```

## The Five Specialists

| Agent | Inlines | Returns |
|---|---|---|
| `spec-agent` | `/spec-create` + `/spec-compile` | spec content, audit_targets, feature path, unit-stub path |
| `dev-agent` | TDD loop (Three Laws) + `/spec-simplify` | changed files, iteration counter, diff summary |
| `verify-agent` | `/spec-test` (incl. log validation) | structured BDD + unit + log verdict |
| `review-agent` | `/spec-crap` + `/code-review` plugin | CRAP report + filtered findings + blocking flag |
| `docs-agent` | `/docs-update` + optional `/revise-claude-md` | updated docs list |

Each agent persona lives in `.claude/skills/<agent>/SKILL.md`. Each is spawned once at workflow start via `Task` and persists across the run, receiving directives via `SendMessage` until terminated at finalization or checkpoint.

## The Supervisor's Three Jobs

The supervisor (`.claude/skills/spec-build/SKILL.md`) does exactly three things:

1. **Conversational front-end.** Only voice the user hears. Asks clarifying questions, surfaces decisions, gates checkpoints.
2. **Audit gate.** After every spec-agent draft, scans for hallucination markers (invented identifiers, numbers without origin, implementation specifics, unconfirmed edge cases, external dependencies, compound requirements). Each flagged marker becomes an `AskUserQuestion` for the user to ratify or correct.
3. **Responsibility router.** Reads structured agent verdicts and dispatches the next agent by lookup table at `.claude/skills/spec-build/routing-table.md`. **Never improvises routing.**

## Handoffs

Strict contracts in `.claude/skills/spec-build/handoff-schema.md`:

**Supervisor → Agent (directive):**
```json
{
  "directive": "<verb>", "run_id": "...", "iteration": N, "mode": "first|normal|resume|terminate",
  "context": {
    "user_intent": "...", "prior_artifacts": [...],
    "diagnostic_from_prior_agent": {...}, "corrections": [...]
  }
}
```

**Agent → Supervisor (verdict):**
```json
{
  "agent": "...", "status": "PASS|FAIL|NEEDS_INPUT|ERROR",
  "next_responsibility": "<routing-table keyword>",
  "diagnostic": { "summary": "...", "details": {...}, "artifacts": [...], "assumptions_made": [...] }
}
```

Diagnostics are forwarded **verbatim** when routing FAIL back to a fixer agent — the supervisor never summarizes away detail.

## Run State on Disk

Every run produces:

```
.planning/builds/<run-id>/
├── state.json           — current phase, iteration counters, last verdicts, artifact paths
├── timeline.html        — append-only HTML log of every supervisor decision and agent verdict
├── CHECKPOINT.md        — written when checkpoint is taken (resume entry point)
└── <agent>-checkpoint.json / <agent>-final.json — per-agent state
```

The HTML timeline is the primary debugging surface. Open it in a browser to see the chronological flow of decisions, verdicts, user questions, and checkpoints.

State is **append-only and survives context compaction.** If a run is interrupted, `/spec-build --resume <run-id>` rebuilds the supervisor's understanding from these files and re-spawns all five agents with their checkpoint state.

## Hallucination Audit (the new safety gate)

Manual mode trusts you to catch when the spec author over-specifies. Multi-agent mode catches it deterministically.

After every spec-agent draft, the supervisor scans for six marker types:

| Marker | Example |
|---|---|
| `invented_identifier` | `users` table, `auth_middleware.py` — neither in user message nor codebase |
| `number_without_origin` | "lock after 5 attempts", "200ms timeout" |
| `implementation_specific` | "bcrypt hashing", "PostgreSQL storage", "Redis cache" |
| `unconfirmed_edge_case` | "what if username is empty/null" |
| `external_dependency` | "calls SendGrid API to email user" |
| `compound_requirement` | "passwords are hashed AND salted AND rate-limited" |

Each marker becomes an `AskUserQuestion` prompt. The user ratifies, corrects, or removes. Spec-agent revises via `SendMessage`. The audit re-runs. Loop until clean.

This is why multi-agent mode produces tighter specs than manual mode on the same description.

## Checkpoint and Resume

Supervisor offers checkpoint at workflow boundaries (post-spec, post-RED, post-GREEN, post-REVIEW, post-DOCS) and on the high-iteration heuristic (`dev_iter >= 4` with verify still failing).

If the user accepts:
1. Supervisor writes `CHECKPOINT.md`.
2. All five agents write their own state to `<agent>-checkpoint.json`.
3. Supervisor terminates all agents.
4. User runs `/compact` then `/spec-build --resume <run-id>`.
5. Supervisor re-spawns all five agents with `mode: "resume"` and their prior context.
6. Workflow continues from `CHECKPOINT.md`'s `next_planned_agent + next_directive`.

## Routing Table

The supervisor consults `.claude/skills/spec-build/routing-table.md` on every verdict. Routing is **deterministic** — never LLM judgment. See that file for the complete `(phase, status, next_responsibility) → next agent` lookup.

## How Multi-Agent Maps to the 9-Step Workflow

| Supervisor phase | 9-step name | Agent doing the work |
|---|---|---|
| 0 Initiation | — | (supervisor only) |
| 1 Spec Discovery + audit | 1 SPEC | spec-agent |
| 2 Compile | 1 SPEC | spec-agent |
| 3 RED check | 2 RED | verify-agent |
| 4 TDD inner loop | 3 TDD LOOP | dev-agent + verify-agent |
| 5 GREEN | 4 GREEN | verify-agent |
| 6 Simplify | 5 SIMPLIFY | dev-agent |
| 7 Verify post-simplify | 6 VERIFY | verify-agent |
| 8 Regression | 7 REGRESSION | verify-agent |
| 9 Review | 8 REVIEW | review-agent (runs `/spec-crap` + `/code-review`) |
| 10 Docs | 9 DOCS | docs-agent |
| 11 Finalize | — | (supervisor only) |

Every gate is the same as manual mode. CRAP > 8 still fails. `/code-review` ≥80-confidence findings still block. RED still fails before GREEN. The only differences:

- The supervisor enforces the order (you can't accidentally skip RED).
- The hallucination audit catches over-specification before downstream work.
- A timeline.html records the full flow.
- A run can be paused and resumed across `/compact`.

## Coexistence With Manual Mode

The original slash commands (`/spec-create`, `/spec-compile`, `/spec-test`, `/spec-simplify`, `/spec-crap`, `/docs-update`, `/app-restart`) remain fully functional. They are the manual escape hatches.

Multi-agent mode **inlines** the equivalent behavior into each specialist's `SKILL.md` rather than calling the slash commands. This means agent skills are the canonical source of truth for orchestrated runs.

**Caveat:** if you fix a bug in `/spec-test` (manual mode), also fix the equivalent block in `verify-agent/SKILL.md` (orchestrated mode), or behavior will drift between modes.

## The Forge Variant (`/spec-forge`)

`/spec-forge` is a sibling of `/spec-build`. **Same 9-step behavior**, **same gates**, **same `.planning/builds/<run-id>/` state structure**. The differences are all about *discipline of execution*:

| Phase | `/spec-build` | `/spec-forge` adds |
|---|---|---|
| 0 Initiation | Spawn 5 agents | + 0a: invoke `using-git-worktrees` for isolation |
| 1 Spec Discovery | Multi-round audit | Optional pre-step: `brainstorming` if description is vague |
| 2.5 Plan | — | **NEW:** invoke `writing-plans` → `docs/superpowers/plans/<date>-<slug>.md` |
| 4 TDD inner loop | dev-agent iterates by test | forge-dev-agent executes plan tasks one-by-one with `test-driven-development`, commits per task |
| 4 TDD loop (iter ≥ 3) | dev-agent retries | **forge-dev-agent invokes `systematic-debugging`** — 4-phase scientific method; escalates on architectural issues |
| Every dev verdict | declared in persona | forge-dev-agent invokes `verification-before-completion` before claiming PASS |
| 9 Review | review-agent invokes `/code-review` plugin (**requires GitHub PR**) | forge-review-agent invokes `requesting-code-review` (**local subagent**, no PR needed) |
| 9 Review FAIL | dev-agent applies fixes | forge-dev-agent invokes `receiving-code-review` first — verifies findings before complying |
| 11 Finalize | Report summary | + invoke `finishing-a-development-branch` for merge/PR/cleanup choice |

### Why two variants?

The honest answer: `/spec-build` is the floor (lean, fewer gates) and `/spec-forge` is the ceiling (enforces every discipline at every gate). The discipline skills are **bundled with Genesis** (vendored from the MIT-licensed superpowers project — see `NOTICE.md`), so both variants work out of the box with no external plugin required.

`/spec-forge` is recommended for production work because the disciplines actively prevent the most common failure modes:

- **TDD without `test-driven-development` skill:** dev-agent's persona declares Three Laws but Claude can drift over a long session. The skill enforces them fresh on each task.
- **Stuck loops without `systematic-debugging` skill:** dev-agent at iter 3+ will keep guessing. The skill forces root-cause analysis and surfaces architectural problems.
- **False completion claims without `verification-before-completion`:** dev-agent claims PASS without running the verification command, leading to "should work" rationalizations.
- **Code review without a PR:** the `/code-review` plugin can't run on a local branch; `requesting-code-review` does the same job locally.

### When NOT to use `/spec-forge`

- You don't have the `superpowers` plugin installed.
- You're prototyping and want minimal overhead per task.
- You're already in a worktree (the Phase 0a worktree skill is a no-op then, but the rest of the discipline still runs).

## Verifying the Multi-Agent Workflow Locally

- Unit tests for the timeline writer: `pytest test/unit/test_timeline_writer.py -v`
- End-to-end: invoke `/spec-build <small feature description>` and watch `timeline.html` populate as the supervisor routes between agents.
- Manual parity: run the same feature through both modes on separate branches; diff the produced artifacts. Substantive content should match.
