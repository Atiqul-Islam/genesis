//! Vector store — SQLite + `sqlite-vec` KNN over per-agent memory embeddings.
//!
//! The store owns a `rusqlite::Connection` with the `sqlite-vec` extension registered
//! (`sqlite3_auto_extension(sqlite3_vec_init)`) and a `vec0(embedding float[384])`
//! virtual table. KNN uses the verified form
//! `WHERE embedding MATCH ?1 ORDER BY distance LIMIT k` (default L2 distance; because
//! embeddings are L2-normalized, L2 order == cosine order). See
//! `docs/SPEC_FORGE_RUST_UPDATE.md` §2.3b.

use anyhow::Result;

/// A per-agent vector store backed by SQLite + `sqlite-vec`.
#[derive(Debug)]
pub struct VectorStore {
    // TODO(spec): rusqlite::Connection (bundled) with the sqlite-vec extension registered.
}

impl VectorStore {
    /// Opens (or creates) the store at `path`, registering the `sqlite-vec` extension
    /// and ensuring the `memories` table + `vec_items` virtual table exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or the extension fails to load.
    pub fn open(_path: &str) -> Result<Self> {
        unimplemented!("Implement via TDD — sqlite3_auto_extension + vec0 DDL (§2.3b)")
    }

    /// Inserts a memory row and its embedding (rowid shared with `memories.id`).
    ///
    /// # Errors
    ///
    /// Returns an error on a SQL failure or an embedding-dimension mismatch
    /// (dimension mismatch must return `Err`, never panic — §5 best-practice #4).
    pub fn insert(&mut self, _id: i64, _text: &str, _embedding: &[f32]) -> Result<()> {
        unimplemented!("Implement via TDD")
    }

    /// Returns the `k` nearest memories to `query` as `(id, distance)`, ordered by
    /// ascending distance (nearest first).
    ///
    /// # Errors
    ///
    /// Returns an error on a SQL failure or an embedding-dimension mismatch.
    pub fn knn(&self, _query: &[f32], _k: usize) -> Result<Vec<(i64, f64)>> {
        unimplemented!("Implement via TDD — WHERE embedding MATCH ?1 ORDER BY distance LIMIT k")
    }
}
