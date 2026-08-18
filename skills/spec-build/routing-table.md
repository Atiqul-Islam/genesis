# Routing Table

The supervisor consults this table on every agent verdict. **Routing is deterministic — never LLM judgment.** The supervisor reads the table and dispatches; it does not improvise.

## Lookup Rules

| Current phase | Agent verdict status | next_responsibility | → next agent | Notes |
|---|---|---|---|---|
| Spec Discovery | NEEDS_INPUT | clarify | (user via AskUserQuestion) | Audit found hallucination markers |
| Spec Discovery | PASS | compile | spec-agent (compile mode) | Spec ratified by user |
| Compile | PASS | red_check | verify-agent | Feature + stubs generated |
| Compile | FAIL | repair_spec | spec-agent | bddgen failed or missing steps |
| RED check | PASS (all fail) | tdd_loop | dev-agent | RED is healthy |
| RED check | FAIL (some pass) | regenerate_stubs | spec-agent | Tests don't actually test |
| TDD loop | PASS (test green, more tests left) | next_test | dev-agent | Continue inner loop |
| TDD loop | PASS (all unit tests green) | green_check | verify-agent | Move to outer loop |
| TDD loop | FAIL | fix_unit_test | dev-agent (with diagnostic) | Failed assertion |
| GREEN check | PASS | simplify | dev-agent (simplify mode) | BDD + unit all pass |
| GREEN check | FAIL | fix_failing_scenario | dev-agent (with diagnostic) | BDD scenarios still failing |
| Simplify | PASS | verify_post_simplify | verify-agent | Simplification applied |
| Verify post-simplify | PASS | regression | verify-agent (full suite) | Simplification didn't break |
| Verify post-simplify | FAIL | unbreak | dev-agent (with diagnostic) | Simplification broke something |
| Regression | PASS | review | review-agent | All tests green across suite |
| Regression | FAIL | fix_regression | dev-agent (with diagnostic) | New change broke an old test |
| Review | PASS | docs | docs-agent | CRAP ≤ 8, no ≥80-confidence findings |
| Review | FAIL (blocking) | fix_review_findings | dev-agent (with findings) | CRAP > 8 or blocking finding |
| Docs | PASS | finalize | (supervisor finalization) | Docs synced |
| Any | ERROR | escalate | (supervisor halts, asks user) | Agent reported ERROR — unrecoverable |

## Routing Algorithm (supervisor pseudocode)

```
on verdict V from agent A:
  log V to timeline.html
  update state.json (agent_iterations[A] += 1, last_verdicts[A] = V)

  if V.status == "ERROR":
    halt; ask user via AskUserQuestion how to proceed

  if V.status == "NEEDS_INPUT":
    ask user via AskUserQuestion (one question per item in V.diagnostic.details)
    on user response: SendMessage(A, { directive: "incorporate_corrections", corrections: [...] })
    continue loop

  # PASS or FAIL → route by table
  next = lookup(current_phase, V.status, V.next_responsibility)
  if next is user:
    AskUserQuestion(...)
  else:
    SendMessage(next, {
      directive: <derived from routing entry>,
      diagnostic: V.diagnostic,  # forward verbatim
      iteration: state.agent_iterations[next] + 1
    })
```

## Forbidden Behavior

- **Never** decide routing from V.diagnostic.summary text — only from V.status + V.next_responsibility.
- **Never** skip a row in this table because "it feels redundant."
- **Never** add new rows without updating the supervisor SKILL.md.
- **Never** route a FAIL to the same agent that produced it without supplying the diagnostic verbatim.
