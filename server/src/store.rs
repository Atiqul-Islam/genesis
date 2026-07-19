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

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Database location" and "Vector store").
#[cfg(test)]
mod tests {
    // ─── Database location ───────────────────────────────────────────────────

    /// Read the SQLite database path from the environment variable `GENESIS_MEMORY_DB`.
    #[test]
    fn db_path_is_read_from_the_genesis_memory_db_env_var() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Read the SQLite database path from the environment variable GENESIS_MEMORY_DB"
        );
    }

    /// Fall back to `genesis-memory.db` in the process working directory when
    /// `GENESIS_MEMORY_DB` is unset.
    #[test]
    fn db_path_falls_back_to_genesis_memory_db_in_the_working_directory() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Fall back to genesis-memory.db in the process working directory when GENESIS_MEMORY_DB is unset"
        );
    }

    // ─── Vector store ────────────────────────────────────────────────────────

    /// Register the `sqlite-vec` extension with `sqlite3_auto_extension(sqlite3_vec_init)`
    /// before opening the connection.
    #[test]
    fn sqlite_vec_is_registered_before_the_connection_is_opened() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Register the sqlite-vec extension with sqlite3_auto_extension(sqlite3_vec_init) before opening the connection"
        );
    }

    /// Create the vector table with
    /// `CREATE VIRTUAL TABLE vec_items USING vec0(embedding float[384])`.
    #[test]
    fn vec_items_is_created_as_vec0_embedding_float_384() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Create the vector table with CREATE VIRTUAL TABLE vec_items USING vec0(embedding float[384])"
        );
    }

    /// Create a `memories` table.
    #[test]
    fn a_memories_table_is_created() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Create a `memories` table");
    }

    /// Give `memories` the columns `id`, `agent_id`, `text`, `created_at`, `last_used_at`,
    /// `use_count`, `base_score`, `superseded_by`.
    #[test]
    fn memories_has_the_eight_specified_columns() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Give memories the columns id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by"
        );
    }

    /// Declare `memories.id` as `INTEGER PRIMARY KEY` so SQLite assigns it.
    #[test]
    fn memories_id_is_an_integer_primary_key() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Declare memories.id as INTEGER PRIMARY KEY so SQLite assigns it"
        );
    }

    /// Make `memories.superseded_by` nullable and reference `memories.id`.
    #[test]
    fn memories_superseded_by_is_nullable_and_references_memories_id() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Make memories.superseded_by nullable and reference memories.id"
        );
    }

    /// Keep `vec_items.rowid` equal to `memories.id`.
    #[test]
    fn vec_items_rowid_equals_memories_id() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Keep vec_items.rowid equal to memories.id");
    }

    /// Read the assigned id with `last_insert_rowid()` immediately after inserting the
    /// `memories` row.
    #[test]
    fn the_assigned_id_is_read_with_last_insert_rowid() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Read the assigned id with last_insert_rowid() immediately after inserting the memories row"
        );
    }

    /// Insert the embedding into `vec_items` under that same rowid, so the
    /// `vec_items.rowid == memories.id` invariant is established in exactly one place.
    #[test]
    fn the_embedding_is_inserted_under_that_same_rowid() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Insert the embedding into vec_items under that same rowid, so the vec_items.rowid == memories.id invariant is established in exactly one place"
        );
    }

    /// Return the assigned `i64` from `VectorStore::insert`.
    #[test]
    fn insert_returns_the_assigned_i64() {
        // TODO: Implement
        unimplemented!("Implement via TDD — Return the assigned i64 from VectorStore::insert");
    }

    /// Do not accept a caller-supplied `id` parameter in `VectorStore::insert`.
    #[test]
    fn insert_accepts_no_caller_supplied_id() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Do not accept a caller-supplied id parameter in VectorStore::insert"
        );
    }

    /// Pass embeddings to SQLite as bytes via `bytemuck::cast_slice::<f32, u8>`.
    #[test]
    fn embeddings_are_passed_to_sqlite_via_bytemuck_cast_slice() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Pass embeddings to SQLite as bytes via bytemuck::cast_slice::<f32, u8>"
        );
    }

    /// Issue KNN as
    /// `SELECT rowid, distance FROM vec_items WHERE embedding MATCH ?1 ORDER BY distance LIMIT k`.
    #[test]
    fn knn_uses_the_verified_match_order_by_distance_limit_form() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Issue KNN as SELECT rowid, distance FROM vec_items WHERE embedding MATCH ?1 ORDER BY distance LIMIT k"
        );
    }

    /// Keep `vec0`'s default L2 distance metric (no `distance_metric=cosine` DDL).
    #[test]
    fn vec0_keeps_its_default_l2_distance_metric() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Keep vec0's default L2 distance metric (no distance_metric=cosine DDL)"
        );
    }

    /// Take `agent_id` in `VectorStore::insert` and record the row under it.
    #[test]
    fn insert_takes_agent_id_and_records_the_row_under_it() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Take agent_id in VectorStore::insert and record the row under it"
        );
    }

    /// Take `agent_id` in `VectorStore::knn` and restrict results to that agent.
    #[test]
    fn knn_takes_agent_id_and_restricts_results_to_that_agent() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Take agent_id in VectorStore::knn and restrict results to that agent"
        );
    }

    /// Exclude rows with a non-null `superseded_by` from `recall` results.
    #[test]
    fn rows_with_a_non_null_superseded_by_are_excluded_from_recall() {
        // TODO: Implement
        unimplemented!(
            "Implement via TDD — Exclude rows with a non-null superseded_by from recall results"
        );
    }
}
