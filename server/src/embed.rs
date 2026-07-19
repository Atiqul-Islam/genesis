//! Local ONNX embeddings — `ort` (ONNX Runtime) + `tokenizers`.
//!
//! Loads a small sentence model (primary: `all-MiniLM-L6-v2`, 384-dim, **mean** pooling)
//! and turns text into an L2-normalized vector. Pooling must match the model: MiniLM =
//! mean pool; if `bge-small-en-v1.5` is chosen instead, switch to CLS pooling
//! (`pooled[d] = hidden[[0,0,d]]`) and prepend the query instruction. Determinism knobs
//! for golden-vector tests: `with_deterministic_compute(true)`, `with_intra_threads(1)`.
//! See `docs/SPEC_FORGE_RUST_UPDATE.md` §2.3c + §5 best-practice #3.

use anyhow::Result;

/// Embedding vector dimensionality (all-MiniLM-L6-v2 / bge-small-en-v1.5 = 384).
pub const EMBED_DIM: usize = 384;

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
    // ─── Embeddings ──────────────────────────────────────────────────────────

    /// Fix the embedding dimensionality at `EMBED_DIM = 384`.
    #[test]
    fn embed_dim_is_fixed_at_384() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Fix the embedding dimensionality at EMBED_DIM = 384");
    }

    /// Use the `all-MiniLM-L6-v2` model (384-dim, mean pooling) and not `bge-small-en-v1.5`.
    #[test]
    fn the_model_is_all_minilm_l6_v2_and_not_bge_small() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Use the all-MiniLM-L6-v2 model (384-dim, mean pooling) and not bge-small-en-v1.5"
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
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Provide scripts/fetch-model, which downloads the model and tokenizer into server/models/"
        );
    }

    /// Fetch from the Hugging Face repository `sentence-transformers/all-MiniLM-L6-v2`.
    #[test]
    fn the_model_is_fetched_from_the_sentence_transformers_repository() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Fetch from the Hugging Face repository sentence-transformers/all-MiniLM-L6-v2"
        );
    }

    /// Fetch the files `onnx/model.onnx` and `tokenizer.json`.
    #[test]
    fn the_fetched_files_are_model_onnx_and_tokenizer_json() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Fetch the files onnx/model.onnx and tokenizer.json");
    }

    /// Pin an explicit repository revision in `scripts/fetch-model`.
    #[test]
    fn fetch_model_pins_an_explicit_repository_revision() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Pin an explicit repository revision in scripts/fetch-model"
        );
    }

    /// Download only from that pinned revision.
    #[test]
    fn downloads_come_only_from_the_pinned_revision() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Download only from that pinned revision");
    }

    /// Record in `scripts/fetch-model` that the revision is load-bearing because §6.2 #6
    /// states ONNX exports of the same model differ in output shape and in whether
    /// `token_type_ids` is required.
    #[test]
    fn fetch_model_records_why_the_revision_is_load_bearing() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Record in scripts/fetch-model that the revision is load-bearing because §6.2 #6 states ONNX exports of the same model differ in output shape (pooled output vs last_hidden_state) and in whether token_type_ids is required"
        );
    }

    /// Keep the model artifacts out of git (`.gitignore` already ignores `*.onnx`).
    #[test]
    fn the_model_artifacts_are_kept_out_of_git() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Keep the model artifacts out of git (.gitignore already ignores *.onnx)"
        );
    }

    /// Assert the pinned SHA-256 of the fetched model file before running embedding tests.
    #[test]
    fn the_pinned_model_sha256_is_asserted_before_embedding_tests() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Assert the pinned SHA-256 of the fetched model file before running embedding tests"
        );
    }

    /// Commit the pinned revision string as a constant, captured at first fetch.
    #[test]
    fn the_pinned_revision_string_is_committed_as_a_constant() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Commit the pinned revision string as a constant, captured at first fetch"
        );
    }

    /// Commit the pinned SHA-256 digest as a constant, captured at first fetch.
    #[test]
    fn the_pinned_sha256_digest_is_committed_as_a_constant() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Commit the pinned SHA-256 digest as a constant, captured at first fetch"
        );
    }

    /// Make embedding tests fail — never skip — with a message directing the developer to
    /// run `scripts/fetch-model` when the model file is absent.
    #[test]
    fn embedding_tests_fail_rather_than_skip_when_the_model_is_absent() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Make embedding tests fail, never skip, with a message directing the developer to run scripts/fetch-model when the model file is absent"
        );
    }
}
