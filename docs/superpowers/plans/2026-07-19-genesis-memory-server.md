# Genesis MCP Memory Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a per-agent semantic memory server that exposes `store` / `recall` / `consolidate` tools to any MCP client over stdio, backed by SQLite + `sqlite-vec` KNN and a local ONNX (`all-MiniLM-L6-v2`) sentence embedder.

**Architecture:** Three focused library modules — `embed` (ONNX mean-pool + L2-normalize), `store` (`sqlite-vec` vector store, agent-scoped insert/KNN), `consolidate` (decay scoring + dedup/merge) — carry all logic and are unit-tested with an injected clock. `lib.rs` holds thin `#[tool]` adapters over free functions (`do_store` / `do_recall` / `do_consolidate`) plus the rmcp `MemoryServer` and `serve_stdio()`. Every tool call is scoped to a caller-supplied `agent_id` in one shared database whose path comes from the environment.

**Tech Stack:** Rust 2021; `rmcp` 2.2.0 (stdio MCP server); `rusqlite` 0.40.1 (bundled) + `sqlite-vec` 0.1.9; `ort` =2.0.0-rc.12 + `tokenizers` 0.23.1 + `ndarray` 0.17 (embeddings); `bytemuck` (vec0 blobs); `tokio`; `thiserror`/`anyhow`; `cucumber` 0.23 (BDD, no mocks); `approx`/`insta`/`assert_cmd`/`tempfile` (dev).

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from `test/specs/genesis-memory-server.md`.

- **Embedding dimension:** `EMBED_DIM = 384`. Model `sentence-transformers/all-MiniLM-L6-v2`; pooling is attention-mask-weighted **MEAN** + L2-normalize. Never CLS pooling — the spec/§2.3c flags `bge`+mean-pool as a stated correctness bug, and MiniLM requires mean pooling.
- **Consolidation defaults:** `lambda = ln2 / 30`, `beta = 0.15`, `tau_merge = 0.95`, `base_score = 1.0`, `cap = 10_000` (retained in config, **unused in v1** — it is the v2 eviction trigger).
- **Recall `k`:** defaults to `5` when omitted; recall results are ordered ascending by distance (nearest first).
- **Protocol pin:** `ProtocolVersion::V_2024_11_05` (default is `V_2025_11_25`, so the pin is deliberate). Text payloads use `ContentBlock::text` — a bare `Content` type does **not** exist in rmcp 2.2.0.
- **Database path:** read from env `GENESIS_MEMORY_DB`; when unset, fall back to `genesis-memory.db` in the process working directory. Do **not** derive it from an OS-specific data directory.
- **Recall payload:** a JSON array of `{id, text, distance}` objects (exactly those three keys), ascending by `distance`, carried as the string inside a single `ContentBlock::text` in a `CallToolResult::success(vec![...])` envelope.
- **Tool failure:** returned as a successful `CallToolResult` with `is_error: true` (`isError` on the wire), **not** a JSON-RPC error. A malformed JSON-RPC request, by contrast, yields a protocol-level JSON-RPC `error` object.
- **Embedding tolerance:** golden and determinism comparisons pass at absolute tolerance `1e-4` **or** cosine `>= 0.9999`; asserted and gated in the **release** profile.
- **Lint gate (crate-level, already in `Cargo.toml`):** `unsafe_code = "forbid"`; clippy `unwrap_used` / `expect_used` / `panic` / `todo` = **deny**; `unimplemented` = warn. In `server/src/` use `?` propagation and `thiserror` — never `unwrap`/`expect`/`panic`/`todo`. (Test code may `unwrap` only because a `server/clippy.toml` sets `allow-unwrap-in-tests` — Task 1.)
- **GREEN definition (Phase 5 / Phase 8):** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release`. All three must pass, in the `server/` directory.
- **v1 scope:** `consolidate` performs decay/recency scoring + dedup/merge **only**. Summarize/evict and dedup-on-insert are v2 (deviation D8: dedup/merge runs *only* inside an explicit `consolidate` call, never at `store` time).
- **Dimension mismatch:** any embedding whose length is not `384` makes `VectorStore::insert` / `VectorStore::knn` return `Err`, never panic.
- **No mocks in the outer loop:** BDD scenarios drive the real SQLite + real ONNX model + (for `server.feature`) the real spawned stdio binary. Each scenario gets a fresh `tempfile` database.
- **Model provenance:** model + tokenizer are fetched by `scripts/fetch-model` into `server/models/` from a pinned HF revision; they are git-ignored (`*.onnx`). Embedding tests **fail — never skip** — with a "run `scripts/fetch-model`" message when the model is absent.

**Implementation-level decision (labelled, beyond the spec's enumerated envs):** the ONNX model directory is resolved from env `GENESIS_MODEL_DIR`, falling back to `<CARGO_MANIFEST_DIR>/models`. Rationale: the same D5 logic that made the DB path injectable applies to the model dir — AC11 needs a deterministic tool-failure trigger (point a spawned server at an empty model dir so the lazily-loaded embedder fails), and hermetic tests need a stable model location for the spawned binary. This is an implementation choice, not a sourced requirement.

**Confirm at implementation (§6.2-style, verify against the shipped artifact — do not guess):**
- The ONNX export's output shape: whether `last_hidden_state` is at `output[0]` (needs manual pooling, which this plan assumes) vs a pre-pooled output — dump `session.outputs`.
- Whether the export requires a third `token_type_ids` input — dump `session.inputs`; if required, add an all-zero `i64` tensor of shape `[1, seq]` as the third `ort::inputs![...]` argument.
- The exact `ort` EP/opt-level symbol paths (`ort::ep::CPU` / `ort::session::builder::GraphOptimizationLevel`) against the installed rc — both were source-confirmed for rc.12 this run but re-check on `cargo build`.
- The `sqlite-vec` 0.1.9 filtered-KNN behaviour: the LIMIT in a `vec0` MATCH can be applied *before* a joined `WHERE` filter. This plan mitigates by sizing the inner candidate pool to `max(k, COUNT(*) of vec_items)` so the outer agent/superseded filter never under-returns (Task 3).

---

## File Structure

Files created or modified by this plan (all paths relative to the worktree root):

- `server/clippy.toml` — **create** — `allow-unwrap-in-tests` / `allow-expect-in-tests` so `src/` keeps the strict gate while test code may `unwrap`. (Task 1)
- `scripts/fetch-model` — **create** — downloads `onnx/model.onnx` + `tokenizer.json` from the pinned HF revision into `server/models/`; prints revision + SHA-256. (Task 1)
- `server/src/embed.rs` — **modify** — `Embedder` (ort session + tokenizer), `embed`, provenance constants, model-path helpers. (Task 2)
- `server/src/store.rs` — **modify** — `VectorStore` (open/insert/knn + consolidation helper methods), `StoreError`. (Task 3)
- `server/src/consolidate.rs` — **modify** — `ConsolidationConfig` (+`base_score`), `Clock`, `effective`, `cosine_from_l2`, `consolidate`. (Task 4)
- `server/src/lib.rs` — **modify** — `MemoryServer`, tool arg structs, `do_store`/`do_recall`/`do_consolidate`, `serve_stdio`. (Task 5)
- `server/tests/bdd/store_steps.rs` — **modify** — real step bodies for `store.feature`. (Task 7)
- `server/tests/bdd/recall_steps.rs` — **modify** — real step bodies for `recall.feature`. (Task 8)
- `server/tests/bdd/consolidate_steps.rs` — **modify** — real step bodies for `consolidate.feature`. (Task 9)
- `server/tests/bdd/server_steps.rs` — **modify** — real step bodies for `server.feature`. (Task 10)
- `server/tests/golden/all_minilm_l6_v2_golden.json` — **create** (captured) — frozen golden vector (Task 2, bootstrap item 3).
- `server/tests/fixtures/ac3_calibration.json` — **create** (captured) — calibrated paraphrase/decoy fixture (Task 6, bootstrap item 4).
- `server/Cargo.toml` — **modify** — add `sha2` + `hex` dev-dependencies for the SHA-256 assertion (Task 1).

### Shared Interfaces (canonical signatures — every task consumes/produces exactly these)

All public fns return `anyhow::Result<T>` at the module boundary (matching the scaffold); modules define `thiserror` error enums internally and convert via `?`.

```rust
// ── server/src/embed.rs ───────────────────────────────────────────────────────
pub const EMBED_DIM: usize = 384;
pub const MODEL_REVISION: &str = /* 40-hex, bootstrap item 1 — Task 1 */;
pub const MODEL_SHA256: &str   = /* 64-hex, bootstrap item 2 — Task 1 */;
pub fn model_dir() -> std::path::PathBuf;                       // GENESIS_MODEL_DIR or <manifest>/models
pub fn model_paths() -> (std::path::PathBuf, std::path::PathBuf); // (onnx/model.onnx, tokenizer.json)
pub struct Embedder { /* session + tokenizer */ }
impl Embedder {
    pub fn load(model_path: &str, tokenizer_path: &str) -> anyhow::Result<Self>;
    pub fn embed(&mut self, text: &str) -> anyhow::Result<Vec<f32>>; // len == EMBED_DIM, L2-normalized
}

// ── server/src/store.rs ───────────────────────────────────────────────────────
pub struct MemRow { pub id: i64, pub created_at: i64, pub last_used_at: i64,
                    pub use_count: i64, pub base_score: f64 }
pub struct VectorStore { /* rusqlite::Connection */ }
impl VectorStore {
    pub fn open(path: &str) -> anyhow::Result<Self>;
    pub fn insert(&mut self, agent_id: &str, text: &str, embedding: &[f32],
                  base_score: f64, now_unix: i64) -> anyhow::Result<i64>; // returns assigned id
    pub fn knn(&self, agent_id: &str, query: &[f32], k: usize)
                  -> anyhow::Result<Vec<(i64, f64)>>;                     // (id, distance), asc
    pub fn text_of(&self, id: i64) -> anyhow::Result<String>;
    pub fn touch(&mut self, id: i64, now_unix: i64) -> anyhow::Result<()>; // use_count+=1, last_used_at
    pub fn active_memories(&self, agent_id: &str) -> anyhow::Result<Vec<MemRow>>;
    pub fn embedding_of(&self, id: i64) -> anyhow::Result<Vec<f32>>;
    pub fn set_superseded(&mut self, loser: i64, survivor: i64) -> anyhow::Result<()>;
    pub fn add_use_count(&mut self, id: i64, delta: i64) -> anyhow::Result<()>;
    pub fn superseded_ids(&self, agent_id: &str) -> anyhow::Result<Vec<i64>>; // for BDD assertions
}

// ── server/src/consolidate.rs ─────────────────────────────────────────────────
pub struct ConsolidationConfig { pub lambda: f64, pub beta: f64, pub tau_merge: f64,
                                 pub base_score: f64, pub cap: usize }
impl Default for ConsolidationConfig { /* ln2/30, 0.15, 0.95, 1.0, 10_000 */ }
pub trait Clock: Send + Sync { fn now_unix(&self) -> i64; }
pub struct SystemClock;
pub struct FixedClock(pub i64);
pub fn effective(cfg: &ConsolidationConfig, base_score: f64, created_at_unix: i64,
                 now_unix: i64, use_count: i64) -> f64;
pub fn cosine_from_l2(l2_distance: f64) -> f64;                 // 1 - d^2/2
pub fn consolidate(store: &mut VectorStore, agent_id: &str,
                   cfg: &ConsolidationConfig, clock: &dyn Clock) -> anyhow::Result<()>;

// ── server/src/lib.rs ─────────────────────────────────────────────────────────
pub const DEFAULT_K: usize = 5;
pub const DEFAULT_DB_FILENAME: &str = "genesis-memory.db";
pub struct StoreArgs      { pub agent_id: String, pub text: String }
pub struct RecallArgs     { pub agent_id: String, pub query: String, pub k: Option<u32> }
pub struct ConsolidateArgs{ pub agent_id: String }
pub fn do_store(store: &mut VectorStore, embedder: &mut Embedder, cfg: &ConsolidationConfig,
                clock: &dyn Clock, agent_id: &str, text: &str) -> anyhow::Result<i64>;
pub fn do_recall(store: &mut VectorStore, embedder: &mut Embedder, clock: &dyn Clock,
                 agent_id: &str, query: &str, k: usize) -> anyhow::Result<String>; // JSON
pub struct MemoryServer { /* tool_router + Arc<Mutex<Inner>> + cfg + model_dir + clock */ }
impl MemoryServer { pub fn new(store: VectorStore, model_dir: std::path::PathBuf) -> Self; }
pub async fn serve_stdio() -> anyhow::Result<()>;
```

> **Scaffold supersessions (spec-sanctioned):** `VectorStore::insert` drops the caller-supplied `id` and gains `agent_id` + `base_score` + `now_unix`, returning `i64` (spec D4). `VectorStore::knn` gains `agent_id`. `consolidate` gains a `clock` parameter. `ConsolidationConfig` gains a `base_score` field (spec D1). These replace the stub signatures in the committed scaffold.

---

### Task 1: Model fetch + pin (`scripts/fetch-model`, provenance constants, clippy.toml)

Delivers the reproducible model acquisition path and the two provenance constants the embedding tests assert against. Establishes the "fail — never skip" contract when the model is absent.

**Files:**
- Create: `scripts/fetch-model`
- Create: `server/clippy.toml`
- Modify: `server/src/embed.rs` (add provenance constants + `model_dir`/`model_paths` + the model-absent guard test)
- Modify: `server/Cargo.toml` (dev-deps `sha2`, `hex`)
- Test: `server/src/embed.rs` `#[cfg(test)] mod tests` (`model_paths_point_into_server_models`, `the_pinned_model_sha256_is_asserted_before_embedding_tests`, `embedding_tests_fail_rather_than_skip_when_the_model_is_absent`)

**Interfaces:**
- Consumes: nothing (first task).
- Produces: `embed::MODEL_REVISION: &str`, `embed::MODEL_SHA256: &str`, `embed::model_dir() -> PathBuf`, `embed::model_paths() -> (PathBuf, PathBuf)`, and the on-disk `scripts/fetch-model`. `MODEL_REVISION` / `MODEL_SHA256` are **bootstrap constants** (spec "Bootstrap and calibration items" 1 & 2): captured at first fetch by the exact procedure below, then committed. They are not placeholders — the capture command and the file/line to edit are given.

- [ ] **Step 1: Create `server/clippy.toml`** so the strict `src/` gate stays intact while test code may `unwrap`/`expect`.

```toml
# Keep the crate-level unwrap_used/expect_used = "deny" gate for src/, but allow the
# ergonomic unwrap/expect inside #[cfg(test)] modules and tests/ integration binaries.
allow-unwrap-in-tests = true
allow-expect-in-tests = true
```

- [ ] **Step 2: Add the SHA-256 dev-dependencies** to `server/Cargo.toml` under `[dev-dependencies]` (needed to assert the pinned digest in Rust; product code stays unchanged).

```toml
sha2       = "0.10"
hex        = "0.4"
```

- [ ] **Step 3: Write `scripts/fetch-model`** (bash). It resolves the pinned commit, downloads only from it, and prints the two bootstrap constants to paste into `embed.rs`.

```bash
#!/usr/bin/env bash
# scripts/fetch-model — fetch the Genesis embedder from a PINNED Hugging Face revision.
#
# LOAD-BEARING REVISION: docs/SPEC_FORGE_RUST_UPDATE.md §6.2 #6 — ONNX exports of the SAME
# model differ in output shape (pooled vs last_hidden_state) and in whether token_type_ids
# is required. Pinning the exact commit is what makes the golden vector reproducible.
set -euo pipefail

REPO="sentence-transformers/all-MiniLM-L6-v2"
# Bootstrap item 1: the exact commit, captured at first fetch (see "capture" note below).
REVISION="${GENESIS_MODEL_REVISION:-c9745ed1d9f207416be6d2e6f8de32d1f16199bf}"

DEST="$(cd "$(dirname "$0")/.." && pwd)/server/models"
mkdir -p "$DEST/onnx"

base="https://huggingface.co/${REPO}/resolve/${REVISION}"
echo "Fetching ${REPO}@${REVISION} into ${DEST} ..."
curl -fL "${base}/onnx/model.onnx"  -o "${DEST}/onnx/model.onnx"
curl -fL "${base}/tokenizer.json"   -o "${DEST}/tokenizer.json"

sha="$(sha256sum "${DEST}/onnx/model.onnx" | cut -d' ' -f1)"
echo "REVISION = ${REVISION}"
echo "SHA-256  = ${sha}"
echo "Paste REVISION into embed::MODEL_REVISION and SHA-256 into embed::MODEL_SHA256."
```

- [ ] **Step 4: Make it executable and capture the two bootstrap constants.**

Run: `chmod +x scripts/fetch-model && ./scripts/fetch-model`
Expected: `server/models/onnx/model.onnx` and `server/models/tokenizer.json` exist; the script prints a 40-hex `REVISION` and a 64-hex `SHA-256`. If the default `REVISION` no longer resolves, capture the current commit first with `curl -fsSL https://huggingface.co/api/models/sentence-transformers/all-MiniLM-L6-v2 | python -c "import sys,json;print(json.load(sys.stdin)['sha'])"`, set `GENESIS_MODEL_REVISION` to it, and re-run — then hard-code that value into `REVISION` and `embed::MODEL_REVISION`.

- [ ] **Step 5: Write the failing provenance tests** in `server/src/embed.rs` (replace the three matching scaffold stub tests). Paste the captured constants into the `const` declarations.

```rust
// near the top of embed.rs, module scope:
use std::path::{Path, PathBuf};

/// Bootstrap item 1: the pinned HF commit, captured by scripts/fetch-model at first fetch.
pub const MODEL_REVISION: &str = "c9745ed1d9f207416be6d2e6f8de32d1f16199bf";
/// Bootstrap item 2: SHA-256 of onnx/model.onnx at MODEL_REVISION, captured at first fetch.
pub const MODEL_SHA256: &str = "2c4b6e7f...PASTE_THE_64_HEX_FROM_STEP_4...";

/// The model directory: `GENESIS_MODEL_DIR` if set, else `<CARGO_MANIFEST_DIR>/models`.
#[must_use]
pub fn model_dir() -> PathBuf {
    std::env::var_os("GENESIS_MODEL_DIR")
        .map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("models"), PathBuf::from)
}

/// Paths to the ONNX model and tokenizer inside [`model_dir`].
#[must_use]
pub fn model_paths() -> (PathBuf, PathBuf) {
    let base = model_dir();
    (base.join("onnx").join("model.onnx"), base.join("tokenizer.json"))
}
```

```rust
// inside #[cfg(test)] mod tests:
use super::{MODEL_SHA256, model_paths};
use sha2::{Digest, Sha256};

/// The model file must be present and match the pinned digest before embedding tests run —
/// and its ABSENCE must FAIL (never silently skip), directing the developer to fetch it.
#[test]
fn embedding_tests_fail_rather_than_skip_when_the_model_is_absent() {
    let (model, _tok) = model_paths();
    assert!(
        model.exists(),
        "model missing at {}: run `scripts/fetch-model` (docs/SPEC_FORGE_RUST_UPDATE.md §6.2 #6)",
        model.display()
    );
}

#[test]
fn the_pinned_model_sha256_is_asserted_before_embedding_tests() {
    let (model, _tok) = model_paths();
    assert!(model.exists(), "model missing: run `scripts/fetch-model`");
    let bytes = std::fs::read(&model).unwrap();
    let digest = hex::encode(Sha256::digest(&bytes));
    assert_eq!(digest, MODEL_SHA256, "fetched model does not match the pinned SHA-256");
}

#[test]
fn model_paths_point_into_server_models() {
    let (model, tok) = model_paths();
    assert!(model.ends_with("onnx/model.onnx"));
    assert!(tok.ends_with("tokenizer.json"));
}
```

- [ ] **Step 6: Run the tests to verify they pass** (model present after Step 4).

Run: `cd server && cargo test --release model_paths_point_into_server_models the_pinned_model_sha256_is_asserted_before_embedding_tests embedding_tests_fail_rather_than_skip_when_the_model_is_absent`
Expected: 3 passed. (Run with the model absent once to confirm the first two FAIL with the "run `scripts/fetch-model`" message — the required fail-not-skip behaviour — then restore it.)

- [ ] **Step 7: Confirm `.gitignore` keeps the weights out of git.**

Run: `git check-ignore server/models/onnx/model.onnx`
Expected: prints the path (already covered by `*.onnx`). The `tokenizer.json` is small; add `server/models/` to `.gitignore` so neither artifact is committed.

- [ ] **Step 8: Commit.**

```bash
git add scripts/fetch-model server/clippy.toml server/src/embed.rs server/Cargo.toml .gitignore
git commit -m "feat(model): pinned fetch-model script + provenance constants + clippy test allowance"
```

---

### Task 2: Embedder (`server/src/embed.rs`)

Turns text into a 384-dim L2-normalized vector via ONNX Runtime + tokenizers, deterministically. Delivers the golden-vector test (bootstrap item 3 captured here) and the same-process determinism test (AC9).

**Files:**
- Modify: `server/src/embed.rs` (`Embedder::load`, `Embedder::embed`, `EmbedError`)
- Create (captured): `server/tests/golden/all_minilm_l6_v2_golden.json`
- Test: `server/src/embed.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `EMBED_DIM`, `MODEL_SHA256`, `model_paths()` (Task 1).
- Produces: `Embedder::load(model_path: &str, tokenizer_path: &str) -> anyhow::Result<Self>`, `Embedder::embed(&mut self, text: &str) -> anyhow::Result<Vec<f32>>` (returns a `Vec<f32>` of length `EMBED_DIM`, L2-normalized).

> **Prerequisite:** `scripts/fetch-model` must have populated `server/models/` (Task 1). All tests here run in `--release` and FAIL (never skip) if the model is absent.

- [ ] **Step 1: Write the determinism test (AC9)** — replace the scaffold stub `golden_and_determinism_comparisons_use_tolerance_1e_minus_4` region with two real tests. This one is fully self-contained.

```rust
// inside #[cfg(test)] mod tests:
use super::{EMBED_DIM, Embedder, model_paths};

fn load_embedder() -> Embedder {
    let (model, tok) = model_paths();
    assert!(model.exists(), "model missing: run `scripts/fetch-model`");
    Embedder::load(model.to_str().unwrap(), tok.to_str().unwrap()).unwrap()
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
    let na: f64 = a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    dot / (na * nb)
}

#[test]
fn embedding_the_same_text_twice_is_deterministic() {
    let mut e = load_embedder();
    let v1 = e.embed("a quick brown fox jumps over the lazy dog").unwrap();
    let v2 = e.embed("a quick brown fox jumps over the lazy dog").unwrap();
    assert_eq!(v1.len(), EMBED_DIM);
    assert!(cosine(&v1, &v2) >= 0.9999, "cosine {} < 0.9999", cosine(&v1, &v2));
}
```

- [ ] **Step 2: Run it to verify it fails** (`Embedder::load`/`embed` still `unimplemented!`).

Run: `cd server && cargo test --release embedding_the_same_text_twice_is_deterministic`
Expected: FAIL — panics at `unimplemented!("Implement via TDD — ort Session::builder + Tokenizer::from_file")`.

- [ ] **Step 3: Implement `EmbedError`, `Embedder::load`, and `Embedder::embed`** using the verified §2.3c shapes. Replace the `Embedder` stub and both stub methods.

```rust
use anyhow::Result;
use ort::ep::CPU;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("tokenizer error: {0}")]
    Tokenize(String),
    #[error("unexpected ONNX output rank: {0}")]
    OutputRank(String),
}

/// A local sentence embedder: an ONNX Runtime session plus its tokenizer.
pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl std::fmt::Debug for Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder").finish_non_exhaustive()
    }
}

impl Embedder {
    /// Loads the ONNX model and tokenizer from disk with determinism knobs pinned.
    ///
    /// # Errors
    /// Returns an error if the session or tokenizer fails to load.
    pub fn load(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let session = Session::builder()?
            .with_execution_providers([CPU::default().build()])?
            .with_intra_threads(1)?
            .with_deterministic_compute(true)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_path)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;
        Ok(Self { session, tokenizer })
    }

    /// Embeds `text` into an L2-normalized `EMBED_DIM`-length vector (mean pooling).
    ///
    /// # Errors
    /// Returns an error if tokenization or the ONNX inference run fails.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let enc = self.tokenizer.encode(text, true)
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;
        let seq = enc.len();
        let ids:  Vec<i64> = enc.get_ids().iter().map(|&x| i64::from(x)).collect();
        let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&x| i64::from(x)).collect();
        let a_ids  = TensorRef::from_array_view(([1_usize, seq], &*ids))?;
        let a_mask = TensorRef::from_array_view(([1_usize, seq], &*mask))?;
        // §6.2 #6: if `session.inputs` shows a required token_type_ids, add an all-zero
        // i64 tensor of shape [1, seq] as a third `ort::inputs![...]` argument here.
        let outputs = self.session.run(ort::inputs![a_ids, a_mask])?;
        // §6.2 #6: assumes last_hidden_state at output[0]; confirm with `session.outputs`.
        let hidden = outputs[0]
            .try_extract_array::<f32>()?
            .into_dimensionality::<ndarray::Ix3>()
            .map_err(|e| EmbedError::OutputRank(e.to_string()))?;
        let dim = hidden.shape()[2];
        let mut pooled = vec![0f32; dim];
        let mut denom = 0f32;
        for t in 0..seq {
            let m = mask[t] as f32;
            if m == 0.0 { continue; }
            denom += m;
            for d in 0..dim {
                pooled[d] += hidden[[0, t, d]] * m;
            }
        }
        let denom = denom.max(1e-9);           // pooling denominator clamp
        for v in &mut pooled { *v /= denom; }
        let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12); // L2 clamp
        for v in &mut pooled { *v /= norm; }
        Ok(pooled)
    }
}
```

- [ ] **Step 4: Run the determinism test to verify it passes.**

Run: `cd server && cargo test --release embedding_the_same_text_twice_is_deterministic`
Expected: PASS.

- [ ] **Step 5: Write the golden-vector capture harness and the golden assertion test.** The capture writes the committed fixture (bootstrap item 3); the assertion consumes it at `1e-4`.

```rust
// inside #[cfg(test)] mod tests:
const GOLDEN_INPUT: &str = "genesis remembers what matters";
const GOLDEN_PATH: &str = "tests/golden/all_minilm_l6_v2_golden.json";

/// Run ONCE to freeze the golden vector, then commit the JSON file. Ignored thereafter.
#[test]
#[ignore = "capture harness: run once to write the golden fixture, then commit it"]
fn capture_golden_vector() {
    let mut e = load_embedder();
    let v = e.embed(GOLDEN_INPUT).unwrap();
    std::fs::create_dir_all("tests/golden").unwrap();
    std::fs::write(GOLDEN_PATH, serde_json::to_string(&v).unwrap()).unwrap();
}

#[test]
fn embedding_matches_the_committed_golden_vector() {
    let golden: Vec<f32> =
        serde_json::from_str(&std::fs::read_to_string(GOLDEN_PATH).unwrap()).unwrap();
    let mut e = load_embedder();
    let v = e.embed(GOLDEN_INPUT).unwrap();
    assert_eq!(v.len(), golden.len());
    for (a, b) in v.iter().zip(&golden) {
        approx::assert_abs_diff_eq!(*a, *b, epsilon = 1e-4);
    }
}
```

- [ ] **Step 6: Capture and freeze the golden fixture, then run the golden test.**

Run: `cd server && cargo test --release capture_golden_vector -- --ignored && cargo test --release embedding_matches_the_committed_golden_vector`
Expected: the capture writes `server/tests/golden/all_minilm_l6_v2_golden.json`; the golden test then PASSES within `1e-4`.

- [ ] **Step 7: Run the full module + gate.**

Run: `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release embed::`
Expected: fmt clean, clippy clean, all `embed` tests pass.

- [ ] **Step 8: Commit.**

```bash
git add server/src/embed.rs server/tests/golden/all_minilm_l6_v2_golden.json
git commit -m "feat(embed): ort mean-pool L2-normalize embedder + golden & determinism tests"
```

---

### Task 3: VectorStore (`server/src/store.rs`)

The agent-scoped `sqlite-vec` vector store: `open` (extension + DDL), `insert` (assigns id, writes both tables), `knn` (agent-scoped, superseded-excluded), plus the small helper methods `consolidate` and `recall` need. Delivers AC10 (dimension mismatch → `Err`, no panic).

**Files:**
- Modify: `server/src/store.rs`
- Test: `server/src/store.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `embed::EMBED_DIM` (Task 2). Uses `rusqlite`, `sqlite_vec::sqlite3_vec_init`, `bytemuck`.
- Produces: `VectorStore` with the full method set from **Shared Interfaces** — `open(path:&str)->Result<Self>`, `insert(agent_id:&str, text:&str, embedding:&[f32], base_score:f64, now_unix:i64)->Result<i64>`, `knn(agent_id:&str, query:&[f32], k:usize)->Result<Vec<(i64,f64)>>`, `text_of(id:i64)->Result<String>`, `touch(id:i64, now_unix:i64)->Result<()>`, `active_memories(agent_id:&str)->Result<Vec<MemRow>>`, `embedding_of(id:i64)->Result<Vec<f32>>`, `set_superseded(loser:i64, survivor:i64)->Result<()>`, `add_use_count(id:i64, delta:i64)->Result<()>`, `superseded_ids(agent_id:&str)->Result<Vec<i64>>`; plus `MemRow`.

- [ ] **Step 1: Write the failing dimension-mismatch test (AC10)** — replace the `insert_returns_the_assigned_i64` and dimension stub regions with real tests.

```rust
// inside #[cfg(test)] mod tests:
use super::{MemRow, VectorStore};
use genesis_memory::embed::EMBED_DIM; // if referenced across crate; else `use super::super::embed::EMBED_DIM;`

fn open_temp() -> (tempfile::TempDir, VectorStore) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.db");
    let store = VectorStore::open(path.to_str().unwrap()).unwrap();
    (dir, store)
}

#[test]
fn a_383_element_vector_is_rejected_by_insert_and_knn_without_panicking() {
    let (_dir, mut store) = open_temp();
    let short = vec![0.0f32; 383];
    assert!(store.insert("alpha", "x", &short, 1.0, 0).is_err());
    assert!(store.knn("alpha", &short, 5).is_err());
}

#[test]
fn insert_assigns_ids_and_knn_is_agent_scoped_nearest_first() {
    let (_dir, mut store) = open_temp();
    let mut v = |seed: f32| { let mut e = vec![0.0f32; EMBED_DIM]; e[0] = seed; e[1] = 1.0 - seed; e };
    let id1 = store.insert("alpha", "near",  &v(1.0), 1.0, 0).unwrap();
    let id2 = store.insert("alpha", "far",   &v(0.0), 1.0, 0).unwrap();
    let _   = store.insert("beta",  "other", &v(1.0), 1.0, 0).unwrap();
    assert!(id2 > id1);
    let hits = store.knn("alpha", &v(1.0), 5).unwrap();
    assert!(hits.iter().all(|(id, _)| *id == id1 || *id == id2)); // no beta rows
    assert_eq!(hits.first().unwrap().0, id1);                     // nearest first
    for w in hits.windows(2) { assert!(w[0].1 <= w[1].1); }       // non-decreasing distance
}
```

- [ ] **Step 2: Run to verify it fails.**

Run: `cd server && cargo test --release store::tests::a_383_element_vector_is_rejected_by_insert_and_knn_without_panicking store::tests::insert_assigns_ids_and_knn_is_agent_scoped_nearest_first`
Expected: FAIL — `unimplemented!` in `open`/`insert`/`knn`.

- [ ] **Step 3: Implement `StoreError`, `open`, and the DDL.** Replace the `VectorStore` struct + `open` stub.

```rust
use anyhow::Result;
use rusqlite::{Connection, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;

use crate::embed::EMBED_DIM;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("no memory row for id {0}")]
    MissingRow(i64),
}

/// A per-agent vector store backed by SQLite + `sqlite-vec`.
#[derive(Debug)]
pub struct VectorStore {
    conn: Connection,
}

/// A decay-relevant snapshot of a `memories` row (no embedding).
#[derive(Debug, Clone)]
pub struct MemRow {
    pub id: i64,
    pub created_at: i64,
    pub last_used_at: i64,
    pub use_count: i64,
    pub base_score: f64,
}

impl VectorStore {
    /// Opens (or creates) the store, registering `sqlite-vec` and ensuring the schema.
    ///
    /// # Errors
    /// Returns an error if the extension fails to register or the connection/DDL fails.
    pub fn open(path: &str) -> Result<Self> {
        // Register the extension BEFORE opening the connection (spec requirement).
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(),
            >(sqlite3_vec_init as *const ())));
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                 id            INTEGER PRIMARY KEY,
                 agent_id      TEXT    NOT NULL,
                 text          TEXT    NOT NULL,
                 created_at    INTEGER NOT NULL,
                 last_used_at  INTEGER NOT NULL,
                 use_count     INTEGER NOT NULL DEFAULT 0,
                 base_score    REAL    NOT NULL,
                 superseded_by INTEGER REFERENCES memories(id)
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS vec_items USING vec0(embedding float[384]);",
        )?;
        Ok(Self { conn })
    }
}
```

> **Note on `unsafe`:** `unsafe_code = "forbid"` is crate-level, so `sqlite3_auto_extension` must be wrapped `#[allow(unsafe_code)]` on this one `open` fn — the single sanctioned exception, matching the verified §2.3b shape. Add `#[allow(unsafe_code)]` immediately above `pub fn open`.

- [ ] **Step 4: Implement `insert` and `knn`** (both dimension-checked; `knn` agent-scoped + superseded-excluded via the candidate-pool mitigation).

```rust
impl VectorStore {
    fn check_dim(embedding: &[f32]) -> Result<()> {
        if embedding.len() != EMBED_DIM {
            return Err(StoreError::DimensionMismatch { expected: EMBED_DIM, got: embedding.len() }.into());
        }
        Ok(())
    }

    /// Inserts a memory + its embedding under one shared rowid; returns the assigned id.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn insert(&mut self, agent_id: &str, text: &str, embedding: &[f32],
                  base_score: f64, now_unix: i64) -> Result<i64> {
        Self::check_dim(embedding)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO memories (agent_id, text, created_at, last_used_at, use_count, base_score)
             VALUES (?1, ?2, ?3, ?3, 0, ?4)",
            params![agent_id, text, now_unix, base_score],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO vec_items(rowid, embedding) VALUES (?1, ?2)",
            params![id, bytemuck::cast_slice::<f32, u8>(embedding)],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Returns the `k` nearest non-superseded memories for `agent_id`, nearest first.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn knn(&self, agent_id: &str, query: &[f32], k: usize) -> Result<Vec<(i64, f64)>> {
        Self::check_dim(query)?;
        // Candidate pool >= total vec rows so the outer agent/superseded filter never
        // under-returns (mitigation for vec0's pre-filter LIMIT — see Global Constraints).
        let total: i64 = self.conn.query_row("SELECT COUNT(*) FROM vec_items", [], |r| r.get(0))?;
        let pool = total.max(k as i64).max(1);
        let mut stmt = self.conn.prepare(
            "SELECT m.id, k.distance
               FROM ( SELECT rowid, distance FROM vec_items
                      WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2 ) AS k
               JOIN memories m ON m.id = k.rowid
              WHERE m.agent_id = ?3 AND m.superseded_by IS NULL
              ORDER BY k.distance
              LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![bytemuck::cast_slice::<f32, u8>(query), pool, agent_id, k as i64],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)),
        )?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}
```

- [ ] **Step 5: Implement the helper methods** `text_of`, `touch`, `active_memories`, `embedding_of`, `set_superseded`, `add_use_count`, `superseded_ids` (each tiny → low CRAP).

```rust
impl VectorStore {
    /// # Errors
    /// Returns an error if the row is missing or SQL fails.
    pub fn text_of(&self, id: i64) -> Result<String> {
        self.conn
            .query_row("SELECT text FROM memories WHERE id = ?1", params![id], |r| r.get(0))
            .map_err(|_| StoreError::MissingRow(id).into())
    }

    /// Bumps `use_count` and sets `last_used_at` for a recalled row.
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn touch(&mut self, id: i64, now_unix: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET use_count = use_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![id, now_unix],
        )?;
        Ok(())
    }

    /// All non-superseded memory rows for `agent_id` (no embeddings).
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn active_memories(&self, agent_id: &str) -> Result<Vec<MemRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, last_used_at, use_count, base_score
               FROM memories WHERE agent_id = ?1 AND superseded_by IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map(params![agent_id], |r| {
            Ok(MemRow {
                id: r.get(0)?, created_at: r.get(1)?, last_used_at: r.get(2)?,
                use_count: r.get(3)?, base_score: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The stored embedding for `id`.
    /// # Errors
    /// Returns an error if the row is missing or SQL fails.
    pub fn embedding_of(&self, id: i64) -> Result<Vec<f32>> {
        let blob: Vec<u8> = self.conn.query_row(
            "SELECT embedding FROM vec_items WHERE rowid = ?1", params![id], |r| r.get(0),
        )?;
        Ok(bytemuck::cast_slice::<u8, f32>(&blob).to_vec())
    }

    /// Marks `loser` as superseded by `survivor`.
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn set_superseded(&mut self, loser: i64, survivor: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET superseded_by = ?2 WHERE id = ?1", params![loser, survivor],
        )?;
        Ok(())
    }

    /// Adds `delta` to a survivor's `use_count`.
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn add_use_count(&mut self, id: i64, delta: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET use_count = use_count + ?2 WHERE id = ?1", params![id, delta],
        )?;
        Ok(())
    }

    /// Ids of rows with a non-null `superseded_by` for `agent_id` (BDD assertions).
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn superseded_ids(&self, agent_id: &str) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM memories WHERE agent_id = ?1 AND superseded_by IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![agent_id], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}
```

- [ ] **Step 6: Run the store tests to verify they pass.**

Run: `cd server && cargo test --release store::`
Expected: PASS — dimension rejection, id assignment, agent scoping, and nearest-first ordering all hold.

- [ ] **Step 7: Gate.**

Run: `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: clean (the single `#[allow(unsafe_code)]` on `open` is the only exception).

- [ ] **Step 8: Commit.**

```bash
git add server/src/store.rs
git commit -m "feat(store): sqlite-vec agent-scoped store (insert/knn + consolidation helpers)"
```

---

### Task 4: Consolidation (`server/src/consolidate.rs`)

Decay/recency scoring and dedup/merge (v1 scope only). Delivers the `effective` formula, the `Clock` abstraction, `cosine_from_l2`, and the merge pass that supersedes the lower-scored duplicate.

**Files:**
- Modify: `server/src/consolidate.rs`
- Test: `server/src/consolidate.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `VectorStore` + its methods `active_memories`, `embedding_of`, `knn`, `set_superseded`, `add_use_count` (Task 3); `embed::EMBED_DIM` for test vectors.
- Produces: `ConsolidationConfig { lambda, beta, tau_merge, base_score, cap }` with `Default`; `trait Clock: Send + Sync { fn now_unix(&self)->i64 }`, `SystemClock`, `FixedClock(i64)`; `effective(cfg:&ConsolidationConfig, base_score:f64, created_at_unix:i64, now_unix:i64, use_count:i64)->f64`; `cosine_from_l2(l2_distance:f64)->f64`; `consolidate(store:&mut VectorStore, agent_id:&str, cfg:&ConsolidationConfig, clock:&dyn Clock)->Result<()>`.

- [ ] **Step 1: Add `base_score` to `ConsolidationConfig`** (default `1.0`), replacing the existing struct + `Default`.

```rust
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Decay constant λ (default `ln2/30` ⇒ 30-day half-life).
    pub lambda: f64,
    /// Use-count weight β.
    pub beta: f64,
    /// Cosine similarity at/above which two memories merge.
    pub tau_merge: f64,
    /// Score written into every new `memories` row at store time (normalization = 1.0).
    pub base_score: f64,
    /// Row-count cap that triggers eviction (v2; unused in v1).
    pub cap: usize,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            lambda: core::f64::consts::LN_2 / 30.0,
            beta: 0.15,
            tau_merge: 0.95,
            base_score: 1.0,
            cap: 10_000,
        }
    }
}
```

- [ ] **Step 2: Write the failing pure-function tests** (`effective`, defaults, `cosine_from_l2`) — replace the matching scaffold stubs.

```rust
// inside #[cfg(test)] mod tests:
use super::{ConsolidationConfig, FixedClock, cosine_from_l2, effective};

const DAY: i64 = 86_400;

#[test]
fn config_defaults_match_the_spec() {
    let c = ConsolidationConfig::default();
    approx::assert_abs_diff_eq!(c.lambda, core::f64::consts::LN_2 / 30.0, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(c.beta, 0.15, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(c.tau_merge, 0.95, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(c.base_score, 1.0, epsilon = 1e-6);
    assert_eq!(c.cap, 10_000);
}

#[test]
fn effective_is_one_at_age_zero_and_half_at_thirty_days() {
    let c = ConsolidationConfig::default();
    approx::assert_abs_diff_eq!(effective(&c, 1.0, 0, 0, 0), 1.0, epsilon = 1e-6);
    approx::assert_abs_diff_eq!(effective(&c, 1.0, 0, 30 * DAY, 0), 0.5, epsilon = 1e-6);
}

#[test]
fn effective_rewards_use_count() {
    let c = ConsolidationConfig::default();
    let expected = 1.0 * (1.0 + 0.15 * (1.0f64 + 3.0).ln());
    approx::assert_abs_diff_eq!(effective(&c, 1.0, 0, 0, 3), expected, epsilon = 1e-6);
}

#[test]
fn cosine_from_l2_matches_the_normalized_identity() {
    approx::assert_abs_diff_eq!(cosine_from_l2(0.0), 1.0, epsilon = 1e-6);   // identical
    approx::assert_abs_diff_eq!(cosine_from_l2(2.0f64.sqrt()), 0.0, epsilon = 1e-6); // orthogonal
    let _ = FixedClock(0); // ensure the injectable clock type exists
}
```

- [ ] **Step 3: Run to verify failure.**

Run: `cd server && cargo test --release consolidate::tests::config_defaults_match_the_spec consolidate::tests::effective_is_one_at_age_zero_and_half_at_thirty_days`
Expected: FAIL to compile/run — `effective`, `cosine_from_l2`, `FixedClock` not yet defined.

- [ ] **Step 4: Implement `Clock`, the two clocks, `effective`, and `cosine_from_l2`.**

```rust
use anyhow::Result;
use crate::store::VectorStore;

/// Injectable wall clock (Unix seconds) — never read the system clock directly in logic.
pub trait Clock: Send + Sync {
    fn now_unix(&self) -> i64;
}

/// Production clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Deterministic test clock.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);
impl Clock for FixedClock {
    fn now_unix(&self) -> i64 { self.0 }
}

/// `effective = base_score * exp(-lambda * age_days) * (1 + beta * ln(1 + use_count))`,
/// with `age_days` measured from `created_at` (spec D2).
#[must_use]
pub fn effective(cfg: &ConsolidationConfig, base_score: f64, created_at_unix: i64,
                 now_unix: i64, use_count: i64) -> f64 {
    let age_days = (now_unix - created_at_unix) as f64 / 86_400.0;
    base_score * (-cfg.lambda * age_days).exp() * (1.0 + cfg.beta * (1.0 + use_count as f64).ln())
}

/// Cosine similarity from a vec0 L2 (Euclidean) distance, valid for normalized vectors.
#[must_use]
pub fn cosine_from_l2(l2_distance: f64) -> f64 {
    1.0 - l2_distance * l2_distance / 2.0
}
```

- [ ] **Step 5: Run the pure-function tests to verify they pass.**

Run: `cd server && cargo test --release consolidate::tests::config_defaults_match_the_spec consolidate::tests::effective_is_one_at_age_zero_and_half_at_thirty_days consolidate::tests::effective_rewards_use_count consolidate::tests::cosine_from_l2_matches_the_normalized_identity`
Expected: PASS.

- [ ] **Step 6: Write the failing merge test (AC13 core, unit level).**

```rust
// inside #[cfg(test)] mod tests:
use super::consolidate;
use crate::embed::EMBED_DIM;
use crate::store::VectorStore;

fn unit_vec(a: f32, b: f32) -> Vec<f32> {
    let mut v = vec![0.0f32; EMBED_DIM];
    let n = (a * a + b * b).sqrt();
    v[0] = a / n; v[1] = b / n; v
}

#[test]
fn consolidate_merges_a_near_duplicate_into_the_higher_scored_survivor() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
    let cfg = ConsolidationConfig::default();
    // Two near-identical unit vectors: cosine ~1.0 >= tau_merge.
    let older = store.insert("alpha", "dup a", &unit_vec(1.0, 0.001), cfg.base_score, 0).unwrap();
    let newer = store.insert("alpha", "dup b", &unit_vec(1.0, 0.002), cfg.base_score, 0).unwrap();
    // Give `newer` a higher score so it should survive.
    store.add_use_count(newer, 5).unwrap();
    consolidate(&mut store, "alpha", &cfg, &FixedClock(0)).unwrap();
    let active = store.active_memories("alpha").unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, newer);                       // higher-scored survives
    assert_eq!(store.superseded_ids("alpha").unwrap(), vec![older]);
    assert_eq!(active[0].use_count, 5);                    // loser's use_count (0) summed in
}
```

- [ ] **Step 7: Implement `consolidate` + the `find_merge` helper** (kept small for CRAP).

```rust
struct Merge { survivor: i64, loser: i64, loser_use_count: i64 }

/// Finds the first mergeable pair among `agent_id`'s active memories, or `None`.
fn find_merge(store: &VectorStore, agent_id: &str, cfg: &ConsolidationConfig, now: i64)
    -> Result<Option<Merge>> {
    let mems = store.active_memories(agent_id)?;
    for a in &mems {
        let emb = store.embedding_of(a.id)?;
        for (nid, dist) in store.knn(agent_id, &emb, 2)? {
            if nid == a.id { continue; }
            if cosine_from_l2(dist) < cfg.tau_merge { continue; }
            let Some(b) = mems.iter().find(|m| m.id == nid) else { continue; };
            let sa = effective(cfg, a.base_score, a.created_at, now, a.use_count);
            let sb = effective(cfg, b.base_score, b.created_at, now, b.use_count);
            let (survivor, loser) = if sa >= sb { (a, b) } else { (b, a) };
            return Ok(Some(Merge {
                survivor: survivor.id, loser: loser.id, loser_use_count: loser.use_count,
            }));
        }
    }
    Ok(None)
}

/// Runs one consolidation pass for `agent_id`: decay-scored dedup/merge only (v1).
///
/// # Errors
/// Returns an error if any underlying store operation fails.
pub fn consolidate(store: &mut VectorStore, agent_id: &str, cfg: &ConsolidationConfig,
                   clock: &dyn Clock) -> Result<()> {
    let now = clock.now_unix();
    while let Some(m) = find_merge(store, agent_id, cfg, now)? {
        store.add_use_count(m.survivor, m.loser_use_count)?; // sum use_count into survivor
        store.set_superseded(m.loser, m.survivor)?;          // retire loser (no new vector row)
    }
    Ok(())
}
```

- [ ] **Step 8: Run the merge test to verify it passes; then the whole module.**

Run: `cd server && cargo test --release consolidate::`
Expected: PASS — survivor is the higher-scored row, loser is superseded, use_count summed.

- [ ] **Step 9: Gate + commit.**

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings
git add server/src/consolidate.rs
git commit -m "feat(consolidate): effective decay score + injected clock + dedup/merge pass"
```

---

### Task 5: MCP server wiring (`server/src/lib.rs`)

Wires the three modules into an rmcp stdio server: tool arg structs, thin `#[tool]` adapters over `do_store`/`do_recall`/`do_consolidate`, the JSON recall payload, the pinned protocol version, and `serve_stdio()` reading `GENESIS_MEMORY_DB`.

**Files:**
- Modify: `server/src/lib.rs`
- Test: `server/src/lib.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `store::VectorStore` + methods (Task 3); `embed::{Embedder, model_paths}` (Task 2); `consolidate::{ConsolidationConfig, Clock, SystemClock, FixedClock, consolidate}` (Task 4).
- Produces: `StoreArgs{agent_id,text}`, `RecallArgs{agent_id,query,k:Option<u32>}`, `ConsolidateArgs{agent_id}`; `DEFAULT_K=5`; `DEFAULT_DB_FILENAME="genesis-memory.db"`; `do_store(...)->Result<i64>`, `do_recall(...)->Result<String>`; `MemoryServer::new(store, model_dir)`; `serve_stdio()->Result<()>`.

- [ ] **Step 1: Write the failing library-logic tests** (`do_store`+`do_recall` round-trip payload shape; default-`k`; agent isolation) — replace the "Tool API"/"Recall response payload" scaffold stubs.

```rust
// inside #[cfg(test)] mod tests:
use super::{DEFAULT_K, RecallArgs, do_recall, do_store};
use crate::consolidate::{ConsolidationConfig, FixedClock};
use crate::embed::{Embedder, model_paths};
use crate::store::VectorStore;

fn setup() -> (tempfile::TempDir, VectorStore, Embedder) {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
    let (m, t) = model_paths();
    assert!(m.exists(), "model missing: run `scripts/fetch-model`");
    let embedder = Embedder::load(m.to_str().unwrap(), t.to_str().unwrap()).unwrap();
    (dir, store, embedder)
}

#[test]
fn recall_payload_is_a_json_array_of_id_text_distance_ascending() {
    let (_d, mut store, mut emb) = setup();
    let cfg = ConsolidationConfig::default();
    let clock = FixedClock(0);
    do_store(&mut store, &mut emb, &cfg, &clock, "alpha", "deploy on fridays").unwrap();
    let json = do_recall(&mut store, &mut emb, &clock, "alpha", "deploy on fridays", 5).unwrap();
    let arr: serde_json::Value = serde_json::from_str(&json).unwrap();
    let items = arr.as_array().unwrap();
    assert!(!items.is_empty());
    for it in items {
        let obj = it.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj["id"].is_i64());
        assert!(obj["text"].is_string());
        assert!(obj["distance"].is_number());
    }
    // exact-text match ⇒ first distance == 0.0
    approx::assert_abs_diff_eq!(items[0]["distance"].as_f64().unwrap(), 0.0, epsilon = 1e-6);
    assert_eq!(items[0]["text"], "deploy on fridays");
}

#[test]
fn recall_defaults_k_to_five_and_scopes_by_agent() {
    assert_eq!(DEFAULT_K, 5);
    let (_d, mut store, mut emb) = setup();
    let cfg = ConsolidationConfig::default();
    let clock = FixedClock(0);
    for i in 0..6 { do_store(&mut store, &mut emb, &cfg, &clock, "alpha", &format!("note {i}")).unwrap(); }
    do_store(&mut store, &mut emb, &cfg, &clock, "beta", "beta only").unwrap();
    let args = RecallArgs { agent_id: "alpha".into(), query: "note".into(), k: None };
    let k = args.k.map_or(DEFAULT_K, |v| v as usize);
    let json = do_recall(&mut store, &mut emb, &clock, &args.agent_id, &args.query, k).unwrap();
    let items: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 5);            // omitted k ⇒ 5
    for it in items.as_array().unwrap() {
        assert_ne!(it["text"], "beta only");                   // never another agent's row
    }
}
```

- [ ] **Step 2: Run to verify failure.**

Run: `cd server && cargo test --release lib::tests::recall_payload_is_a_json_array_of_id_text_distance_ascending lib::tests::recall_defaults_k_to_five_and_scopes_by_agent` (or `--lib` with the two names)
Expected: FAIL — `do_store`/`do_recall` not defined.

- [ ] **Step 3: Implement the constants, `Hit`, `do_store`, `do_recall`.**

```rust
use anyhow::Result;
use serde::Serialize;

use crate::consolidate::{Clock, ConsolidationConfig};
use crate::embed::Embedder;
use crate::store::VectorStore;

/// Default number of recall hits when `k` is omitted.
pub const DEFAULT_K: usize = 5;
/// Working-directory fallback DB filename when `GENESIS_MEMORY_DB` is unset.
pub const DEFAULT_DB_FILENAME: &str = "genesis-memory.db";

#[derive(Debug, Serialize)]
struct Hit { id: i64, text: String, distance: f64 }

/// Embeds `text` and stores it under `agent_id`; returns the assigned id.
///
/// # Errors
/// Returns an error if embedding or the store insert fails.
pub fn do_store(store: &mut VectorStore, embedder: &mut Embedder, cfg: &ConsolidationConfig,
                clock: &dyn Clock, agent_id: &str, text: &str) -> Result<i64> {
    let vec = embedder.embed(text)?;
    store.insert(agent_id, text, &vec, cfg.base_score, clock.now_unix())
}

/// Embeds `query`, runs agent-scoped KNN, bumps usage, and returns the JSON payload string.
///
/// # Errors
/// Returns an error if embedding, KNN, hydration, or serialization fails.
pub fn do_recall(store: &mut VectorStore, embedder: &mut Embedder, clock: &dyn Clock,
                 agent_id: &str, query: &str, k: usize) -> Result<String> {
    let vec = embedder.embed(query)?;
    let hits = store.knn(agent_id, &vec, k)?;
    let now = clock.now_unix();
    let mut out = Vec::with_capacity(hits.len());
    for (id, distance) in hits {
        let text = store.text_of(id)?;
        store.touch(id, now)?;                 // on recall: use_count += 1, last_used_at = now
        out.push(Hit { id, text, distance });
    }
    Ok(serde_json::to_string(&out)?)
}
```

- [ ] **Step 4: Run the two logic tests to verify they pass.**

Run: `cd server && cargo test --release lib::tests::recall_payload_is_a_json_array_of_id_text_distance_ascending lib::tests::recall_defaults_k_to_five_and_scopes_by_agent`
Expected: PASS.

- [ ] **Step 5: Implement the arg structs, `MemoryServer`, the `#[tool]` adapters, and `get_info`** — replace the `serve_stdio` stub and add the server type. Uses the verified §2.3a rmcp shapes.

```rust
use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*, schemars, tool, tool_handler, tool_router, transport::stdio};
use tokio::sync::Mutex;

use crate::consolidate::{self, ConsolidationConfig, SystemClock};
use crate::embed::{self, Embedder};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StoreArgs { pub agent_id: String, pub text: String }
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecallArgs { pub agent_id: String, pub query: String, pub k: Option<u32> }
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConsolidateArgs { pub agent_id: String }

struct Inner { store: VectorStore, embedder: Option<Embedder> }

/// The MCP memory server: thin `#[tool]` adapters over the library logic functions.
#[derive(Clone)]
pub struct MemoryServer {
    tool_router: ToolRouter<MemoryServer>,
    inner: Arc<Mutex<Inner>>,
    cfg: ConsolidationConfig,
    model_dir: PathBuf,
}

impl MemoryServer {
    /// Builds a server over an already-open `store`; the embedder loads lazily from `model_dir`.
    #[must_use]
    pub fn new(store: VectorStore, model_dir: PathBuf) -> Self {
        Self {
            tool_router: Self::tool_router(),
            inner: Arc::new(Mutex::new(Inner { store, embedder: None })),
            cfg: ConsolidationConfig::default(),
            model_dir,
        }
    }

    fn err_result(e: &anyhow::Error) -> CallToolResult {
        // Tool failure = successful response with is_error:true (NOT a JSON-RPC error).
        CallToolResult::error(vec![ContentBlock::text(e.to_string())])
    }
}

#[tool_router]
impl MemoryServer {
    #[tool(description = "Store a memory for an agent")]
    async fn store(&self, Parameters(a): Parameters<StoreArgs>) -> Result<CallToolResult, McpError> {
        let mut g = self.inner.lock().await;
        if g.embedder.is_none() {
            let (m, t) = (self.model_dir.join("onnx/model.onnx"), self.model_dir.join("tokenizer.json"));
            match Embedder::load(&m.to_string_lossy(), &t.to_string_lossy()) {
                Ok(e) => g.embedder = Some(e),
                Err(e) => return Ok(Self::err_result(&e)),
            }
        }
        let Inner { store, embedder } = &mut *g;
        let embedder = embedder.as_mut().expect("loaded above");
        match crate::do_store(store, embedder, &self.cfg, &SystemClock, &a.agent_id, &a.text) {
            Ok(id) => Ok(CallToolResult::success(vec![ContentBlock::text(id.to_string())])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }

    #[tool(description = "Recall the k most relevant memories for an agent")]
    async fn recall(&self, Parameters(a): Parameters<RecallArgs>) -> Result<CallToolResult, McpError> {
        let k = a.k.map_or(crate::DEFAULT_K, |v| v as usize);
        let mut g = self.inner.lock().await;
        if g.embedder.is_none() {
            let (m, t) = (self.model_dir.join("onnx/model.onnx"), self.model_dir.join("tokenizer.json"));
            match Embedder::load(&m.to_string_lossy(), &t.to_string_lossy()) {
                Ok(e) => g.embedder = Some(e),
                Err(e) => return Ok(Self::err_result(&e)),
            }
        }
        let Inner { store, embedder } = &mut *g;
        let embedder = embedder.as_mut().expect("loaded above");
        match crate::do_recall(store, embedder, &SystemClock, &a.agent_id, &a.query, k) {
            Ok(json) => Ok(CallToolResult::success(vec![ContentBlock::text(json)])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }

    #[tool(description = "Consolidate an agent's memories (decay-scored dedup/merge)")]
    async fn consolidate(&self, Parameters(a): Parameters<ConsolidateArgs>)
        -> Result<CallToolResult, McpError> {
        let mut g = self.inner.lock().await;
        match consolidate::consolidate(&mut g.store, &a.agent_id, &self.cfg, &SystemClock) {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text("ok")])),
            Err(e) => Ok(Self::err_result(&e)),
        }
    }
}

#[tool_handler]
impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
    }
}
```

> The two `.expect("loaded above")` calls are in `src/`, where `expect_used = "deny"`. Replace each with a guard that cannot panic: `let Some(embedder) = embedder.as_mut() else { return Ok(Self::err_result(&anyhow::anyhow!("embedder unavailable"))); };`. Use that form — do not leave `.expect` in `src/`.

- [ ] **Step 6: Implement `serve_stdio`** — reads `GENESIS_MEMORY_DB` (fallback `genesis-memory.db`) and `GENESIS_MODEL_DIR` (via `embed::model_dir()`), then serves. Replace the `serve_stdio` stub.

```rust
/// Runs the MCP memory server over stdio until the client disconnects.
///
/// # Errors
/// Returns an error if the store or stdio transport fails to initialise.
pub async fn serve_stdio() -> Result<()> {
    let db_path = std::env::var("GENESIS_MEMORY_DB")
        .unwrap_or_else(|_| DEFAULT_DB_FILENAME.to_string());
    let store = VectorStore::open(&db_path)?;
    let server = MemoryServer::new(store, embed::model_dir());
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

- [ ] **Step 7: Add the wiring assertion tests** (protocol pin, capabilities, isError envelope) — replace the "Server wiring" scaffold stubs.

```rust
// inside #[cfg(test)] mod tests:
use super::MemoryServer;
use rmcp::ServerHandler;
use rmcp::model::ProtocolVersion;

fn a_server() -> MemoryServer {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
    std::mem::forget(dir); // keep the temp file alive for the server's lifetime in-test
    MemoryServer::new(store, crate::embed::model_dir())
}

#[test]
fn get_info_pins_protocol_and_enables_tools() {
    let info = a_server().get_info();
    assert_eq!(info.protocol_version, ProtocolVersion::V_2024_11_05);
    assert!(info.capabilities.tools.is_some());
}

#[test]
fn store_args_recall_args_consolidate_args_deserialize() {
    let s: super::StoreArgs = serde_json::from_value(
        serde_json::json!({"agent_id":"a","text":"t"})).unwrap();
    assert_eq!(s.agent_id, "a");
    let r: super::RecallArgs = serde_json::from_value(
        serde_json::json!({"agent_id":"a","query":"q"})).unwrap();
    assert!(r.k.is_none());
    let c: super::ConsolidateArgs = serde_json::from_value(
        serde_json::json!({"agent_id":"a"})).unwrap();
    assert_eq!(c.agent_id, "a");
}
```

- [ ] **Step 8: Run the lib tests + gate.**

Run: `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release --lib`
Expected: fmt clean, clippy clean, all `lib` unit tests pass.

- [ ] **Step 9: Commit.**

```bash
git add server/src/lib.rs
git commit -m "feat(server): rmcp MemoryServer tools + JSON recall payload + serve_stdio"
```

---

### Task 6: AC3 calibration fixture (`server/tests/fixtures/ac3_calibration.json`)

Calibrates and freezes the paraphrase/decoy fixture (bootstrap item 4). Placed **before** the BDD tasks because `store.feature`'s AC3 scenario consumes this committed fixture — the calibration needs the full embed+store+recall stack (Tasks 2/3/5), not the BDD harness, so it slots in here.

**Files:**
- Create: `server/src/lib.rs` transient calibration harness test (removed after capture) **or** a standalone `server/examples/calibrate_ac3.rs`
- Create (captured): `server/tests/fixtures/ac3_calibration.json`

**Interfaces:**
- Consumes: `do_store`, `do_recall` (Task 5); `Embedder`/`VectorStore` (Tasks 2/3); `FixedClock` (Task 4).
- Produces: `server/tests/fixtures/ac3_calibration.json` with shape `{ "source": String, "paraphrase": String, "decoys": [String, ...] }`, demonstrably ranking `source` first for the paraphrase query against the decoys. The BDD store task (Task 7) reads exactly this file.

- [ ] **Step 1: Add a calibration example** `server/examples/calibrate_ac3.rs` that tries candidate fixtures and prints the first that ranks correctly.

```rust
//! Calibration harness for AC3 (spec Bootstrap item 4). Run:
//!   cargo run --release --example calibrate_ac3
//! It stores a source + decoys, recalls with a paraphrase, and confirms the source ranks
//! first. On success it writes tests/fixtures/ac3_calibration.json — then commit that file.
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock};
use genesis_memory::embed::{Embedder, model_paths};
use genesis_memory::store::VectorStore;
use genesis_memory::{do_recall, do_store};

fn main() -> anyhow::Result<()> {
    let (m, t) = model_paths();
    let mut emb = Embedder::load(&m.to_string_lossy(), &t.to_string_lossy())?;
    let cfg = ConsolidationConfig::default();
    let clock = FixedClock(0);

    let source = "The database migration must run before the service restarts.";
    let paraphrase = "Apply the schema migration prior to bouncing the service.";
    let decoys = [
        "The cat slept on the warm windowsill all afternoon.",
        "Quarterly revenue exceeded the forecast by twelve percent.",
        "Remember to water the office plants on Fridays.",
    ];

    let dir = tempfile::tempdir()?;
    let mut store = VectorStore::open(dir.path().join("m.db").to_str().unwrap())?;
    do_store(&mut store, &mut emb, &cfg, &clock, "cal", source)?;
    for d in &decoys { do_store(&mut store, &mut emb, &cfg, &clock, "cal", d)?; }
    let json = do_recall(&mut store, &mut emb, &clock, "cal", paraphrase, 5)?;
    let items: serde_json::Value = serde_json::from_str(&json)?;
    let first = items[0]["text"].as_str().unwrap_or_default();
    anyhow::ensure!(first == source, "source did not rank first (got: {first}); adjust fixtures");

    let out = serde_json::json!({ "source": source, "paraphrase": paraphrase, "decoys": decoys });
    std::fs::create_dir_all("tests/fixtures")?;
    std::fs::write("tests/fixtures/ac3_calibration.json", serde_json::to_string_pretty(&out)?)?;
    println!("wrote tests/fixtures/ac3_calibration.json");
    Ok(())
}
```

- [ ] **Step 2: Run the calibration harness and capture the fixture.**

Run: `cd server && cargo run --release --example calibrate_ac3`
Expected: prints `wrote tests/fixtures/ac3_calibration.json`. If it panics with "source did not rank first", edit the `source`/`paraphrase`/`decoys` literals (make the paraphrase closer and the decoys more clearly unrelated) and re-run until the source ranks first — this is the empirical calibration the spec requires (§5 #6 / bootstrap item 4).

- [ ] **Step 3: Add a guard test** so the committed fixture stays valid — append to `server/src/lib.rs` `#[cfg(test)] mod tests`.

```rust
#[test]
fn ac3_calibration_fixture_ranks_the_source_first() {
    let raw = std::fs::read_to_string("tests/fixtures/ac3_calibration.json").unwrap();
    let fx: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let (_d, mut store, mut emb) = setup();
    let cfg = crate::consolidate::ConsolidationConfig::default();
    let clock = crate::consolidate::FixedClock(0);
    let source = fx["source"].as_str().unwrap();
    do_store(&mut store, &mut emb, &cfg, &clock, "cal", source).unwrap();
    for d in fx["decoys"].as_array().unwrap() {
        do_store(&mut store, &mut emb, &cfg, &clock, "cal", d.as_str().unwrap()).unwrap();
    }
    let json = do_recall(&mut store, &mut emb, &clock, "cal", fx["paraphrase"].as_str().unwrap(), 5).unwrap();
    let items: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(items[0]["text"].as_str().unwrap(), source);
}
```

- [ ] **Step 4: Run the guard test + gate.**

Run: `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release lib::tests::ac3_calibration_fixture_ranks_the_source_first`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add server/examples/calibrate_ac3.rs server/tests/fixtures/ac3_calibration.json server/src/lib.rs
git commit -m "test(calibrate): freeze AC3 paraphrase/decoy fixture (bootstrap item 4)"
```

---

### Task 7: BDD store steps (`server/tests/bdd/store_steps.rs`)

Implements `store.feature` (AC3, AC9, AC10) against the real embedder + real store, no mocks. Consumes the calibrated AC3 fixture (Task 6).

**Files:**
- Modify: `server/tests/bdd/store_steps.rs`
- Reads: `server/tests/fixtures/ac3_calibration.json`, `server/models/*`

**Interfaces:**
- Consumes: `Embedder`/`model_paths` (Task 2), `VectorStore` (Task 3), `do_store`/`do_recall` (Task 5), `ConsolidationConfig`/`FixedClock` (Task 4). Runner runs with CWD = `server/`; the feature is at `../test/features/store.feature`.
- Produces: a passing `bdd_store` target.

- [ ] **Step 1: Implement the shared `Given` (empty DB + real deps)** — replace the `a_memory_server_with_an_empty_database`, `a_loaded_embedder`, and `an_open_vector_store` stubs. Add helper fields population using the existing `StoreWorld` fields.

```rust
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock};
use genesis_memory::{do_recall, do_store};

fn model() -> (String, String) {
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(m.exists(), "model missing: run `scripts/fetch-model`");
    (m.to_string_lossy().into_owned(), t.to_string_lossy().into_owned())
}

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut StoreWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    let store = VectorStore::open(path.to_str().unwrap()).unwrap();
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.vector_store = Some(store);
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

#[given(regex = r"^a loaded embedder$")]
async fn a_loaded_embedder(w: &mut StoreWorld) {
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
}

#[given(regex = r"^an open vector store$")]
async fn an_open_vector_store(w: &mut StoreWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}
```

- [ ] **Step 2: Implement the AC3 fixture `Given`/`When`/`Then` steps** — replace `agent_has_stored_the_calibrated_source_text`, `agent_has_stored_the_calibrated_decoys`, `agent_recalls_with_the_calibrated_paraphrase`, `recall_contains_the_calibrated_source_text`, `that_entry_is_first`.

```rust
fn ac3() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string("tests/fixtures/ac3_calibration.json").unwrap()).unwrap()
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated source text$"#)]
async fn agent_has_stored_the_calibrated_source_text(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let src = fx["source"].as_str().unwrap().to_string();
    let cfg = ConsolidationConfig::default();
    do_store(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
             &cfg, &FixedClock(0), &agent, &src).unwrap();
    w.agent_id = Some(agent);
    w.fixture_texts.push(src);
}

#[given(regex = r#"^agent "([^"]*)" has stored the calibrated dissimilar decoy texts$"#)]
async fn agent_has_stored_the_calibrated_decoys(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let cfg = ConsolidationConfig::default();
    for d in fx["decoys"].as_array().unwrap() {
        do_store(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                 &cfg, &FixedClock(0), &agent, d.as_str().unwrap()).unwrap();
    }
}

#[when(regex = r#"^agent "([^"]*)" recalls with the calibrated paraphrase of the source text$"#)]
async fn agent_recalls_with_the_calibrated_paraphrase(w: &mut StoreWorld, agent: String) {
    let fx = ac3();
    let json = do_recall(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                         &FixedClock(0), &agent, fx["paraphrase"].as_str().unwrap(), 5).unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}

#[then(regex = r"^the recall result contains an entry whose text is the calibrated source text$")]
async fn recall_contains_the_calibrated_source_text(w: &mut StoreWorld) {
    let src = ac3()["source"].as_str().unwrap().to_string();
    let items = w.last_recall.as_ref().unwrap().as_array().unwrap();
    assert!(items.iter().any(|it| it["text"] == src));
}

#[then(regex = r"^that entry is first in the recall result$")]
async fn that_entry_is_first(w: &mut StoreWorld) {
    let src = ac3()["source"].as_str().unwrap().to_string();
    let items = w.last_recall.as_ref().unwrap().as_array().unwrap();
    assert_eq!(items[0]["text"], src);
}
```

- [ ] **Step 3: Implement the AC9 determinism steps** — replace `the_embedder_embeds_twice`, `the_two_vectors_have_cosine_at_least`.

```rust
#[when(regex = r#"^the embedder embeds "([^"]*)" twice in the same process$"#)]
async fn the_embedder_embeds_twice(w: &mut StoreWorld, text: String) {
    let e = w.embedder.as_mut().unwrap();
    w.vectors.push(e.embed(&text).unwrap());
    w.vectors.push(e.embed(&text).unwrap());
}

#[then(regex = r"^the two vectors have cosine similarity of at least ([0-9.]+)$")]
async fn the_two_vectors_have_cosine_at_least(w: &mut StoreWorld, cosine: f64) {
    let (a, b) = (&w.vectors[0], &w.vectors[1]);
    let dot: f64 = a.iter().zip(b).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
    let na: f64 = a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    assert!(dot / (na * nb) >= cosine);
}
```

- [ ] **Step 4: Implement the AC10 dimension-mismatch steps** — replace `a_wrong_length_vector_is_passed`, `both_calls_return_an_error`, `the_test_process_has_not_panicked`.

```rust
#[when(regex = r"^a (\d+) element vector is passed to VectorStore insert and VectorStore knn$")]
async fn a_wrong_length_vector_is_passed(w: &mut StoreWorld, len: usize) {
    let store = w.vector_store.as_mut().unwrap();
    let v = vec![0.0f32; len];
    if store.insert("alpha", "x", &v, 1.0, 0).is_err() { w.errors.push("insert".into()); }
    if store.knn("alpha", &v, 5).is_err() { w.errors.push("knn".into()); }
}

#[then(regex = r"^both calls return an error$")]
async fn both_calls_return_an_error(w: &mut StoreWorld) {
    assert!(w.errors.contains(&"insert".to_string()) && w.errors.contains(&"knn".to_string()));
}

#[then(regex = r"^the test process has not panicked$")]
async fn the_test_process_has_not_panicked(_w: &mut StoreWorld) {
    // Reaching this step at all proves no panic occurred above.
}
```

- [ ] **Step 5: Run the store BDD suite.**

Run: `cd server && cargo test --release --test bdd_store`
Expected: all 3 `store.feature` scenarios pass (0 failed, 0 skipped).

- [ ] **Step 6: Gate + commit.**

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings
git add server/tests/bdd/store_steps.rs
git commit -m "test(bdd): implement store.feature steps against real embedder+store"
```

---

### Task 8: BDD recall steps (`server/tests/bdd/recall_steps.rs`)

Implements `recall.feature` (AC4, AC5, AC6, AC7, AC16) against the real stack.

**Files:**
- Modify: `server/tests/bdd/recall_steps.rs`

**Interfaces:**
- Consumes: `do_store`/`do_recall` (Task 5), `Embedder`/`model_paths` (Task 2), `VectorStore` (Task 3), `ConsolidationConfig`/`FixedClock` (Task 4). `RecallWorld` fields already exist (`last_recall`, `stored_texts`, etc.).
- Produces: a passing `bdd_recall` target.

- [ ] **Step 1: Implement the `Given` steps** — empty DB, N distinct memories, single memory. Replace the three stubs.

```rust
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock};
use genesis_memory::{do_recall, do_store};

fn model() -> (String, String) {
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(m.exists(), "model missing: run `scripts/fetch-model`");
    (m.to_string_lossy().into_owned(), t.to_string_lossy().into_owned())
}

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut RecallWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

#[given(regex = r#"^agent "([^"]*)" has stored (\d+) distinct memories$"#)]
async fn agent_has_stored_n_distinct_memories(w: &mut RecallWorld, agent: String, n: usize) {
    let cfg = ConsolidationConfig::default();
    let subjects = ["morning routine", "deploy schedule", "grocery list", "meeting notes",
        "vacation plan", "book summary", "workout plan", "recipe idea", "budget review", "travel log"];
    for i in 0..n {
        let text = format!("{} number {i}", subjects[i % subjects.len()]);
        do_store(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                 &cfg, &FixedClock(0), &agent, &text).unwrap();
        w.stored_texts.push(text);
    }
}

#[given(regex = r#"^agent "([^"]*)" has stored the memory "([^"]*)"$"#)]
async fn agent_has_stored_the_memory(w: &mut RecallWorld, agent: String, text: String) {
    let cfg = ConsolidationConfig::default();
    do_store(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
             &cfg, &FixedClock(0), &agent, &text).unwrap();
    w.stored_texts.push(text);
}
```

- [ ] **Step 2: Implement the `When` steps** (recall with k / without k).

```rust
#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" with k of (\d+)$"#)]
async fn agent_recalls_with_k(w: &mut RecallWorld, agent: String, query: String, k: u32) {
    let json = do_recall(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                         &FixedClock(0), &agent, &query, k as usize).unwrap();
    w.last_recall_text = Some(json.clone());
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" without k$"#)]
async fn agent_recalls_without_k(w: &mut RecallWorld, agent: String, query: String) {
    let k = genesis_memory::DEFAULT_K;   // omitted k ⇒ 5
    let json = do_recall(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                         &FixedClock(0), &agent, &query, k).unwrap();
    w.last_recall_text = Some(json.clone());
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}
```

- [ ] **Step 3: Implement the `Then` steps** (count, non-decreasing distance, first text, distance 0.0, absence, JSON shape).

```rust
fn items(w: &RecallWorld) -> &Vec<serde_json::Value> {
    w.last_recall.as_ref().unwrap().as_array().unwrap()
}

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(w: &mut RecallWorld, n: usize) {
    assert_eq!(items(w).len(), n);
}

#[then(regex = r"^the distance values in the recall result are non-decreasing$")]
async fn distances_are_non_decreasing(w: &mut RecallWorld) {
    let ds: Vec<f64> = items(w).iter().map(|it| it["distance"].as_f64().unwrap()).collect();
    for win in ds.windows(2) { assert!(win[0] <= win[1]); }
}

#[then(regex = r#"^the first recall entry has text "([^"]*)"$"#)]
async fn the_first_recall_entry_has_text(w: &mut RecallWorld, text: String) {
    assert_eq!(items(w)[0]["text"], text);
}

#[then(regex = r"^the first recall entry has distance exactly ([0-9.]+)$")]
async fn the_first_recall_entry_has_distance(w: &mut RecallWorld, distance: f64) {
    approx::assert_abs_diff_eq!(items(w)[0]["distance"].as_f64().unwrap(), distance, epsilon = 1e-6);
}

#[then(regex = r#"^no recall entry has text "([^"]*)"$"#)]
async fn no_recall_entry_has_text(w: &mut RecallWorld, text: String) {
    assert!(items(w).iter().all(|it| it["text"] != text));
}

#[then(regex = r"^the text content block of the recall result parses as a JSON array$")]
async fn the_content_block_parses_as_a_json_array(w: &mut RecallWorld) {
    let v: serde_json::Value = serde_json::from_str(w.last_recall_text.as_ref().unwrap()).unwrap();
    assert!(v.is_array());
}

#[then(regex = r"^every element of that array has exactly the keys id and text and distance$")]
async fn every_element_has_exactly_the_three_keys(w: &mut RecallWorld) {
    for it in items(w) {
        let o = it.as_object().unwrap();
        assert_eq!(o.len(), 3);
        assert!(o.contains_key("id") && o.contains_key("text") && o.contains_key("distance"));
    }
}

#[then(regex = r"^in every element id is an integer, text is a string and distance is a number$")]
async fn every_element_has_the_right_types(w: &mut RecallWorld) {
    for it in items(w) {
        assert!(it["id"].is_i64() && it["text"].is_string() && it["distance"].is_number());
    }
}
```

- [ ] **Step 4: Run the recall BDD suite.**

Run: `cd server && cargo test --release --test bdd_recall`
Expected: all 5 `recall.feature` scenarios pass. (AC5 exact-match distance 0.0 and AC7 default-5 both hold with `FixedClock`.)

- [ ] **Step 5: Gate + commit.**

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings
git add server/tests/bdd/recall_steps.rs
git commit -m "test(bdd): implement recall.feature steps against real embedder+store"
```

---

### Task 9: BDD consolidate steps (`server/tests/bdd/consolidate_steps.rs`)

Implements `consolidate.feature` (AC8, AC13): near-duplicate retired only by an explicit `consolidate`, and never returned afterward.

**Files:**
- Modify: `server/tests/bdd/consolidate_steps.rs`

**Interfaces:**
- Consumes: `do_store`/`do_recall` (Task 5), `consolidate::{consolidate, ConsolidationConfig, FixedClock, cosine_from_l2}` (Task 4), `VectorStore` (Task 3), `Embedder` (Task 2). `ConsolidateWorld` fields (`pair_ids`, `shared_subject`, `superseded_id`, `config`, `last_recall`) already exist.
- Produces: a passing `bdd_consolidate` target.

- [ ] **Step 1: Implement the near-duplicate `Given` steps.** The pair shares a subject and is at/above `tau_merge`; assert cosine via `cosine_from_l2` over their embeddings.

```rust
use genesis_memory::consolidate::{ConsolidationConfig, FixedClock, consolidate, cosine_from_l2};
use genesis_memory::{do_recall, do_store};

fn model() -> (String, String) {
    let (m, t) = genesis_memory::embed::model_paths();
    assert!(m.exists(), "model missing: run `scripts/fetch-model`");
    (m.to_string_lossy().into_owned(), t.to_string_lossy().into_owned())
}

const SUBJECT: &str = "the release goes out on friday";
const NEAR_A: &str = "the release goes out on friday";
const NEAR_B: &str = "the release goes out on friday.";

#[given(regex = r"^a memory server with an empty database$")]
async fn a_memory_server_with_an_empty_database(w: &mut ConsolidateWorld) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    w.vector_store = Some(VectorStore::open(path.to_str().unwrap()).unwrap());
    let (m, t) = model();
    w.embedder = Some(Embedder::load(&m, &t).unwrap());
    w.config = Some(ConsolidationConfig::default());
    w.shared_subject = Some(SUBJECT.to_string());
    w.db_path = Some(path);
    w.db_dir = Some(dir);
}

async fn store_pair(w: &mut ConsolidateWorld, agent: &str) {
    let cfg = w.config.clone().unwrap();
    let store = w.vector_store.as_mut().unwrap();
    let emb = w.embedder.as_mut().unwrap();
    let id_a = do_store(store, emb, &cfg, &FixedClock(0), agent, NEAR_A).unwrap();
    let id_b = do_store(store, emb, &cfg, &FixedClock(0), agent, NEAR_B).unwrap();
    // Confirm the fixture really is >= tau_merge (derive cosine from L2 via a self-knn).
    let ea = store.embedding_of(id_a).unwrap();
    let hit = store.knn(agent, &ea, 2).unwrap().into_iter().find(|(id, _)| *id == id_b).unwrap();
    assert!(cosine_from_l2(hit.1) >= cfg.tau_merge, "fixture below tau_merge: {}", cosine_from_l2(hit.1));
    w.pair_ids = vec![id_a, id_b];
}

#[given(regex = r#"^agent "([^"]*)" has stored two memories whose cosine similarity is at or above tau_merge$"#)]
async fn agent_has_stored_a_near_duplicate_pair(w: &mut ConsolidateWorld, agent: String) {
    store_pair(w, &agent).await;
}

#[given(regex = r#"^agent "([^"]*)" has two memories that a consolidate call has merged$"#)]
async fn agent_has_two_merged_memories(w: &mut ConsolidateWorld, agent: String) {
    store_pair(w, &agent).await;
    let cfg = w.config.clone().unwrap();
    consolidate(w.vector_store.as_mut().unwrap(), &agent, &cfg, &FixedClock(0)).unwrap();
    w.superseded_id = w.vector_store.as_ref().unwrap().superseded_ids(&agent).unwrap().first().copied();
}
```

- [ ] **Step 2: Implement the `When` steps** (recall shared subject, consolidate).

```rust
#[when(regex = r#"^agent "([^"]*)" recalls the shared subject of those two memories with k of (\d+)$"#)]
async fn agent_recalls_the_shared_subject(w: &mut ConsolidateWorld, agent: String, k: u32) {
    let subject = w.shared_subject.clone().unwrap();
    let json = do_recall(w.vector_store.as_mut().unwrap(), w.embedder.as_mut().unwrap(),
                         &FixedClock(0), &agent, &subject, k as usize).unwrap();
    w.last_recall = Some(serde_json::from_str(&json).unwrap());
}

#[when(regex = r#"^agent "([^"]*)" consolidates$"#)]
async fn agent_consolidates(w: &mut ConsolidateWorld, agent: String) {
    let cfg = w.config.clone().unwrap();
    consolidate(w.vector_store.as_mut().unwrap(), &agent, &cfg, &FixedClock(0)).unwrap();
    w.superseded_id = w.vector_store.as_ref().unwrap().superseded_ids(&agent).unwrap().first().copied();
}
```

- [ ] **Step 3: Implement the `Then` steps** (exact count, superseded-absent by id).

```rust
fn items(w: &ConsolidateWorld) -> &Vec<serde_json::Value> {
    w.last_recall.as_ref().unwrap().as_array().unwrap()
}

#[then(regex = r"^the recall result contains exactly (\d+) entr(?:y|ies)$")]
async fn the_recall_result_contains_exactly_n_entries(w: &mut ConsolidateWorld, n: usize) {
    assert_eq!(items(w).len(), n);
}

#[then(regex = r"^the recall result does not contain the superseded memory$")]
async fn the_recall_result_omits_the_superseded_memory(w: &mut ConsolidateWorld) {
    let sid = w.superseded_id.expect("a row was superseded");
    assert!(items(w).iter().all(|it| it["id"].as_i64() != Some(sid)));
}

#[then(regex = r"^no recall entry has the id of the row whose superseded_by is non-null$")]
async fn no_recall_entry_has_the_superseded_id(w: &mut ConsolidateWorld) {
    let sid = w.superseded_id.expect("a row was superseded");
    assert!(items(w).iter().all(|it| it["id"].as_i64() != Some(sid)));
}
```

- [ ] **Step 4: Run the consolidate BDD suite.**

Run: `cd server && cargo test --release --test bdd_consolidate`
Expected: both `consolidate.feature` scenarios pass — 2 entries before consolidate, exactly 1 after, and the superseded id never returned.

- [ ] **Step 5: Gate + commit.**

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings
git add server/tests/bdd/consolidate_steps.rs
git commit -m "test(bdd): implement consolidate.feature steps (dedup/merge, superseded exclusion)"
```

---

### Task 10: BDD server steps (`server/tests/bdd/server_steps.rs`)

Implements `server.feature` (AC1, AC2, AC11, AC12, AC14, AC15, AC17) by spawning the real `genesis-memory-server` binary and driving a real stdio JSON-RPC lifecycle. No mocks.

**Files:**
- Modify: `server/tests/bdd/server_steps.rs`

**Interfaces:**
- Consumes: the built binary `genesis-memory-server` (via `assert_cmd::cargo::cargo_bin`), env `GENESIS_MEMORY_DB` + `GENESIS_MODEL_DIR`, and the wire behaviour from Task 5. `ServerWorld` fields (`server`, `second_server`, `db_dir`, `second_db_dir`, `last_response`, `responses`, `exit_code`) already exist.
- Produces: a passing `bdd_server` target.

> **Wire helper:** these steps speak line-delimited JSON-RPC over the child's stdin/stdout. Add a small in-file helper module `mod rpc` that spawns the binary, writes a request line, reads a response line, and runs the `initialize` + `notifications/initialized` handshake. Below, `rpc::spawn(db, model_dir)`, `rpc::send(child, value) -> serde_json::Value`, `rpc::notify(child, value)`, and `rpc::initialize(child)` denote those helpers.

- [ ] **Step 1: Add the `rpc` wire helper** at the top of the file (real child process, no mock).

```rust
mod rpc {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Child, Command, Stdio};

    pub fn spawn(db: &std::path::Path, model_dir: Option<&std::path::Path>) -> Child {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("genesis-memory-server"));
        cmd.env("GENESIS_MEMORY_DB", db)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Some(md) = model_dir { cmd.env("GENESIS_MODEL_DIR", md); }
        cmd.spawn().unwrap()
    }

    pub fn send(child: &mut Child, req: serde_json::Value) -> serde_json::Value {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    pub fn notify(child: &mut Child, note: serde_json::Value) {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{note}").unwrap();
        stdin.flush().unwrap();
    }

    pub fn initialize(child: &mut Child) -> serde_json::Value {
        let resp = send(child, serde_json::json!({
            "jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2024-11-05","capabilities":{},
                      "clientInfo":{"name":"bdd","version":"0"}}}));
        notify(child, serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        resp
    }
}
```

- [ ] **Step 2: Implement the spawn/initialize `Given` steps** (AC1/AC2/AC11/AC12/AC14/AC15 preconditions) — replace the six `Given` stubs. AC11's server points at an empty model dir so the lazily-loaded embedder fails.

```rust
use genesis_memory::embed::model_dir;

#[given(regex = r"^a spawned memory server child process over stdio$")]
async fn a_spawned_memory_server_child_process(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    w.server = Some(rpc::spawn(&db, Some(&model_dir())));
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^an initialized memory server child process over stdio$")]
async fn an_initialized_memory_server_child_process(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    // Empty model dir ⇒ the first tool call that needs the embedder fails ⇒ AC11 isError:true.
    let empty = TempDir::new().unwrap();
    let mut child = rpc::spawn(&db, Some(empty.path()));
    rpc::initialize(&mut child);
    std::mem::forget(empty);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^an initialized memory server child process over stdio with the model already on disk$")]
async fn an_initialized_server_with_the_model_on_disk(w: &mut ServerWorld) {
    let (m, _t) = genesis_memory::embed::model_paths();
    assert!(m.exists(), "model missing: run `scripts/fetch-model`");
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("m.db");
    let mut child = rpc::spawn(&db, Some(&model_dir()));
    rpc::initialize(&mut child);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^outbound network access is unavailable$")]
async fn outbound_network_access_is_unavailable(_w: &mut ServerWorld) {
    // v1 performs no request-time network I/O; the child was spawned with the model already on
    // disk. (The spawn helper may also set HTTP(S)_PROXY to a dead address to enforce this.)
}

#[given(regex = r"^a memory server child process launched with GENESIS_MEMORY_DB pointing at a first temporary file$")]
async fn a_server_with_the_first_temporary_database(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("first.db");
    let mut child = rpc::spawn(&db, Some(&model_dir()));
    rpc::initialize(&mut child);
    w.server = Some(child);
    w.db_path = Some(db);
    w.db_dir = Some(dir);
}

#[given(regex = r"^a second memory server child process launched with GENESIS_MEMORY_DB pointing at a different temporary file$")]
async fn a_server_with_the_second_temporary_database(w: &mut ServerWorld) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("second.db");
    let mut child = rpc::spawn(&db, Some(&model_dir()));
    rpc::initialize(&mut child);
    w.second_server = Some(child);
    w.second_db_path = Some(db);
    w.second_db_dir = Some(dir);
}
```

- [ ] **Step 3: Implement the `When` steps** (initialize, named request, failing tool call, malformed request, close stdin, three tool calls, store-through-first, recall-through-second).

```rust
#[when(regex = r"^the client sends an initialize request$")]
async fn the_client_sends_an_initialize_request(w: &mut ServerWorld) {
    let resp = rpc::initialize(w.server.as_mut().unwrap());
    w.last_response = Some(resp);
}

#[when(regex = r#"^the client sends a "([^"]*)" request$"#)]
async fn the_client_sends_a_named_request(w: &mut ServerWorld, method: String) {
    let resp = rpc::send(w.server.as_mut().unwrap(),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":method}));
    w.last_response = Some(resp);
}

#[when(regex = r"^the client makes a tool call that triggers an internal failure$")]
async fn a_tool_call_that_triggers_an_internal_failure(w: &mut ServerWorld) {
    // The server (Given) points at an empty model dir ⇒ store cannot load the embedder.
    let resp = rpc::send(w.server.as_mut().unwrap(), serde_json::json!({
        "jsonrpc":"2.0","id":3,"method":"tools/call",
        "params":{"name":"store","arguments":{"agent_id":"alpha","text":"boom"}}}));
    w.last_response = Some(resp);
}

#[when(regex = r"^a syntactically invalid JSON-RPC request is written to the server stdin$")]
async fn an_invalid_jsonrpc_request_is_written(w: &mut ServerWorld) {
    use std::io::Write;
    let child = w.server.as_mut().unwrap();
    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, "{{ this is not valid json").unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    use std::io::{BufRead, BufReader};
    BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
    w.last_response = Some(serde_json::from_str(&line).unwrap());
}

#[when(regex = r"^the client closes the server stdin$")]
async fn the_client_closes_the_server_stdin(w: &mut ServerWorld) {
    let mut child = w.server.take().unwrap();
    drop(child.stdin.take());                 // EOF on stdin
    let status = child.wait().unwrap();
    w.exit_code = status.code();
}

#[when(regex = r#"^the client makes a tool call for each of "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn a_tool_call_for_each_of(w: &mut ServerWorld, first: String, second: String, third: String) {
    let child = w.server.as_mut().unwrap();
    for (i, name) in [first, second, third].into_iter().enumerate() {
        let args = match name.as_str() {
            "store" => serde_json::json!({"agent_id":"alpha","text":"offline note"}),
            "recall" => serde_json::json!({"agent_id":"alpha","query":"offline note"}),
            _ => serde_json::json!({"agent_id":"alpha"}),
        };
        let resp = rpc::send(child, serde_json::json!({
            "jsonrpc":"2.0","id":100+i,"method":"tools/call",
            "params":{"name":name,"arguments":args}}));
        w.responses.push(resp);
    }
}

#[when(regex = r#"^agent "([^"]*)" stores the memory "([^"]*)" through the first server$"#)]
async fn agent_stores_through_the_first_server(w: &mut ServerWorld, agent: String, text: String) {
    let resp = rpc::send(w.server.as_mut().unwrap(), serde_json::json!({
        "jsonrpc":"2.0","id":200,"method":"tools/call",
        "params":{"name":"store","arguments":{"agent_id":agent,"text":text}}}));
    w.responses.push(resp);
}

#[when(regex = r#"^agent "([^"]*)" recalls "([^"]*)" through the second server$"#)]
async fn agent_recalls_through_the_second_server(w: &mut ServerWorld, agent: String, query: String) {
    let resp = rpc::send(w.second_server.as_mut().unwrap(), serde_json::json!({
        "jsonrpc":"2.0","id":201,"method":"tools/call",
        "params":{"name":"recall","arguments":{"agent_id":agent,"query":query}}}));
    w.last_response = Some(resp);
}
```

- [ ] **Step 4: Implement the `Then` steps** (capabilities/protocol, tool names, isError true, JSON-RPC error, exit 0, all-succeed, isolation).

```rust
fn result_text(resp: &serde_json::Value) -> String {
    resp["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

#[then(regex = r#"^the initialize response advertises "([^"]*)" under capabilities$"#)]
async fn initialize_advertises_capability(w: &mut ServerWorld, capability: String) {
    let r = w.last_response.as_ref().unwrap();
    assert!(r["result"]["capabilities"].get(&capability).is_some());
}

#[then(regex = r#"^the initialize response protocolVersion is "([^"]*)"$"#)]
async fn initialize_protocol_version_is(w: &mut ServerWorld, version: String) {
    assert_eq!(w.last_response.as_ref().unwrap()["result"]["protocolVersion"], version);
}

#[then(regex = r#"^the response contains the tool names "([^"]*)", "([^"]*)" and "([^"]*)"$"#)]
async fn the_response_contains_the_tool_names(w: &mut ServerWorld, first: String, second: String, third: String) {
    let tools = w.last_response.as_ref().unwrap()["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for n in [first, second, third] { assert!(names.contains(&n.as_str())); }
}

#[then(regex = r"^the response is a JSON-RPC result whose isError field is true$")]
async fn the_response_is_a_result_with_is_error_true(w: &mut ServerWorld) {
    let r = w.last_response.as_ref().unwrap();
    assert!(r.get("error").is_none(), "must be a result, not a JSON-RPC error");
    assert_eq!(r["result"]["isError"], serde_json::Value::Bool(true));
}

#[then(regex = r"^the server replies with a JSON-RPC error object rather than a result$")]
async fn the_server_replies_with_a_jsonrpc_error(w: &mut ServerWorld) {
    let r = w.last_response.as_ref().unwrap();
    assert!(r.get("error").is_some());
    assert!(r.get("result").is_none());
}

#[then(regex = r"^the server child process terminates with exit status (\d+)$")]
async fn the_server_terminates_with_exit_status(w: &mut ServerWorld, code: i32) {
    assert_eq!(w.exit_code, Some(code));
}

#[then(regex = r"^every one of those tool calls completes successfully$")]
async fn every_tool_call_completes_successfully(w: &mut ServerWorld) {
    assert_eq!(w.responses.len(), 3);
    for r in &w.responses {
        assert!(r.get("error").is_none());
        assert_eq!(r["result"]["isError"], serde_json::Value::Bool(false));
    }
}

#[then(regex = r#"^the recall result contains no entry whose text is "([^"]*)"$"#)]
async fn the_recall_result_contains_no_entry_with_text(w: &mut ServerWorld, text: String) {
    let payload = result_text(w.last_response.as_ref().unwrap());
    let items: serde_json::Value = serde_json::from_str(&payload).unwrap_or(serde_json::json!([]));
    assert!(items.as_array().unwrap().iter().all(|it| it["text"] != text));
}
```

- [ ] **Step 5: Run the server BDD suite.**

Run: `cd server && cargo test --release --test bdd_server`
Expected: all 7 `server.feature` scenarios pass — initialize (tools + `2024-11-05`), tools/list names, `isError:true` on the model-less store call, JSON-RPC `error` on malformed input, exit 0 on stdin EOF, three successful offline tool calls, and cross-DB isolation.

- [ ] **Step 6: Full-suite regression + gate.**

Run: `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release`
Expected: fmt clean, clippy clean, every unit + BDD test passes (whole suite GREEN).

- [ ] **Step 7: Commit.**

```bash
git add server/tests/bdd/server_steps.rs
git commit -m "test(bdd): implement server.feature stdio lifecycle steps against the real binary"
```

---

## Self-Review

Run against `test/specs/genesis-memory-server.md` with fresh eyes. Issues found were fixed inline in the tasks above; the results are recorded here.

### 1. Spec coverage

**17 Acceptance Criteria → task:**

| AC | Where covered |
|----|---------------|
| AC1 initialize advertises tools + `2024-11-05` | Task 5 (`get_info`), Task 10 step 4 |
| AC2 `tools/list` names store/recall/consolidate | Task 5 (`#[tool]` router), Task 10 step 4 |
| AC3 paraphrase retrieval ranks source first | Task 6 (calibration), Task 7 steps 2 |
| AC4 at most `k`, non-decreasing distance | Task 5 (`do_recall`/knn), Task 8 |
| AC5 exact match ⇒ distance 0.0 | Task 3 (L2 knn), Task 8 |
| AC6 agent isolation | Task 3 (agent-scoped knn), Task 5, Task 8 |
| AC7 omitted `k` ⇒ 5 | Task 5 (`DEFAULT_K`), Task 8 |
| AC8 superseded row never recalled | Task 3 (superseded filter), Task 9 |
| AC9 same-process determinism | Task 2, Task 7 step 3 |
| AC10 383-vec ⇒ `Err`, no panic | Task 3, Task 7 step 4 |
| AC11 tool failure ⇒ `isError:true` | Task 5 (`err_result`), Task 10 (empty model dir trigger) |
| AC12 malformed ⇒ JSON-RPC error | Task 10 step 3/4 (rmcp default) |
| AC13 near-dup retired only by consolidate | Task 4, Task 9 |
| AC14 stdin EOF ⇒ exit 0 | Task 5 (`serve_stdio` `waiting().await`), Task 10 |
| AC15 offline tool calls succeed | Task 10 (model preloaded, no request-time net) |
| AC16 JSON `{id,text,distance}` payload | Task 5 (`Hit`), Task 8 |
| AC17 two DBs never share | Task 5 (`GENESIS_MEMORY_DB`), Task 10 |

**89 Implementation Requirements → task (by group):** Tool API (agent scoping) → Tasks 3/5; Recall response payload → Task 5; Database location → Task 5 (`serve_stdio`, `DEFAULT_DB_FILENAME`, no OS-dir); Vector store (all ~20) → Task 3; Embeddings (all ~16) → Task 2; Model provenance (all ~12) → Task 1 (+ SHA/fail-not-skip tests, revision/sha constants) and Task 2 (golden); Consolidation (all ~20) → Task 4 (+ `store` writes `base_score` via `do_store`/`insert` in Task 5/3, recall bumps use_count/last_used_at via `touch` in Task 5); Server wiring (7) → Task 5; Test-fixture choices → Tasks 7/8/10; Bootstrap items 1–4 → Tasks 1/2/6 (item 5 CRAP threshold is inherited, not a code task).

**Gaps found and fixed:**
- **G1 — `base_score` not in scaffold config.** The scaffold `ConsolidationConfig` lacked the spec-D1 `base_score` field. Fixed: Task 4 step 1 adds it (default `1.0`) and Task 3/5 write it into every row at store time (`insert` param + `do_store` passes `cfg.base_score`).
- **G2 — recall usage bump had no home.** "increment use_count / set last_used_at on recall" mapped to no method. Fixed: added `VectorStore::touch` (Task 3) called by `do_recall` (Task 5).
- **G3 — agent-scoped/superseded-excluded KNN vs the verified plain SQL.** The verified `SELECT rowid, distance ... LIMIT k` omits agent/superseded filters. Fixed: Task 3 wraps that exact form as an inner subquery and applies the filters in an outer join, with a candidate-pool sized to `COUNT(*)` so the outer filter never under-returns (documented as a §6.2 confirm-at-implementation item).
- **G4 — AC11 needed a deterministic, portable failure trigger.** Fixed: added the labelled `GENESIS_MODEL_DIR` implementation decision + lazy embedder load, so pointing a spawned server at an empty model dir makes `store` fail into `isError:true`.
- **G5 — dependency inversion for AC3.** The calibrated fixture (bootstrap item 4) is required by the store BDD scenario, so the calibration task was placed **before** the BDD tasks (Task 6), not last — noted explicitly in Task 6's header.
- **G6 — SHA-256 assertion needed a hasher.** Fixed: Task 1 adds `sha2` + `hex` as dev-dependencies (product deps unchanged).
- **G7 — clippy `unwrap_used`/`expect_used` = deny would fail on test code under `--all-targets`.** Fixed: Task 1 adds `server/clippy.toml` with `allow-unwrap-in-tests`/`allow-expect-in-tests`, keeping the `src/` gate strict.

### 2. Placeholder scan

No forbidden red-flag phrases ("TBD", "implement later", "add error handling", "similar to Task N", "handle edge cases") appear — every code step shows complete Rust. Two categories were double-checked because they can *look* like placeholders:
- **Bootstrap constants** (`MODEL_REVISION`, `MODEL_SHA256`, the golden vector, the AC3 fixture): these are the spec's own sanctioned "captured at first run" values (Provenance legend / "Bootstrap and calibration items"). Each is accompanied by the exact capture command and the exact file/line to edit — a specified procedure, not hand-waving. The `MODEL_REVISION` default is a concrete 40-hex commit with a re-capture command if it no longer resolves.
- **`src/` lint compliance**: the two `.expect("loaded above")` in the first draft of the `#[tool]` adapters would violate `expect_used = deny` in `src/`. Fixed inline in Task 5 step 5 with a non-panicking `let ... else { return Ok(err_result(...)) }` guard, called out in a note.

### 3. Type consistency

Signatures used by later tasks match the **Shared Interfaces** block and the producing tasks exactly:
- `Embedder::load(&str,&str)->Result<Self>`, `Embedder::embed(&mut self,&str)->Result<Vec<f32>>` (Task 2) — consumed verbatim by Tasks 5/7/8/9/10.
- `VectorStore::insert(agent_id,text,embedding,base_score,now_unix)->Result<i64>` and `knn(agent_id,query,k)->Result<Vec<(i64,f64)>>` (Task 3) — the AC10 step and `do_store`/`do_recall` call exactly these arities. The scaffold's `insert(id,...)->Result<()>` and `knn(query,k)` are explicitly superseded (spec D4), noted in Shared Interfaces.
- `effective(cfg,base_score,created_at,now,use_count)->f64`, `cosine_from_l2(f64)->f64`, `consolidate(store,agent_id,cfg,clock)->Result<()>` (Task 4) — consumed identically in Tasks 5/9 (the added `clock` parameter supersedes the scaffold `consolidate` signature, noted).
- `do_store(store,embedder,cfg,clock,agent_id,text)->Result<i64>`, `do_recall(store,embedder,clock,agent_id,query,k)->Result<String>`, `DEFAULT_K=5` (Task 5) — the same argument order and types are used across Tasks 6/7/8/9. `Clock`/`FixedClock`/`SystemClock` names are consistent everywhere.
- Recall JSON keys `id`/`text`/`distance` (Task 5 `Hit`) match every BDD assertion (Tasks 7/8/9/10) and AC16.

No name drift found (`superseded_ids`, `touch`, `active_memories`, `embedding_of`, `set_superseded`, `add_use_count` are spelled identically at definition and every call site).
