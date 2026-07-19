# Applying Expertise: Making an Agent USE What It Knows (not just hold it)

**A distilled, evidence-backed engineering guide.** Author: Genesis research team (3 agents + orchestrator synthesis).
Date: 2026-07-19. Status: **complete v1**. Full evidence + citations live in the sibling research reports:
`docs/research/expertise-application-general.md` (A, 45 sources), `…-opus4x.md` (B, Opus-4.x primary docs),
`…-resolution.md` (contradiction resolutions, primary-verified). This file is the decoupled expertise an
agent applies; the reports are the proof.

> **Labels.** [VERIFIED] = read from a cited primary source. [INFERRED] = reasoned corollary. Every
> load-bearing claim below traces to a tag in the three reports.

---

## 0. The thesis (one sentence)

You cannot make *comprehension* deterministic; you **can** convert each rule into a mechanically-checkable
predicate a harness enforces, gate those (guaranteed), and route the semantic remainder to a grounded critic
(raised + audited). **Determinism lives outside the forward pass.**

## 1. Presence ≠ application — real, large, measurable

- A guideline in-context buys a *probability* of use, not use. [VERIFIED]
- It decays: **−39%** single→multi-turn (Laban 2505.06120); **≥50%** gone by 32K tokens (NoLiMa 2502.05167).
- U-shaped attention reads the edges, not the middle (Lost-in-the-Middle 2307.03172).
- Erodes further with distractors, irrelevant context, and social pressure (sycophancy). None zero-risk.
- **So "it's in the context" guarantees almost nothing.** Enforcement must come from elsewhere.

## 2. The determinism frontier (the operating principle)

| Can be made deterministic | Stays probabilistic |
|---|---|
| Checkable properties of output/action | Semantic comprehension / judgment |
| Constrained decoding, harness gates, forced tools, capability removal | The forward pass itself |
| P(holds) = 1 by construction, or blocked | P(applied) < 1; raised, never guaranteed |

**Maximize the fraction of expertise expressed as a checkable predicate; guarantee those; audit the rest.**

## 3. The design rules (apply these to every agent)

1. **Decompose expertise into scoped, positively-framed, ID'd imperative rules.** [VERIFIED, highest leverage]
   Opus 4.x is *literal* and will **not** auto-generalize — "apply to everything" under-applies. Say "apply to
   every X," never "apply generally." Give each rule a stable ID.
2. **Compile each rule into a bucket:** (a) mechanically-checkable predicate → harness gate (preferred), or
   (b) semantic rubric item → independent grounded judge.
3. **Force a plan that CITES the governing rule-IDs before acting** (forced tool choice + strict structured
   output) — turns "consulted the rule" into a checkable artifact.
4. **Re-assert the top rules at the TAIL** before consequential actions and every few turns — cache-safe via
   Opus 4.8 in-`messages` system entries / `UserPromptSubmit` hooks. Fights decay without cache-bust.
5. **Independent, grounded, fail-closed review.** A separate agent judges; never self-grade; ground on the
   expertise text + artifacts; may only block.
6. **Govern outputs and actions, not the reasoning trace** — Opus 4.x follows output rules strongly, hidden-
   trace rules weakly (System Card §6.5). Never phrase a rule as "in your thinking…".

## 4. The three resolutions (settled against primary sources)

- **C1 — self-critique** helps *only* as a **visible output step grounded on external evidence** (test /
  validator / retrieved rule / independent judge). Intrinsic self-critique **degrades** (Huang 2310.01798);
  reasoning-trace instructions are unreliable on 4.8. [VERIFIED]
- **C2 — LLM-as-judge** must be **independent** (never self-grade), evidence-grounded, **fail-closed**; Opus
  4.8 games graders, so a "pass" is *never* proof of success — prove success with deterministic checks. [VERIFIED]
- **C3 — re-inject vs cache**: no real conflict. **Immutable high prefix** (cache + primacy) **+ tail
  re-assertion** (4.8 in-`messages` entries, no cache-bust) uses *both* edges of the attention curve. [VERIFIED]

## 5. Measure adherence like a test suite

- Per-rule **adherence rate**: deterministic checks (IFEval-style) for bucket-(a), independent-judge rate for
  bucket-(b), plus a **multi-turn-stability** slice (SysBench/Multi-IF).
- Track it every change. **Stop *assuming* application** — measure it.

## 6. Honest limits (do not oversell)

- Constrained decoding guarantees **form, not correctness** — a schema can force a `rule_id` field, not the
  *right* rule.
- **No one has empirically compared "prompt-raise" vs "hard-gate" on the same ruleset** (gap G2) — the
  gate-checkable/raise-the-rest split is *reasoned*, not benchmarked. An original eval Genesis could run.

## 7. The Genesis design delta (what to ADD beyond inject/gate/validate)

Genesis already guarantees **presence** (`inject`), mechanical **output gating** (`gate`/`validate`), and
**independent review** (Method) — the research validated this architecture. The "always *uses* it" gap closes by
adding, in leverage order: **(1) rule decomposition/scoping**, **(3) cite-the-rule-ID forcing**, **(4) tail
re-assertion** (today `inject` is SessionStart-only — add per-turn / pre-action re-surface), and **(5) adherence
measurement**. Those convert a *present* guideline into an *applied* one, on the model Genesis actually runs on.
