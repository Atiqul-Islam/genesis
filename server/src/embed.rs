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

/// A local sentence embedder: an ONNX Runtime session plus its tokenizer.
#[derive(Debug)]
pub struct Embedder {
    // TODO(spec): ort::session::Session + tokenizers::Tokenizer, pinned by SHA-256 fixture.
}

impl Embedder {
    /// Loads the ONNX model and tokenizer from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the model or tokenizer cannot be loaded.
    pub fn load(_model_path: &str, _tokenizer_path: &str) -> Result<Self> {
        unimplemented!("Implement via TDD — ort Session::builder + Tokenizer::from_file")
    }

    /// Embeds `text` into an L2-normalized `EMBED_DIM`-length vector.
    ///
    /// # Errors
    ///
    /// Returns an error if tokenization or the ONNX inference run fails.
    pub fn embed(&mut self, _text: &str) -> Result<Vec<f32>> {
        unimplemented!("Implement via TDD — encode → run → mean-pool → L2-normalize (§2.3c)")
    }
}

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Embeddings" and "Model provenance").
#[cfg(test)]
mod tests {
    use super::{model_paths, MODEL_REPO, MODEL_REVISION, MODEL_SHA256};
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

    /// Fix the embedding dimensionality at `EMBED_DIM = 384`.
    #[test]
    fn embed_dim_is_fixed_at_384() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Fix the embedding dimensionality at EMBED_DIM = 384");
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
    #[test]
    fn tokenizer_is_loaded_from_file_and_encodes_with_special_tokens() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Load the tokenizer with Tokenizer::from_file and encode with special tokens enabled"
        );
    }

    /// Pool token hidden states with an attention-mask-weighted mean.
    #[test]
    fn token_hidden_states_are_pooled_with_an_attention_mask_weighted_mean() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Pool token hidden states with an attention-mask-weighted mean"
        );
    }

    /// L2-normalize the pooled vector before storing or querying.
    #[test]
    fn the_pooled_vector_is_l2_normalized_before_storing_or_querying() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — L2-normalize the pooled vector before storing or querying"
        );
    }

    /// Clamp the pooling denominator at `1e-9` before dividing.
    #[test]
    fn the_pooling_denominator_is_clamped_at_1e_minus_9() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Clamp the pooling denominator at 1e-9 before dividing");
    }

    /// Clamp the L2 norm at `1e-12` before dividing.
    #[test]
    fn the_l2_norm_is_clamped_at_1e_minus_12() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Clamp the L2 norm at 1e-12 before dividing");
    }

    /// Build the ONNX session with `with_deterministic_compute(true)`.
    #[test]
    fn the_session_is_built_with_deterministic_compute_true() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Build the ONNX session with with_deterministic_compute(true)"
        );
    }

    /// Build the ONNX session with `with_intra_threads(1)`.
    #[test]
    fn the_session_is_built_with_intra_threads_one() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Build the ONNX session with with_intra_threads(1)");
    }

    /// Build the ONNX session with a pinned graph-optimization level.
    #[test]
    fn the_session_is_built_with_a_pinned_graph_optimization_level() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Build the ONNX session with a pinned graph-optimization level"
        );
    }

    /// Build the ONNX session with the CPU execution provider.
    #[test]
    fn the_session_is_built_with_the_cpu_execution_provider() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Build the ONNX session with the CPU execution provider"
        );
    }

    /// Commit the ONNX session from the model file with `commit_from_file`.
    #[test]
    fn the_session_is_committed_from_the_model_file() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Commit the ONNX session from the model file with commit_from_file"
        );
    }

    /// Assert golden and determinism embedding comparisons at absolute tolerance `1e-4`,
    /// or equivalently cosine `>= 0.9999`.
    #[test]
    fn golden_and_determinism_comparisons_use_tolerance_1e_minus_4() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Assert golden and determinism embedding comparisons at absolute tolerance 1e-4, or equivalently cosine >= 0.9999"
        );
    }

    /// Confirm at implementation whether the shipped ONNX export emits `last_hidden_state`
    /// at output[0] — §6.2 #6.
    #[test]
    fn the_onnx_export_emits_last_hidden_state_at_output_zero() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Confirm at implementation whether the shipped ONNX export emits last_hidden_state at output[0] (§6.2 #6)"
        );
    }

    /// Confirm at implementation whether the shipped ONNX export requires a third
    /// `token_type_ids` input (`Session::inputs` dump) — §6.2 #6.
    #[test]
    fn whether_the_onnx_export_requires_token_type_ids_is_confirmed() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Confirm at implementation whether the shipped ONNX export requires a third token_type_ids input (Session::inputs dump) (§6.2 #6)"
        );
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
