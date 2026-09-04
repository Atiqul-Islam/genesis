# Feature: vector-based completeness WARNING for expertise declarations

## Context / Problem

The validate (Stop) hook enforces that DECLARED rules are real and evidenced, but it cannot tell whether the
agent SKIPPED a rule its response actually engaged (that is a judgment; deterministic code can't infer it).
We want a cheap, deterministic, **warn-only** nudge — no LLM — that surfaces rules the response looks like it
used but did not declare, using the embedder Genesis already ships (the ONNX memory server).

Honest scope: this is a **heuristic assist, not a guarantee**. Vector similarity ≈ applicability only
loosely (`memory-management.md` mm-35: cosine is ≈ chance for this class of judgment), so it WARNS, never
blocks, and its threshold is a calibration item (mm-26), never a blind constant.

## Expected Behavior

1. After a turn, the hook compares the agent's **response text** against each rule of the agent's required
   expertise, by embedding similarity.
2. Rules whose similarity to the response exceeds a configured threshold are **candidates** (likely engaged).
3. Any candidate rule NOT declared this turn is listed in a **WARNING**.
4. The warning **never blocks** the turn — it is surfaced (hook-decisions log + non-blocking notice).
5. The existing deterministic checks (real id, evidence, floor) are unchanged and still block as today.

## Acceptance Criteria

- AC1: A response strongly about testing, with no test-driven rule declared, yields a WARNING naming the
  likely-missed rule(s) — and the turn still FINISHES (not blocked).
- AC2: A response that declares its candidate rules yields NO warning.
- AC3: The warning never blocks — the hook decision is never `block` because of it, under any input.
- AC4: Embedder unavailable / any embed error → no warning, turn finishes (fail-open).
- AC5: Rule vectors are PRECOMPUTED; a turn embeds only the response (no re-embed of all rules each Stop).
- AC6: The similarity threshold is read from config; changing it changes which rules warn.

## Implementation Requirements

- **Precompute rule vectors** (embed each rule's text once; store alongside the manifest / a sidecar) so the
  Stop path embeds only the response. (ratified choice — perf; the Stop hook must stay fast.)
- **Embed the response via the existing ONNX embedder** (memory server / a `genesis-cli embed` path). The
  exact reach-from-Stop mechanism + latency is validated by a spike BEFORE full build (see Risks).
- **Threshold is config, calibrated on a small labeled sample** (mm-26); ship a default, keep it tunable;
  never hardcode blindly.
- **Warn surface:** the hook records the warning (hook-decisions.log) and emits a non-blocking notice; it
  MUST NOT set `decision: block`.
- **Fail-open:** any embedder/threshold/IO error → no warning, never block, never break Stop.
- Deterministic given a fixed model + threshold (same input → same warning). No LLM call.

## Risks / spike-first (verify before committing the full build)

- Can the Stop path embed the response fast enough (model already loaded / warm) without slowing finish?
- Does ANY threshold actually separate "engaged" from "not" on a small labeled set of real responses?
  If no threshold separates, vector-only warn is not worth shipping — report that instead of shipping noise.
