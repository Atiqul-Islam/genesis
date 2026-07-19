# Handoff Schema — `/spec-forge`

Identical structure to `/spec-build`'s schema, with two added fields:

- Directives include a `superpowers_skills_to_invoke: []` list when the supervisor wants an agent to invoke specific skills (e.g., for dev-agent's `execute_plan_task` directive, the supervisor specifies `["superpowers:test-driven-development", "superpowers:verification-before-completion"]`).
- Verdicts include a `skills_invoked: []` list so the supervisor can verify the agent actually followed the discipline.

See `.claude/skills/spec-build/handoff-schema.md` for the base schema; only the deltas are documented here.

## Supervisor → Agent: Directive Block (delta)

```json
{
  "directive": "<verb>",
  "run_id": "...",
  "iteration": <int>,
  "mode": "first | normal | resume | terminate",
  "context": {
    "user_intent": "...",
    "prior_artifacts": [...],
    "diagnostic_from_prior_agent": null,
    "corrections": [...],
    "plan_path": "docs/superpowers/plans/<date>-<slug>.md",       // NEW: plan-aware
    "task_number": <int>,                                          // NEW: which task in plan
    "worktree_path": "...",                                        // NEW: isolation
    "systematic_debug": <bool>                                     // NEW: TDD iter >= 3 flag
  },
  "superpowers_skills_to_invoke": ["superpowers:test-driven-development", "..."],  // NEW
  "expected_response_schema": "verdict-v1"
}
```

## Agent → Supervisor: Verdict Block (delta)

```json
{
  "agent": "forge-<role>-agent",
  "iteration": <int>,
  "status": "PASS | FAIL | NEEDS_INPUT | ERROR",
  "next_responsibility": "<keyword>",
  "diagnostic": {
    "summary": "...",
    "details": { ... },
    "artifacts": [...],
    "assumptions_made": [...],
    "skills_invoked": ["superpowers:test-driven-development", "..."]   // NEW: audit trail
  }
}
```

## New Directive Verbs (in addition to /spec-build's set)

| Agent | New verbs (forge-specific) |
|---|---|
| forge-dev-agent | `execute_plan_task` (executes one task from writing-plans output), `systematic_debug` (forces 4-phase debugging) |
| forge-review-agent | `local_review` (local subagent code review via requesting-code-review skill, not `/code-review` plugin) |

All other verbs are inherited from /spec-build's schema.

## Per-Agent Verdict Detail Shapes (delta)

**forge-dev-agent (execute_plan_task):**

```json
{
  "iteration_in_loop": <int>,
  "task_number": <int>,
  "task_summary": "<task title from plan>",
  "changed_files": ["..."],
  "tests_targeted": ["..."],
  "summary": "<what was changed and why>",
  "skills_invoked": ["superpowers:test-driven-development",
                     "superpowers:verification-before-completion"],
  "commit_sha": "<created on task completion>"
}
```

**forge-dev-agent (systematic_debug):**

```json
{
  "iteration_in_loop": <int>,
  "systematic_phases_completed": ["phase_1_root_cause",
                                  "phase_2_pattern_analysis",
                                  "phase_3_hypothesis",
                                  "phase_4_implementation"],
  "root_cause": "<identified root cause>",
  "single_fix_applied": "<minimal fix>",
  "skills_invoked": ["superpowers:systematic-debugging",
                     "superpowers:test-driven-development",
                     "superpowers:verification-before-completion"]
}
```

If `systematic-debugging` reports "architecture problem" (3+ fixes failed), the verdict is `status: ERROR, next_responsibility: escalate` with details about why the architectural pattern looks wrong.

**forge-review-agent (local_review):**

```json
{
  "crap_report": { ... same as /spec-build ... },
  "code_reviewer_findings": [
    { "severity": "Critical | Important | Minor",
      "file": "...", "line": <int>, "issue": "...", "suggested_fix": "..." }
  ],
  "base_sha": "<git rev-parse HEAD~N>",
  "head_sha": "<git rev-parse HEAD>",
  "blocking": <bool>,
  "skills_invoked": ["superpowers:requesting-code-review"]
}
```

`blocking: true` if any function has CRAP > 8 OR any Critical/Important finding from the reviewer subagent.

## Repair Protocol

Same as /spec-build.

## Why Track `skills_invoked`

Two reasons:

1. **Audit trail for the timeline.** When the user looks at `timeline.html` and sees a dev-agent verdict, they can see exactly which discipline skills the agent invoked. If dev-agent claims PASS without `superpowers:verification-before-completion` in the list, the supervisor downgrades to NEEDS_INPUT and re-issues the directive.
2. **Behavior parity check.** If a future run reports different results than a prior run on the same feature, comparing `skills_invoked` lists across runs surfaces which discipline was skipped.
