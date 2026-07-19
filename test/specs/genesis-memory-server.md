# test/specs/genesis-memory-server.md

> **Status: RATIFIED** (spec-forge run `20260719-052630-genesis-memory-server`, Phase 1).
> Hallucination audit clean across 3 iterations (markers 34 → 9 → 0), then explicitly approved
> by Atiqul. Grounded solely in `docs/SPEC_FORGE_RUST_UPDATE.md` §2/§5/§6.2 and the `server/`
> scaffold. Read the Provenance legend below before treating any claim as sourced.

## Feature: Genesis MCP Memory Server

Per-agent semantic memory over SQLite + `sqlite-vec` with local ONNX embeddings,
exposing `store` / `recall` / `consolidate` tools to any MCP client over stdio.
Every tool call is scoped to a caller-supplied `agent_id` in one shared database.

### Provenance legend (how to read this spec)

Every claim below is one of three kinds, and each unsourced claim says so inline:

- **Sourced** — traceable to `docs/SPEC_FORGE_RUST_UPDATE.md` (§ref) or to the committed
  `server/` scaffold. Cited in place.
- **(ratified choice — unsourced; rationale: …)** — no source exists; a human ratified the
  choice in iteration 3 (decisions D1–D8). Labelled inline everywhere it appears. These are
  decisions, not findings — do not read them as sourced.
- **Bootstrap / calibration item** — a value that can only be measured once the artifact
  exists (a file digest, an empirical fixture pair). The spec states the *requirement* to pin
  it; the literal value is captured at first run and committed as a constant. Same shape as
  §1.1's CRAP-threshold calibration caveat. Collected in "Bootstrap and calibration items".

**Out of scope (v2): summarize/evict, and dedup-on-insert.**
`THETA_EVICT`, `MIN_AGE`, `TAU_SIM`, `K`, and `E%` are named by §2.4 but carry no
values, and §6.2 #8 marks all of §2.4 INFERRED. v1 `consolidate` implements only the
two sourced halves — decay/recency scoring and dedup/merge. The `cap` config key
(`10_000`, per the `ConsolidationConfig::default` in `server/src/consolidate.rs`) is
retained but unused in v1; it is the v2 eviction trigger. Summary rows, summarizer
injection, and eviction ranking are deferred with it. **Deferred alongside them: running
dedup/merge at `store` time**, which §2.4 titles "Dedup/compress **on insert**" — see
"Deliberate deviations from source" (D8).

### Expected Behavior

1. An MCP client that completes `initialize` sees a server advertising a tools capability and protocol version `2024-11-05`.
2. `tools/list` includes the tools `store`, `recall`, and `consolidate`.
3. A memory sent to `store` for an agent is afterwards returned by a `recall` of closely related (paraphrased) text for that same agent, when that memory is the only one stored or is clearly nearest among deliberately dissimilar alternatives.
4. `recall` returns at most `k` memories, ordered nearest first (ascending distance).
5. `recall` of text identical to a stored memory returns that memory with distance `0.0`.
6. `recall` for one `agent_id` never returns memories stored under a different `agent_id`.
7. `recall` called without `k` still succeeds and returns at most 5 memories.
8. `recall` never returns a memory that a previous `consolidate` retired as a duplicate.
9. Embedding the same text twice inside one server process yields the same vector.
10. An embedding whose length is not 384 makes `store`/`recall` return an `Err` instead of panicking.
11. A failing tool call comes back as a successful MCP response carrying `isError: true`, not a JSON-RPC error.
12. A malformed JSON-RPC request comes back as a protocol-level JSON-RPC error, not an `isError` result.
13. A near-duplicate memory stops being returned separately by `recall` only after an explicit `consolidate` call; storing it alone never retires anything.
14. The server serves over stdio and exits cleanly when the client closes stdin.
15. The server answers `store` / `recall` / `consolidate` calls with no network access at request time.
16. `recall` reports each hit's identifier, original text, and distance in a machine-readable payload rather than as prose.
17. The server reads its database location from the environment, so two clients configured with different locations never see each other's memories.

### Acceptance Criteria

1. An `initialize` response advertises `tools` under `capabilities` and `protocolVersion == "2024-11-05"`.
2. A `tools/list` response contains the tool names `store`, `recall`, and `consolidate`.
3. Given an empty store, when `store` is called with `agent_id` A and text T and `recall` is then called with `agent_id` A and a paraphrase of T, the returned array contains an entry whose `text` equals T; the same holds when the store also contains the calibrated dissimilar-decoy fixture, in which case T's entry is first. No absolute similarity threshold is asserted.
4. Given more stored memories than `k`, a `recall` with `k` returns exactly `k` entries whose `distance` values are non-decreasing.
5. Given a memory stored with text T, a `recall` with query text T returns that memory first with `distance` exactly `0.0`.
6. Given one memory under `agent_id` A and one under `agent_id` B, a `recall` for B contains no memory belonging to A.
7. Given 6 or more memories for `agent_id` A, a `recall` for A whose arguments omit `k` returns exactly 5 entries.
8. Given two memories merged by `consolidate`, a `recall` never returns the row whose `superseded_by` is non-null.
9. `Embedder::embed` called twice on the same text in the same process returns two vectors with cosine `>= 0.9999` (equivalently, equal within absolute tolerance `1e-4`), asserted in the release profile.
10. `VectorStore::insert` and `VectorStore::knn` called with a 383-element vector each return `Err` and the test process does not panic.
11. A `tools/call` that triggers an internal failure returns a JSON-RPC result whose `isError` field is `true`.
12. A syntactically invalid JSON-RPC request written to stdin yields a JSON-RPC `error` object rather than a result.
13. Given two memories for the same `agent_id` whose cosine similarity is at or above `tau_merge`, a `recall` issued after `store` but before `consolidate` returns both, and a `recall` issued after `consolidate` returns exactly one of them and never the superseded one.
14. A server child process that receives EOF on stdin terminates with exit status 0.
15. With the model already present on disk, a `tools/call` for each of `store`, `recall`, and `consolidate` completes successfully while the suite runs with outbound network access unavailable.
16. The text content block of a `recall` result parses as a JSON array whose every element has exactly the keys `id` (integer), `text` (string), and `distance` (number).
17. Given two server processes launched with `GENESIS_MEMORY_DB` pointing at two different temporary files, a memory stored through the first is absent from a `recall` issued to the second for the same `agent_id`.

### Implementation Requirements

**Tool API (agent scoping)**

- Accept `store` arguments as `StoreArgs { agent_id: String, text: String }`.
- Accept `recall` arguments as `RecallArgs { agent_id: String, query: String, k: Option<u32> }`.
- Accept `consolidate` arguments as `ConsolidateArgs { agent_id: String }`.
- Default `k` to 5 when the `recall` arguments omit it (ratified in iteration 2).
- Serve all agents from one SQLite database rather than one database per agent.
- Scope every `memories` query by `agent_id`.

**Recall response payload**

- Serialize the `recall` result as a JSON array of objects (ratified choice — unsourced shape; rationale: §2.3a sources only the `ContentBlock::text` envelope, and a machine-readable body makes criteria 3/4/5/16 assertable instead of substring-sniffing prose).
- Give each `recall` result object exactly the keys `id`, `text`, and `distance` (ratified choice — unsourced key names; rationale: same as above).
- Order the `recall` result array by ascending `distance`, nearest first — honouring §2.3b's `ORDER BY distance LIMIT k`.
- Carry that JSON array as the string inside a single `ContentBlock::text`, keeping §2.3a's sourced `CallToolResult::success(vec![ContentBlock::text(..)])` envelope unchanged.

**Database location**

- Read the SQLite database path from the environment variable `GENESIS_MEMORY_DB` (ratified choice — unsourced variable name; rationale: §5 #2 requires the BDD `World` to hold a `tempfile` DB path per scenario, so the path MUST be injectable, and environment is the conventional MCP configuration channel because clients control env and rarely control argv).
- Fall back to `genesis-memory.db` in the process working directory when `GENESIS_MEMORY_DB` is unset (ratified choice — unsourced default; rationale: keep the fallback simple and platform-neutral).
- Do not derive the default database path from an OS-specific data directory.

**Vector store**

- Register the `sqlite-vec` extension with `sqlite3_auto_extension(sqlite3_vec_init)` before opening the connection.
- Create the vector table with `CREATE VIRTUAL TABLE vec_items USING vec0(embedding float[384])`.
- Create a `memories` table.
- Give `memories` the columns `id`, `agent_id`, `text`, `created_at`, `last_used_at`, `use_count`, `base_score`, `superseded_by`.
- Declare `memories.id` as `INTEGER PRIMARY KEY` so SQLite assigns it.
- Make `memories.superseded_by` nullable and reference `memories.id`.
- Keep `vec_items.rowid` equal to `memories.id`.
- Read the assigned id with `last_insert_rowid()` immediately after inserting the `memories` row.
- Insert the embedding into `vec_items` under that same rowid, so the `vec_items.rowid == memories.id` invariant is established in exactly one place.
- Return the assigned `i64` from `VectorStore::insert`.
- Do not accept a caller-supplied `id` parameter in `VectorStore::insert` — the scaffold's `insert(&mut self, id: i64, …)` in `server/src/store.rs` is a stub whose body is `unimplemented!("Implement via TDD")`, and this requirement supersedes that signature.
- Pass embeddings to SQLite as bytes via `bytemuck::cast_slice::<f32, u8>`.
- Issue KNN as `SELECT rowid, distance FROM vec_items WHERE embedding MATCH ?1 ORDER BY distance LIMIT k`.
- Keep `vec0`'s default L2 distance metric (no `distance_metric=cosine` DDL).
- Take `agent_id` in `VectorStore::insert` and record the row under it.
- Take `agent_id` in `VectorStore::knn` and restrict results to that agent.
- Exclude rows with a non-null `superseded_by` from `recall` results.

**Embeddings**

- Fix the embedding dimensionality at `EMBED_DIM = 384`.
- Use the `all-MiniLM-L6-v2` model (384-dim, mean pooling) and not `bge-small-en-v1.5`.
- Load the tokenizer with `Tokenizer::from_file` and encode with special tokens enabled.
- Pool token hidden states with an attention-mask-weighted mean.
- L2-normalize the pooled vector before storing or querying.
- Clamp the pooling denominator at `1e-9` before dividing.
- Clamp the L2 norm at `1e-12` before dividing.
- Build the ONNX session with `with_deterministic_compute(true)`.
- Build the ONNX session with `with_intra_threads(1)`.
- Build the ONNX session with a pinned graph-optimization level.
- Build the ONNX session with the CPU execution provider.
- Commit the ONNX session from the model file with `commit_from_file`.
- Assert golden and determinism embedding comparisons at absolute tolerance `1e-4`, or equivalently cosine `>= 0.9999`.
- Tune and gate those embedding tolerances in the release profile.
- Confirm at implementation whether the shipped ONNX export emits `last_hidden_state` at output[0] — §6.2 #6.
- Confirm at implementation whether the shipped ONNX export requires a third `token_type_ids` input (`Session::inputs` dump) — §6.2 #6.

**Model provenance**

- Provide `scripts/fetch-model`, which downloads the model and tokenizer into `server/models/` (ratified in iteration 2).
- Fetch from the Hugging Face repository `sentence-transformers/all-MiniLM-L6-v2` — §2.3c names `all-MiniLM-L6-v2` as the primary model and §6.1 verifies its pooling mode from that repo's `1_Pooling/config.json`.
- Fetch the files `onnx/model.onnx` and `tokenizer.json` — §2.3c refers to "the raw HF `onnx/model.onnx`".
- Pin an explicit repository revision in `scripts/fetch-model`.
- Download only from that pinned revision.
- Record in `scripts/fetch-model` that the revision is load-bearing because §6.2 #6 states ONNX exports of the same model differ in output shape (pooled output vs `last_hidden_state`) and in whether `token_type_ids` is required.
- Keep the model artifacts out of git (`.gitignore` already ignores `*.onnx`).
- Assert the pinned SHA-256 of the fetched model file before running embedding tests.
- Commit the pinned revision string as a constant, captured at first fetch — see "Bootstrap and calibration items".
- Commit the pinned SHA-256 digest as a constant, captured at first fetch — see "Bootstrap and calibration items".
- Make embedding tests fail — never skip — with a message directing the developer to run `scripts/fetch-model` when the model file is absent, because the BDD suites use the real model with no mocks (§5 #2).
- Do not rely on `ort`'s `download-binaries` feature to supply the model: it fetches ONNX Runtime shared libraries at build time only, never the model weights.

**Consolidation (decay + dedup/merge only)**

- Compute `effective = base_score * exp(-lambda * age_days) * (1 + beta * ln(1 + use_count))`.
- Default `lambda` to `ln2 / 30`.
- Default `beta` to `0.15`.
- Default `tau_merge` to `0.95`.
- Default `base_score` to `1.0` as a `ConsolidationConfig` field, not a magic literal (ratified choice — unsourced; this is a chosen **normalization**, not a measured value: with `use_count = 0` and age 0 the formula yields `1.0 * exp(0) * (1 + beta * ln(1)) = exactly 1.0`, which makes the sourced 30-day half-life directly assertable as `effective = 0.5` at 30 days).
- Write the configured `base_score` default into every new `memories` row at `store` time rather than a literal in the insert statement, so the normalization stays config-exposed and overridable per test.
- Measure `age_days` from `created_at`, not from `last_used_at` (ratified interpretation of §2.4 — unsourced: §2.4 writes `exp(-lambda*age_days)` without naming the basis; rationale: recency already enters through the separate `(1 + beta * ln(1 + use_count))` usage term, and §2.4 bumps `use_count` and `last_used_at` on recall, so measuring age from `last_used_at` would double-count recency).
- Keep `cap` in configuration with the value `10_000` (per `ConsolidationConfig::default` in `server/src/consolidate.rs`), unused in v1, as the v2 eviction trigger.
- Derive cosine similarity from L2 distance as `1 - L2^2 / 2` (valid for normalized vectors).
- Select the dedup candidate as the KNN top-1 result restricted to the same `agent_id`.
- Merge two memories when their cosine similarity is at or above `tau_merge`.
- Run dedup/merge only inside an explicit `consolidate` call, never as part of `store` — a deliberate documented deviation from §2.4's "Dedup/compress on insert" phrasing; see "Deliberate deviations from source" (D8).
- On merge, keep the higher-scored row as the survivor.
- On merge, sum `use_count` into the survivor.
- On merge, set the loser's `superseded_by` to the survivor's id.
- On merge, write no new vector row.
- On recall, increment `use_count` for each returned memory.
- On recall, set `last_used_at` for each returned memory.
- Inject `now` through a clock abstraction rather than reading the wall clock.
- Expose every consolidation threshold as configuration, assertable to within `1e-6`.

**Server wiring**

- Keep `store` / `recall` / `consolidate` logic in library modules taking parsed arguments plus an injected store and embedder.
- Keep the `#[tool]` methods as thin adapters over those library functions.
- Build server info as `ServerInfo::new(ServerCapabilities::builder().enable_tools().build())`.
- Pin the advertised protocol version to `ProtocolVersion::V_2024_11_05`.
- Return tool text payloads with `ContentBlock::text`.
- Return tool failures as a `CallToolResult` with `isError` true rather than a JSON-RPC error.
- Return `Ok(())` from `main` after `service.waiting().await?` so stdin EOF yields exit status 0.

**Test fixture choices (derived, not sourced)**

- The 383-element dimension-mismatch vector is derived from `EMBED_DIM = 384` (any length other than 384 satisfies §5 best-practice #4).
- The "6 or more memories" fixture in criterion 7 is derived from the ratified default `k = 5`.
- The two temporary database files in criterion 17 are derived from §5 #2's per-scenario `tempfile` DB requirement.
- No `cap`-derived fixture exists in v1; it returns with the deferred eviction criterion.

### Bootstrap and calibration items

These are stated as requirements here and resolved to literal committed constants at
implementation. They are not open spec questions; they are values that cannot exist before
the artifact does. Same shape as §1.1's CRAP-threshold calibration caveat.

1. **Model revision string** — the explicit `sentence-transformers/all-MiniLM-L6-v2` revision
   pinned in `scripts/fetch-model`, read from the repository at first fetch and committed.
2. **Model SHA-256 digest** — computed over the `onnx/model.onnx` fetched at that revision
   and committed as the fixture constant the embedding tests assert against.
3. **Golden embedding vector** — the frozen output of `(model.onnx, tokenizer.json, mean
   pooling, L2 normalization)` for a fixed input, committed per §5 #3.
4. **Paraphrase/decoy fixture pair for criterion 3** — the source text, its paraphrase, and
   the deliberately dissimilar decoys are calibrated empirically at implementation and
   committed once they demonstrably rank as criterion 3 requires. §5 #6 is the reason this is
   a calibration item rather than a hardcoded similarity threshold: flaky tests produce flaky
   coverage, which destabilizes the CRAP gate.
5. **CRAP threshold** — inherited calibration item from §6.2 #1; unchanged by this spec.

### Deliberate deviations from source

**D8 — dedup timing.** §2.4 titles the rule "**Dedup/compress on insert**" and describes a
KNN top-1 lookup within `agent_id` at write time. This spec deliberately does **not** do that
in v1: dedup/merge runs only inside an explicit `consolidate` call. This is a deviation, not a
reading of the source — §2.4 says "on insert" and v1 does not.

- Rationale: it keeps `store` deterministic and cheap, avoids an implicit KNN on every write,
  and matches the v1 scope discipline ratified in iteration 2.
- Consequence recorded in the spec: expected behavior 13 / criterion 13 assert that a
  near-duplicate is still separately recallable *before* `consolidate` runs.
- v2 option: run dedup/merge at `store` time exactly as §2.4 phrases it, listed in
  "Out of scope (v2)" alongside summarize/evict.
- Stakes: low. §6.2 #8 marks all of §2.4 as INFERRED net-new design with no prior art and all
  thresholds as placeholders. The deviation is nonetheless written down rather than silently
  absorbed.

**D9 — AC12 "syntactically invalid" → structurally-invalid-but-parseable.** (Discovered at
implementation, Phase 9; recorded here for Atiqul's confirmation — it clarifies the *test* for a
ratified criterion, not the server's behavior.) Expected Behavior 12 / Acceptance Criterion 12 say a
"**syntactically invalid** JSON-RPC request … yields a JSON-RPC error object". The chosen MCP SDK
(`rmcp` 2.2.0) **silently ignores** byte-garbage that is not valid JSON (verified against
`rmcp/src/transport/async_rw.rs`; rust-sdk#938) — it sends no reply at all — and returns a JSON-RPC
`error` (`-32600 Invalid Request`) only for **well-formed JSON that is not a valid JSON-RPC message**.

- What is actually asserted: `server.feature` sends `{"jsonrpc":"2.0","id":1,"not_a_valid_member":"x"}`
  (parseable JSON, invalid JSON-RPC) and asserts the server replies with a JSON-RPC `error` object
  (not an `isError` result). This is the malformed-request protocol-error path §5 #5 sanctions
  ("reserve protocol-error assertions for malformed requests").
- Why not the literal wording: intercepting pre-parse byte-garbage would mean fighting the SDK's
  transport (it drops such input by design). The observable, testable protocol-error boundary is the
  structurally-invalid-but-parseable request.
- Stakes: low. The distinction (`isError` result vs JSON-RPC error) that AC12 exists to pin is fully
  exercised; only the specific "syntactically invalid" input class is narrowed to what the transport
  actually surfaces. **Flagged for Atiqul's sign-off** — if he wants the literal byte-garbage path,
  it requires a pre-parse guard around the transport (out of scope for v1).

### Changelog v2 → v3

- **D1 — `base_score` initial value.** Added `base_score` to `ConsolidationConfig` with default
  `1.0` and required `store` to write the configured default rather than a literal. Labelled
  inline as a chosen normalization, unsourced and config-exposed, with the arithmetic that
  makes the sourced 30-day half-life assertable. *Why: v2 used `base_score` in the effective
  formula without ever saying what `store` writes, so every decay assertion scaled by an
  unknown.*
- **D2 — `age_days` basis.** Required `age_days` to be measured from `created_at`, labelled
  inline as a ratified interpretation of §2.4 with the double-counting rationale. *Why: §2.4
  writes `exp(-lambda*age_days)` without naming the basis; `last_used_at` would double-count
  recency already carried by the `use_count` term.*
- **D3 — model repo and revision.** Added requirements naming the HF repo
  `sentence-transformers/all-MiniLM-L6-v2` and the files `onnx/model.onnx` + `tokenizer.json`
  (both traced to §2.3c/§6.1), plus an explicit pinned revision in `scripts/fetch-model` and a
  note that §6.2 #6 is why the revision is load-bearing. *Why: v2 fetched an unnamed artifact
  from an unnamed source, and exports of the same model differ in output shape.*
- **M1 — SHA-256 digest.** Kept the requirement to pin the digest and moved the literal value
  into the new "Bootstrap and calibration items" section. *Why: the digest can only be
  measured from the fetched artifact; it is a bootstrap value, not a spec defect.*
- **D4 — id allocation.** Required `memories.id` to be `INTEGER PRIMARY KEY`, the store to read
  `last_insert_rowid()` and insert the vector under that rowid, and `VectorStore::insert` to
  return the assigned `i64` and take no caller-supplied `id`. *Why: the §2.4 invariant
  `vec_items.rowid == memories.id` is sourced but v2 never said who assigns the id; this puts
  it in exactly one place and supersedes the scaffold's stub signature.*
- **D5 — database path.** Added a "Database location" requirement group: env var
  `GENESIS_MEMORY_DB` with a `genesis-memory.db` working-directory fallback, both labelled
  ratified-unsourced, plus an explicit prohibition on an OS-specific data-dir scheme. Added
  expected behavior 17 / criterion 17 to make injectability observable. *Why: §5 #2 requires a
  per-scenario `tempfile` DB, so the path MUST be injectable, and v2 left the channel open.*
- **D6 — recall payload.** Added a "Recall response payload" requirement group specifying a
  JSON array of `{id, text, distance}` inside the sourced `ContentBlock::text` envelope,
  ordered by ascending distance; labelled the shape as a ratified choice. Rewrote criteria 3,
  4, and 5 to assert against parsed fields, and added expected behavior 16 / criterion 16 for
  the payload shape itself. *Why: v2's criterion 3 ("the response contains T") assumed a
  verbatim-text response that §2.3a never specified.*
- **D7 — AC3 paraphrase retrieval.** Rewrote expected behavior 3 and criterion 3 to assert
  retrieval when the memory is the only one stored or is clearly nearest among deliberately
  dissimilar decoys, with an explicit "no absolute similarity threshold is asserted" clause,
  and registered the fixture pair as calibration item 4 citing §5 #6. *Why: ranking a
  paraphrase is a model-quality assumption with no sourced threshold; an unconditioned
  assertion is a flaky gate, and flaky coverage destabilizes CRAP.*
- **D8 — dedup timing.** Recorded consolidate-only as a documented deviation in a new
  "Deliberate deviations from source" section, extended "Out of scope (v2)" with the
  on-insert option, added the "run dedup/merge only inside consolidate" requirement, and
  strengthened expected behavior 13 / criterion 13 to assert both the pre-consolidate and
  post-consolidate states. *Why: §2.4 says "on insert"; v1 does not, and that gap must read as
  a deviation rather than as the source's position.*
- **Structural.** Added a "Provenance legend" so ratified-unsourced choices cannot be misread
  as sourced. Expected Behavior and Acceptance Criteria grew 15 → 17, still 1:1. Every
  Implementation Requirement remains atomic.
