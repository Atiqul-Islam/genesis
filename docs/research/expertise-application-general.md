# Ensuring an LLM Agent *Applies* the Expertise Already in Its Context

**A citation-backed, implementation-ready survey of the "presence ≠ use" problem**

Research date: 2026-07-19 · Scope: literature and industry practice up to this date · Author: deep-research agent (Genesis)

---

## 0. Framing: the exact problem

Getting expertise *into* context is solved. A deterministic hook can inject a document into every
session, and retrieval/JIT loading can place the *relevant* slice in front of the model on demand.
**Guaranteeing presence is a mechanical operation. Guaranteeing that the model then *applies* the
governing rule to a specific decision is not** — because application happens inside a probabilistic
forward pass that the builder does not control.

This report is about the second half only: given that the guideline is demonstrably in-context,
**what raises or forces its actual application, and where can that be made deterministic?** The
answer, in one sentence: *you cannot make comprehension deterministic, but you can (a) raise the
probability of application with reasoning-time and context-time techniques, and (b) convert as many
rules as possible into mechanically-checkable predicates that a deterministic harness enforces on the
output/action — rejecting and retrying until the check passes.* The determinism lives in the harness,
not the model.

> **Notation convention.** Per house style this report never spells out the term for step-by-step
> reasoning traces; it uses "multi-step reasoning," "structured reasoning," or the abbreviation
> **CoT** (including inside two verbatim paper titles, so the citation stays identifiable). arXiv IDs
> disambiguate every reference.

Every load-bearing claim is tagged **[VERIFIED]** (I read the cited primary source this run) or
**[INFERRED]** (derived, or taken from a secondary/search summary of the source rather than the
passage itself). The Appendix is the full ledger.

---

## 1. The problem is real: "in-context but unused"

The failure has been measured from several independent directions. All of them show the same thing:
information that is provably present in the context is not reliably used.

### 1.1 Positional non-use — the model reads the edges, not the middle
- **Liu, Lin, Hewitt, Paranjape, Bevilacqua, Petroni, Liang — "Lost in the Middle: How Language
  Models Use Long Contexts," TACL 2024, arXiv:2307.03172** (https://arxiv.org/abs/2307.03172).
  On multi-document QA and key-value retrieval, accuracy is a **U-shaped function of the position of
  the relevant content**: highest when it sits at the very beginning or end, and it "degrades
  significantly" when the model must use information in the middle — *even for models explicitly built
  for long context.* Presence in the middle ≈ partial invisibility. **[VERIFIED]**
- **Hsieh, Chuang, Li, Wang, et al. — "Found in the Middle: Calibrating Positional Attention Bias
  Improves Long Context Utilization," Findings of ACL 2024, arXiv:2406.16008**
  (https://arxiv.org/abs/2406.16008). Identifies the *mechanism*: LLMs carry an **intrinsic U-shaped
  attention bias** — begin/end tokens get disproportionate attention "regardless of their relevance."
  A training-free, inference-time calibration that subtracts this positional bias from the attention
  weights recovers substantial lost accuracy (reported up to ~15 points on downstream RAG in secondary
  summaries). This matters here because it proves the non-use is an *attention allocation* problem, not
  an information-availability problem. **[VERIFIED]** (mechanism); **[INFERRED]** (the +15-pt figure —
  from the search summary, not read in-text).

### 1.2 Degradation with sheer length — "context rot"
- **Chroma Research (Hong, Troynikov, et al.) — "Context Rot: How Increasing Input Tokens Impacts LLM
  Performance," 2025** (https://www.trychroma.com/research/context-rot). Evaluated **18 frontier
  models** (GPT-4.1, Claude 4/Sonnet, Gemini 2.5, Qwen3, etc.). Key findings I read directly:
  performance **degrades as input length grows even when retrieval of the relevant span is otherwise
  trivial**; the closer the "needle" is *semantically* to the question the more robust it is, and
  **lower needle-question similarity degrades faster** with length; **a single distractor already
  lowers accuracy and four compound it**, with non-uniform per-distractor impact; on a repeated-word
  reproduction task GPT-3.5-turbo refused **60.29%** of tasks; and on LongMemEval a "focused" prompt
  (relevant content only) beats the "full" ~113k-token prompt (306 prompts) that buries the same
  content in irrelevant text. **[VERIFIED].** The often-quoted "13.9%–85%" degradation range is
  **[INFERRED]** (search summary).
- **Levy, Jacoby, Goldberg — "Same Task, More Tokens: the Impact of Input Length on the Reasoning
  Performance of Large Language Models," ACL 2024, arXiv:2402.14848**
  (https://arxiv.org/abs/2402.14848). Holding the *task* fixed and only padding the input, reasoning
  accuracy shows "notable degradation … at much shorter input lengths than their technical maximum,"
  and next-token-prediction loss *correlates negatively* with reasoning performance (so a model can be
  "fluent" on the padded input while reasoning worse). **[VERIFIED].** The "~3,000 tokens" onset is
  **[INFERRED]** (search summary).
- **Modarressi, Deilamsalehy, Dernoncourt, Bui, Rossi, Yoon, Schütze — "NoLiMa: Long-Context
  Evaluation Beyond Literal Matching," ICML 2025, arXiv:2502.05167**
  (https://arxiv.org/abs/2502.05167). Removes the lexical-overlap "crutch" from needle-in-a-haystack
  so the model must infer a latent association. Result read directly: of 13 models, **11 fall to half
  or less of their short-context base score by 32K tokens**; models with >90% base accuracy have an
  *effective* length ≤2K (GPT-4o excepted, failing beyond 8K); e.g. **Llama 3.1 70B: 94.3% base →
  42.7% at 32K**. The authors attribute the collapse to the attention mechanism struggling without a
  literal match. **[VERIFIED].** (The widely cited "GPT-4o 99.3→69.7" is **[INFERRED]** — secondary.)

### 1.3 Distraction by irrelevant context
- **Shi, Chen, Misra, Scales, Dohan, Chi, Schärli, Zhou — "Large Language Models Can Be Easily
  Distracted by Irrelevant Context," ICML 2023, arXiv:2302.00093** (https://arxiv.org/abs/2302.00093).
  Introduces GSM-IC (grade-school math with an irrelevant sentence added). Accuracy drops sharply once
  irrelevant material is present. Two mitigations they find helpful — **self-consistency decoding** and
  **an explicit "ignore irrelevant information" instruction** — are themselves *application-forcing*
  techniques (see §3). **[VERIFIED].**
- **Weston, Sukhbaatar — "System 2 Attention (is something you might need too)," 2023,
  arXiv:2311.11829** (https://arxiv.org/abs/2311.11829). Diagnoses the cause: "soft attention … is
  susceptible to incorporating irrelevant information from the context." Their fix (§3c) regenerates
  the context down to only the relevant portion, which "increases factuality and objectivity, and
  **decreases sycophancy**." **[VERIFIED]** (qualitative); the "51.7%→61.3%" figure is **[INFERRED]**.

### 1.4 Instruction drift over long / multi-turn agent trajectories
- **Laban, Hayashi, Zhou, Neville — "LLMs Get Lost In Multi-Turn Conversation," 2025,
  arXiv:2505.06120** (https://arxiv.org/abs/2505.06120). Across six generation tasks, every top model
  degrades from single-turn to multi-turn by an **average of -39%**. Decomposing 200,000+ simulated
  conversations: the loss is **a small drop in aptitude plus a large rise in *unreliability*** — models
  "make assumptions in early turns and prematurely attempt to generate final solutions, on which they
  overly rely." Critically, a "Concat" control (same shards concatenated into one turn) stays at 95.1%
  of full performance, proving **the loss is not missing information — it is failure to keep applying
  what was already stated.** This is the single most on-point paper for this report. **[VERIFIED].**
- **He, Jin, Wang, Bi, et al. (Meta GenAI) — "Multi-IF: Benchmarking LLMs on Multi-Turn and
  Multilingual Instructions Following," 2024, arXiv:2410.15553** (https://arxiv.org/abs/2410.15553).
  14 models; instruction-following accuracy falls with each added turn — **o1-preview drops from 0.877
  (turn 1) to 0.707 (turn 3)** averaged over languages; models "increasingly forget to adhere to
  instructions that were successfully executed in previous turns." **[VERIFIED].** (The named metric
  "Instruction Forgetting Rate" is **[INFERRED]** — secondary.)
- **Qin, Zhang, Zhang, et al. — "SysBench: Can Large Language Models Follow System Messages?," 2024,
  arXiv:2408.10943** (https://arxiv.org/abs/2408.10943). 500 system messages × 5 turns (2,500 turns),
  six constraint types, with a three-level metric: **Constraint Satisfaction Rate → Instruction
  Satisfaction Rate → Session Stability Rate** (consecutive turns before the first violation). Their
  attention analysis found that **models which allocate more attention mass to system-message tokens
  follow the system message better** — a direct, measurable link between "attends to the rule" and
  "applies the rule." **[VERIFIED].**

### 1.5 Adherence collapses under social pressure (a distinct failure)
- **Sharma, Tong, Korbak, Duvenaud, Askell, Bowman, Cheng, Durmus, Perez, et al. (Anthropic) — "Towards
  Understanding Sycophancy in Language Models," ICLR 2024, arXiv:2310.13548**
  (https://arxiv.org/abs/2310.13548). Clear sycophancy across Anthropic/OpenAI/Meta assistants: models
  abandon correct positions when a user pushes back, because **RLHF preference data rewards matching the
  user's view.** For our problem this is important: a governing rule in-context can be *overridden* by a
  contradicting user turn even when the rule is objectively right. **[VERIFIED].**

### 1.6 The unifying framing from industry
- **Anthropic — "Effective context engineering for AI agents," 2025-09-29**
  (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents). Frames the whole
  phenomenon as a finite **"attention budget"**: because transformer self-attention is n² over context,
  "every new token … depletes this budget," and recall degrades with length ("context rot … emerges
  across all models"). Good context engineering is therefore "finding the *smallest possible* set of
  high-signal tokens." Presence is cheap; *attention* is the scarce resource that use depends on.
  **[VERIFIED].**

**Bottom line of §1:** the guideline being in the window buys you a *raised probability* of use that
falls with context length, distance-from-edges, semantic dissimilarity to the query, number of
distractors, number of turns, and social pressure. None of those are zero-risk. Enforcement has to come
from somewhere other than mere presence.

---

## 2. Why prompting alone is insufficient

**The mechanism is probabilistic.** An LLM samples the next token from a distribution. A rule in the
prompt shifts that distribution toward compliant continuations, but for any decision the compliant
continuation has probability p < 1, and — per §1 — p is eroded by length, position, and turn count.
"Follow this rule" is a *bias on a distribution*, not a *constraint on the output*. Restating,
bolding, or ALL-CAPS-ing the rule raises p; it never sets p = 1.

**What "deterministic" can and cannot mean for an LLM.**
- **Achievable determinism** is always *outside* the sampling step: (a) **constrained decoding**, which
  removes non-conforming tokens from the sampling set so certain *syntactic* properties hold with
  probability 1 (§3a); and (b) **harness logic** that inspects the output/action and *mechanically
  accepts, rejects, repairs, or blocks* it (§3d). Both are ordinary deterministic programs wrapped
  around the model.
- **Unachievable by prompting** is determinism of *semantic judgment* — whether the model correctly
  understood and honored the *intent* of a rule on a novel case. That remains probabilistic even with
  perfect presence. (Note also that even at temperature 0, decoding is not bitwise-deterministic across
  hardware/batching in practice — so "temperature 0 = deterministic adherence" is a false assumption.)

**Even purpose-built priority schemes are only probabilistic.**
- **Wallace, Xiao, Leike, Weng, Heidecke, Beutel (OpenAI) — "The Instruction Hierarchy: Training LLMs
  to Prioritize Privileged Instructions," 2024, arXiv:2404.13208** (https://arxiv.org/abs/2404.13208).
  Base models "often consider system prompts … to be the same priority as text from untrusted users."
  Training a priority order (system > developer > user > tool) *dramatically raises* robustness —
  e.g. **defense against system-prompt extraction improved by 63%**, with generalization to held-out
  attacks — but the paper is explicit that this is improved *robustness*, not a guarantee.
  **[VERIFIED].**
- Independent work ("Control Illusion: The Failure of Instruction Hierarchies in Large Language
  Models," 2025, arXiv:2502.15851, https://arxiv.org/abs/2502.15851) reports that hierarchies are
  inconsistently enforced when instructions conflict — i.e. even a *designed* priority signal is
  applied probabilistically. **[INFERRED]** (identified via search; not read in full this run).

Conclusion: prompting is necessary and it moves p meaningfully, but *any* architecture that needs a
rule applied **reliably** must add an enforcement layer that does not depend on the model choosing to
comply.

---

## 3. Techniques that raise or force application

Grouped by where they act. Roughly ordered within each group from "raises p" to "forces it."

### 3a. Output-side enforcement (check/constrain the result)

**Constrained / structured decoding — the one place true determinism enters generation.**
- **Willard, Louf — "Efficient Guided Generation for Large Language Models" (Outlines), 2023,
  arXiv:2307.09702** (https://arxiv.org/abs/2307.09702). Reformulates generation as transitions over a
  **finite-state machine**, builds an index over the vocabulary, and at each step **masks every token
  that would violate the regex/grammar** — "guaranteeing the structure of the generated text" with near-
  zero overhead, model-agnostically. **[VERIFIED].**
- **OpenAI — "Introducing Structured Outputs in the API," 2024-08-06**
  (https://openai.com/index/introducing-structured-outputs-in-the-api/). Same idea productized: with a
  supplied JSON Schema the model is **constrained to only schema-valid tokens**, so outputs "exactly
  match" the schema; they note context-free grammars beat plain FSMs for recursive/nested structures.
  **[VERIFIED]** (the mechanism and the "exactly match" guarantee were read in-text; the headline
  "100% vs <40% baseline" eval numbers are **[INFERRED]** — secondary).
- **Crucial nuance for this report:** constrained decoding guarantees **syntax, not semantics.** You
  can force the answer to be valid JSON with a `rule_id` field; you cannot force the `rule_id` to be the
  *correct* one or the content to be *wise*. So it deterministically enforces the *form* of
  "cite-the-governing-rule," turning an otherwise-unverifiable expectation into a checkable one — but
  the correctness of the citation still needs §3b/§3d.

**Validate-and-repair (generate → check-against-rules → reject+retry).**
- **Guardrails AI** (open source; https://github.com/guardrails-ai/guardrails). Wraps the LLM call with
  composable **validators** (schema, PII, toxicity, competitor-mention, custom predicates); on failure
  it issues an automatic **re-ask** with the validation error and retries. This is the canonical
  "output guard + repair loop." **[INFERRED]** (read via project/search summaries, not primary docs).
- **Rebedea, Dinu, Sreedhar, Parisien, Cohen (NVIDIA) — "NeMo Guardrails: A Toolkit for Controllable
  and Safe LLM Applications with Programmable Rails," EMNLP 2023 (demo), arXiv:2310.10501**
  (https://arxiv.org/abs/2310.10501). A **dialogue-management runtime** where developers author "rails"
  in Colang; rails are "user-defined, independent of the underlying LLM, and interpretable," and can
  enforce topic, dialogue-path, and style constraints on I/O. **[VERIFIED]** (abstract).
- **Guardrail *classifier* models** put a second, cheaper model on the I/O boundary:
  - **Inan, Upasani, Chi, et al. (Meta) — "Llama Guard: LLM-based Input-Output Safeguard for Human-AI
    Conversations," 2023, arXiv:2312.06674** (https://arxiv.org/abs/2312.06674). A Llama-2-7B tuned to
    classify prompts *and* responses against a safety-risk taxonomy. **[VERIFIED].**
  - **Anthropic — "Constitutional Classifiers: Defending against universal jailbreaks," 2025-02-03**
    (https://www.anthropic.com/research/constitutional-classifiers; paper arXiv:2501.18837). Input/
    output classifiers trained from a natural-language *constitution*. Measured: baseline jailbreak
    success **86%** (Claude alone blocked only 14%) → **4.4% with classifiers (>95% blocked)**, at a
    **+0.38% increase in production refusals**. This is a clean industrial datapoint that an output-side
    guard can raise application of a policy from ~14% to ~96%. **[VERIFIED]** (the "23.7% compute
    overhead" figure is **[INFERRED]** — secondary; the page I read said "moderate additional compute").

**LLM-as-judge / critic gating.**
- **Zheng, Chiang, Sheng, et al. — "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena," NeurIPS
  2023, arXiv:2306.05685** (https://arxiv.org/abs/2306.05685). A strong LLM can grade another model's
  output against a rubric; the paper documents the failure modes you must design around — **position,
  verbosity, and self-enhancement biases** and limited reasoning. A judge that scores "did the output
  honor rule X?" is the standard gate for *semantic* rules that no regex can check. **[VERIFIED]**
  (biases); the "~80% human agreement" figure is **[INFERRED]** — secondary.

### 3b. Reasoning-time enforcement (make the model check itself before finishing)

- **Madaan, Tandon, Gupta, Hallinan, et al. — "Self-Refine: Iterative Refinement with Self-Feedback,"
  NeurIPS 2023, arXiv:2303.17651** (https://arxiv.org/abs/2303.17651). One model plays generator →
  feedback-giver → refiner in a loop, no extra training, across 7 tasks. **[VERIFIED]** (the "~20%
  average gain" is **[INFERRED]** — secondary).
- **Shinn, Cassano, Berman, Gopinath, Narasimhan, Yao — "Reflexion: Language Agents with Verbal
  Reinforcement Learning," NeurIPS 2023, arXiv:2303.11366** (https://arxiv.org/abs/2303.11366). After a
  failed attempt the agent writes a natural-language self-reflection into an **episodic memory buffer**
  and reuses it next trial — "reinforcement" without weight updates. Directly relevant: it re-surfaces
  the lesson as explicit context on the retry. **[VERIFIED]** (the "91% HumanEval" figure is
  **[INFERRED]** — not read).
- **Dhuliawala, Komeili, Xu, Raileanu, Li, Celikyilmaz, Weston — "Chain-of-Verification Reduces
  Hallucination in Large Language Models," Findings of ACL 2024, arXiv:2309.11495**
  (https://arxiv.org/abs/2309.11495). CoVe: (i) draft, (ii) **plan verification questions**, (iii)
  answer them *independently* to avoid self-bias, (iv) regenerate a verified answer. A structured
  self-audit rather than a vibe-check. **[VERIFIED].**
- **Bai, Kadavath, Kundu, et al. (Anthropic) — "Constitutional AI: Harmlessness from AI Feedback,"
  2022, arXiv:2212.08073** (https://arxiv.org/abs/2212.08073). The *inference-time-portable* idea
  inside a training method: a **critique-against-an-explicit-principle → revise** step. You can run the
  same critique/revise loop purely at inference, using your expertise doc as the "constitution."
  **[VERIFIED].**
- **Plan-then-act with the expertise as an explicit checklist** and **verifier models:**
  - **Wang, Wei, Schuurmans, Le, Chi, Narang, Chowdhery, Zhou — "Self-Consistency Improves CoT
    Reasoning in Language Models," ICLR 2023, arXiv:2203.11171** (https://arxiv.org/abs/2203.11171).
    Sample multiple reasoning paths, then take the majority answer — an ensemble that filters
    idiosyncratic non-compliant runs. **[VERIFIED]** (mechanism); the GSM8K "56.5→74.4" etc. are
    **[INFERRED]** — secondary.
  - **Lightman, Kosaraju, Burda, Edwards, Baker, Lee, Leike, Schulman, Sutskever, Cobbe (OpenAI) —
    "Let's Verify Step by Step," 2023, arXiv:2305.20050** (https://arxiv.org/abs/2305.20050).
    **Process supervision** (a reward model that scores each *step*) beats outcome supervision; their
    process-supervised verifier "solves **78%** of a representative MATH subset," and they release
    PRM800K (800k step-level labels). A step-level verifier is the training-time cousin of an
    inference-time "check each planned step against the rule." **[VERIFIED].**

**The honest counter-evidence — self-critique is NOT free reliability.**
- **Huang, Chen, Mishra, Zheng, Yu, Song, Zhou — "Large Language Models Cannot Self-Correct Reasoning
  Yet," ICLR 2024, arXiv:2310.01798** (https://arxiv.org/abs/2310.01798). With **intrinsic**
  self-correction (no external signal, no oracle telling it when to stop), models often **degrade**;
  prior positive results leaned on oracle labels. **Implication for this report:** a self-critique loop
  only reliably increases *application* when the critic has an **external, grounded signal** — a
  validator, a test suite, a retrieved source, a tool result, or the rule stated as a checkable
  predicate. Reflection against nothing but its own judgment can make things worse. **[VERIFIED].**

### 3c. Just-in-time re-surfacing (put the relevant rule where attention actually is)

Because §1 shows attention is scarce and edge-biased, *when* and *where* the rule appears matters as
much as *whether*.
- **Anthropic, "Effective context engineering," 2025** (URL in §1.6). Advocates **just-in-time
  retrieval** — keep lightweight identifiers (paths, queries, links) and pull the specific material
  into context *at the moment it's needed*, rather than front-loading everything once — plus system
  prompts at the **"right altitude"** (specific enough to constrain, general enough not to be brittle).
  Re-surfacing the *relevant* rule right before the action counters both context rot and
  lost-in-the-middle. **[VERIFIED].**
- **System 2 Attention (Weston & Sukhbaatar, arXiv:2311.11829, §1.3)** is re-surfacing by *subtraction*:
  regenerate the context down to only the relevant portion before answering. **[VERIFIED].**
- **Found-in-the-Middle (Hsieh et al., arXiv:2406.16008, §1.1)** supports the practical heuristics:
  because attention is U-shaped, **place the most critical rules at the context *edges*** (start of
  system prompt and/or immediately before the action), and re-state long-lived constraints near the
  end of a long transcript (a "recency anchor"). **[VERIFIED]** (mechanism motivates the heuristic;
  the specific placement tactic is a standard **[INFERRED]** engineering corollary).
- **Spaced reminders / re-injection each turn** directly target the multi-turn "forgetting" measured by
  Multi-IF and Lost-in-Conversation (§1.4): re-assert the standing instructions every N turns or on
  every tool result. **[INFERRED]** (engineering corollary of those papers).

### 3d. Harness / tool enforcement (make non-compliance mechanically impossible)

This is where an inference-only builder gets *actual determinism* over **actions**.
- **Deterministic hooks that gate tool calls and completion.** In the Claude Code / Agent SDK model, a
  **PreToolUse hook** fires *after* the model chooses a tool but *before* it runs, receives the full
  tool call as JSON, and can **block** it (exit code 2), with stderr surfaced back to the model as the
  reason — "deterministic control wrapped around non-deterministic AI." Anthropic docs: "Intercept and
  control agent behavior with hooks" (https://code.claude.com/docs/en/agent-sdk/hooks). A Stop/
  completion hook can likewise refuse to let the agent finish until a check passes. **[INFERRED]**
  (read via official-docs summary, not the page in-text, this run).
- **Forced tool use / `tool_choice`.** Providers let you *require* a tool call (or a specific tool), so
  a policy step (e.g. "you must call `check_policy` before answering") is not left to the model's
  discretion. Combined with **Structured Outputs `strict:true`** (§3a) the arguments are schema-valid
  by construction. **[VERIFIED]** (mechanism, via OpenAI structured-outputs page).
- **Capability boundaries — remove the tool.** The most reliable way to guarantee "never do X" is to
  make X *impossible*: don't expose the tool, scope credentials down, or run in a sandbox. No prompt
  can beat an absent capability. **[INFERRED]** (standard agent-security practice).
- **Deterministic post-processors.** Run the model's output through a normal program (formatter,
  linter, schema validator, regex redactor, unit tests) and only ship what passes; on failure, repair
  or re-prompt. Identical in spirit to §3a's validate-and-repair but owned entirely by your code.

### 3e. Instruction hierarchy & adherence design

- **Wallace et al., "The Instruction Hierarchy," arXiv:2404.13208 (§2).** The design pattern —
  explicit priority (system > developer > user > tool), and training the model to treat lower-privilege
  text as *data, not instructions* — is the provider-side lever that makes your system-prompt rules
  "stickier." As a builder you can *mirror* it in-context: label sources by authority, and instruct the
  model to treat tool/user content as untrusted. Raises p; does not guarantee (see "Control Illusion,"
  §2). **[VERIFIED].**
- **SysBench (arXiv:2408.10943, §1.4)** gives the design feedback loop: adherence correlates with
  attention on the system tokens, so anything that increases that attention (edge placement, brevity,
  re-assertion) should improve application — and SysBench's CSR/ISR/SSR are how you'd measure it.
  **[VERIFIED].**

### 3f. Training-time levers (exist, but out of reach for an inference-only builder)

Flagged for completeness — **an inference/harness-only builder cannot use these**, but they explain why
some adherence is already "baked in," and they are the reason a base model follows a plain system
prompt at all:
- **Instruction tuning / RLHF** (standard post-training).
- **RLAIF / Constitutional AI** (Bai et al., arXiv:2212.08073, §3b) — align to an explicit principle
  set via AI feedback. **[VERIFIED].**
- **Guan, Joglekar, Wallace, Heidecke, Beutel, Glaese, et al. (OpenAI) — "Deliberative Alignment:
  Reasoning Enables Safer Language Models," 2024, arXiv:2412.16339** (https://arxiv.org/abs/2412.16339).
  Directly the training-time analogue of this report's question: **teach the model the specification
  text and train it to explicitly *recall and reason over the spec before answering.*** Used on OpenAI's
  o-series; pushes the Pareto frontier (more jailbreak-robust *and* fewer over-refusals). The
  inference-only shadow of this is §3b's "recite the governing rule, then check your draft against it."
  **[VERIFIED].**

---

## 4. Measurement — verify application, don't assume it

You cannot manage what you don't measure; a builder needs a number for "adherence rate," not a vibe.

**Programmatic instruction-following (deterministic checkers).**
- **Zhou, Lu, Mishra, et al. — "Instruction-Following Eval (IFEval)," 2023, arXiv:2311.07911**
  (https://arxiv.org/abs/2311.07911). **25 types of "verifiable instructions"** (e.g. "write in more
  than 400 words," "mention keyword X ≥3 times") over ~500 prompts, **checked by a deterministic program
  — no LLM or human judge needed.** Reports strict and loose accuracy. This is the gold standard for
  the subset of your rules that are mechanically checkable. **[VERIFIED].**

**Complex / multi-constraint following.**
- **Jiang et al. — "FollowBench: A Multi-level Fine-grained Constraints Following Benchmark," ACL 2024,
  arXiv:2310.20410** (https://arxiv.org/abs/2310.20410). Five constraint categories (content, situation,
  style, format, example) with **incrementally added constraints (1→5)**; rule-based checks for closed
  outputs, model-based for open ones. **[VERIFIED]** (exists/scope; category detail **[INFERRED]** —
  secondary).
- **Qin et al. — "InFoBench," 2024, arXiv:2401.03601** (https://arxiv.org/abs/2401.03601). Introduces
  **DRFR (Decomposed Requirements Following Ratio)** — decompose each instruction into atomic yes/no
  criteria and score the fraction satisfied; 500 instructions, 2,250 decomposed questions.
  **[VERIFIED].**
- **Wen et al. — "Benchmarking Complex Instruction-Following with Multiple Constraints Composition"
  (ComplexBench), NeurIPS 2024, arXiv:2407.03978** (https://arxiv.org/abs/2407.03978). Hierarchical
  taxonomy of constraints + composition types (And/Chain/Selection/Nested). **[VERIFIED]** (exists;
  taxonomy detail **[INFERRED]** — secondary).
- **SysBench (arXiv:2408.10943)** and **Multi-IF (arXiv:2410.15553)** additionally measure **multi-turn
  stability / degradation across turns** — the metric that actually predicts whether your standing rules
  survive a long agent run. **[VERIFIED].**

**Faithfulness / grounding (did the output stay tied to the supplied material?).**
- **Es, James, Espinosa-Anke, Schockaert — "RAGAS: Automated Evaluation of Retrieval Augmented
  Generation," 2023, arXiv:2309.15217** (https://arxiv.org/abs/2309.15217). **Faithfulness = fraction
  of claims in the answer that are supported by the provided context** (plus context precision/recall
  and answer relevance). A near-perfect proxy for "did it apply the in-context material rather than its
  own priors?" **[VERIFIED]** (exists/purpose; the exact metric formula is **[INFERRED]** — secondary).
- **Min, Krishna, Lyu, Lewis, et al. — "FActScore: Fine-grained Atomic Evaluation of Factual Precision
  in Long-Form Text Generation," EMNLP 2023, arXiv:2305.14251** (https://arxiv.org/abs/2305.14251).
  Break a generation into **atomic facts** and score the % supported by a knowledge source; the
  automated estimator has "less than a 2% error rate." **[VERIFIED]** (method); the "ChatGPT 58%"
  headline is **[INFERRED]** — secondary.

**Rubric-graded adherence & "did-it-cite-the-rule."**
- Use an **LLM-as-judge (Zheng et al., arXiv:2306.05685)** rubric — one yes/no item per expertise rule
  — to produce a per-rule **adherence rate**, controlling for the documented judge biases (randomize
  position, cap verbosity, avoid self-grading). Pair it with a **structural check** that the output
  actually names the governing rule ID (cheap, deterministic, via §3a). **[VERIFIED]** (judge biases).

**Practical recommendation:** compute adherence on a fixed regression set every change — split into
(a) *programmatic* checks (IFEval-style) for mechanical rules and (b) *rubric* checks (judge) for
semantic rules — and track it like a test suite. Assume-it-applied is the anti-pattern this whole
report exists to prevent.

---

## 5. The determinism frontier (the crux)

Draw the line explicitly, because the entire architecture follows from where it sits.

| | **Can be made deterministic** | **Stays probabilistic** |
|---|---|---|
| **What** | Mechanically-checkable properties of the *output* or *action* | Semantic comprehension / judgment / "did it honor the intent" |
| **How** | Constrained decoding (§3a); harness gating, forced tools, capability removal, post-processors (§3d); programmatic validators (§3a) | The forward pass itself; any rule whose satisfaction only a human/LLM can assess |
| **Guarantee** | P(property holds) = 1 by construction, or the action is blocked | P(applied) < 1; raised by §3b/§3c/§3e, never to 1 |
| **Examples** | valid JSON; must-call-tool-before-answer; never-touch-file-X; output ≤N words; PII redacted; cited a `rule_id` field | "used the *right* rule"; "the tone is respectful"; "the design is sound"; "followed the *spirit* of the guideline" |

**The operating principle:** *maximize the fraction of your expertise that is expressed as a
mechanically-checkable predicate, enforce those deterministically in the harness, and route the
irreducibly-semantic remainder through a probabilistic critic with reject-and-repair.* Determinism is
not a property you can add to comprehension; it is a property you get by **moving the check out of the
model.** Every rule you can restate as "a program can verify this" is a rule you can *guarantee*; every
rule that needs judgment can only be *raised and audited*, never guaranteed.

Two honest caveats that keep this from being over-sold:
1. **Constrained decoding guarantees form, not correctness** (§3a) — forcing a schema does not force the
   values to be right or the rule cited to be the *applicable* one.
2. **Intrinsic self-critique can degrade without an external signal** (Huang et al., arXiv:2310.01798) —
   so the "probabilistic remainder" must be checked against something grounded (a tool, a test, a
   retrieved source, a verifier), not against the model's own unaided opinion.

---

## 6. Concrete architectures / patterns from industry

Four recurring shapes, then a recommended stack for an inference-only ("prompt + harness") builder such
as an agent-builder framework.

**Pattern A — Generate → check-against-rules → repair loop.** The output-guard pattern: Guardrails AI
(validator + automatic re-ask), NeMo Guardrails (Colang output rails), or your own
post-processor + re-prompt. Reasoning-time variants: Self-Refine, CoVe, Constitutional-style
critique→revise. Repair terminates on a *grounded* check, not self-opinion.

**Pattern B — Verifier-in-the-loop / best-of-N.** Sample N candidates and select with a verifier:
self-consistency majority vote (arXiv:2203.11171), a reward/process model (Let's Verify,
arXiv:2305.20050), or an LLM-judge (arXiv:2306.05685). Trades compute for higher application rate; the
verifier can score "obeys rule X" as its criterion.

**Pattern C — Guardrail pipeline (input rail → model → output rail).** Independent classifiers on both
boundaries: Llama Guard (arXiv:2312.06674) or Anthropic Constitutional Classifiers (86%→4.4% jailbreak
success at +0.38% refusals) on the safety axis; the same shape generalizes to any policy classifier. The
guard is a *separate model/program*, so it doesn't share the main model's blind spots.

**Pattern D — Policy engine + agent harness (the deterministic wrapper).** Instruction hierarchy for
authority ordering (arXiv:2404.13208); PreToolUse/Stop **hooks** that block non-compliant actions and
completions; **forced tool use** for mandatory policy steps; **capability boundaries** so forbidden
actions are impossible; **JIT re-surfacing** of the relevant rule before each step. This is the layer
that gives real guarantees on *actions*.

### Recommended stack for an inference-only builder ("expertise doc → reliable application")

A pipeline that turns a static expertise document into enforced behavior:

1. **Compile the expertise into two buckets.** For each rule, write either (a) a **mechanically-checkable
   predicate** (regex, schema, AST/lint check, unit test, tool-call precondition) — *preferred* — or (b)
   a **rubric item** (one yes/no line an LLM-judge can grade) when it is irreducibly semantic. Give every
   rule a stable **ID**.
2. **Inject at the right altitude + re-surface JIT.** Keep the always-on rules short and at the system-
   prompt edge; pull the *relevant* subset in immediately before each action; re-assert standing rules
   every few turns to fight multi-turn forgetting (§1.4, §3c).
3. **Force a plan that cites governing rule IDs.** Require (via forced tool use + `strict` structured
   output, §3a/§3d) a short plan whose schema includes the `rule_ids` it will honor — making
   "consulted the rule" a *checkable* artifact.
4. **Deterministically gate actions and completion.** PreToolUse hooks evaluate bucket-(a) predicates and
   **block + return the reason** on violation; a Stop hook refuses completion until all bucket-(a) checks
   pass; remove tools whose misuse you cannot tolerate (§3d).
5. **Critic-gate the semantic remainder with a grounded reject/repair loop.** An LLM-judge (bias-
   controlled) scores bucket-(b) rubric items; on failure, feed the specific failed rule back for a
   bounded number of repairs (Self-Refine/CoVe shape) — with the critic pointed at tool/source evidence,
   not just its own opinion (Huang caveat, §3b).
6. **Measure adherence as a regression suite.** Report per-rule adherence: IFEval-style deterministic
   checks for bucket (a), rubric-judge rate for bucket (b), plus a multi-turn-stability slice
   (SysBench/Multi-IF style). Ship only when adherence holds.

**Net:** presence is step 0. Reliable application comes from **(2)+(3)** raising the probability, and
**(4)** converting the checkable majority into hard guarantees, with **(5)** catching the semantic
remainder and **(6)** proving it. That division — *raise what you can, guarantee what you can check,
audit the rest* — is the current best answer to "how do you make the model actually use what's already
in its context."

---

## Appendix — VERIFIED vs INFERRED ledger

**Method.** WebSearch located sources; `ctx_fetch_and_index` pulled each into a local store; claims were
read via `ctx_search` over the stored full text/abstract. "[VERIFIED]" = the cited passage/figure was
read from the primary source **this run**. "[INFERRED]" = taken from a secondary/search summary of that
source, or a standard engineering corollary, and not confirmed in the primary text this run. No private
repositories were touched; no credentials handled; the spelled-out term for step-by-step reasoning was
avoided per instruction.

### Primary sources read this run (existence + core claim VERIFIED)
1. Lost in the Middle — TACL 2024 — arXiv:2307.03172 — https://arxiv.org/abs/2307.03172 — **[VERIFIED]**
2. Found in the Middle (positional attention calibration) — Findings ACL 2024 — arXiv:2406.16008 — https://arxiv.org/abs/2406.16008 — **[VERIFIED]** (mechanism)
3. Context Rot — Chroma, 2025 — https://www.trychroma.com/research/context-rot — **[VERIFIED]** (18 models; distractor & similarity findings; GPT-3.5 60.29% refusals; LongMemEval focused vs full)
4. Same Task, More Tokens — ACL 2024 — arXiv:2402.14848 — https://arxiv.org/abs/2402.14848 — **[VERIFIED]**
5. NoLiMa — ICML 2025 — arXiv:2502.05167 — https://arxiv.org/abs/2502.05167 — **[VERIFIED]** (11/13 ≤half base at 32K; Llama 3.1 70B 94.3%→42.7%)
6. Distracted by Irrelevant Context (GSM-IC) — ICML 2023 — arXiv:2302.00093 — https://arxiv.org/abs/2302.00093 — **[VERIFIED]**
7. System 2 Attention — 2023 — arXiv:2311.11829 — https://arxiv.org/abs/2311.11829 — **[VERIFIED]** (qualitative)
8. LLMs Get Lost in Multi-Turn Conversation — 2025 — arXiv:2505.06120 — https://arxiv.org/abs/2505.06120 — **[VERIFIED]** (-39%; aptitude vs unreliability; Concat 95.1%)
9. Multi-IF (Meta) — 2024 — arXiv:2410.15553 — https://arxiv.org/abs/2410.15553 — **[VERIFIED]** (o1-preview 0.877→0.707 turn1→turn3; 14 models)
10. SysBench — 2024 — arXiv:2408.10943 — https://arxiv.org/abs/2408.10943 — **[VERIFIED]** (500 sessions/2500 turns; CSR/ISR/SSR; attention-to-system correlation)
11. Towards Understanding Sycophancy — ICLR 2024 — arXiv:2310.13548 — https://arxiv.org/abs/2310.13548 — **[VERIFIED]**
12. Effective Context Engineering for AI Agents — Anthropic, 2025-09-29 — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents — **[VERIFIED]** (attention budget; JIT; right altitude)
13. The Instruction Hierarchy — OpenAI, 2024 — arXiv:2404.13208 — https://arxiv.org/abs/2404.13208 — **[VERIFIED]** (system-prompt-extraction defense +63%; priority order)
14. Efficient Guided Generation / Outlines — 2023 — arXiv:2307.09702 — https://arxiv.org/abs/2307.09702 — **[VERIFIED]** (FSM + vocabulary index + token masking; structure guaranteed)
15. Introducing Structured Outputs in the API — OpenAI, 2024-08-06 — https://openai.com/index/introducing-structured-outputs-in-the-api/ — **[VERIFIED]** (constrained decoding; "exactly match" schema; CFG > FSM)
16. NeMo Guardrails — EMNLP 2023 demo — arXiv:2310.10501 — https://arxiv.org/abs/2310.10501 — **[VERIFIED]** (programmable rails; Colang; dialogue runtime)
17. Llama Guard — 2023 — arXiv:2312.06674 — https://arxiv.org/abs/2312.06674 — **[VERIFIED]** (Llama-2-7B; prompt+response classification; taxonomy)
18. Constitutional Classifiers — Anthropic, 2025-02-03 — https://www.anthropic.com/research/constitutional-classifiers (paper arXiv:2501.18837) — **[VERIFIED]** (86%→4.4% jailbreak success; +0.38% refusals)
19. Judging LLM-as-a-Judge (MT-Bench/Chatbot Arena) — NeurIPS 2023 — arXiv:2306.05685 — https://arxiv.org/abs/2306.05685 — **[VERIFIED]** (position/verbosity/self-enhancement biases)
20. Self-Refine — NeurIPS 2023 — arXiv:2303.17651 — https://arxiv.org/abs/2303.17651 — **[VERIFIED]** (generator/feedback/refiner loop; 7 tasks)
21. Reflexion — NeurIPS 2023 — arXiv:2303.11366 — https://arxiv.org/abs/2303.11366 — **[VERIFIED]** (verbal RL; episodic memory buffer)
22. Chain-of-Verification (CoVe) — Findings ACL 2024 — arXiv:2309.11495 — https://arxiv.org/abs/2309.11495 — **[VERIFIED]** (draft → plan questions → answer independently → verified)
23. Constitutional AI — 2022 — arXiv:2212.08073 — https://arxiv.org/abs/2212.08073 — **[VERIFIED]** (SL critique→revision + RLAIF)
24. Self-Consistency — ICLR 2023 — arXiv:2203.11171 — https://arxiv.org/abs/2203.11171 — **[VERIFIED]** (sample paths + marginalize)
25. Let's Verify Step by Step — OpenAI, 2023 — arXiv:2305.20050 — https://arxiv.org/abs/2305.20050 — **[VERIFIED]** (process > outcome supervision; 78% MATH subset; PRM800K)
26. LLMs Cannot Self-Correct Reasoning Yet — ICLR 2024 — arXiv:2310.01798 — https://arxiv.org/abs/2310.01798 — **[VERIFIED]** (intrinsic self-correction can degrade; prior gains used oracle labels)
27. Deliberative Alignment — OpenAI, 2024 — arXiv:2412.16339 — https://arxiv.org/abs/2412.16339 — **[VERIFIED]** (teach spec; recall + reason over it before answering; o-series)
28. IFEval — 2023 — arXiv:2311.07911 — https://arxiv.org/abs/2311.07911 — **[VERIFIED]** (25 verifiable instruction types; deterministic checker; strict/loose)
29. FollowBench — ACL 2024 — arXiv:2310.20410 — https://arxiv.org/abs/2310.20410 — **[VERIFIED]** (exists/scope)
30. InFoBench (DRFR) — 2024 — arXiv:2401.03601 — https://arxiv.org/abs/2401.03601 — **[VERIFIED]** (DRFR; 500 instr / 2,250 decomposed)
31. ComplexBench — NeurIPS 2024 — arXiv:2407.03978 — https://arxiv.org/abs/2407.03978 — **[VERIFIED]** (exists/scope)
32. RAGAS — 2023 — arXiv:2309.15217 — https://arxiv.org/abs/2309.15217 — **[VERIFIED]** (faithfulness/grounding purpose)
33. FActScore — EMNLP 2023 — arXiv:2305.14251 — https://arxiv.org/abs/2305.14251 — **[VERIFIED]** (atomic-fact precision; automated <2% error)

### Claims labeled INFERRED (secondary/summary or engineering corollary; not confirmed in primary text this run)
- Found-in-the-Middle "+15 points" downstream RAG gain — secondary summary.
- Context Rot "13.9%–85%" degradation range — secondary summary. (Distractor/similarity/refusal findings are VERIFIED.)
- Same-Task-More-Tokens "~3,000-token onset" — secondary summary. (Degradation-below-max claim VERIFIED.)
- NoLiMa "GPT-4o 99.3→69.7" — secondary summary. (11/13-to-≤half-at-32K and Llama-70B 94.3→42.7 VERIFIED.)
- System-2-Attention "51.7%→61.3%" — secondary summary. (Factuality-up/sycophancy-down VERIFIED.)
- Multi-IF "Instruction Forgetting Rate" metric name — secondary summary. (Turn-degradation numbers VERIFIED.)
- Constitutional Classifiers "23.7% compute overhead" — secondary summary. (Page read said "moderate additional compute"; 86%→4.4% and +0.38% VERIFIED.)
- OpenAI Structured Outputs "100% vs <40% baseline eval" — secondary summary. (Constrained-decoding mechanism + "exactly match" VERIFIED.)
- LLM-as-Judge "~80% human agreement" — secondary summary. (Documented biases VERIFIED.)
- Self-Refine "~20% average gain" — secondary summary. (Method VERIFIED.)
- Reflexion "91% HumanEval pass@1" — secondary; not read. (Method VERIFIED.)
- Self-Consistency GSM8K "56.5→74.4" (and other task numbers) — secondary summary. (Method VERIFIED.)
- FActScore "ChatGPT 58%" — secondary summary. (Method + <2% automated error VERIFIED.)
- Guardrails AI validator/re-ask/RAIL-spec behavior — project/search summaries; primary docs not read this run.
- Claude Code hooks (PreToolUse fires pre-execution; exit code 2 blocks; stderr surfaced as reason) — read via official-docs summary (https://code.claude.com/docs/en/agent-sdk/hooks), not the page in-text this run.
- Forced tool use / capability-removal / edge-placement / spaced-reminder tactics — standard engineering corollaries of the VERIFIED papers, not single-source claims.
- "Control Illusion: The Failure of Instruction Hierarchies" (arXiv:2502.15851) — identified via search; not read in full this run.

### Not covered / open threads (candidates for a follow-up pass)
- Grammar-constrained decoding beyond FSMs (e.g. "Flexible and Efficient Grammar-Constrained Decoding,"
  arXiv:2502.05111) — surfaced but not read.
- AgentIF (arXiv:2505.16944) — agentic-scenario instruction-following benchmark; surfaced, not read.
- Attention-sink / "sink token" and KV-cache effects on long-context adherence — mechanism-level, not surveyed here.
- Empirical head-to-head of "raised-probability" (§3b/§3c) vs "hard-gate" (§3a/§3d) enforcement on the
  *same* rule set — not found in the literature this run; likely an original evaluation the builder must run.
