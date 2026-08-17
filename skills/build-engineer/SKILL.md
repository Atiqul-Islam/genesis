---
name: build-engineer
description: The mechanizable recipe Sensei follows to build a complete engineer — a single senior developer that owns a target repo end to end (develop via strict TDD + spec-driven development, deploy, operate, and grow itself) — from one fixed recipe with two modes (deep-read for a real repo, grill for an empty folder) chosen by the target's state. Use whenever `/genesis:build-engineer` runs.
---

# Building a complete engineer

This is your mechanizable build procedure for `/genesis:build-engineer`. It specializes `build-agent`
(same interview → delegate-to-Method → assemble → verify shape) into ONE fixed, opinionated recipe: always
a single engineer (never a team — D2), that OWNS a target repo end to end. Follow every step, in order;
skip none. Two hard rules govern the whole procedure (stated in full below) — hold them in mind at every step.

## Step 1 — Detect mode
- Run `git rev-parse --is-inside-work-tree` in the target. If it succeeds, OR the target folder contains
  any non-binary source file, the target is a real project ⇒ **deep-read mode**.
- If the target folder is empty (or contains no git repository AND no non-binary source) ⇒ **grill mode**.
- If detection is genuinely ambiguous — e.g. stray files with no git repo, or mixed/inconclusive signals —
  do NOT guess. STOP and ask the user which mode to use.

## Step 2 — Minimal asks
The recipe is fixed (D1) — ask only the per-target specifics, nothing else:
- The engineer's **name** — default `<repo>-engineer`, derived from the target folder's basename.
- **Confirm** (do not re-decide) that this build produces a single engineer installed as a **subagent**
  (D2/D3 — always a single agent, never a team; installs as a subagent by default, with promotion offered
  later in Step 8). If the user wants a team or a different recipe, that is out of scope here — point them
  at the open-ended `/genesis:new` build instead.
- Never re-ask a fixed-recipe choice (persona shape, expertise set, install order) — those are baked in.

## Step 3 — Bootstrap
- Ensure the target has a `.genesis/` workspace. If missing, run:
  ```
  <genesis>/bin/genesis-cli bootstrap <target_repo> <genesis_home>
  ```
  (From a bare plugin install: `node <plugin>/bin/genesis-memory.js --run-cli bootstrap <target_repo>
  <genesis_home>` — same arguments.)
- Make the generic recipe expertise modules available in the target's workspace: stage each of
  `expertise-application`, `memory-management`, `test-driven-development`, `spec-driven-development`,
  `system-operation-maintenance`, `git-mastery`, `engineering-leadership` (guide + manifest) from the
  plugin's `expertise/` store into `<target_repo>/.genesis/expertise/` (+ `manifests/`), so every module
  the recipe requires exists locally before assembly registers it.

## Step 4 — Expertise
- **Deep-read branch:** ENUMERATE (glob/ls) the target, then fully READ every relevant non-binary file —
  never grep file content to skip a read; an unread file is unknown, not assumed. Then invoke the
  `research-expertise` skill to author a `<target>-stack-mastery` module (guide + manifest + test) at the
  target's exact pinned dependency versions, written into `<target_repo>/.genesis/expertise/`.
- **Grill branch:** skip stack research now — no `<target>-stack-mastery` module is authored at build time.
  The finished engineer runs the `grill` skill, post-build, to gain complete project expertise through an
  exhaustive onboarding interview and to author its own project-knowledge/stack module from the answers.

## Step 5 — Delegate to Method
Spawn Method (Agent tool) with a TASK-SPEC parameterized from the proven complete-engineer template at
`team/genesis-engineer/` (`persona.md`, `behavior.md`, `meta.json`) — the shape this recipe reproduces for
`<target_repo>`:
- **`persona.md` + `behavior.md`** — each ≤200 lines; mirror genesis-engineer's Identity/Character/Values/
  Boundaries/Voice/Done-means/Failure-modes shape, generalized to `<name>` and the target repo/stack, with
  no genesis-only assumptions baked in.
- **`meta.json`** — `tools` OMITS `Grep` (capability removal enforces "never grep to skip a read"; the same
  omission genesis-engineer itself carries — `team/genesis-engineer/meta.json`). `required_expertise` = the
  fixed recipe: `expertise-application, memory-management, test-driven-development, spec-driven-development,
  system-operation-maintenance, git-mastery, engineering-leadership, <target>-stack-mastery` (deep-read
  branch adds the stack-mastery slot now; grill branch omits it until the built engineer's `grill` run
  authors it).
- **Skills** — ship `grow-safely` (self-extension) and `grill` (empty-target onboarding) into the bundle.
- Output dir: `<genesis>/team/<name>/`.
- Method writes acceptance tests FIRST, runs them, and returns files + `tests: "<n> pass / <m> fail"` +
  `confidence` + `gaps` — per build-agent Step 3b, treat this as untrusted until checked against the
  TASK-SPEC; do NOT accept a result with failing tests or unresolved gaps — send it back with a sharper spec
  (bounded: max 2 re-delegations, then escalate to the user).

## Step 6 — Assemble
Run the assembler as a **subagent** install (never `--main` here — Step 8 is the only path to main-Claude
promotion):
```
<genesis>/bin/genesis-cli assemble <source_member_dir> <name> <target_repo> <genesis_home>
```
Writes `<target_repo>/.claude/agents/<name>.md`; registers `required_expertise` into
`<target_repo>/.genesis/expertise/required.json`; wires the inject/gate/validate hooks + independent review —
identical machinery to every Genesis-built agent.

## Step 7 — Verify
- Confirm Method's acceptance tests pass on a FRESH run this turn (never a recalled or delegated claim).
- Confirm the assembled agent's frontmatter is valid (`name`, `description`, `tools` all present and well-formed).
- Confirm `tools` does not include `Grep`.
- Confirm `required_expertise` is registered in `.genesis/expertise/required.json` and every named module's
  guide + manifest exists under `.genesis/expertise/`.
- Anything missing or failing → do not proceed to Step 8; send back to Method or re-run assembly.

## Step 8 — Offer promote
- Ask the user ONCE: promote `<name>` to be this folder's main Claude? Promotion is always a separate,
  explicit offer — never assumed, and silence is not consent.
- On **yes**: run `<genesis>/bin/genesis-cli promote <name> [target_repo] [genesis_home]`.
- On **no** (or no answer): stop here. The engineer remains installed as a subagent — that alone satisfies
  the build.

## The two hard rules (govern every step, in both modes)
1. **Never speculate.** Read everything, or ask everything. Deep-read mode enumerates then reads every
   relevant non-binary file in full before acting — never grep-to-skip a read. Grill mode asks the user
   every topic on its checklist and does not fill a gap with a guess. Whenever the target, the recipe, or
   an answer is unclear at any step above, STOP and ask rather than assume.
2. **Never shortcut.** Everything this recipe produces is production-ready: spec → RED → GREEN → refactor
   for all development, for both Method's authoring (Step 5) and everything the built engineer does
   afterward. No implementation before a failing test exists; no "done" claimed without a fresh passing run
   proven this turn (Step 7).

## Never
- Never pick or change the fixed recipe (persona shape, expertise set, single-agent choice) without the
  user — those are ratified decisions, not build-time judgment calls.
- Never assemble with `--main` directly — main-Claude installation only happens via the explicit Step 8 offer.
- Never accept Method's result on trust — verify tests pass fresh before assembling (Step 6/7).
- Never write the banned reasoning-trace phrase named in the house rules into any authored file; use
  "structured reasoning" / "step-by-step reasoning" instead.
- Never write a credential value anywhere — reference it as "credential present at `<path>`".
