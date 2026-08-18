---
name: grill
description: The exhaustive onboarding interview a built engineer runs to gain complete project expertise when there is no code to deep-read — an empty target folder. Use when deep-read is impossible; ask the user everything, one question at a time, and do not finish until every topic is answered.
---

# The grill — exhaustive onboarding interview

## Purpose
When the target folder is EMPTY — no git repository, no non-binary source to enumerate and read —
deep-read is impossible. `grill` is how you gain COMPLETE project expertise instead: an exhaustive
interview with the user, in place of reading files you do not have. Never guess what a deep-read would
have told you; ask the user directly.

## Topics to cover in full (the checklist)
Work through every item below. None may be skipped, assumed, or inferred:
1. **Goals & success criteria** — what the project is for; what "working" and "done" mean to the user.
2. **Intended stack + EXACT versions** — every language, framework, library, and service, pinned to the
   exact version the user intends (never "latest" — a version you can later cite as `crate@version` /
   `pkg@version`).
3. **Architecture & components** — the intended structure: modules/services, how they talk, data flow.
4. **Conventions & coding standards** — style, naming, formatting, lint gates, house rules to enforce.
5. **Testing & CI approach** — test framework(s), coverage expectations, what must be green and when.
6. **Deployment & operations** — how it ships, where it runs, who/what triggers a release, rollback plan.
7. **Done-criteria** — the concrete, checkable definition of "this feature/task is finished."
8. **Escalation triggers** — what the user wants asked/confirmed before you act (irreversible actions,
   scope changes, anything outward-facing).

## Method
- Ask **ONE question at a time** — never bundle multiple topics into a single message.
- **NEVER speculate.** If an answer is ambiguous, incomplete, or you are tempted to fill a gap yourself,
  ask a follow-up instead of assuming.
- Work the checklist in order, but follow the conversation — a user's answer may raise a new question on
  the same topic; ask it before moving to the next topic.
- **Do not finish until every topic is answered.** Track which of the 8 topics still lack a real answer
  and keep asking — ensure it gets all questions answered before you declare onboarding done.

## Capture
- As each answer lands, **store it to memory under your own `agent_id`** (a durable fact/decision, per
  your memory-management expertise) — never leave an answer only in the transcript.
- Once every topic is answered, **author a project-knowledge module from the captured answers** — the
  same kind of guide + enforceable rules the deep-read branch would have produced via `research-expertise`,
  but sourced from this interview instead of files. This becomes the engineer's project/stack expertise.
- Never write a credential value into memory or the module — reference it as "credential present at
  `<path>`".

## Completion — the checklist you mark off
Before declaring onboarding done, confirm every line is true:
- [ ] Goals & success criteria answered.
- [ ] Intended stack + exact versions answered.
- [ ] Architecture & components answered.
- [ ] Conventions & coding standards answered.
- [ ] Testing & CI approach answered.
- [ ] Deployment & operations answered.
- [ ] Done-criteria answered.
- [ ] Escalation triggers answered.
- [ ] Every answer stored to memory under your `agent_id`.
- [ ] A project-knowledge module authored from the captured answers.

Onboarding is complete only when every box above is checked — not before.
