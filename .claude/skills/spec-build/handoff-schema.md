# Handoff Schema

Strict contracts for messages between the supervisor and the five specialist agents. Both sides MUST conform — agents that return malformed verdicts get a `repair_verdict` directive from the supervisor and may not proceed.

---

## Supervisor → Agent: Directive Block

Every `Task` spawn and every `SendMessage` from the supervisor uses this shape:

```json
{
  "directive": "<verb specific to the agent's role>",
  "run_id": "<timestamp>-<feature-slug>",
  "iteration": <int, 1-based>,
  "mode": "first | normal | resume | terminate",
  "context": {
    "user_intent": "<verbatim block of user-ratified facts>",
    "prior_artifacts": [
      { "path": "<absolute or repo-relative>", "kind": "spec|feature|unit_test|src|doc" }
    ],
    "diagnostic_from_prior_agent": { ... } | null,
    "corrections": [
      { "claim": "<thing the agent stated>", "user_correction": "<truth from user>" }
    ]
  },
  "expected_response_schema": "verdict-v1"
}
```

### Directive verbs by agent

| Agent | Verbs |
|---|---|
| spec-agent | `draft_spec`, `revise_spec`, `compile_to_gherkin`, `regenerate_stubs`, `repair_spec` |
| dev-agent | `start_tdd_loop`, `fix_unit_test`, `fix_failing_scenario`, `simplify`, `unbreak`, `fix_regression`, `fix_review_findings` |
| verify-agent | `red_check`, `tdd_green_check`, `full_green_check`, `verify_post_simplify`, `regression` |
| review-agent | `run_review` (combines /spec-crap + /code-review) |
| docs-agent | `update_docs`, `capture_learnings` |
| any | `checkpoint`, `terminate`, `repair_verdict` |

---

## Agent → Supervisor: Verdict Block

Every agent response MUST be a JSON code block conforming to this schema:

```json
{
  "agent": "spec-agent | dev-agent | verify-agent | review-agent | docs-agent",
  "iteration": <int>,
  "status": "PASS | FAIL | NEEDS_INPUT | ERROR",
  "next_responsibility": "<keyword matching a row in routing-table.md>",
  "diagnostic": {
    "summary": "<one line, ≤120 chars, suitable for timeline.html>",
    "details": { ... role-specific shape ... },
    "artifacts": [
      { "path": "...", "kind": "spec|feature|unit_test|src|doc|report" }
    ],
    "assumptions_made": [
      { "claim": "<what the agent assumed>", "evidence": "<why it assumed>" }
    ]
  }
}
```

### Status semantics

| Status | Meaning | Supervisor action |
|---|---|---|
| `PASS` | Agent's task succeeded; ready for next phase | Route via table |
| `FAIL` | Agent's task did not succeed; diagnostic must say why | Route per table (usually back to dev-agent) |
| `NEEDS_INPUT` | Agent cannot proceed without user clarification | Supervisor asks user; sends corrections back to this agent |
| `ERROR` | Unrecoverable: agent crashed, tooling unavailable, contradiction in input | Supervisor halts, asks user how to proceed |

### `next_responsibility` keyword

This MUST be one of the values listed in the `next_responsibility` column of `routing-table.md`. If an agent has no preferred next step, it returns the natural-flow value for its current phase (e.g., spec-agent after `compile_to_gherkin` returns `red_check`).

### Per-agent `diagnostic.details` shapes

**spec-agent (draft response):**
```json
{
  "spec_path": "test/specs/<name>.md",
  "spec_content": "<full markdown of draft>",
  "audit_targets": [
    { "marker_type": "invented_identifier | number_without_origin | implementation_specific | unconfirmed_edge_case | external_dependency | compound_requirement",
      "claim": "<the suspicious statement>",
      "line": <int>
    }
  ]
}
```

**spec-agent (compile response):**
```json
{
  "feature_path": "test/features/<name>.feature",
  "unit_test_path": "test/unit/test_<name>.py",
  "scenarios_generated": <int>,
  "steps_reused": <int>,
  "steps_created": <int>,
  "unit_stubs_generated": <int>
}
```

**dev-agent:**
```json
{
  "iteration_in_loop": <int>,
  "changed_files": ["..."],
  "tests_targeted": ["..."],
  "summary": "<what was changed and why>",
  "simplify_diff_summary": "<if simplify directive>"
}
```

**verify-agent:**
```json
{
  "phase": "red_check | tdd_green | full_green | verify_post_simplify | regression",
  "bdd": { "total": <int>, "passed": <int>, "failed": <int>, "failing_scenarios": [{"name": "...", "error": "..."}] },
  "unit": { "total": <int>, "passed": <int>, "failed": <int>, "failing_tests": [{"name": "...", "error": "..."}] },
  "log_validation": { "verdict": "CLEAN | ISSUES | SKIPPED", "issues": ["..."] }
}
```

**review-agent:**
```json
{
  "crap_report": {
    "max_crap": <float>,
    "max_crap_function": "<file>:<name>",
    "above_target_count": <int>,
    "above_alert_count": <int>,
    "above_fail_count": <int>,
    "offenders": [{"file": "...", "function": "...", "cc": <int>, "coverage": <float>, "crap": <float>}]
  },
  "findings": [
    { "confidence": <int 0-100>, "severity": "low|medium|high",
      "file": "...", "line": <int>, "issue": "...", "suggested_fix": "..." }
  ],
  "blocking": <bool>
}
```

**docs-agent:**
```json
{
  "docs_updated": ["docs/architecture.md", ...],
  "claude_md_updated": <bool>,
  "structural_changes_detected": ["..."]
}
```

---

## Repair Protocol

If supervisor receives a verdict that fails JSON parsing or violates the schema:

1. Log to timeline.html: `verdict_repair_requested: <agent>, reason: <parse_error|schema_violation>`
2. SendMessage to agent: `{ directive: "repair_verdict", reason: "...", malformed_response_excerpt: "..." }`
3. Agent re-emits the verdict in correct shape.
4. If repair fails twice in a row → escalate to user as ERROR.

This protocol keeps the supervisor strict about structured data without making agents brittle on the first try.
