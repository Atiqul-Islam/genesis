# Memory-Management Expertise — Engineering an LLM Agent's Long-Term Memory

> **Purpose.** This is the definitive, evidence-backed practitioner guide to **LLM memory management** —
> how a Claude agent stores, recalls, consolidates, supersedes, merges, and forgets durable knowledge
> across sessions. It is written to **mechanize** memory work: it feeds **Mneme**, the Genesis
> memory-specialist agent that structures every write, runs the conflict conversation, and owns the
> `/genesis:memory` suite over a `sqlite-vec` store + `.jsonl` mirror. Every taxonomy, threshold,
> algorithm, and guardrail here is meant to be executed by a tool, not merely read.
>
> **Source material (this file is a faithful distillation — do not contradict):**
> `MNEME_MEMORY_RESEARCH.md` (Part A architectures/frameworks · Part B hygiene/operations/thresholds ·
> Part C failure-modes/safety/eval, three research agents, 2026-08-12) is the PRIMARY source; every rule
> below traces to it. `MNEME_MEMORY_SPEC.md` (the design of record, 2026-08-12) is the binding project
> design — the §15 rules encode it and nothing here may conflict with it.
>
> **Evidence discipline.** Facts carry inline citations (paper name + arXiv id, exactly as they appear in
> the research). **[VERIFIED]** = a finding the research attributes to a primary source. **Vendor-measured**
> benchmark numbers are labelled as such — per **mm-20**, never repeat an "X% better" number without naming
> who measured it and the config. **[DESIGN]** = a binding decision from the spec / Atiqul, not a
> general-literature fact. **[INFERENCE]** = this guide's engineering judgment over the sources.
>
> **Every actionable rule has a stable id (`mm-N`).** The companion manifest
> `manifests/memory-management.json` indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1 — distilled from `MNEME_MEMORY_RESEARCH.md` (Parts A/B/C) + `MNEME_MEMORY_SPEC.md`
> (design of record). No research dropped. Date: 2026-08-14.**

---

## 0. Executive summary — the memory lifecycle, then the load-bearing rules

Agent memory is not a bucket you append to. It is a **lifecycle** with correctness, cost, and safety
properties at every stage, and the wrong default at any stage silently degrades the whole system —
usually with **no error raised**. Three truths dominate everything below, so they lead.

**mm-1 (principle).** Model memory as a **six-phase lifecycle — Write → Store → Retrieve → Execute →
Share → Forget** — crossed with four objectives (Integrity, Confidentiality, Availability, Governance).
The **strongest correctness and safety mitigations act at WRITE time**: *"content never written can't be
extracted; content retained indefinitely almost certainly will be."* Gate quality, provenance, secrets,
and structure at the write, not at answer time. *(Toward Mnemonic Sovereignty, arXiv 2604.16548.)*

**mm-2 (principle).** The single most important correctness feature for a long-lived agent is:
**UPDATE = supersede, don't blind-append; invalidate, don't delete.** New knowledge retires old
knowledge while the history survives. Blind-append stores accumulate contradictions and serve stale
facts; delete-on-update destroys the audit trail and the ability to answer "as-of" questions.
*(Mem0 UPDATE/DELETE arXiv 2504.19413; Zep edge invalidation arXiv 2501.13956.)*

**mm-3 (checkable).** The **vector DB stays the recall engine.** Structure — `(subject, relation,
object)`, bi-temporal validity, provenance — is **added metadata layered on top of semantic search,
NOT a replacement for it.** Do not swap vector recall for a pure graph/relational store; augment it.
Recall must still work (via vector similarity) even for rows that have not yet been structured. **[DESIGN]**

The rest of this guide is the mechanism behind those three: the taxonomy (§1), the operation model (§2),
the framework landscape and design levers (§3–§4), the record schema and hygiene algorithms with their
thresholds (§5–§8), **the conflict machinery — the crux** (§9), retrieval and embedding hygiene
(§10–§11), merge/sync (§12), the failure-mode and safety catalogue (§13), evaluation (§14), and the
binding Mneme design-of-record (§15). A tunable **defaults table** and a **source ledger** close it.

---

## 1. Memory taxonomy — the shared vocabulary (CoALA)

**mm-4 (checkable).** Use the **CoALA four-type taxonomy** *(Cognitive Architectures for Language Agents,
arXiv 2309.02427)* as the vocabulary for every memory:
- **Working** — active info for the current cycle (the context window / scratchpad). One session.
- **Episodic** — past events/experiences, time-stamped ("what happened last time"). Cross-session.
- **Semantic** — decontextualized facts about the world/user ("user is vegetarian"). Cross-session, updated.
- **Procedural** — how-to: skills, rules, behaviors (system prompt, tool code, weights). Slowly changing.

**mm-5 (principle).** Know the orthogonal axis and the human-memory roots. The survey *(arXiv 2505.00675)*
splits memory into **parametric** (in the weights) vs **contextual** (external: unstructured text or
structured KG/tables); agent frameworks — and this system — are almost entirely **contextual**. The
cognitive-science roots a specialist should recognize: Atkinson–Shiffrin multi-store (→ context = STM,
DB = LTM), Baddeley working memory, Tulving's episodic/semantic/procedural split, and Complementary
Learning Systems (fast hippocampal episodes → slow neocortical consolidation → the basis for
**episodic→semantic consolidation**).

**mm-6 (checkable).** Apply the taxonomy **per-type** in this design **[DESIGN, spec §2]**:
- **semantic** — durable facts ("the denier component uses an async stream mediator"). Gets the full
  `(subject, relation, object)` + bi-temporal validity + supersession. **This is where conflicts live.**
- **episodic** — time-stamped events ("on 08-12 the build failed with X"). **Immutable**, decay-eligible,
  **NO supersession** — events don't contradict, they accumulate.
- **procedural** — how-to / rules / preferences ("user wants ≤30-word bullets"). Superseded by a newer
  rule for the same subject; key = `(subject, relation='rule', object)`.

---

## 2. The operation model — six atomic operations

**mm-7 (checkable).** Express all memory behavior as **six atomic operations: store, recall, consolidate,
merge, validate, serialize/deserialize** (the underlying primitives are consolidate, update, index,
forget, retrieve, compress). Mneme's whole surface is these six.

**mm-8 (checkable).** **STORE** in the general model = extract salient facts (an LLM call) then
**update-against-existing** (dedup / merge / supersede) — **NOT blind-append** — and index (embeddings +
BM25 + graph edges), optionally compressing/summarizing first. **In THIS design the agent hot-path store
is deliberately LIGHT** **[DESIGN, spec §4]**: write raw `text` + `type` + `scope` + provenance,
content-hash the id, embed, then `exact-hash gate → top-k ≥0.95 vector → UPDATE/NOOP else ADD`. Fast; **no
extraction on the hot path** — extraction/structuring is Mneme's decoupled job (§15).

**mm-9 (checkable).** **RECALL** = dense (cosine) **+** sparse (BM25) [+ graph traversal] then **rerank**
with multi-signal ranking (recency + importance + relevance). Never pure vector (§10).

**mm-10 (checkable).** **CONSOLIDATE** = summarize + **reflect** (higher-level insights from episode
clusters) + fact-extraction + **conflict resolution** (episodic→semantic, invalidate superseded) +
forget/decay + tier promotion. Split the work: **light hot-path writes + heavy background ("subconscious")
consolidation** — never block the responding agent on heavy consolidation.

---

## 3. The framework landscape — blueprints to reason from

You are not inventing memory architecture from scratch; you are selecting and combining known patterns.
Know them well enough to justify a choice and to recognize when a proposal is re-deriving a solved problem.

**mm-11 (principle).** The two **foundational blueprints**:
- **Generative Agents (Park 2023, arXiv 2304.03442)** — the **memory stream** (append-only NL
  observations) + a **reflection tree**. The inherited composite retrieval score is
  `α_recency·recency + α_importance·importance + α_relevance·relevance`, each min-max normalized;
  recency = exponential decay (~0.99–0.995) on last-access; importance = an LLM-assigned 1–10 "poignancy";
  relevance = cosine to query. Reflection triggers when summed recent importance crosses ~150.
- **CoALA (arXiv 2309.02427)** — the reference taxonomy (working/episodic/semantic/procedural) + a
  decision loop; "learning" is defined as **writing to episodic/semantic/procedural memory**.

**mm-12 (principle).** Know the framework landscape and what each contributes:
- **MemGPT / Letta (arXiv 2310.08560)** — LLM-as-OS **virtual context paging** (core memory blocks + FIFO
  vs external recall/archival vector store); agent self-edits via tools. Inspectable, developer-controlled
  **memory blocks**. DMR 32.1%→92.5% (GPT-4).
- **Mem0 (arXiv 2504.19413)** — production fact-memory: two-phase **extract** (rolling summary + last ~10
  msgs → candidate facts) then **update** (top ~10 similar → an LLM tool call picks **ADD / UPDATE /
  DELETE / NOOP**). **Mem0^g** graph variant: entities+triplets, **invalidate not delete** on
  contradiction. The most widely deployed OSS pattern. (Reported ~26% rel. accuracy gain / ~91% lower p95
  latency / ~90% fewer tokens vs full-context on LOCOMO — **vendor-measured**.)
- **Zep + Graphiti (arXiv 2501.13956)** — a **bi-temporal knowledge graph**; every fact edge stores FOUR
  timestamps (created/expired system time + valid/invalid world time); on a temporally-overlapping
  contradiction an LLM **invalidates** the old edge, never deletes — answering "what does the user believe
  NOW" while keeping "as-of" history. Non-lossy. DMR 94.8%. Best when facts change / time matters / audit
  is needed.
- **A-MEM (arXiv 2502.12110)** — **Zettelkasten** self-organizing notes with write-time **link generation**
  and **memory evolution** (a new note updates linked notes). Excels at **multi-hop** (~6× on LOCOMO
  multi-hop), ~85–93% token reduction, at real search overhead.
- **LangMem / LangGraph** — SDK over `BaseStore`; all three CoALA types; **namespaced by user_id**;
  defining axis = **hot-path** vs **background**; procedural memory via a **prompt optimizer**.
- **Cognee** — **ECL pipeline** (Extract→Cognify→Load) → hybrid graph+vector with ontologies; GraphRAG.
- **2025–2026 advances:** **MemoryOS (arXiv 2506.06326)** three tiers STM→MTM→LPM with **heat-scored**
  promotion/eviction; **Titans (arXiv 2501.00663)** **parametric** memory that writes to its own weights at
  test time, gated by a **surprise** signal + adaptive forgetting; **MemoryBank (arXiv 2305.10250)** the
  **Ebbinghaus forgetting curve** (strength decays, reinforced on recall); **HippoRAG (NeurIPS 2024)**
  OpenIE KG + Personalized PageRank for one-shot multi-hop.

**mm-13 (judgment).** **Match the pattern to the problem** — inspectable self-managed context →
Letta/MemGPT · cheap fast fact-memory → Mem0 · evolving/contradictory/time-sensitive + audit →
Zep/Graphiti · emergent multi-hop without a schema → A-MEM · three types + hot/background → LangMem ·
ontology-KG from docs → Cognee · parametric/test-time → Titans. **This system is a deliberate hybrid:
Mem0-style fact operations (ADD/UPDATE/DELETE/NOOP) + Zep-style bi-temporal supersession, layered over a
`sqlite-vec` recall engine.** **[INFERENCE, grounded in spec §1.]**

---

## 4. Cross-cutting design decisions — the levers

**mm-14 (judgment).** **Representation is the biggest lever.** Raw text (lossless, fuzzy retrieval, tokens
grow) vs extracted facts (compact/cheap, weakens multi-hop/temporal) vs KG (multi-hop/temporal/relational,
costly + drift). **Counter-current: verbatim chunks can BEAT lossy extraction** because extraction discards
information — never assume "graph/extraction is strictly better." Always **keep the raw text** so
re-embedding and re-extraction remain possible.

**mm-15 (checkable).** **Temporal modeling is first-class** — model **bi-temporal** validity (world-valid
time SEPARATE from system-transaction time), not a single timestamp, whenever facts can change.

**mm-16 (checkable).** **Consolidation timing** — combine **light hot-path writes** (immediate, minimal
latency/reasoning-budget cost) with **heavy background consolidation** (fast responses, eventual
consistency). Do not run heavy consolidation on the response path.

**mm-17 (checkable).** **Namespacing / isolation is non-negotiable** in any multi-user or multi-agent
store — **scope every read and write** (here the scope is `agent_id`). Cross-scope bleed is a privacy
failure.

**mm-18 (checkable).** **Provenance / auditability** — every derived fact keeps a link back to its source
("why do you believe this?"): `derived_from` / `source` / `evidence`.

**mm-19 (checkable).** **Forgetting is a feature.** Unbounded memory hurts **both** cost **and** retrieval
precision (distractors crowd out signal). Design decay / heat-eviction / importance-pruning / TTL
**deliberately** — a store with no forgetting mechanism is a defect, not a safe default.

**mm-20 (checkable).** **Benchmark integrity.** LOCOMO / LongMemEval (arXiv 2410.10813) / DMR are the
benchmarks. **NEVER repeat an "X% better" number without naming who measured it and the config** — vendor
benchmarks are marketing until independently reproduced, the Mem0-vs-Zep numbers were publicly disputed and
corrected, token-footprint claims are config-dependent, and LOCOMO carries annotation-noise criticism.

---

## 5. The record schema — the substrate (get this right first)

Most later problems are missing-field problems. Design the record before the algorithms.

**mm-21 (checkable).** The `id` is **content-addressed** = `sha256(normalized_text + type + scope)` — **NOT
a random UUID**. This makes dedup + cross-machine sync **idempotent and order-independent** (the same fact
on two machines collides to one row). In this design `content_id = sha256(normalized_text + type +
agent_id)`. **[DESIGN, spec §3.]**

**mm-22 (checkable).** The **embedding contract is ONE atomic unit** stored per record:
`embedding` + `embedding_model` + `embedding_version` + `dim` + `metric` + `normalized`.

**mm-23 (checkable).** Store **bi-temporal** fields: `valid_from` / `valid_to` (the world-truth window)
**SEPARATE** from `ingested_at` / `expired_at` (when the system learned / retracted it). A row is
**active ⇔ `valid_to IS NULL AND expired_at IS NULL`**. This separation is exactly what lets you supersede
without deleting.

**mm-24 (checkable).** Round out the row with: the structured triple **`subject` / `relation` / `object`**
(nullable for episodic and until structured); lifecycle (`created_at`, `last_used_at`, `use_count`,
`importance` 1–10); provenance/trust (`source` = session/machine/agent, `principal`, `asserted_by` ∈
{user, agent-inferred}, `confidence`, `evidence` pointer); and links (`supersedes` / `superseded_by` /
`derived_from`, `content_hash`). Maintain an **FTS5 shadow table** over `text` (+ `subject`/`object`) for
BM25. **[DESIGN, spec §3.]**

---

## 6. Deduplication — three layers, calibrated thresholds

**mm-25 (checkable).** Deduplicate in **three layers**: **exact** = SHA-256 of **normalized** text
(lowercase, collapse whitespace, strip markup) — the free gate; **near-dup** = MinHash/SimHash (LSH) for
paraphrases and templates; **semantic** = cosine threshold / clustering — the right layer for short,
paraphrase-heavy agent memories. For batch semantic dedup, **SemDeDup (NeMo, arXiv 2303.09540)**: k-means
then keep the item closest to the centroid (`which_to_keep=easy`).

**mm-26 (checkable).** Dedup thresholds (cosine, unit-norm) are literature **defaults to sweep on a labeled
sample, never blindly hardcode**: **≥0.95 = write-path "same → UPDATE/NOOP"**; **0.85–0.92 = consolidation
batch dedup**; **0.70 is too aggressive** (merges related-but-distinct). Apply exact-hash + conservative
**≥0.95 on WRITE**; reserve aggressive clustering / LLM-merge for **BACKGROUND** consolidation.

**mm-27 (judgment).** Where an LLM is available, **prefer Mem0 arbitration over a fixed threshold**:
retrieve the top-s≈10 similar → an LLM tool call returns **ADD / UPDATE / DELETE / NOOP**. The threshold is
a cheap gate; the LLM is the precise arbiter.

**mm-28 (checkable — gotcha).** **High cosine can mean CONTRADICTION**, not duplication ("I like tea" vs
"I hate tea" embed close). **Route the near-but-not-identical band (0.80 ≤ cos < 0.95) to the contradiction
check (§9), NOT to the trash.** Merging it would silently destroy one side of a real conflict.

---

## 7. Consolidation & reflection

**mm-29 (checkable).** **Trigger reflection by importance, not the clock**: when Σ importance of recent
events > ~150, take ~100 recent memories → have the LLM ask ~3 salient questions → retrieve → synthesize
insights → **STORE the insights as new retrievable memories** → build a hierarchical reflection tree.
*(Generative Agents, arXiv 2304.03442.)*

**mm-30 (checkable).** **Decouple consolidation from responding.** Run a **separate consolidator** in idle
time ("sleep-time compute", Letta) — reported ~5× less test-time compute and ~2.5× lower per-query cost.
**Mneme is that decoupled consolidator** in this design.

**mm-31 (checkable).** **NEVER destroy the source** when summarizing — derive semantic/summary memories that
carry `derived_from` back to the raw episodics (Zep is explicitly non-lossy). **Consolidate by
entity/topic, not by time-window alone.** Make consolidation **idempotent** (key off content-hash +
`superseded_by`) so re-runs don't duplicate.

---

## 8. Decay & forgetting

**mm-32 (checkable).** **recency = decay^(hours since LAST ACCESS)**, decay ≈ **0.995** — decay by
**access, not creation**. Retrieval **reinforces**: update `last_used_at` / `use_count` on every recall
("use it or lose it"). *(MemoryBank Ebbinghaus curve, arXiv 2305.10250.)*

**mm-33 (checkable).** Score staying-power as **`strength = w_r·recency + w_f·log(1+access_count) +
w_i·importance_norm`** (defaults `w_r=1, w_f=0.5, w_i=1`).

**mm-34 (checkable).** **Golden rule — tier + archive, NEVER hard-delete semantic facts.** Move
low-strength rows to a cold tier (still retrievable on deep search); supersede facts via `valid_to` /
`t_invalid` (keep the row). **Hard-delete ONLY** exact duplicates, expired TTL scratch, or an explicit
user "forget X". TTL is for ephemeral scratch only.

---

## 9. The conflict machinery — deterministic supersession + contradiction (THE crux)

This is the section that most changes outcomes, and where the intuitive approach is provably wrong.

**mm-35 (checkable — the killer finding).** **You CANNOT detect staleness or contradiction by embedding
similarity.** On labeled pairs, cosine **AUROC for contradiction-vs-duplicate = 0.59 (≈ chance), max
precision 0.67** — the safety floor is unreachable; **contradictions embed MORE similar to the original
than rephrased duplicates do**, and similarity-only RAG serves superseded values **15–40%** of the time.
**NEVER gate supersession or the conflict decision on a similarity threshold alone.**
*(MemStrata, arXiv 2606.26511; the "Implicit Conflict" failure — STALE, arXiv 2605.06527.)* **[VERIFIED]**

**mm-36 (checkable).** **The SOTA staleness fix is DETERMINISTIC supersession keyed by `(subject, relation,
object)`.** On a new value for an existing `(scope, subject, relation)` key, retire the prior active row in
the bi-temporal ledger — **NO similarity threshold, NO LLM call.** In this design:
`supersede_by_key(agent_id, subject, relation, new_valid_from)` sets the prior active row's
`valid_to = new_valid_from` and **KEEPS the row** (active = `valid_to IS NULL AND expired_at IS NULL`).
This yields 0.95–1.00 accuracy on evolving knowledge (vs similarity-RAG 0.20–0.47) and drives
**superseded-serve → ~0%**. *(MemStrata arXiv 2606.26511; spec §4/§12.)* **[VERIFIED + DESIGN]**

**mm-37 (checkable).** **Conflict = SEMANTIC CONTRADICTION only** — two **ACTIVE** facts with the **SAME
`(agent, subject, relation)` but a DIFFERENT `object`**, with overlapping validity ("ball is blue" vs
"ball is green"). Structure, keys, ordering, and metadata are **not** conflicts. *(Atiqul, verbatim; spec
§5.)* **[DESIGN]**

**mm-38 (checkable).** **Generate contradiction CANDIDATES by key-collision + vector-neighbourhood**, then
**judge** — a two-stage funnel: (1) candidates = same-`(subject,relation)` key **OR** high-cosine (≳0.80)
but below the exact-dup cutoff (opposite claims about one entity embed near each other); (2) an LLM/NLI
judge over `(new, candidate)` → {entails, contradicts, neutral}. **Candidate generation is key-collision +
neighbourhood; the decision is never similarity-alone.**

**mm-39 (judgment).** **Resolve contradictions per-case, never globally hardcoded:**
- **Supersede** — the new fact is a newer state, same relation, `new.valid_from > old.valid_from` → set
  `old.valid_to`; keep the row.
- **Keep-both** — both can be true (different validity/scope, or low confidence) → the **default when
  unsure**.
- **Ask-human** — low confidence **OR** high importance **OR** identity/safety-critical → **never silently
  pick**.

**mm-40 (checkable).** **Human-in-the-loop is the correct mitigation for the hard cases** (even LLM
judgment is imperfect). Mneme flags genuine contradictions in an **HTML report (user given the full path)**,
discusses until resolved, then applies the resolution (supersede one / keep-both-scoped / edit). This is the
`validate` / `merge` conflict path. *(Spec §5/§6.)* **[DESIGN]**

---

## 10. Retrieval & ranking — hybrid, fused, diversified, budgeted

**mm-41 (checkable).** **Retrieval is HYBRID — dense (cosine) + sparse (BM25).** Vector alone under-recalls
names, ids, and exact tokens.

**mm-42 (checkable).** **Fuse rankings with Reciprocal Rank Fusion:** `Σ 1/(k + rank)`, **k = 60** —
rank-based, scale-free, the industry default. Use a min-max score fusion (Weaviate `relativeScoreFusion`)
only when you must preserve score margins.

**mm-43 (checkable).** **Re-score with the composite prior** `α_rel·relevance + α_rec·recency +
α_imp·importance` (each min-max normalized; recency = `decay^Δhours`, 0.995), then apply a **validity
filter** (active rows only, unless the query is explicitly historical). **Bump `last_used_at` / `use_count`
on served rows.**

**mm-44 (checkable).** **Diversify with MMR:** `argmax[ λ·rel − (1−λ)·max sim-to-already-selected ]`,
**λ ≈ 0.7** — stops the top-N from becoming five paraphrases of one fact.

**mm-45 (checkable).** **Avoid over-recall.** Cap injected context to a token budget, enforce an absolute
**similarity FLOOR** (drop < ~0.2–0.3 even if it is top-k), and favor **precision over recall for injected
context**. Full pipeline: **BM25 + cosine → RRF(60) → composite re-score → validity filter → MMR(0.7) →
top-N under budget + floor → optional cross-encoder rerank.**

---

## 11. Embedding hygiene & migration

**mm-46 (checkable).** **Normalize embeddings to unit L2 at BOTH write and query** — then cosine and L2
give identical rankings; `sqlite-vec` KNN is L2-oriented, so normalizing makes **L2-KNN == cosine** (the
server already L2-normalizes). The **metric must match the model's training** (modern embedders →
cosine/dot). `sqlite-vec`'s `vec_distance_cosine` is valid for float32/int8 only and errors on mismatched
type/length — store `dim` + element type consistently.

**mm-47 (checkable).** **The embedding contract `{model, version, preprocessing, normalization, metric}` is
ONE atomic unit** — change any part → version bump → **RE-EMBED THE WHOLE STORE**. **NEVER mix embedding
generations** in one index (silent recall degradation, **no error raised**); even an unchanged API
model-string can hide new vectors. **Version-stamp every embedding and REFUSE to serve a mixed-model
index.** On migration, re-embed the whole corpus (blue-green) **or** use a learned Drift-Adapter (recovers
95–99% recall — arXiv 2509.23471); keeping the raw text (mm-14) is what makes re-embedding always possible.

**mm-48 (checkable).** **Run a drift canary in `validate`:** re-embed canary texts and compare to stored —
**<0.001 stable · 0.001–0.02 minor · 0.02–0.05 significant · >0.05 severe → re-embed all**; also check
nearest-neighbor stability. Monitor recall against a golden set on a schedule.

---

## 12. Merge / sync & serialization

**mm-49 (checkable).** **Merge is UNION WITH RECONCILIATION, not concatenation** — reuse **ADD / UPDATE /
DELETE / NOOP** as the merge operator over content-addressed ids (the same fact on two machines collides to
one row; order-independent). Algorithm: exact-hash → **NOOP**; ≥0.95 near-dup → **UPDATE** (keep the higher
importance, union `access_count`); contradiction → **supersede via bi-temporal** (keep both histories);
else **ADD**.

**mm-50 (checkable).** **Cross-machine conflict = a contradiction with a different `source`** → resolve by
**bi-temporal supersession** so both histories survive. **Last-writer-wins is allowed ONLY for
single-valued profile fields, NEVER for the append-only collection.**

**mm-51 (checkable).** **The JSONL mirror is append-only, one object per line, DETERMINISTICALLY ordered
with stable key order** → clean git diffs and line-level merges. It carries full contract metadata
(model/version per record) and **re-embeds on import if the contract differs**.

**mm-52 (judgment — documented tension, do not silently flip).** **`.db` (`sqlite-vec`) is the source of
truth; `.jsonl` is the derived mirror** — Atiqul's decision. **[DESIGN, spec §1.]** The best-practice
literature (LangMem, and the merge/sync section of the research) treats the **JSONL as the sync source of
truth and the vector DB as a rebuildable index — the OPPOSITE.** Both can coexist (JSONL = mergeable sync
substrate; `.db` = ready runtime store), but **"which is authoritative on a conflicting pull" is a real,
open decision.** Present it to Atiqul; **do not flip it silently.**

---

## 13. Failure modes & safety — the gotchas a specialist MUST know

**mm-53 (checkable).** **Treat EVERY write as a privileged, validated state transition** — writable memory
is a persistent attack surface, and poisoning is real and stealthy: **MINJA (arXiv 2503.03704)** query-only
poisoning by an ordinary user, >95% injection / >70% attack success, **existing moderation ineffective**;
**AgentPoison** (<0.1% poison rate); **MemoryGraft (arXiv 2512.16962)** poisons **procedural/experience**
memory via benign README/task-log artifacts with no trigger (factual verification misses it); **MemMorph
(arXiv 2605.26154)** tool-hijack, 85.9% ASR with 3 records. **Mitigate:** generate-then-freeze with
human/policy verification for durable writes; **provenance on every record** (source, principal, version,
sensitivity, time); **never trust retrieved/tool/other-agent claims at user-fact level**; principal-scoped
access; **adaptive-attacker testing** (static robustness lies — 12 defenses were bypassed at >90% ASR).

**mm-54 (checkable).** **Do not store hallucinations as durable facts.** They arise at **extraction/update**
and propagate to QA, so **gate at WRITE, not answer time** *(HaluMem, arXiv 2511.03506: fabrication,
errors, conflicts, omissions)*. **Verify before persist**; record `asserted_by` (user-asserted vs
model-inferred) + `confidence` + `evidence` **in the record**; **never let a summarizer mint facts
silently** — store as `type=summary, unverified`, linked to its source.

**mm-55 (checkable).** **NEVER store credentials / secrets / keys / PII** — write-time minimization beats
store-time encryption. Irreversibly mask secrets and store **"credential present at `<ref>`"**; redact PII
at write. Use **retention tiers** (audit-mandatory / operational / user-deletable) + TTL; **lineage
tracking so deletes CASCADE** to summaries/derivatives; a **user-visible memory inventory**; monitor
tool/inter-agent channels (they leak). Unlearning is imperfect — a weights backstop, not the runtime
control. *(Aligns with the hard rule: never persist credentials.)*

**mm-56 (checkable).** **Fight bloat / context-pollution** — **over-store AND over-recall both degrade
reasoning before the window fills** *(MemGuard arXiv 2605.28009; Mem0)*. **Precision > recall at
injection:** consolidate near-dups into UPDATEs; **type-separate** memory (facts vs episodic vs rules vs
procedures) and **retrieve within type**; scope by user/topic/time; store salient facts, not raw chunks;
budget injected context.

**mm-57 (checkable).** **Avoid catastrophic forgetting** — binary keep-all/lose-all is wrong *(FadeMem
arXiv 2601.18642; SSGM arXiv 2603.11768)*. Use **differential decay** by relevance + frequency + recency +
importance; **pin/protect critical memories** (safety, identity, audit tier) from decay/consolidation; make
consolidation a **reversible UPDATE with content-addressable snapshots** (rollback); **decouple
memory-evolution from execution** (never let a live task rewrite durable memory).

---

## 14. Evaluation

**mm-58 (checkable).** **Evaluate across the WHOLE lifecycle, not just final QA.** Benchmarks:
**LongMemEval (arXiv 2410.10813)** — 5 abilities: extraction, multi-session, temporal, knowledge-update,
abstention; **LoCoMo** — multi-session QA (small → saturation risk); **HaluMem** — op-level hallucination;
**STALE (arXiv 2605.06527)** — belief revision (State Resolution / Premise Resistance / Implicit Policy
Adaptation). **Watch MemDelta** (eval confounds — hold k / model / prompt fixed).

**mm-59 (checkable).** **Track the lifecycle metrics:** recall/precision@k, MRR/nDCG, injected-context
precision; op-level extraction/update accuracy; knowledge-update + temporal accuracy; **superseded-serve
rate (~0% TARGET)**; contradiction rate; abstention rate; ISR/ASR under red-team; tokens/conversation, p95
retrieval latency, storage, ingestion→retrieval delay. **Validate the LLM judge against humans.**

---

## 15. Mneme design-of-record bindings (spec consistency — these are BINDING)

These rules encode `MNEME_MEMORY_SPEC.md`. Nothing elsewhere in this file may conflict with them.

**mm-60 (checkable).** **Division of labour** *(spec §1)*: the **SERVER** does storage + embeddings +
retrieval (**NO generative LLM**); **MNEME** is the LLM-driven, **DECOUPLED** specialist that
extracts/structures/dedupes/supersedes/consolidates and **owns the conflict conversation**; **OTHER AGENTS**
store raw memories fast (the light hot-path of mm-8). **[DESIGN]**

**mm-61 (checkable).** **Mneme STRUCTURES ON WRITE, SYNCHRONOUSLY** *(spec §11, D-a/D-b resolved)* — it
structures the moment an agent writes: extracts `(subject, relation, object)`, refines `type`, and rates
`importance` 1–10 (D-e). The schema allows S-R-O **nullable until structured**, so a write is never lost if
structuring is momentarily pending; recall still works via vector in the meantime. Exact mechanism is
decided at P3. **[DESIGN]**

**mm-62 (checkable).** **The `/genesis:memory` suite is Mneme-owned and canonical/in-place** *(spec §6)*:
- **validate** — compare `.db` vs `.jsonl`; run the embedding-contract/drift canary (mm-48); detect orphaned
  provenance; **FLAG contradictions** → HTML report on conflict.
- **deserialize** — `.db` → `.jsonl` (server `export`).
- **serialize** — `.jsonl` → `.db`: **PROMPT + WARN** (because `.db` is base truth, mm-52);
  **timestamp-rename the old `.db`**, then rebuild + re-embed (server `import`).
- **merge `[<db-path>]`** — union by content-hash; **supersede on key-collision**; **contradictions → HTML
  → resolve → merge**; **stray stores are READ-ONLY**. **[DESIGN]**

**mm-63 (checkable).** **The `.jsonl` mirror format EXTENDS** to carry the new structured/bi-temporal/
provenance fields; serialize/merge must **round-trip losslessly**; new fields are **additive** *(spec §8,
D-c)*. **Migration on a Genesis update:** **ASK the user first**, then Mneme runs an LLM extraction pass
rewriting flat memories into the structured schema (S-R-O + type + bi-temporal defaults `valid_from =
created_at`); grandfathered until migrated (recall still works via vector); **idempotent and resumable**.
**[DESIGN]**

---

## Defaults table (literature defaults — tune on our own labeled data)

`content_id = sha256(normalized_text + type + agent_id)` · exact-dedup = SHA-256(normalized) ·
write-dup cosine **≥0.95** · consolidation-dup **0.85–0.92** · contradiction candidate band
**0.80 ≤ cos < 0.95** · recency **decay 0.995 / hour-since-access** · strength weights
`w_r=1, w_f≈0.5·log(1+access), w_i=1` · reflection trigger **Σ importance > ~150** · composite α's = 1 ·
**RRF k = 60** · **MMR λ ≈ 0.7** · similarity floor **~0.2–0.3** · embedding drift **>0.02 investigate /
>0.05 re-embed all** · superseded-serve target **~0%** · supersession = **deterministic by `(subject,
relation, object)` — never by similarity**.

---

## Source ledger

**Primary source (this file is its faithful distillation):** `MNEME_MEMORY_RESEARCH.md` — Part A
(architectures & frameworks), Part B (hygiene/operations/thresholds), Part C (failure-modes/safety/eval),
three research agents, 2026-08-12. **Binding design:** `MNEME_MEMORY_SPEC.md` (P0 design of record,
2026-08-12) — encoded in §15.

**Cited works (arXiv id / venue exactly as they appear in the research):**
CoALA 2309.02427 · memory-ops/taxonomy survey 2505.00675 · human→AI memory 2504.15965 · episodic-memory
gap 2502.06975 · Generative Agents 2304.03442 · MemGPT/Letta 2310.08560 · Mem0 2504.19413 · Zep+Graphiti
2501.13956 · A-MEM 2502.12110 · MemoryOS 2506.06326 · Titans 2501.00663 · MemoryBank 2305.10250 · HippoRAG
(NeurIPS 2024) · SemDeDup 2303.09540 · Drift-Adapter 2509.23471 · **MemStrata 2606.26511 (the killer
finding for the conflict design)** · STALE 2605.06527 · MINJA 2503.03704 · MemoryGraft 2512.16962 ·
MemMorph 2605.26154 · HaluMem 2511.03506 · MemGuard 2605.28009 · FadeMem 2601.18642 · SSGM 2603.11768 ·
Toward Mnemonic Sovereignty 2604.16548 · LongMemEval 2410.10813 · LoCoMo · DMR.

**Evidence flags carried from the research:** vendor-measured benchmark numbers (Mem0 LOCOMO deltas) are
labelled vendor-measured and never repeated as neutral fact (mm-20); the `.db`-vs-`.jsonl` source-of-truth
question is a live tension surfaced (mm-52), not silently resolved; every numeric threshold is a literature
DEFAULT to calibrate on our own labeled sample (mm-26, Defaults table).

*Colophon: v1, 2026-08-14. Distilled with zero shortcuts from `MNEME_MEMORY_RESEARCH.md` (Parts A/B/C) and
`MNEME_MEMORY_SPEC.md`; 63 rules (mm-1…mm-63); consistent with the design of record; verified vs
design-decision vs inference separated throughout.*
