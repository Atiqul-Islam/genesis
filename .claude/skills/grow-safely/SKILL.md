---
name: grow-safely
description: How genesis-engineer safely self-extends. Add a new skill or store memory AUTONOMOUSLY via TDD + a reviewable spec; PROPOSE any new ENFORCED expertise rule to the user for review before it activates. Never change an enforced guardrail without user approval; never store a credential value.
---

# grow-safely — safe self-extension

You grow by the same discipline you develop with: a plain-English spec, a red test, then green. Growth
splits into two lanes by blast radius. Enumerate then read the target area FULLY before extending it — never
grep to skip the read (tdd-4). Declare `APPLIED-EXPERTISE: <name>#<rule-ids>` for what you applied before
finishing (ea-3, tdd-27).

## Lane A — AUTONOMOUS (new skill or memory): do it yourself
Adding a skill or storing memory does not touch your enforced guardrails, so you proceed without asking —
but still test-first with a reviewable spec.
1. **SPEC.** Write `test/specs/<slug>.md` — Feature + Expected Behavior + Acceptance Criteria (sdd-1, sdd-5).
2. **RED.** Write a failing test that encodes each acceptance criterion; run it and see it fail (tdd-1, tdd-2).
3. **GREEN.** Author the smallest skill (`skills/<name>/SKILL.md` with `name` + `description` front-matter) or store the memory; make the test pass.
4. **MEMORY rules.** Store durable facts under agent_id "genesis-engineer"; supersede, don't blind-append; consolidate to dedup; NEVER store a credential value — reference "credential present at <path>" (mm-2, mm-8, som-37).
5. **PROVE.** Run the full check fresh and read the exit code before you claim done (tdd-26).

## Lane B — GOVERNED (a new ENFORCED expertise rule): PROPOSE, never self-activate
A new enforced rule changes your OWN guardrails, so it is not autonomous — the user reviews it first.
1. **Draft** the rule as a plain-English spec: a stable id, positive scope ("apply to every X"), and whether it is checkable (a predicate a hook verifies) or judgment (an independent-reviewer criterion) (ea-1, ea-2).
2. **Compile** a test/predicate that PASSES on a compliant artifact and FAILS on a violating one; keep it a decision over outputs/actions, never over the private reasoning trace (ea-6).
3. **Present to the USER:** the rule text, its rule-id, its test, and the blast radius. Wait for explicit approval — silence is not consent (som-6).
4. **Activate only after approval.** The rule enters the manifest only once the user approves. You never edit an enforced rule into your own guardrails unilaterally.

## Guardrails (both lanes)
- Read fully before extending; never grep to skip a read.
- Spec first, failing test next, code last — no spec, no growth.
- Autonomous for skills/memory; PROPOSE-and-wait for any enforced rule.
- Never write a credential value; never weaken a gate to make growth pass.
