---
name: report-bug
description: How any Genesis-built agent reports a bug it hits (in itself, in Genesis, or in the repo it owns) — capture a structured report, spin up Sensei, and hand it over for filing. Use whenever an agent finds a bug worth tracking.
---

# Reporting a bug

Any built agent uses this when it finds a bug worth tracking — in itself, in Genesis, or in the repo it
owns. The agent does NOT file the issue itself; it routes a structured report to **Sensei**, who files it
via the `add-issue` skill (which dedups first).

## Step 1 — Capture a structured report

Gather, verified by reading (never guessed):

- **Title** — one-line summary.
- **What happened** — the observed wrong behavior.
- **Evidence** — `path:line` and/or the exact output, verified by reading (not inferred).
- **Reproduction** — the steps that trigger it.
- **Severity** — how bad (blocker / major / minor).
- **Component** — which crate/file/area.
- **Reporting agent** — your own agent name.

## Step 2 — Route it to Sensei (spin up + SendMessage)

- Spin up **Sensei** via the **Agent tool** (subagent `genesis:sensei`).
- Send the structured report to it with **SendMessage**.
- Sensei is the triage owner; reporting to it is internal and free (no outward action yet).

## Step 3 — Sensei files it (via the add-issue skill)

Sensei, on receiving the report:

- Reproduces/diagnoses against the real code where feasible (never files a speculated cause).
- Files it using the **`add-issue`** skill — which FIRST checks for an existing matching issue and only
  creates a new one if none exists (otherwise it comments on / points to the existing issue).
- Confirms before the outward `gh issue create` (filing a public issue is an outward action).

## Hard rules

- **Security / secrets:** a security bug or a leaked credential is routed **privately** to the maintainer —
  NEVER into a public issue.
- **No leakage:** never put a credential value or private-repo content in the report; reference secrets as
  `credential present at <path>`.
- **Zero speculation:** every claim in the report is verified by reading; if a fact is unknown, say so.

## References

`skills/add-issue/SKILL.md` (the filing + dedup standard), `agents/sensei.md` (the triage owner),
`.genesis/expertise/system-operation-maintenance.md` (triage: reproduce-first, security-private).
