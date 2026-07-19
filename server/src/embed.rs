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
