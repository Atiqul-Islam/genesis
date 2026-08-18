# test/specs/build-engineer.md

> **Status: DRAFT** — design ratified with the maintainer via the `brainstorming` skill on 2026-08-16;
> pending the maintainer's review of this written spec before an implementation plan is drawn.
> Grounded in the existing build surface (`commands/new.md`, the `build-agent` + `research-expertise`
> skills, the `genesis-cli` subcommands `assemble`/`promote`/`bootstrap`/`validate`) and in the
> just-completed manual `genesis-engineer` build. Read the Provenance legend before treating any claim as sourced.

## Feature: `/genesis:build-engineer`

A single slash-command that reproducibly builds a **complete engineer** — a senior developer that OWNS a
target project end to end (develops via strict TDD, follows spec-driven development, deploys, operates,
and grows itself) — from one fixed, opinionated recipe. It replaces the manual, many-step build we ran by
hand to produce `genesis-engineer`. It has **two modes**, chosen by the target's state:

- **deep-read mode** (the target is a real repo with source): the engineer gains expertise by reading
  every relevant non-binary file in full and researching the stack at exact pinned versions.
- **grill mode** (the target is an empty / no-repo folder): the engineer is still built, then gains its
  project expertise through an exhaustive onboarding interview ("grill") with the maintainer.

### Provenance legend (how to read this spec)

- **Sourced** — traceable to a committed file cited in place (`commands/new.md`, `skills/build-agent/SKILL.md`,
  `skills/research-expertise/SKILL.md`, a `genesis-cli` subcommand, or the `genesis-engineer` artifacts under
  `team/genesis-engineer/` and `.genesis/expertise/`).
- **(ratified decision — unsourced; from the 2026-08-16 brainstorming)** — a choice the maintainer approved in
  this session's Q&A. Labelled inline. These are decisions, not findings.

### Ratified decisions (2026-08-16 brainstorming)

- D1 **Fixed recipe, minimal asks.** The recipe (persona + behavior + expertise set) is baked in; the build asks
  only per-target specifics.
- D2 **Always a single engineer** (never a team) that MAY spawn helper subagents for large deep-reads.
- D3 **Install as a subagent, then OFFER to promote** to the folder's main Claude (one confirmation).
- D4 **Empty-folder mode = exhaustive onboarding grill** right after the build; ask-don't-assume is a universal rule.
- D5 **Expertise recipe** = generic reusable modules (authored once, shipped) `expertise-application`,
  `memory-management`, `test-driven-development`, `spec-driven-development`, `system-operation-maintenance`,
  `git-mastery` (new), `engineering-leadership` (new) + a per-target `<target>-stack-mastery` researched each build.
- D6 **Approach A** — a thin `commands/build-engineer.md` + a new `build-engineer` skill run by Sensei, reusing
  `research-expertise` + Method + the existing `assemble`/`promote`/`bootstrap` CLI; plus a new `grill` skill.

### Out of scope (v1)

- No new native `genesis-cli` subcommand — repo-vs-empty detection is done in the skill via `git`/`ls`
  (ratified decision; Approach A over C). New CLI work is deferred.
- No supervisor-led team output (D2). Team builds remain the domain of the open-ended `/genesis:new`.
- The two new generic modules (`git-mastery`, `engineering-leadership`) are authored generically; deepening
  any existing Genesis-specific module is not part of this feature.

## Expected Behavior

1. Running `/genesis:build-engineer` (optionally with a target path argument) starts a build that invokes the
   `sensei` agent, which follows the new `build-engineer` skill — mirroring how `commands/new.md` invokes Sensei.
2. The build detects its mode: a target that is a git repository and/or contains non-binary source files is
   **deep-read mode**; an empty folder with no repository is **grill mode**. A genuinely ambiguous target
   (e.g. only stray files, no git) makes the build ask the maintainer which mode to use rather than guess.
3. The build asks only the minimal per-target questions: the engineer's name (defaulting to `<repo>-engineer`),
   and confirmation of the single-agent + subagent install. It never re-asks the fixed-recipe choices (D1).
4. Before authoring, the build ensures the target has a `.genesis/` workspace (running `bootstrap` if missing)
   and makes the generic recipe expertise modules available in it.
5. In deep-read mode, the build reads every relevant non-binary file of the target in full (never grepping file
   content to skip a read) and researches the target's dependencies at their exact pinned versions, authoring a
   `<target>-stack-mastery` expertise module via the `research-expertise` procedure.
6. In grill mode, no stack research runs at build time; instead the finished engineer runs the `grill` skill —
   an exhaustive onboarding interview covering goals, intended stack, architecture, conventions, coding
   standards, done-criteria, and escalation triggers — and does not stop until it has the answers it needs,
   capturing them into its own memory (and authoring its project-knowledge/stack module from them).
7. Method authors the engineer test-first: a `persona.md` and `behavior.md` (each ≤200 lines) parameterised from
   the proven `genesis-engineer` template, a `meta.json` whose `tools` list omits `Grep` and whose
   `required_expertise` is the full recipe (D5), and the `grow-safely` + `grill` skills.
8. The engineer's built-in rules enforce the two hard rules in both modes: it never speculates (it reads
   everything or asks everything) and it never shortcuts (spec → RED → GREEN → refactor for all development;
   production-ready output).
9. The engineer is assembled as a subagent with full Genesis enforcement (inject/gate/validate + independent
   review), its acceptance tests are verified to pass, and the build then offers to promote it to the folder's
   main Claude.
10. The engineer can grow autonomously: it adds new skills and memory on its own, and proposes any new ENFORCED
    expertise rule to the maintainer for review before it activates.

## Acceptance Criteria

1. A file `commands/build-engineer.md` exists with valid front-matter (a `description`) and its body instructs
   invoking the `sensei` agent and following the `build-engineer` skill with the detected mode and fixed recipe.
2. A skill `skills/build-engineer/SKILL.md` exists with valid front-matter (`name`, `description`) whose body
   contains, in order, the mechanizable steps: detect-mode, minimal-asks, bootstrap, expertise (deep-read vs
   grill branch), delegate-to-Method, assemble-subagent, verify, offer-promote.
3. The `build-engineer` skill body specifies the mode-detection rule (git repo and/or non-binary source →
   deep-read; empty/no-repo → grill; ambiguous → ask) in checkable terms.
4. A skill `skills/grill/SKILL.md` exists with valid front-matter and a body covering an exhaustive onboarding
   interview whose topics include goals, stack, architecture, conventions, standards, done-criteria, and
   escalation, plus capturing answers into memory; and it states it does not finish until answered.
5. For each generic module in {`git-mastery`, `engineering-leadership`, `test-driven-development`,
   `spec-driven-development`, `system-operation-maintenance`}, the plugin's `expertise/<name>.md`,
   `expertise/manifests/<name>.json`, and a passing manifest test exist; each manifest matches the store schema
   (top-level `expertise`/`source`/`rules`/`sections_accounted`; every rule typed `checkable|judgment|principle`
   with a `predicate` or `reviewer_criterion` accordingly; unique ids; `sections_accounted` covers every guide
   section).
6. No file authored by this feature contains the banned reasoning-trace phrase named in the house rules (the one
   for which "structured reasoning" is the required substitute) or a credential value; the `persona.md` and
   `behavior.md` templates are each ≤200 lines.
7. The engineer template's `meta.json` `tools` array does not include `Grep`, and its `required_expertise` array
   equals the D5 recipe plus the per-target `<target>-stack-mastery` slot.
8. An integration check builds an engineer against a small real-repo fixture and produces a valid subagent file
   under the fixture's `.claude/agents/` whose front-matter validates and whose acceptance tests pass.
9. An integration check builds an engineer against an empty-folder fixture and produces a valid engineer that
   carries the `grill` skill and defers stack knowledge to it (no `<target>-stack-mastery` authored at build time).
10. Generated artifacts are idempotent: re-running the build against an unchanged target produces no diff
    (no-drift), matching the store's existing no-drift/idempotence testing discipline.

## Components to build (maps to the plan phases)

1. Generic expertise modules (`git-mastery`, `engineering-leadership`, generic `test-driven-development`,
   `spec-driven-development`, `system-operation-maintenance`) — each deep-researched, with guide + manifest + test.
2. `skills/grill/SKILL.md` — the exhaustive onboarding-interview skill.
3. `skills/build-engineer/SKILL.md` — the mechanizable build procedure (the two-mode recipe).
4. `commands/build-engineer.md` — the slash-command entry point.
5. Integration fixtures + tests (real-repo fixture, empty-folder fixture) and no-drift/idempotence guards.
