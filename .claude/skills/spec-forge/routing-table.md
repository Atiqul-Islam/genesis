# Routing Table — `/spec-forge`

Same routing rules as `/spec-build`, with two added rows for the new phases (Phase 0a Worktree, Phase 2.5 Plan).

## Lookup Rules

| Current phase | Agent verdict status | next_responsibility | → next agent | Notes |
|---|---|---|---|---|
| Initiation | — | worktree | (supervisor invokes `superpowers:using-git-worktrees`) | Phase 0a |
| Worktree | PASS | spawn_agents | (supervisor spawns 5 forge-agents) | |
| Worktree | ERROR | escalate | (supervisor halts, asks user) | Worktree creation failed |
| Spec Discovery | NEEDS_INPUT | clarify | (user via AskUserQuestion) | Audit markers |
| Spec Discovery | PASS | compile | forge-spec-agent (compile mode) | Spec ratified |
| Compile | PASS | plan | (supervisor invokes `superpowers:writing-plans`) | **Phase 2.5 new** |
| Compile | FAIL | repair_spec | forge-spec-agent | |
| Plan | PASS | red_check | forge-verify-agent | Plan written to `docs/superpowers/plans/` |
| Plan | ERROR | escalate | (supervisor halts) | Plan-skill failed |
| RED check | PASS (all fail) | tdd_loop | forge-dev-agent (execute_plan_task task 1) | RED healthy |
| RED check | FAIL (some pass) | regenerate_stubs | forge-spec-agent | |
| TDD loop | PASS (task complete, more tasks left) | next_task | forge-dev-agent (next task) | |
| TDD loop | PASS (all tasks done) | green_check | forge-verify-agent | Move to outer loop |
| TDD loop | FAIL (iter 1-2) | fix_unit_test | forge-dev-agent (with diagnostic) | Quick retry |
| TDD loop | FAIL (iter 3+) | systematic_debug | forge-dev-agent (invoke systematic-debugging) | **Switch from guessing to scientific method** |
| TDD loop | ERROR (architecture problem from systematic-debugging) | escalate | (supervisor asks user) | 3+ fixes failed — pattern wrong |
| GREEN check | PASS | simplify | forge-dev-agent (simplify mode) | |
| GREEN check | FAIL | fix_failing_scenario | forge-dev-agent (with diagnostic) | |
| Simplify | PASS | verify_post_simplify | forge-verify-agent | |
| Verify post-simplify | PASS | regression | forge-verify-agent (full suite) | |
| Verify post-simplify | FAIL | unbreak | forge-dev-agent (with diagnostic) | |
| Regression | PASS | review | forge-review-agent | |
| Regression | FAIL | fix_regression | forge-dev-agent (with diagnostic) | |
| Review | PASS | docs | forge-docs-agent | |
| Review | FAIL (blocking) | fix_review_findings | forge-dev-agent (invoke receiving-code-review) | |
| Docs | PASS | finalize | (supervisor invokes `superpowers:finishing-a-development-branch`) | **Augmented Phase 11** |
| Finalize | PASS | terminate | (terminate all agents) | |
| Any | ERROR | escalate | (supervisor halts, asks user) | |

## Routing Algorithm (same as /spec-build, with new transitions)

```
on verdict V from agent A:
  log V to timeline.html
  update state.json (agent_iterations[A] += 1, last_verdicts[A] = V)

  if V.status == "ERROR":
    halt; ask user via AskUserQuestion how to proceed

  if V.status == "NEEDS_INPUT":
    ask user via AskUserQuestion
    SendMessage(A, { directive: "incorporate_corrections", corrections: [...] })

  # PASS or FAIL → route by table
  next = lookup(current_phase, V.status, V.next_responsibility)

  # The two new branches:
  if next == "supervisor invokes superpowers:writing-plans":
    invoke via Skill tool; on completion advance phase
  if next == "supervisor invokes superpowers:finishing-a-development-branch":
    invoke via Skill tool; on completion terminate agents

  # Existing branches as in /spec-build
  if next is user:
    AskUserQuestion(...)
  else:
    SendMessage(next, { directive: <derived>, diagnostic: V.diagnostic, iteration: <next> })
```

## TDD Loop Iteration Counter (the key behavioral knob)

The TDD-loop FAIL row branches on iteration count, NOT on the supervisor's judgment:

- Iterations 1–2: retry with diagnostic. Same as /spec-build.
- Iteration ≥ 3: **dev-agent must invoke `superpowers:systematic-debugging`** before another fix. This is a hard rule, encoded in dev-agent's persona. The supervisor's routing simply passes a flag (`systematic_debug: true`) in the directive.

This is the single most important divergence from /spec-build's routing: it prevents the "keep trying random fixes" thrash pattern by forcing scientific method after 2 failures.

## Forbidden Behavior (same as /spec-build)

- **Never** decide routing from V.diagnostic.summary text — only from V.status + V.next_responsibility + iteration count.
- **Never** skip a row in this table because "it feels redundant."
- **Never** add new rows without updating the supervisor SKILL.md.
- **Never** route a FAIL to the same agent that produced it without supplying the diagnostic verbatim.
