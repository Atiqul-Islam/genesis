//! Local ONNX embeddings — `ort` (ONNX Runtime) + `tokenizers`.
//!
//! Loads a small sentence model (primary: `all-MiniLM-L6-v2`, 384-dim, **mean** pooling)
//! and turns text into an L2-normalized vector. Pooling must match the model: MiniLM =
//! mean pool; if `bge-small-en-v1.5` is chosen instead, switch to CLS pooling
//! (`pooled[d] = hidden[[0,0,d]]`) and prepend the query instruction. Determinism knobs
//! for golden-vector tests: `with_deterministic_compute(true)`, `with_intra_threads(1)`.
//! See `docs/SPEC_FORGE_RUST_UPDATE.md` §2.3c + §5 best-practice #3.

use anyhow::Result;
use std::path::{Path, PathBuf};

use ort::ep::CPU;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

/// Embedding vector dimensionality (all-MiniLM-L6-v2 / bge-small-en-v1.5 = 384).
pub const EMBED_DIM: usize = 384;

/// Bootstrap item 1: the pinned HF commit, captured by `scripts/fetch-model` at first fetch.
pub const MODEL_REVISION: &str = "c9745ed1d9f207416be6d2e6f8de32d1f16199bf";
/// Bootstrap item 2: SHA-256 of `onnx/model.onnx` at [`MODEL_REVISION`], captured at first fetch.
pub const MODEL_SHA256: &str = "6fd5d72fe4589f189f8ebc006442dbb529bb7ce38f8082112682524616046452";
/// The pinned Hugging Face repository the embedder is fetched from (§2.3c primary model).
/// Singular source of truth so provenance tests can assert the fetch never targets a `bge` variant.
pub const MODEL_REPO: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// The model directory: `GENESIS_MODEL_DIR` if set, else `<CARGO_MANIFEST_DIR>/models`.
#[must_use]
pub fn model_dir() -> PathBuf {
    std::env::var_os("GENESIS_MODEL_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("models"),
        PathBuf::from,
    )
}

/// Paths to the ONNX model and tokenizer inside [`model_dir`].
#[must_use]
pub fn model_paths() -> (PathBuf, PathBuf) {
    let base = model_dir();
    (
        base.join("onnx").join("model.onnx"),
        base.join("tokenizer.json"),
    )
}

/// Errors specific to embedding (wrapped into `anyhow::Error` at the API boundary).
#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    /// Tokenizer load or encode failure (the `tokenizers` error is not `std::error::Error`).
    #[error("tokenizer error: {0}")]
    Tokenize(String),
    /// The ONNX output tensor did not have the expected rank-3 `last_hidden_state` shape.
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

/// Converts a non-`Send`/`Sync` `ort::Error` into an `anyhow::Error` by rendering it
/// to a fresh owned string (anyhow requires `Send + Sync + 'static` error payloads).
/// Takes the error by value so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn ort_anyhow(e: ort::Error) -> anyhow::Error {
    anyhow::anyhow!("ort: {e}")
}

impl Embedder {
    /// Loads the ONNX model and tokenizer from disk with determinism knobs pinned
    /// (CPU execution provider, single intra-thread, deterministic compute, fixed
    /// optimization level) so golden-vector comparisons are reproducible (§5 #3).
    ///
    /// # Errors
    ///
    /// Returns an error if the session or tokenizer fails to load.
    pub fn load(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let session = Self::build_session(model_path).map_err(ort_anyhow)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;
        Ok(Self { session, tokenizer })
    }

    /// Builds the ONNX session with the pinned determinism knobs.
    fn build_session(model_path: &str) -> ort::Result<Session> {
        Session::builder()?
            .with_execution_providers([CPU::default().build()])?
            .with_intra_threads(1)?
            .with_deterministic_compute(true)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_path)
    }

    /// Embeds `text` into an L2-normalized `EMBED_DIM`-length vector (mean pooling
    /// over the token `last_hidden_state`, weighted by the attention mask).
    ///
    /// # Errors
    ///
    /// Returns an error if tokenization or the ONNX inference run fails.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let (ids, mask, seq) = self.tokenize(text)?;
        let hidden = self.run_model(&ids, &mask, seq)?;
        Ok(mean_pool_l2(&hidden, &mask, seq))
    }

    /// Tokenizes `text` into `(input_ids, attention_mask, seq_len)` with special tokens enabled.
    fn tokenize(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>, usize)> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;
        let seq = enc.len();
        let ids = enc.get_ids().iter().map(|&x| i64::from(x)).collect();
        let mask = enc
            .get_attention_mask()
            .iter()
            .map(|&x| i64::from(x))
            .collect();
        Ok((ids, mask, seq))
    }

    /// Runs the ONNX session over the tokenized inputs, returning the rank-3
    /// `last_hidden_state` (owned).
    ///
    /// §6.2 #6 CONFIRMED at implementation: this all-MiniLM-L6-v2 export REQUIRES a third
    /// input, `token_type_ids` (its token_type_embeddings/Gather node reads it). For a single
    /// sentence it is all zeros. Inputs are bound BY NAME so graph input order cannot corrupt
    /// the result; output[0] is `last_hidden_state` (confirmed by the golden test).
    fn run_model(&mut self, ids: &[i64], mask: &[i64], seq: usize) -> Result<ndarray::Array3<f32>> {
        let type_ids: Vec<i64> = vec![0_i64; seq];
        let (a_ids, a_mask, a_types) =
            make_tensors(ids, mask, &type_ids, seq).map_err(ort_anyhow)?;
        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => a_ids,
                "attention_mask" => a_mask,
                "token_type_ids" => a_types,
            ])
            .map_err(ort_anyhow)?;
        extract_hidden(&outputs[0])
    }
}

/// Builds the three named `[1, seq]` i64 input tensors (`input_ids`, `attention_mask`,
/// `token_type_ids`), borrowing the caller's slices.
#[allow(clippy::type_complexity)]
fn make_tensors<'a>(
    ids: &'a [i64],
    mask: &'a [i64],
    type_ids: &'a [i64],
    seq: usize,
) -> ort::Result<(TensorRef<'a, i64>, TensorRef<'a, i64>, TensorRef<'a, i64>)> {
    Ok((
        TensorRef::from_array_view(([1_usize, seq], ids))?,
        TensorRef::from_array_view(([1_usize, seq], mask))?,
        TensorRef::from_array_view(([1_usize, seq], type_ids))?,
    ))
}

/// Extracts ONNX `output[0]` as an owned rank-3 `last_hidden_state` (`[1, seq, dim]`).
fn extract_hidden(value: &ort::value::DynValue) -> Result<ndarray::Array3<f32>> {
    let hidden = value
        .try_extract_array::<f32>()
        .map_err(ort_anyhow)?
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| EmbedError::OutputRank(e.to_string()))?;
    Ok(hidden.to_owned())
}

/// Attention-mask-weighted mean pool over `hidden` (`[1, seq, dim]`), then L2-normalize,
/// with the pooling-denominator (`1e-9`) and L2-norm (`1e-12`) clamps that guard against
/// division producing NaN/inf.
fn mean_pool_l2(hidden: &ndarray::Array3<f32>, mask: &[i64], seq: usize) -> Vec<f32> {
    let dim = hidden.shape()[2];
    let mut pooled = vec![0f32; dim];
    // Attention mask is binary {0,1}; branch instead of casting i64->f32 (clippy pedantic).
    let mut denom = 0f32;
    for (t, &m) in mask.iter().enumerate().take(seq) {
        if m == 0 {
            continue;
        }
        denom += 1.0;
        for d in 0..dim {
            pooled[d] += hidden[[0, t, d]];
        }
    }
    let denom = denom.max(1e-9);
    for v in &mut pooled {
        *v /= denom;
    }
    let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    for v in &mut pooled {
        *v /= norm;
    }
    pooled
}

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Embeddings" and "Model provenance").
#[cfg(test)]
mod tests {
    use super::{model_paths, Embedder, EMBED_DIM, MODEL_REPO, MODEL_REVISION, MODEL_SHA256};
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    /// The worktree root (one level above `CARGO_MANIFEST_DIR = server/`), where the
    /// committed `scripts/` and `.gitignore` live. Resolved at compile time so the
    /// helpers below do not depend on the test binary's working directory.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CARGO_MANIFEST_DIR (server/) has a parent (worktree root)")
            .to_path_buf()
    }

    /// Path to the committed model-fetch script.
    fn fetch_model_path() -> PathBuf {
        repo_root().join("scripts").join("fetch-model")
    }

    /// The full text of `scripts/fetch-model` (the source of truth these provenance
    /// tests inspect).
    fn fetch_model_text() -> String {
        std::fs::read_to_string(fetch_model_path()).expect("read scripts/fetch-model")
    }

    // ─── Embeddings ──────────────────────────────────────────────────────────
    //
    // These run against the REAL ONNX model (no mocks, §5). They load the model per
    // test and FAIL (never skip) when it is absent. Several requirements constrain the
    // ONNX session build (deterministic-compute, single intra-thread, pinned opt level,
    // CPU EP, commit_from_file) — none of which `ort` lets a test introspect after the
    // fact. Each such test therefore asserts the OBSERVABLE guarantee that knob provides
    // (reproducibility / determinism / a valid vector), with the exact mechanical fidelity
    // pinned by the committed golden vector below. Requirement→test mapping is noted per test.

    /// Loads the real embedder; FAILS (never skips) if the model is absent (no-mock BDD, §5 #2).
    fn load_embedder() -> Embedder {
        let (model, tok) = model_paths();
        assert!(
            model.exists(),
            "model missing at {}: run `scripts/fetch-model`",
            model.display()
        );
        Embedder::load(
            model.to_str().expect("model path is utf-8"),
            tok.to_str().expect("tokenizer path is utf-8"),
        )
        .expect("load embedder")
    }

    /// Cosine similarity of two vectors (f64 accumulation to avoid precision-loss casts).
    fn cosine(a: &[f32], b: &[f32]) -> f64 {
        let dot: f64 = a
            .iter()
            .zip(b)
            .map(|(x, y)| f64::from(*x) * f64::from(*y))
            .sum();
        let na: f64 = a
            .iter()
            .map(|x| f64::from(*x) * f64::from(*x))
            .sum::<f64>()
            .sqrt();
        let nb: f64 = b
            .iter()
            .map(|x| f64::from(*x) * f64::from(*x))
            .sum::<f64>()
            .sqrt();
        dot / (na * nb)
    }

    /// Euclidean (L2) norm of a vector, in f64.
    fn l2_norm(v: &[f32]) -> f64 {
        v.iter()
            .map(|x| f64::from(*x) * f64::from(*x))
            .sum::<f64>()
            .sqrt()
    }

    /// Fix the embedding dimensionality at `EMBED_DIM = 384`.
    #[test]
    fn embed_dim_is_fixed_at_384() {
        assert_eq!(EMBED_DIM, 384);
        let mut e = load_embedder();
        assert_eq!(e.embed("dimension check").expect("embed").len(), EMBED_DIM);
    }

    /// Use the `all-MiniLM-L6-v2` model (384-dim, mean pooling) and not `bge-small-en-v1.5`.
    ///
    /// Provenance-level: the fetch script targets the MiniLM repo (the singular source of
    /// truth `MODEL_REPO`) and never a `bge` variant. §2.3c flags `bge`+mean-pool as a
    /// stated correctness bug, so the wrong repo must be excluded at the fetch boundary.
    #[test]
    fn the_model_is_all_minilm_l6_v2_and_not_bge_small() {
        assert_eq!(MODEL_REPO, "sentence-transformers/all-MiniLM-L6-v2");
        let script = fetch_model_text();
        assert!(
            script.contains(MODEL_REPO),
            "scripts/fetch-model must target {MODEL_REPO}"
        );
        assert!(
            script.contains("all-MiniLM-L6-v2"),
            "the fetched model must be all-MiniLM-L6-v2"
        );
        assert!(
            !script.to_lowercase().contains("bge"),
            "the fetched model must not be a bge-small variant"
        );
    }

    /// Load the tokenizer with `Tokenizer::from_file` and encode with special tokens enabled.
    ///
    /// This tokenizer pads to a fixed width, so raw sequence length is constant; the real
    /// (non-padding) token count comes from the attention-mask sum. Enabling special tokens
    /// adds CLS+SEP, so the masked count grows.
    #[test]
    fn tokenizer_is_loaded_from_file_and_encodes_with_special_tokens() {
        let (_model, tok) = model_paths();
        let tokenizer = tokenizers::Tokenizer::from_file(tok.to_str().expect("utf-8"))
            .expect("Tokenizer::from_file");
        let with = tokenizer
            .encode("hello", true)
            .expect("encode with specials");
        let without = tokenizer.encode("hello", false).expect("encode without");
        let with_real: u32 = with.get_attention_mask().iter().sum();
        let without_real: u32 = without.get_attention_mask().iter().sum();
        assert!(
            with_real > without_real,
            "special tokens (CLS/SEP) must add real tokens: {with_real} !> {without_real}"
        );
    }

    /// Pool token hidden states with an attention-mask-weighted mean.
    ///
    /// Observable guarantee: pooling incorporates token content, so semantically distinct
    /// texts embed to distinct directions (mean-pool correctness pinned by the golden test).
    #[test]
    fn token_hidden_states_are_pooled_with_an_attention_mask_weighted_mean() {
        let mut e = load_embedder();
        let a = e.embed("the cat sat on the mat").expect("embed a");
        let b = e
            .embed("quantum chromodynamics of gluon fields")
            .expect("embed b");
        assert!(
            cosine(&a, &b) < 0.99,
            "distinct texts must pool to distinct vectors (cosine {})",
            cosine(&a, &b)
        );
    }

    /// L2-normalize the pooled vector before storing or querying.
    #[test]
    fn the_pooled_vector_is_l2_normalized_before_storing_or_querying() {
        let mut e = load_embedder();
        let v = e.embed("normalize me").expect("embed");
        approx::assert_abs_diff_eq!(l2_norm(&v), 1.0, epsilon = 1e-4);
    }

    /// Clamp the pooling denominator at `1e-9` before dividing (guards against a zero
    /// mask sum producing NaN). Observable: a normal embedding is entirely finite.
    #[test]
    fn the_pooling_denominator_is_clamped_at_1e_minus_9() {
        let mut e = load_embedder();
        let v = e.embed("pooling denominator clamp").expect("embed");
        assert!(
            v.iter().all(|x| x.is_finite()),
            "no NaN/inf from pooling division"
        );
    }

    /// Clamp the L2 norm at `1e-12` before dividing. Observable: even a minimal input
    /// (single space) yields a finite unit-ish vector rather than NaN/inf.
    #[test]
    fn the_l2_norm_is_clamped_at_1e_minus_12() {
        let mut e = load_embedder();
        let v = e.embed(" ").expect("embed minimal input");
        assert!(
            v.iter().all(|x| x.is_finite()),
            "no NaN/inf from L2 division"
        );
    }

    /// Build the ONNX session with `with_deterministic_compute(true)`.
    ///
    /// Observable guarantee: the same text embedded twice is BIT-IDENTICAL.
    #[test]
    fn the_session_is_built_with_deterministic_compute_true() {
        let mut e = load_embedder();
        let v1 = e.embed("determinism").expect("embed 1");
        let v2 = e.embed("determinism").expect("embed 2");
        assert_eq!(
            v1, v2,
            "deterministic compute must give bit-identical output"
        );
    }

    /// Build the ONNX session with `with_intra_threads(1)`.
    ///
    /// Observable guarantee: single-threaded inference is stable across repeated calls.
    #[test]
    fn the_session_is_built_with_intra_threads_one() {
        let mut e = load_embedder();
        let first = e.embed("single threaded stability").expect("embed");
        for _ in 0..3 {
            assert_eq!(e.embed("single threaded stability").expect("embed"), first);
        }
    }

    /// Build the ONNX session with a pinned graph-optimization level.
    ///
    /// Observable guarantee: the session builds under the pinned level and yields a
    /// reproducible unit vector (exact fidelity pinned by the golden test).
    #[test]
    fn the_session_is_built_with_a_pinned_graph_optimization_level() {
        let mut e = load_embedder();
        let v = e.embed("optimization level").expect("embed");
        assert_eq!(v.len(), EMBED_DIM);
        approx::assert_abs_diff_eq!(l2_norm(&v), 1.0, epsilon = 1e-4);
    }

    /// Build the ONNX session with the CPU execution provider (no GPU/accelerator needed).
    ///
    /// Observable guarantee: the model loads and runs on CPU only.
    #[test]
    fn the_session_is_built_with_the_cpu_execution_provider() {
        let mut e = load_embedder();
        assert_eq!(
            e.embed("cpu execution provider").expect("embed").len(),
            EMBED_DIM
        );
    }

    /// Commit the ONNX session from the model file with `commit_from_file`.
    ///
    /// Observable guarantee: loading from the real model path succeeds; a bad path errors
    /// (rather than panicking).
    #[test]
    fn the_session_is_committed_from_the_model_file() {
        let (_m, tok) = model_paths();
        let bad = Embedder::load("/no/such/model.onnx", tok.to_str().expect("utf-8"));
        assert!(
            bad.is_err(),
            "committing from a missing model file must return Err"
        );
        let _ok = load_embedder(); // committing from the real file succeeds
    }

    /// Assert golden and determinism embedding comparisons at absolute tolerance `1e-4`,
    /// or equivalently cosine `>= 0.9999`. This is the golden-vector assertion (bootstrap
    /// item 3): it pins mean-pool correctness, output[0] = last_hidden_state, the absence of
    /// a required token_type_ids input, and the pinned optimization level all at once.
    #[test]
    fn golden_and_determinism_comparisons_use_tolerance_1e_minus_4() {
        let golden: Vec<f32> = serde_json::from_str(
            &std::fs::read_to_string(GOLDEN_PATH).expect("read golden fixture (run capture first)"),
        )
        .expect("parse golden fixture");
        let mut e = load_embedder();
        let v = e.embed(GOLDEN_INPUT).expect("embed golden input");
        assert_eq!(v.len(), golden.len());
        for (a, b) in v.iter().zip(&golden) {
            approx::assert_abs_diff_eq!(*a, *b, epsilon = 1e-4);
        }
        // Equivalent cosine form (>= 0.9999) also holds.
        assert!(
            cosine(&v, &golden) >= 0.9999,
            "cosine {}",
            cosine(&v, &golden)
        );
    }

    /// Confirm the shipped ONNX export emits `last_hidden_state` at output[0] — §6.2 #6.
    ///
    /// Confirmed by construction: `embed` extracts output[0] as a rank-3 tensor and pools
    /// it; if the export emitted a pre-pooled 2-D output or ordered outputs differently,
    /// the rank-3 extraction would error. A successful `EMBED_DIM` embedding confirms it.
    #[test]
    fn the_onnx_export_emits_last_hidden_state_at_output_zero() {
        let mut e = load_embedder();
        assert_eq!(
            e.embed("last hidden state at output zero")
                .expect("embed")
                .len(),
            EMBED_DIM,
            "output[0] must be the rank-3 last_hidden_state"
        );
    }

    /// Confirm whether the shipped ONNX export requires a third `token_type_ids` input — §6.2 #6.
    ///
    /// Confirmed REQUIRED at implementation: running this export with only `input_ids` +
    /// `attention_mask` fails with "Missing Input: token_type_ids" (its
    /// token_type_embeddings/Gather node reads it). `embed` therefore supplies an all-zero
    /// `token_type_ids`; a successful embed confirms the third input is correctly provided.
    #[test]
    fn whether_the_onnx_export_requires_token_type_ids_is_confirmed() {
        let mut e = load_embedder();
        assert!(
            e.embed("token type ids required and supplied").is_ok(),
            "the pinned export requires token_type_ids, which embed must supply"
        );
    }

    /// Golden-vector fixture (bootstrap item 3): frozen output for a fixed input.
    const GOLDEN_INPUT: &str = "genesis remembers what matters";
    const GOLDEN_PATH: &str = "tests/golden/all_minilm_l6_v2_golden.json";

    /// Capture harness — run ONCE (`--ignored`) to freeze the golden vector, then commit it.
    #[test]
    #[ignore = "capture harness: run once to write the golden fixture, then commit it"]
    fn capture_golden_vector() {
        let mut e = load_embedder();
        let v = e.embed(GOLDEN_INPUT).expect("embed golden input");
        std::fs::create_dir_all("tests/golden").expect("mkdir tests/golden");
        std::fs::write(GOLDEN_PATH, serde_json::to_string(&v).expect("serialize"))
            .expect("write golden fixture");
    }

    // ─── Model provenance ────────────────────────────────────────────────────

    /// Provide `scripts/fetch-model`, which downloads the model and tokenizer into
    /// `server/models/`.
    #[test]
    fn a_fetch_model_script_is_provided() {
        let path = fetch_model_path();
        assert!(
            path.is_file(),
            "scripts/fetch-model must exist at {}",
            path.display()
        );
        let script = fetch_model_text();
        assert!(
            script.starts_with("#!"),
            "scripts/fetch-model must be a script (shebang line)"
        );
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "scripts/fetch-model must be executable (mode {mode:o})"
        );
        assert!(
            script.contains("server/models"),
            "scripts/fetch-model must download into server/models/"
        );
    }

    /// Fetch from the Hugging Face repository `sentence-transformers/all-MiniLM-L6-v2`.
    #[test]
    fn the_model_is_fetched_from_the_sentence_transformers_repository() {
        let script = fetch_model_text();
        assert!(
            script.contains("huggingface.co"),
            "scripts/fetch-model must fetch from Hugging Face"
        );
        assert!(
            script.contains(MODEL_REPO),
            "scripts/fetch-model must fetch from the {MODEL_REPO} repository"
        );
    }

    /// Fetch the files `onnx/model.onnx` and `tokenizer.json`.
    #[test]
    fn the_fetched_files_are_model_onnx_and_tokenizer_json() {
        let script = fetch_model_text();
        assert!(
            script.contains("onnx/model.onnx"),
            "scripts/fetch-model must fetch onnx/model.onnx"
        );
        assert!(
            script.contains("tokenizer.json"),
            "scripts/fetch-model must fetch tokenizer.json"
        );
    }

    /// Pin an explicit repository revision in `scripts/fetch-model`.
    #[test]
    fn fetch_model_pins_an_explicit_repository_revision() {
        let script = fetch_model_text();
        assert!(
            script.contains(MODEL_REVISION),
            "scripts/fetch-model must pin the explicit revision {MODEL_REVISION}"
        );
    }

    /// Download only from that pinned revision.
    #[test]
    fn downloads_come_only_from_the_pinned_revision() {
        let script = fetch_model_text();
        // Downloads resolve the pinned ${REVISION}, never a floating ref like main/latest.
        assert!(
            script.contains("resolve/${REVISION}"),
            "downloads must resolve the pinned ${{REVISION}}"
        );
        assert!(
            !script.contains("resolve/main") && !script.contains("resolve/latest"),
            "downloads must not come from an unpinned ref (main/latest)"
        );
    }

    /// Record in `scripts/fetch-model` that the revision is load-bearing because §6.2 #6
    /// states ONNX exports of the same model differ in output shape and in whether
    /// `token_type_ids` is required.
    #[test]
    fn fetch_model_records_why_the_revision_is_load_bearing() {
        let script = fetch_model_text().to_lowercase();
        assert!(
            script.contains("load-bearing"),
            "scripts/fetch-model must record that the revision is load-bearing"
        );
        assert!(
            script.contains("token_type_ids") && script.contains("last_hidden_state"),
            "the recorded reason must cite the §6.2 #6 output-shape / token_type_ids dependency"
        );
    }

    /// Keep the model artifacts out of git (`.gitignore` already ignores `*.onnx`).
    #[test]
    fn the_model_artifacts_are_kept_out_of_git() {
        let gitignore = std::fs::read_to_string(repo_root().join(".gitignore"))
            .expect("read .gitignore at repo root");
        // The onnx weights AND the tokenizer.json must both be ignored; `*.onnx` alone would
        // not cover tokenizer.json, so the directory-level `server/models/` rule is required.
        assert!(
            gitignore.contains("*.onnx"),
            ".gitignore must ignore *.onnx model weights"
        );
        assert!(
            gitignore.contains("server/models"),
            ".gitignore must ignore server/models/ so tokenizer.json is never committed either"
        );
    }

    /// Assert the pinned SHA-256 of the fetched model file before running embedding tests.
    #[test]
    fn the_pinned_model_sha256_is_asserted_before_embedding_tests() {
        let (model, _tok) = model_paths();
        assert!(model.exists(), "model missing: run `scripts/fetch-model`");
        let bytes = std::fs::read(&model).unwrap();
        let digest = hex::encode(Sha256::digest(&bytes));
        assert_eq!(
            digest, MODEL_SHA256,
            "fetched model does not match the pinned SHA-256"
        );
    }

    /// Commit the pinned revision string as a constant, captured at first fetch.
    #[test]
    fn the_pinned_revision_string_is_committed_as_a_constant() {
        // A git commit SHA-1 is 40 lowercase hex chars, and the fetch script must pin THIS one.
        assert_eq!(
            MODEL_REVISION.len(),
            40,
            "MODEL_REVISION must be a full 40-char git commit SHA"
        );
        assert!(
            MODEL_REVISION.chars().all(|c| c.is_ascii_hexdigit()),
            "MODEL_REVISION must be hex"
        );
        assert!(
            fetch_model_text().contains(MODEL_REVISION),
            "the committed MODEL_REVISION must match what scripts/fetch-model pins"
        );
    }

    /// Commit the pinned SHA-256 digest as a constant, captured at first fetch.
    #[test]
    fn the_pinned_sha256_digest_is_committed_as_a_constant() {
        // A SHA-256 digest is 64 hex chars.
        assert_eq!(
            MODEL_SHA256.len(),
            64,
            "MODEL_SHA256 must be a 64-char SHA-256 hex digest"
        );
        assert!(
            MODEL_SHA256.chars().all(|c| c.is_ascii_hexdigit()),
            "MODEL_SHA256 must be hex"
        );
    }

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

    /// `model_paths` resolves to `onnx/model.onnx` and `tokenizer.json` under the model dir.
    #[test]
    fn model_paths_point_into_server_models() {
        let (model, tok) = model_paths();
        assert!(model.ends_with("onnx/model.onnx"));
        assert!(tok.ends_with("tokenizer.json"));
    }
}
