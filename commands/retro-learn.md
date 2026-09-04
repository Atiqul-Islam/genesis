---
description: Retro-learn — sweep existing conversation history + memory across your Genesis repos, and propose durable expertise rules for you to approve. Report-first, per-item approval, nothing written or committed without you.
argument-hint: "[user-home|whole-system]"
---

You are running **`/genesis:retro-learn`** — a one-time backfill of Feature 2's learning loop over history
that predates it. You are the coordinator; **Mneme** (the memory agent) does the analysis; **the user**
approves every write. NOTHING is written, committed, or pushed without the user's explicit per-item approval.

**State the honest limits up front** (say these to the user before scanning):
- Reaches only `.genesis` repos discoverable **on this machine** within the scan scope — there is no global
  registry; a repo never opened here, or used only on another machine, cannot be reached.
- Learns only from **captured** history: `.genesis/sessions/*.jsonl`, `.genesis/resume-state.md`, the memory
  store, and this machine's `~/.claude/projects/<encoded-cwd>/*.jsonl` (older ones may have been rotated).
- The judgment (is this a durable rule?) is a model call — not deterministic; determinism holds at the
  write layer (dedup + your approval), not at detection.

**1. Pick the scan scope.** `$ARGUMENTS` is `user-home` (default) or `whole-system`. If absent, ask which.

**2. Enumerate repos (read-only).** Find `.genesis` dirs in scope, keeping those that look like a Genesis
workspace:
```
find <scope-root> -type d -name .genesis -not -path '*/node_modules/*' -not -path '*/.git/*' 2>/dev/null
```
Keep a dir when it has `.genesis/expertise/required.json` (its agents are the keys of that file). List the
repos + agents you found and confirm the set with the user before reading anything.

**3. Per repo → per agent, gather the captured history (READ-ONLY).** For each agent, read: its facts from
`.genesis/memory/memory.jsonl`; `.genesis/sessions/*.jsonl`; `.genesis/resume-state.md`; and this machine's
`~/.claude/projects/<encoded-cwd>/*.jsonl` (the encoding maps every non-alphanumeric char in the repo's
absolute path to `-`). **Redact credentials on sight** — never carry a secret value into a proposal;
reference it as "credential present at <path>".

**4. Ask Mneme to propose (no writes).** Invoke the **`mneme`** agent per repo with the gathered, redacted
history and: "Propose durable, generalizable expertise rules (always/never that would prevent a repeat
mistake), one sentence each, with the target bucket. Skip one-off facts. Dedup against the repo's existing
rules (`.genesis/expertise/manifests` + `learned.jsonl`) and flag any that CONTRADICT an existing rule with a
one-line explanation. Return candidates as a list; write NOTHING." Mneme proposes only; it never orchestrates.

**5. Emit ONE report — nothing written yet.** Group `repo → agent → candidate rule → (any contradiction:
both rules in plain English + a 1-line conflict)`. Write it to an HTML file under the current repo (e.g.
`.genesis/retro-learn-report.html`) and give the user its full path. Present each candidate for a decision:
**approve** (enforce it in that repo), **specialize** (scope it to a task/feature), **replace** an existing
rule, or **reject**. There is no "approve everything" shortcut for enforced rules.

**6. Apply ONLY what the user approves**, in that candidate's ORIGIN repo, via the deterministic writer —
never by editing files yourself:
- approve → `"<repo>/.genesis/bin/genesis-cli" expertise-learn "<repo>/.genesis/expertise" add --expertise <bucket> --text "<rule>" --status active --agents <agent>`
- replace → add the new rule, then `set-status --expertise <bucket> --id <old-id> --status retired` (history is kept, never deleted).
- reject → record nothing (or `set-status ... rejected` if a proposal row already exists).
Each write re-migrates that repo's `expertise.db`, so the rule is enforced there from the next turn.

**7. Cross-repo propagation is a SEPARATE, explicit decision.** A rule learned in repo A is proposed only to
A by default. Only if the user explicitly asks do you propose the same rule to another repo — and then only
through step 6 in that repo. Never silently propagate an enforced rule across repos.

**Never** commit or push as part of this sweep — that stays with `/genesis:sync` under the user's control.
Remind the user they may want to `/genesis:sync` each repo whose store you changed.
