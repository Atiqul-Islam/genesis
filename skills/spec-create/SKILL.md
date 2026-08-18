---
name: spec-create
description: Create or update a plain English feature spec
context: full
---

# Spec Create Skill

Create or update a plain English feature specification in `test/specs/`. Specs are the source of truth — all implementation flows from them.

## Usage

```
/spec-create <feature-name-or-description>
```

**Examples:**
```
/spec-create chart date range filtering
/spec-create session management
/spec-create bug: chart crashes on empty data
```

The feature name/description is provided via `$ARGUMENTS`.

## Workflow

### Phase 1: Parse Arguments and Check for Existing Spec

1. **Parse `$ARGUMENTS`** to extract the feature name or description.
   - If no arguments provided, use AskUserQuestion to ask what feature to spec.
   - Derive a kebab-case filename from the feature name (e.g., "chart filtering" → `chart-filtering`).

2. **Check if `test/specs/<feature-name>.md` already exists** using Glob.
   - If it exists, read it and present to the user: "This spec already exists. Would you like to update it or create a new one?"
   - Use AskUserQuestion with options: "Update existing", "Create new with different name"

### Phase 2: Draft the Spec

3. **Draft the spec** using this template:

```markdown
# test/specs/<feature-name>.md

## Feature: <Feature Title>

<1-2 sentence description of what this feature does and why.>

### Expected Behavior
- <Observable behavior 1>
- <Observable behavior 2>
- <Edge case behavior>
- <Error behavior>

### Acceptance Criteria
- <Testable criterion 1>
- <Testable criterion 2>
- <Testable criterion 3>

### Implementation Requirements (optional)
- <Non-UI requirement, e.g., "Passwords are stored with bcrypt hashing">
- <Infrastructure requirement, e.g., "Credentials are stored in PostgreSQL, not in-memory">
- <Performance constraint, e.g., "Search queries return within 200ms for up to 10,000 records">
```

**Guidelines for writing specs:**
- Expected Behavior describes WHAT the user sees, not HOW it's implemented
- Each behavior should be observable in the UI — if it's not UI-observable, it belongs in Implementation Requirements
- Every Expected Behavior bullet MUST have a corresponding Acceptance Criterion. If a behavior cannot be expressed as a testable UI criterion, move it to Implementation Requirements
- Include at least one happy path, one edge case, and one error case
- Acceptance Criteria are concrete, UI-testable conditions (these become Gherkin scenarios)
- Implementation Requirements (optional) capture non-UI concerns: storage backends, hashing algorithms, performance constraints, etc. These are tested via language-appropriate unit tests (`pytest`; Rust: `#[test]` fns), NOT Gherkin scenarios
- `/spec-compile` will generate a language-appropriate unit-test stub for each Implementation Requirement (`pytest.fail(...)`; Rust: `#[test]` with `unimplemented!("Implement via TDD")`) — write them as concrete, testable assertions
- Each Implementation Requirement should be independently verifiable — avoid compound requirements (e.g., split "Passwords are hashed with bcrypt and salted" into two separate requirements)
- Use the user's natural language — no technical jargon

4. **For bug fix specs**, use this adjusted template:

```markdown
# test/specs/<bug-name>.md

## Bug: <Bug Title>

<Description of the bug and how to reproduce it.>

### Current Behavior
- <What currently happens (the bug)>

### Expected Behavior
- <What should happen instead>

### Reproduction Steps
- <Step-by-step to trigger the bug>

### Acceptance Criteria
- <The bug scenario — this becomes a reproducer test>
- <Any regression scenarios>
```

### Phase 3: Review with User

5. **Present the draft spec** to the user for review using AskUserQuestion:
   - Show the full spec content
   - Options: "Looks good — save it", "I have changes"
   - If the user has changes, incorporate them and present again

6. **Write the final spec** to `test/specs/<feature-name>.md`.

7. **Report next step**: "Spec saved to `test/specs/<feature-name>.md`. Next: run `/spec-compile <feature-name>` to generate Gherkin scenarios."

## WORKFLOW COMPLETE

After saving the spec, this skill workflow is FINISHED. Return to normal conversation mode.
