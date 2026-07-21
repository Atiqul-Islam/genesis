---
name: research-expertise
description: The exact procedure Sensei follows to SELECT and (when the user asks) deeply RESEARCH the expertise each built agent must apply — always user-consulted, never chosen silently. Use whenever building an agent or team, before assembling any agent, to establish its required expertise.
---

# Selecting & researching an agent's expertise

Expertise is never chosen silently. It is a decision, and you escalate every decision — so every step here
routes through the user. Run this for each agent you build, BEFORE assembling it (build-agent Step 3a calls
you). Consult your `agent-building` and `agentic-teams` expertise for the judgment calls.

**Precondition — the repo must be a Genesis workspace.** The researched expertise is stored at the
**repo level** in `<repo>/.genesis/` (never a global store). If the target repo has no `.genesis/` workspace
yet, run the bootstrap FIRST so the store, hooks, memory DB, and team exist:
```
python3 <genesis>/install/bootstrap.py <target_repo>
```
Then author expertise into `<target_repo>/.genesis/expertise/`.

## The procedure — six steps, in order

### 1. Pick candidate expertise
- From the confirmed spec (build-agent Step 1), draft the set of expertise THIS agent must apply — your
  judgment, backed by your agent-building/agentic-teams expertise.
- **Always include `expertise-application`** — it is auto-included in every agent, non-negotiable (it is the
  rule-set that makes an agent actually apply its other expertise).

### 2. Suggest & discuss with the user
- Present your proposed set to the user and ask what to **add, drop, or change**.
- Proceed ONLY on the user-confirmed set. Never finalize an expertise the user has not approved.

### 3. Ask whether to deep-research
- For the confirmed set, **ask the user whether to run deep research** now. This is the user's call per
  build — do NOT assume. Reuse an existing store module as-is only if the user is satisfied it covers the need.

### 4. If researching — confirm scope + documents
- Propose the **research requirements** (what each expertise must cover) AND a **source list** drawn from
  BOTH: (a) the user's own documents (ask them to supply files/paths/links) AND (b) sources you propose,
  including web sources you would search.
- The user **confirms both** the scope and the documents. Never research on assumptions — if scope or sources
  are unclear, stop and ask.

### 5. Propose the research method — the user verifies
- Propose HOW you will run the research (an agent team — one researcher per agent when building a team;
  primary-source grounding; no speculation). The user **verifies your method before it runs**.

### 6. Deep research, in parallel — produce a full enforceable module
- Spawn the researchers **in parallel across agents** (one research effort per agent in a team build).
- Each researcher deep-researches its expertise against the confirmed documents and produces a **full
  enforceable module**, same rigor as the existing store:
  1. the **guide** `expertise/<name>.md` (primary-sourced, verified-vs-inferred labelled);
  2. its **rule manifest** `expertise/manifests/<name>.json` — same schema as the existing manifests: each
     rule `{id, section, text, type: checkable|judgment|principle}` with a `predicate` (checkable) or
     `reviewer_criterion` (judgment), plus `sections_accounted` covering every section of the guide;
  3. **tests** that prove the manifest parses and the rules are faithful to the guide.
- Write all three into the **repo-level** `<repo>/.genesis/expertise/` and `.../manifests/`.

## Result — hand back to build-agent
- The user-confirmed expertise names become each agent's `required_expertise` (build-agent Step 3, TASK-SPEC).
- Method writes them into the agent's `meta.json`; the assembler auto-registers them in
  `<repo>/.genesis/expertise/required.json` and wires the Stop hook that blocks finishing until the agent
  DECLARES it applied each (`APPLIED-EXPERTISE: <name>#<rule-ids>`).
- An agent cannot be enforced to apply expertise that does not exist — so any new/deepened expertise is
  researched and authored (steps 4–6) BEFORE assembly.

## Never
- Never pick or finalize expertise without the user (steps 2–3).
- Never research without confirmed scope + documents (step 4) or an unverified method (step 5).
- Never store expertise globally — it is repo-level, in `<repo>/.genesis/` (bootstrap first if missing).
- Never speculate or take a shortcut; if anything is unclear, ask the user.
