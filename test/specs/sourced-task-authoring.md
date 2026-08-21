## Feature: sourced, zero-speculation task authoring (skill + issue template + hard rule)

Every Genesis issue, task hand-off, and skill spec must be **self-contained**: it carries all details plus
links to the reference/source documents a **zero-context** autonomous agent needs to complete it — no
tribal knowledge. And a **hard rule**: the executing agent **never speculates**; when a required fact is
not in the task or its cited sources, it **consults the developer-in-charge** (escalates and waits) rather
than guessing. (Implements issue #6. Motivated by a verified incident, 2026-08-21: an agent grepped a file
instead of reading it and asserted an unverified architectural claim as fact.)

### Expected Behavior

- A new `sourced-task-authoring` skill guides authoring any issue/task/skill to the standard: the required
  sections and the no-speculation + consult-the-developer hard rule.
- The skill states plainly that **resolving an issue is NOT building it**: the agent gathers all details +
  sources, understands the problem, presents resolution OPTIONS, and CONSULTS the developer on which
  resolution — it never defaults to, encourages, or pre-frames a build. Building happens only on an
  explicit "build/implement" instruction. The agent never speculates on the developer's intent.
- A GitHub issue template requires those sections, so filed issues are self-contained.
- The no-speculation hard rule is documented as a hard rule in the skill and the contributor guide.
- Mechanical enforcement of no-speculation *beyond* the existing surface (the shipped no-grep guard + the
  independent review hook) is PROPOSED for maintainer review, not added unilaterally (grow-safely / ea-6).

### Acceptance Criteria

- **AC1** — `skills/sourced-task-authoring/SKILL.md` exists with valid frontmatter (name + description) and
  specifies the required sections (Problem, Evidence, Reproduction, Proposed resolution, Acceptance
  criteria, Constraints, References) and the hard rule.
- **AC2** — a GitHub issue template (`.github/ISSUE_TEMPLATE/…`) requires those sections.
- **AC3** — the skill instructs: cite Evidence as `path:line` or a link, **verified by reading — never
  inferred**; a zero-context agent must have every source it needs to complete the task without asking
  where to look.
- **AC4** — the no-speculation / consult-the-developer hard rule is stated in the contributor guide
  (`CONTRIBUTING.md`) and cross-referenced from the skill.
- **AC5** — a structural test asserts the skill exists, has the required frontmatter, and contains the
  hard-rule statement + a References requirement.

### Implementation Requirements

- *(ratified choice)* Skill name `sourced-task-authoring`, a plugin skill under `skills/`. Adding a skill is
  free per grow-safely (via TDD + this reviewable spec).
- *(ratified choice)* Issue template as a GitHub **Issue Form** (`.github/ISSUE_TEMPLATE/task.yml`) with a
  required field per section; keep the blank-issue option enabled.
- *(INFERRED — verify at build, do NOT assume)* The structural test hooks into the existing plugin test
  suite. The exact mechanism will be confirmed by **reading `hooks/test_plugin.js` and
  `hooks/test_vendored_skills.js` in full** before writing the test — not assumed here.
- **Out of scope (proposed separately, ea-6):** a general *enforced* no-speculation gate. A semantic "no
  speculation" cannot be fully mechanized; the enforcement surface stays the no-grep guard +
  `enforce_research`/review hooks + this skill's discipline. Any new enforced check is proposed to the
  maintainer with its own spec + test + blast radius.

### References

Issue #6; `skills/build-agent/SKILL.md` + `skills/build-engineer/SKILL.md` (skill-authoring pattern);
`hooks/hooks.json` (review hook); `hook/src/enforce_research.rs` + `test/specs/no-grep-guard.md` (the shipped
mechanical facet); `.genesis/expertise/spec-driven-development.md` (grounding/no-invention);
`memory/no-grep-verify-before-claim.md` (the incident).
