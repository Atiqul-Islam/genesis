# Genesis — TODO (deferred work)

Deferred so we can focus first on the core: **agents provably use their assigned expertise** (the checkable
enforcement path — see `docs/EXPERTISE_ENFORCEMENT_PLAN.md`).

## Judgment-rule strengthening — get closer than "raised, not forced"
Pure-judgment rules ("persona feels human," "mission is clear") can't be forced by a program. To shrink that
gap later, biggest lever first:
- [ ] **Carve checkable proxies off each judgment rule** — e.g. "feels human" → has name, ≥3 traits, voice
      section, failure modes. Gate the proxies; only fine quality stays judgment.
- [ ] **Independent cheap judge (Haiku) at the final gate** — for high-stakes judgment rules only (token-aware).
- [ ] **3–5 worked examples + grounded self-critique** per judgment rule (examples steer better than description).
- [ ] **Human escalation** for the handful of rules that must be perfect.
- [ ] **Fix self-grading (C2):** Method builds; a *separate* reviewer (Sensei or a judge) grades the judgment
      part — an agent must never grade its own work.

## Deepen enforcement to full (b) — deferred by Atiqul (2026-07-19)
- [ ] **Phase 0 — per-expertise rule manifests.** Turn each expertise's prose into a checkable rule list
      (`rules.json`: id, text, checkable|judgment, predicate). Then the Stop hook verifies each expertise's
      OWN rules, not just the 3 global ones (banned phrase / credentials / line budget). Completes strictness (b).
- [ ] **Phase 3.2 — gate rule-surfacing.** `gate.py` shows the agent the relevant rules right before a
      Write/Edit (not just block on violation) — prevention at the risky step, saves redo tokens.

## Other deferred
- [ ] **Adherence measurement harness** (Plan Phase 5.3) — per-rule adherence rate + a multi-turn-stability slice.
- [ ] **G2 original eval** — no one has empirically compared prompt-raise vs hard-gate on the same ruleset;
      Genesis could be the first to run it.
- [ ] **Cross-platform `GENESIS_HOME`** — hooks use absolute `/mnt/c` paths; parametrize for Windows.
- [ ] **Starter templates** — coder / reviewer / researcher.
- [ ] **Optional `UserPromptSubmit` re-assert** (Plan Phase 3.4) — enable only if long tasks show rule-decay.
