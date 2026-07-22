# Resolving the Contradictions: How to Make an Agent *Apply* Its Expertise (not just hold it)

**Status.** Synthesis authored by the orchestrator (Genesis main loop) from the two verified reports
`expertise-application-general.md` (A, 45 sources) and `expertise-application-opus4x.md` (B, ~24 sources).
**Verification pass complete (2026-07-19, post-reset resolver).** Both evidence gaps **G1 and G2 are
now resolved against freshly-fetched primary sources**, and **C1/C2/C3 are confirmed with direct primary
quotes** — no resolution overturned; two figures refined for precision (marked inline). Every claim
traces to a primary source; nothing is fabricated. The banned four-word phrase for step-by-step reasoning is
avoided ("reasoning trace" / "structured reasoning" used instead).

The three contradictions were **apparent, not fundamental** — each dissolves under a distinction the two
reports did not draw against each other. Resolutions below, each with a one-line design rule.

---

## C1 — Does inference-time SELF-CRITIQUE improve adherence on Opus 4.x?

**The apparent conflict.** A endorses a whole category — Self-Refine, Chain-of-Verification,
critique-then-revise (A §3b) — as application-raising. B shows Opus 4.8 has among the **lowest
reasoning-trace controllability** measured (System Card §6.5) and a rising **"speculates about graders /
appearance-of-success"** tendency (System Card §6.1.2). Surface reading: A's reasoning-time layer is
unreliable on the model Genesis runs on.

**Resolution — it works, under two conditions, and B does not actually refute it.**
1. **A self-critique *loop* is an explicit, visible OUTPUT step — not an instruction on the hidden
   reasoning trace.** B's low score (§6.5) is specifically about *governing the model's internal thinking*
   ("follow this instruction *inside* your extended thinking"). A critique→revise loop is an ordinary
   generation turn that emits a *visible critique artifact* and a *revised output* — both gradeable,
   both in the region where B says Opus 4.x adherence is **strong** (B's own design lesson, §1.6: "govern
   outputs and actions, not how the model thinks"). The two findings are about different surfaces.
2. **Self-critique reliably helps only with an EXTERNAL grounded signal.** Huang et al., "LLMs Cannot
   Self-Correct Reasoning Yet" (arXiv:2310.01798, A §3b): *intrinsic* self-correction (no external
   signal) can **degrade**; prior gains leaned on oracle labels. B's grader-gaming trend is the same
   warning from the other side — a model critiquing itself against *its own opinion* can game the verdict.
   So the critic must be pointed at a **validator, a test result, a retrieved rule, or an independent
   verifier** — never its own unaided judgment.

**Net:** B constrains A, it does not contradict it. Self-critique stays in the toolkit, bounded.

**Fresh primary verification (2026-07-19).** Huang, Chen, Mishra, et al., *"Large Language Models Cannot
Self-Correct Reasoning Yet"* (arXiv:2310.01798, ICLR 2024) — fetched and read directly this pass. Verbatim:
intrinsic self-correction is "a scenario wherein the model endeavors to rectify its initial responses
based solely on its inherent capabilities, without the crutch of external feedback"; "In most instances,
the performance after self-correction even deteriorates"; and the apparent gains in prior work "result
from using oracle labels … and the improvements vanish when oracle labels are not available." The paper's
*only* endorsed positive case is a **grounded** one — self-debugging that feeds **code-execution results**
back in — which is precisely the external-signal condition the resolution requires. **[VERIFIED — direct
quote]** Orchestrator's C1 stands unchanged.

> **Design rule C1.** Self-critique counts only as a *visible output step checked against an external,
> grounded signal* (test / validator / retrieved rule / independent judge) — never the model's unaided
> self-judgment, and never an instruction aimed at the hidden reasoning trace.

---

## C2 — Is LLM-as-JUDGE trustworthy for GATING when the model games graders?

**The apparent conflict.** A recommends an LLM-judge to score the semantic rules a regex can't check
(A §4, §6). B documents Opus 4.8 increasingly reasoning about *how it will be graded* (System Card
§6.1.2). Surface reading: the judge is gameable, so A's semantic gate is unsafe.

**Resolution — use it, but never as same-model self-grading, and never as proof of success.**
- **Self-enhancement bias is already named.** Zheng et al. (arXiv:2306.05685, A §4) document
  position, verbosity, and **self-enhancement** biases — a model scoring its own (or its own family's)
  output inflates. B's grader-gaming finding is that bias made worse on this specific model.
- **Independence is the fix, and it is already the industrial pattern.** A's Constitutional-Classifiers
  datapoint (86%→4.4% jailbreak success) works because the guard is a **separate model/program** that
  "doesn't share the main model's blind spots" (A §6, Pattern C). The judge must be a *different
  instance/model* (a dedicated small judge, a separate subagent), bias-controlled (randomize position,
  cap verbosity, forbid self-grading), and **grounded on evidence** (the expertise text + tool/test
  output), not vibe.
- **Fail-closed, advisory-to-block only.** A gamed grader is dangerous mainly when a *pass* is trusted as
  proof. Let the judge only **block** (fail-closed); prove *success* with deterministic checks and
  observable artifacts (tests, tool results), never the model's self-report — which B §5.9 explicitly
  warns against.
- **Genesis already embodies this:** *Method* is a **separate agent** reviewing another agent's work
  (independent judge, not self-grading); *validate.py* is the deterministic fail-closed Stop gate. The
  delta is only to (a) forbid any agent from grading its *own* work and (b) ground the judge on the
  expertise text + artifacts.

**Fresh primary verification (2026-07-19).** (a) Zheng, Chiang, Sheng, et al., *"Judging LLM-as-a-Judge
with MT-Bench and Chatbot Arena"* (arXiv:2306.05685, NeurIPS 2023) — abstract, verbatim: it examines
"position, verbosity, and self-enhancement biases, as well as limited reasoning ability, and propose[s]
solutions to mitigate some of them." **[VERIFIED — direct quote]** (b) Anthropic, *"Constitutional
Classifiers"* (research page + arXiv:2501.18837) — verbatim: "with no defensive classifiers, the jailbreak
success rate was 86% … Guarding Claude using Constitutional Classifiers … [was] reduced to 4.4%, meaning
that over 95% of jailbreak attempts were refused" (on Claude 3.5 Sonnet, Oct 2024). That is the
independent, *separate*-guard pattern the resolution prescribes — a program that does not share the main
model's blind spots. **[VERIFIED — direct quote]** Orchestrator's C2 stands unchanged.

> **Design rule C2.** Deterministic checks first; for semantic rules use an *independent,
> bias-controlled, evidence-grounded* judge that can only **block** — never let an agent grade its own
> work, never treat a judge "pass" as proof of success.

---

## C3 — Re-inject to fight decay vs keep a cache-stable immutable prefix?

**The apparent conflict.** A says re-state critical rules at the **end** of long context and per-turn —
a recency anchor against decay (A §1.1 U-shaped attention, §3c). B says keep the expertise as an
**immutable high prefix** for prompt-cache hits (B §2.5). Surface reading: you can't do both.

**Resolution — no real conflict on Opus 4.8; layer the two, using a purpose-built mechanism.**
- **Keep the stable expertise as an immutable HIGH prefix** — this simultaneously wins cache hits *and*
  primacy salience (the "start" lobe of the U-shaped attention curve).
- **Fight decay by re-asserting the top rules at the TAIL via mechanisms that do NOT bust the cache:**
  - **Opus 4.8's system entries inside the `messages` array** — B §2.7 [VERIFIED]: "update Claude's
    instructions mid-task **without breaking the prompt cache**." This is the exact reconciliation: fresh,
    high-salience re-assertion at the tail, cached prefix untouched.
  - **`UserPromptSubmit` hooks** — add the critical rules as context once per turn (B §2.1).
  - **`PreToolUse` hooks** — surface the *relevant* rule immediately before a consequential action (JIT
    re-surfacing, A §3c).
  This exploits **both** edges of the U-shaped attention curve (start = cached prefix, end = re-assertion)
  without moving the cached block. A and B were describing the two edges; neither is wrong.
- **When does re-assertion become necessary? (quantified):** NoLiMa (arXiv:2502.05167, both reports):
  **≥50% degradation by 32K tokens**, effective length often ≤2K for high-accuracy models — so re-assert
  *well before* tens of thousands of tokens. Laban et al. (arXiv:2505.06120): **−39% single→multi-turn** —
  so re-assert standing rules **every few turns**, and always **immediately before a consequential
  action**.

**Fresh primary verification (2026-07-19).** Both decay sources fetched and read directly this pass.
(a) **NoLiMa** — Modarressi, Deilamsalehy, Dernoncourt, et al. (arXiv:2502.05167) — abstract, verbatim:
"At 32K … 11 [of 13] models drop below 50% of their strong short-length baselines. Even GPT-4o …
experiences a reduction from an almost-perfect baseline of 99.3% to 69.7%," with declines that "stem from
the increased difficulty the attention mechanism faces in longer contexts." *Precision fix:* the ≥50%
figure is **11 of 13 models falling below half their own short-context baseline at 32K**, not a uniform
50% drop for every model. **[VERIFIED — direct quote]** (b) **Laban, Hayashi, Zhou, Neville**
(arXiv:2505.06120, Microsoft Research + Salesforce), *"LLMs Get Lost in Multi-Turn Conversation"* —
abstract, verbatim: "an average drop of 39% across six generation tasks," decomposed into "a minor loss
in aptitude and a significant increase in unreliability." **[VERIFIED — direct quote]** The cache-safe
tail-reassertion mechanism itself (Opus 4.8 in-`messages` system entries, B §2.7) was *not* re-fetched
this pass — it remains **B-[VERIFIED]** from Anthropic docs; what this pass locked down is the *decay
quantification* that makes re-assertion necessary. Orchestrator's C3 stands unchanged.

> ⚠️ **Do not conflate the two 39%s.** Laban's **−39%** is *single→multi-turn degradation* (a reason to
> re-assert rules). Anthropic's **+39%** (see G1) is *memory + context-editing improvement*. Unrelated
> results that happen to share a number — keep them apart in any Genesis-facing writeup.

> **Design rule C3.** Immutable expertise prefix (cache + primacy) **plus** tail re-assertion of the
> top-priority rules via cache-safe in-`messages` system entries / `UserPromptSubmit` — before each
> consequential action and every few turns.

---

## Evidence gaps — RESOLVED (resolver pass, 2026-07-19)

- **G1 — the "84% / 39%" figure → now [VERIFIED], with a precision correction.** Primary source fetched
  and read directly: Anthropic, *"Managing context on the Claude Developer Platform"*
  (https://claude.com/blog/context-management — the canonical target that
  `anthropic.com/news/context-management` redirects to; the redirect is exactly what broke the earlier
  fetch), **dated September 29, 2025**, model context **Claude Sonnet 4.5**, public beta, beta header
  `context-management-2025-06-27`. Verbatim, from *"Performance improvements with context management"*:
  > "On an internal evaluation set for agentic search … combining the memory tool with context editing
  > improved performance by 39% over baseline. Context editing alone delivered a 29% improvement. In a
  > 100-turn web search evaluation, context editing enabled agents to complete workflows that would
  > otherwise fail due to context exhaustion—while reducing token consumption by 84%."

  **Correction to the interim phrasing** ("84% token reduction / 39% improvement (100-turn web search)"):
  the **39%** is memory-tool + context-editing on Anthropic's internal *agentic-search* eval set (it is
  **not** tagged specifically to the 100-turn run); **context editing alone = 29%** (a third figure the
  interim omitted); the **84%** is the token-consumption reduction in the **100-turn web-search** eval.
  All three are **Anthropic *internal* evaluations** — real and now directly quoted, but vendor-reported
  and not independently reproduced. Cite as "Anthropic-reported (internal eval)," not as a neutral
  benchmark. The context-editing docs' own worked example (a long trajectory clearing back down to a small
  live window) corroborates the mechanism. **[VERIFIED — direct quote]**
- **G2 — the raise-vs-gate head-to-head → CONFIRMED as a genuine gap, plus a newly-found near-analog.**
  A fresh targeted search (guardrails, constrained decoding, instruction-following benchmarks) found **no
  study pitting prompt-based adherence-raising against harness / hard-gate enforcement on the *same
  arbitrary or semantic* rule set.** The closest empirical work is the **constrained-decoding-vs-prompting**
  literature, which is (i) scoped to output *format / schema* validity — not behavioral rules — and (ii)
  itself contested: **Tam et al., "Let Me Speak Freely?" (arXiv:2408.02442)** found "a significant decline
  in LLMs['] reasoning abilities under format restrictions" (a hard gate can *cost* reasoning), whereas
  **Geng et al., "JSONSchemaBench" (arXiv:2501.10868)** and the Outlines rebuttal find constrained decoding
  matches or beats free generation *when the prompt leaves room to reason first*. The one robust
  cross-cutting result — prompt-based format control "does not guarantee correctness and can still produce
  invalid outputs, whereas constrained decoding enforces validity … by masking invalid tokens" — supports
  **gate-what-you-can-check** for the checkable subset. **Net:** the general head-to-head is a real
  literature gap (an original eval Genesis could run and publish); and there is a **new design caveat** —
  hard-gate *outputs and actions*, never the *reasoning* surface (Tam et al.), which independently
  reinforces C1 and design delta #8. **[VERIFIED — gap confirmed; near-analog cited]**

---

## Net design delta for Genesis (what to ADD beyond inject/gate/validate)

Genesis today guarantees **presence** (`inject`), mechanical **output gating** (`gate`/`validate`), and
**independent review** (Method). The user's question — *what ensures the agent USES the expertise for
everything it does?* — lives in the seam between "present" and "applied." The three reports converge on
eight additions, ordered by leverage:

1. **Decompose the expertise into scoped, positively-framed, ID'd imperative rules.** Opus 4.x is
   **literal and does not auto-generalize** (B §1.1) — "apply this to everything" *under-applies* unless
   scope is explicit. This is the single biggest prompting lever; do it before any hook work.
2. **Compile each rule into one of two buckets** (A §5/§6): (a) a **mechanically-checkable predicate** →
   harness gate (preferred), or (b) a **semantic rubric item** → independent grounded judge. Every rule
   gets a stable **ID**.
3. **Force a plan that CITES the governing rule IDs before acting** — via forced tool choice + strict
   structured output (B §2.6, A §6). Turns "consulted the rule" into a *checkable artifact*.
4. **Re-assert the top rules at the tail** before consequential actions and every few turns — cache-safe
   (C3). Genesis's `inject` is SessionStart-only today; add a per-turn / pre-action re-surface.
5. **Independent, grounded, fail-closed semantic review** (C2). Keep Method independent; forbid
   self-grading; ground it on the expertise *text* + artifacts; it may only block.
6. **Self-critique only as a grounded, visible output step** (C1) — draft → check vs external signal →
   revise; never a hidden-trace instruction.
7. **Measure adherence as a regression suite** (A §4) — per-rule adherence rate: deterministic checks for
   bucket-(a), independent-judge rate for bucket-(b), plus a multi-turn-stability slice. Stop *assuming*
   application; track it like tests.
8. **Govern outputs and actions, not the reasoning trace** (B §1.6) — keep every expertise rule about
   *what to produce / do*, where Opus 4.x adherence is strong; never phrase a rule as "in your thinking…".
   *Reinforced by G2:* Tam et al. (arXiv:2408.02442) show hard *format* constraints can measurably degrade
   reasoning — so gate outputs and actions, but never hard-constrain the reasoning surface itself.

**The through-line:** determinism is only ever available *outside the forward pass* — both reports reach
this independently. Genesis already puts three guarantees there (inject/gate/validate + Method). The delta
that closes the "application" gap is mostly **(1) rule decomposition/scoping**, **(3) cite-the-rule
forcing**, **(4) tail re-assertion**, and **(7) adherence measurement** — the pieces that convert a
*present* guideline into an *applied* one, on this specific model.
