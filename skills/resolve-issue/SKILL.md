---
name: resolve-issue
description: The procedure for resolving a tracker/GitHub issue — interview the user for full context and scope FIRST, then implement autonomously to completion, under a hard rule of zero speculation, shortcuts, and assumptions. Use whenever asked to resolve, start, or work on an issue.
---

# Resolving an issue

This is the fixed procedure for taking an issue from "open" to "resolved". Follow every step; skip none.
It is the approach for ALL issues.

## The hard rules (non-negotiable, every step)

- **Zero speculation.** Never assert a fact you have not verified by reading the actual source in full.
  Label anything unverified `(unverified)`, or ask — never guess.
- **Zero shortcuts.** Read every relevant file fully. Never grep/rg a file to skip a read (piping command
  output to grep is fine; grepping a file is not).
- **Zero assumptions.** Never assume the user's intent, the scope, or the resolution. Ask.
- **Resolving an issue is NOT building it.** Building is ONE possible resolution, chosen ONLY on the user's
  explicit "build/implement" instruction. Confirm the resolution before acting.
- **Follow the user every step.** Consult the developer-in-charge whenever a needed fact is unknown.

## Step 0 — Mark the issue in-progress FIRST (hard rule)

The moment the user says to START or resolve an issue, update its tracker status BEFORE anything else —
add the `in progress` label and assign it (and optionally comment that you are starting). Interviewing is
PART OF starting; asking a clarifying question does NOT defer this status update. Never ask a question
first and leave the issue's status untouched.

## Step 1 — Interview the user (before doing ANYTHING else, including a spec)

- Gather FULL context about the task: what the issue actually asks, the definition of done, constraints.
- Establish SCOPE, and ask explicitly: does resolving this involve **deployment** (a release), or author-only?
- Confirm the intended RESOLUTION: a build/implementation, a doc, a decision, a spike, or something else?
- Read the issue and every source it cites, in full. If the issue is not self-contained + sourced, make it
  so (the sourced-task-authoring standard) or ask for the missing sources.
- Restate the task, scope, and resolution back to the user. Proceed only on confirmed facts.

## Step 2 — Plan (and confirm)

- Lay out the concrete steps for the confirmed resolution; name the files/artifacts touched and the tests.
- Present the plan; get the user's explicit go before implementing.

## Step 3 — Implement autonomously

- Once the resolution + scope are confirmed, implement it end to end, autonomously.
- Spec-first + TDD where code is involved: a failing test before code; the full gate before any "done".
- STOP only when the entire task is **complete**, OR when you have a **question** for the user.
- Never stop half-done for any other reason; never widen or narrow the scope without asking.

## Step 4 — Deliver

- Prove completion with fresh evidence (tests/gates run this turn), not assertion.
- Deploy ONLY if deployment was confirmed in Step 1; otherwise stop at author-only and report.
- Report what was done and which requirement each part satisfies.

## Do / Don't

- **Do:** ask first, verify by reading, follow the user, finish completely.
- **Don't:** speculate, shortcut, assume intent, or treat "resolve" as "build".
