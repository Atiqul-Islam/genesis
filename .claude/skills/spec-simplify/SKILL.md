---
name: spec-simplify
description: Simplify implementation code using the built-in /simplify skill
context: full
---

# Spec Simplify Skill

Run the built-in `/simplify` skill on implementation code. Placed after IMPLEMENT and before GREEN so that tests validate the simplified code, not the raw first-pass.

## Usage

```
/spec-simplify              Simplify all recently modified implementation files
/spec-simplify app.py       Simplify a specific file
/spec-simplify config/      Simplify all files in a directory
```

The arguments are provided via `$ARGUMENTS`.

## Workflow

### Phase 1: Determine Scope

1. **Parse `$ARGUMENTS`**:
   - If a specific file or directory is given, use that as the scope
   - If no arguments, scope to all recently modified implementation files

2. **Scope rules** — only implementation files:
   - **Include**: Implementation source files (typically `src/`, `app/`, `lib/`, `config/`)
   - **Exclude**: `test/` (specs, features, steps, unit tests), `target/`, `.claude/`, `docs/`, `Cargo.lock`
   - **Note**: Unit test files (`test/unit/test_*.py`) are test artifacts and must never be simplified — they are verified in the GREEN step

3. **If a requested file falls outside scope**, warn the user and skip it. Specs, features, and step definitions must never be simplified — they are test artifacts.

### Phase 2: Run Simplifier

4. **Invoke the built-in `/simplify` skill** using the `Skill` tool with `skill: "simplify"`:
   - `/simplify` spawns parallel review agents to check for code reuse, quality, and efficiency, then applies fixes
   - It operates on recently modified code by default, which aligns with the scoped files from Phase 1

5. **Review the changes** — verify the simplifier did not:
   - Alter any test files (specs, features, steps)
   - Change external interfaces or API contracts
   - Remove functionality

### Phase 2.5: Quality Check

After `/simplify` runs, verify the output against the Code Quality Standards in CLAUDE.md. If any violations are found (oversized functions, repeated patterns, inline magic numbers), fix them before proceeding to Phase 3.

### Phase 3: Report Results

6. **Present results** to the user:

**If changes were made:**
```
Simplified implementation code:
- <file>: <brief description of changes>
- <file>: <brief description of changes>

Ready for GREEN step — run /spec-test to validate.
```

**If no changes needed:**
```
No simplification needed — implementation code is already clean.

Ready for GREEN step — run /spec-test to validate.
```

## Rules

- NEVER simplify files in `test/` (specs, features, steps, data)
- NEVER change external behavior — simplification is cosmetic/structural only
- NEVER add new features or functionality during simplification
- If unsure whether a file is in scope, skip it and inform the user

## WORKFLOW COMPLETE

After reporting results, this skill workflow is FINISHED. Return to normal conversation mode.
