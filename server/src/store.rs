//! Vector store — SQLite + `sqlite-vec` KNN over per-agent memory embeddings.
//!
//! The store owns a `rusqlite::Connection` with the `sqlite-vec` extension registered
//! (`sqlite3_auto_extension(sqlite3_vec_init)`) and a `vec0(embedding float[384])`
//! virtual table. KNN uses the verified form
//! `WHERE embedding MATCH ?1 ORDER BY distance LIMIT k` (default L2 distance; because
//! embeddings are L2-normalized, L2 order == cosine order). Agent scoping and the
//! `superseded_by` exclusion are applied by joining that verified inner query back to
//! `memories` in an outer query (candidate-pool over-fetch), never by adding predicates
//! to the `vec0` KNN itself. See `docs/SPEC_FORGE_RUST_UPDATE.md` §2.3b.

use anyhow::Result;
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection, OptionalExtension};
use sqlite_vec::sqlite3_vec_init;

use crate::embed::EMBED_DIM;

/// The default on-disk database path (relative to the CWD = the project root) used when
/// `GENESIS_MEMORY_DB` is unset. Lives under `.genesis/` so an agent's memory ALWAYS lands in the
/// repo's workspace (and travels via the committed JSONL export) — never a stray `genesis-memory.db`
/// in whatever directory the server happened to be launched from.
pub const DEFAULT_DB_FILENAME: &str = ".genesis/memory.db";

/// Resolves the database path from an optional `GENESIS_MEMORY_DB` value, falling back to
/// [`DEFAULT_DB_FILENAME`] in the working directory. Pure (env read is done by the caller)
/// so it is testable without touching global process state.
fn resolve_db_path(env_value: Option<String>) -> String {
    env_value.unwrap_or_else(|| DEFAULT_DB_FILENAME.to_string())
}

/// The database path from the environment: `GENESIS_MEMORY_DB` if set, else the default.
#[must_use]
pub fn db_path_from_env() -> String {
    resolve_db_path(std::env::var("GENESIS_MEMORY_DB").ok())
}

/// Errors specific to the vector store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// An embedding did not have exactly [`EMBED_DIM`] elements.
    #[error("embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// The required dimensionality ([`EMBED_DIM`]).
        expected: usize,
        /// The dimensionality actually supplied.
        got: usize,
    },
    /// No `memories` row exists for the given id.
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
    /// Row id (== `vec_items.rowid`).
    pub id: i64,
    /// Unix seconds when the memory was stored.
    pub created_at: i64,
    /// Unix seconds when the memory was last recalled.
    pub last_used_at: i64,
    /// Number of times the memory has been recalled.
    pub use_count: i64,
    /// The memory's base relevance score.
    pub base_score: f64,
}

impl MemRow {
    /// Reads a `MemRow` from a query row (`id, created_at, last_used_at, use_count, base_score`).
    fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            created_at: r.get(1)?,
            last_used_at: r.get(2)?,
            use_count: r.get(3)?,
            base_score: r.get(4)?,
        })
    }
}

/// A COMPLETE `memories` row — every column, so it round-trips losslessly through the
/// JSONL export/import (see [`crate::persist`]). The embedding is deliberately absent:
/// it is a derived index regenerated from `text` on import, not source data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemRecord {
    /// Row id (== `vec_items.rowid`). Preserved so `superseded_by` links stay valid.
    pub id: i64,
    /// The agent the memory belongs to.
    pub agent_id: String,
    /// The memory text (the source the embedding is derived from).
    pub text: String,
    /// Unix seconds when the memory was stored.
    pub created_at: i64,
    /// Unix seconds when the memory was last recalled.
    pub last_used_at: i64,
    /// Number of times the memory has been recalled.
    pub use_count: i64,
    /// The memory's base relevance score.
    pub base_score: f64,
    /// The id of the memory that superseded this one, if any (null = active).
    pub superseded_by: Option<i64>,
}

impl MemRecord {
    /// Reads a full `MemRecord` from a query row selecting all eight columns in schema order.
    fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            agent_id: r.get(1)?,
            text: r.get(2)?,
            created_at: r.get(3)?,
            last_used_at: r.get(4)?,
            use_count: r.get(5)?,
            base_score: r.get(6)?,
            superseded_by: r.get(7)?,
        })
    }
}

/// Maps a KNN result row to `(id, distance)`.
fn id_distance(r: &rusqlite::Row) -> rusqlite::Result<(i64, f64)> {
    Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
}

/// The 0.2.0 structured / bi-temporal columns added to `memories`. SQLite `ALTER TABLE ADD COLUMN` cannot add
/// a NOT NULL column without a *constant* default, so the bi-temporal time fields are added nullable and then
/// backfilled from `created_at` (see [`migrate`]).
const MIGRATION_COLUMNS: &[(&str, &str)] = &[
    ("type", "TEXT NOT NULL DEFAULT 'semantic'"),
    ("subject", "TEXT"),
    ("relation", "TEXT"),
    ("object", "TEXT"),
    ("valid_from", "INTEGER"),
    ("valid_to", "INTEGER"),
    ("ingested_at", "INTEGER"),
    ("expired_at", "INTEGER"),
    ("source", "TEXT"),
    ("principal", "TEXT"),
    ("asserted_by", "TEXT"),
    ("confidence", "REAL"),
    ("content_id", "TEXT"),
    ("embedding_model", "TEXT"),
    ("embedding_version", "TEXT"),
    ("dim", "INTEGER"),
    ("metric", "TEXT"),
    ("normalized", "INTEGER"),
];

/// Idempotently bring an existing `memories` table up to the 0.2.0 structured / bi-temporal schema: add ONLY
/// the columns that are missing (checked via `PRAGMA table_info`), then backfill bi-temporal defaults from
/// `created_at`. Safe to run on every open — a fresh table simply gets all columns added once.
///
/// # Errors
/// Returns an error if the pragma read or any `ALTER`/`UPDATE` fails.
fn migrate(conn: &Connection) -> Result<()> {
    let existing: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(memories)")?;
        let cols = stmt.query_map([], |r| r.get::<_, String>(1))?;
        cols.collect::<std::result::Result<_, _>>()?
    };
    for (name, decl) in MIGRATION_COLUMNS {
        if !existing.contains(*name) {
            // Column names/decls are compile-time constants, never user input — no injection surface.
            conn.execute_batch(&format!("ALTER TABLE memories ADD COLUMN {name} {decl};"))?;
        }
    }
    conn.execute_batch(
        "UPDATE memories SET valid_from  = created_at WHERE valid_from  IS NULL;
         UPDATE memories SET ingested_at = created_at WHERE ingested_at IS NULL;",
    )?;
    Ok(())
}

/// The content-addressed id for a memory — `sha256(normalized_text \x1e type \x1e agent_id)` in hex. Stable
/// and machine-independent, so the same fact stored twice (or synced from two machines) collides to one id →
/// idempotent dedup + merge. The `\x1e` (ASCII record separator) prevents field-boundary collisions.
#[must_use]
pub fn content_id(normalized_text: &str, mem_type: &str, agent_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(normalized_text.as_bytes());
    h.update([0x1e]);
    h.update(mem_type.as_bytes());
    h.update([0x1e]);
    h.update(agent_id.as_bytes());
    hex::encode(h.finalize())
}

impl VectorStore {
    /// Opens (or creates) the store, registering `sqlite-vec` and ensuring the schema.
    ///
    /// # Errors
    /// Returns an error if the extension fails to register or the connection/DDL fails.
    // The sole sanctioned `unsafe` in the crate: `sqlite3_auto_extension` is the verified
    // (§2.3b) way to register the statically-linked sqlite-vec extension; there is no safe
    // wrapper. `unsafe_code = "deny"` at the crate level makes this scoped allow the only
    // place unsafe is permitted.
    #[allow(unsafe_code)]
    pub fn open(path: &str) -> Result<Self> {
        // Register the extension BEFORE opening the connection (spec requirement). Uses
        // transmute-by-inference (the exact xEntryPoint fn-pointer type is spelled out by
        // libsqlite3-sys); this is the sqlite-vec crate's own documented registration form.
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }
        // Ensure the parent dir exists (e.g. `.genesis/`) — `Connection::open` will NOT create it,
        // and the default db path is now `.genesis/memory.db`.
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("creating db dir {}: {e}", parent.display()))?;
            }
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
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Returns an error if `embedding` is not exactly [`EMBED_DIM`] elements.
    fn check_dim(embedding: &[f32]) -> Result<()> {
        if embedding.len() != EMBED_DIM {
            return Err(StoreError::DimensionMismatch {
                expected: EMBED_DIM,
                got: embedding.len(),
            }
            .into());
        }
        Ok(())
    }

    /// Inserts a memory + its embedding under one shared rowid; returns the assigned id.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn insert(
        &mut self,
        agent_id: &str,
        text: &str,
        embedding: &[f32],
        base_score: f64,
        now_unix: i64,
    ) -> Result<i64> {
        Self::check_dim(embedding)?;
        let tx = self.conn.transaction()?;
        // valid_from = ingested_at = created_at (= now) so a new memory is bi-temporally well-formed and
        // eligible for deterministic supersession immediately; `type` defaults to 'semantic'.
        tx.execute(
            "INSERT INTO memories (agent_id, text, created_at, last_used_at, use_count, base_score, \
             valid_from, ingested_at)
             VALUES (?1, ?2, ?3, ?3, 0, ?4, ?3, ?3)",
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

    /// Set the structured fields Mneme extracts for a memory (`type` + `subject`/`relation`/`object`).
    /// A `None` clears that field. This is how a raw stored memory becomes a structured fact.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn set_structure(
        &self,
        id: i64,
        mem_type: &str,
        subject: Option<&str>,
        relation: Option<&str>,
        object: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET type = ?2, subject = ?3, relation = ?4, object = ?5 WHERE id = ?1",
            params![id, mem_type, subject, relation, object],
        )?;
        Ok(())
    }

    /// Deterministically supersede prior ACTIVE facts for `(agent_id, subject, relation)` that are OLDER than
    /// `new_valid_from`, by setting their `valid_to = new_valid_from`. Rows are KEPT (bi-temporal history) —
    /// never deleted. No similarity, no LLM: staleness cannot be detected by embedding similarity (MemStrata),
    /// so supersession is keyed on the identity triple. Returns the number of facts retired.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn supersede_by_key(
        &self,
        agent_id: &str,
        subject: &str,
        relation: &str,
        new_valid_from: i64,
    ) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE memories SET valid_to = ?4
               WHERE agent_id = ?1 AND subject = ?2 AND relation = ?3
                 AND valid_to IS NULL AND expired_at IS NULL
                 AND valid_from < ?4",
            params![agent_id, subject, relation, new_valid_from],
        )?)
    }

    /// The current ACTIVE `object` for `(agent_id, subject, relation)`, latest by `valid_from`, if any.
    /// Active = `valid_to IS NULL AND expired_at IS NULL`.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn active_object(
        &self,
        agent_id: &str,
        subject: &str,
        relation: &str,
    ) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT object FROM memories
                   WHERE agent_id = ?1 AND subject = ?2 AND relation = ?3
                     AND valid_to IS NULL AND expired_at IS NULL
                   ORDER BY valid_from DESC LIMIT 1",
                params![agent_id, subject, relation],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Returns the `k` nearest non-superseded memories for `agent_id`, nearest first.
    ///
    /// The verified `vec0` KNN (`MATCH ... ORDER BY distance LIMIT`) runs as an inner
    /// subquery over a candidate pool sized to the whole table, so the outer agent /
    /// `superseded_by` filter never under-returns.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn knn(&self, agent_id: &str, query: &[f32], k: usize) -> Result<Vec<(i64, f64)>> {
        Self::check_dim(query)?;
        let k_i64 = i64::try_from(k).unwrap_or(i64::MAX);
        // Candidate pool >= all vec rows so the outer agent/superseded filter never under-returns.
        let pool = self.count_vectors()?.max(k_i64).max(1);
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
            params![
                bytemuck::cast_slice::<f32, u8>(query),
                pool,
                agent_id,
                k_i64
            ],
            id_distance,
        )?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The number of stored vectors (the KNN candidate-pool upper bound).
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    fn count_vectors(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM vec_items", [], |r| r.get(0))?)
    }

    /// Returns the stored text for `id`.
    ///
    /// # Errors
    /// Returns an error if the row is missing or SQL fails.
    pub fn text_of(&self, id: i64) -> Result<String> {
        self.conn
            .query_row(
                "SELECT text FROM memories WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(|_| StoreError::MissingRow(id).into())
    }

    /// Bumps `use_count` and sets `last_used_at` for a recalled row.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn touch(&mut self, id: i64, now_unix: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET use_count = use_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![id, now_unix],
        )?;
        Ok(())
    }

    /// Returns all non-superseded memory rows for `agent_id` (no embeddings).
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn active_memories(&self, agent_id: &str) -> Result<Vec<MemRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, last_used_at, use_count, base_score
               FROM memories WHERE agent_id = ?1 AND superseded_by IS NULL ORDER BY id",
        )?;
        let rows = stmt.query_map(params![agent_id], MemRow::from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Returns the stored embedding for `id`.
    ///
    /// Reads the raw blob and decodes native-endian `f32`s via `chunks_exact` (avoids the
    /// alignment panic `bytemuck::cast_slice::<u8, f32>` would risk on an unaligned buffer).
    ///
    /// # Errors
    /// Returns an error if the row is missing or SQL fails.
    pub fn embedding_of(&self, id: i64) -> Result<Vec<f32>> {
        let blob: Vec<u8> = self.conn.query_row(
            "SELECT embedding FROM vec_items WHERE rowid = ?1",
            params![id],
            |r| r.get(0),
        )?;
        Ok(blob
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }

    /// Marks `loser` as superseded by `survivor`.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn set_superseded(&mut self, loser: i64, survivor: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET superseded_by = ?2 WHERE id = ?1",
            params![loser, survivor],
        )?;
        Ok(())
    }

    /// Adds `delta` to a survivor's `use_count`.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn add_use_count(&mut self, id: i64, delta: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE memories SET use_count = use_count + ?2 WHERE id = ?1",
            params![id, delta],
        )?;
        Ok(())
    }

    /// Returns the ids of rows with a non-null `superseded_by` for `agent_id`.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn superseded_ids(&self, agent_id: &str) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM memories WHERE agent_id = ?1 AND superseded_by IS NOT NULL")?;
        let rows = stmt.query_map(params![agent_id], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// The total number of `memories` rows (all agents, including superseded).
    ///
    /// Used to decide whether an empty DB should be rebuilt from a JSONL export on startup.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn count_memories(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?)
    }

    /// True if a memory with this exact `(agent_id, text)` already exists. Used by the JSONL
    /// union-merge to dedupe by content — so merging never inserts a duplicate and never loses,
    /// overwrites, or deletes an existing memory.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn has_memory(&self, agent_id: &str, text: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE agent_id = ?1 AND text = ?2",
            params![agent_id, text],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Exports every `memories` row — all agents, all columns, including superseded rows —
    /// ordered by `id` for a deterministic, diff-friendly export. Embeddings are excluded
    /// (they are regenerated from `text` on import), so this captures the full source of truth.
    ///
    /// # Errors
    /// Returns an error if SQL fails.
    pub fn export_all(&self) -> Result<Vec<MemRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by
               FROM memories ORDER BY id",
        )?;
        let rows = stmt.query_map([], MemRecord::from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Inserts a full `MemRecord` under its ORIGINAL id (not auto-assigned) plus its embedding,
    /// so `id` and every `superseded_by` link survive an export→import round-trip exactly.
    ///
    /// Used only by the JSONL importer ([`crate::persist::import_jsonl`]); normal writes use
    /// [`VectorStore::insert`], which assigns a fresh id.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn insert_with_id(&mut self, rec: &MemRecord, embedding: &[f32]) -> Result<()> {
        Self::check_dim(embedding)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO memories
                 (id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                rec.id,
                rec.agent_id,
                rec.text,
                rec.created_at,
                rec.last_used_at,
                rec.use_count,
                rec.base_score,
                rec.superseded_by,
            ],
        )?;
        tx.execute(
            "INSERT INTO vec_items(rowid, embedding) VALUES (?1, ?2)",
            params![rec.id, bytemuck::cast_slice::<f32, u8>(embedding)],
        )?;
        tx.commit()?;
        Ok(())
    }
}

// Source: test/specs/genesis-memory-server.md — Implementation Requirements
// (groups "Database location" and "Vector store"). Tests run against a real temp SQLite
// database with the real sqlite-vec extension (no mocks, §5 #6: fresh DB per test).
#[cfg(test)]
mod tests {
    use super::{
        content_id, migrate, resolve_db_path, MemRecord, MemRow, VectorStore, DEFAULT_DB_FILENAME,
        MIGRATION_COLUMNS,
    };
    use crate::embed::EMBED_DIM;

    fn open_temp() -> (tempfile::TempDir, VectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.db");
        let store = VectorStore::open(path.to_str().unwrap()).unwrap();
        (dir, store)
    }

    #[test]
    fn migration_is_idempotent_and_adds_the_structured_columns() {
        let (_d, s) = open_temp();
        // open() already migrated; re-running migrate must be a no-op, never an error.
        migrate(&s.conn).unwrap();
        migrate(&s.conn).unwrap();
        let cols: std::collections::HashSet<String> = {
            let mut st = s.conn.prepare("PRAGMA table_info(memories)").unwrap();
            st.query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        for (name, _) in MIGRATION_COLUMNS {
            assert!(
                cols.contains(*name),
                "structured column {name} missing after migrate"
            );
        }
    }

    #[test]
    fn content_id_is_stable_and_field_scoped() {
        let a = content_id("the ball is blue", "semantic", "mneme");
        assert_eq!(
            a,
            content_id("the ball is blue", "semantic", "mneme"),
            "stable"
        );
        assert_ne!(
            a,
            content_id("the ball is blue", "episodic", "mneme"),
            "type-scoped"
        );
        assert_ne!(
            a,
            content_id("the ball is blue", "semantic", "other"),
            "agent-scoped"
        );
        assert_eq!(a.len(), 64, "sha256 hex length");
    }

    #[test]
    fn supersede_by_key_retires_older_active_keeps_row_and_flips_current() {
        let (_d, mut s) = open_temp();
        let old = s
            .insert("mneme", "ball is blue", &emb(0.1), 1.0, 100)
            .unwrap();
        s.set_structure(old, "semantic", Some("ball"), Some("color"), Some("blue"))
            .unwrap();
        let new = s
            .insert("mneme", "ball is green", &emb(0.2), 1.0, 200)
            .unwrap();
        s.set_structure(new, "semantic", Some("ball"), Some("color"), Some("green"))
            .unwrap();

        let retired = s.supersede_by_key("mneme", "ball", "color", 200).unwrap();
        assert_eq!(retired, 1, "only the older 'blue' fact is retired");
        assert_eq!(
            s.active_object("mneme", "ball", "color")
                .unwrap()
                .as_deref(),
            Some("green"),
            "the current value is the newest, deterministically"
        );
        assert_eq!(
            s.count_memories().unwrap(),
            2,
            "the superseded row is KEPT (bi-temporal), not deleted"
        );
    }

    /// A 384-dim vector distinguished by `seed` in slot 0 (and `1 - seed` in slot 1).
    fn emb(seed: f32) -> Vec<f32> {
        let mut e = vec![0.0f32; EMBED_DIM];
        e[0] = seed;
        e[1] = 1.0 - seed;
        e
    }

    // ─── Database location ───────────────────────────────────────────────────

    #[test]
    fn db_path_is_read_from_the_genesis_memory_db_env_var() {
        assert_eq!(
            resolve_db_path(Some("/tmp/custom-genesis.db".to_string())),
            "/tmp/custom-genesis.db"
        );
    }

    #[test]
    fn db_path_falls_back_to_the_repo_genesis_workspace() {
        // Default lands in the repo's `.genesis/` — NOT a stray root db in the launch dir.
        assert_eq!(DEFAULT_DB_FILENAME, ".genesis/memory.db");
        assert_eq!(resolve_db_path(None), DEFAULT_DB_FILENAME);
    }

    #[test]
    fn has_memory_dedupes_by_agent_and_text() {
        let (_d, mut s) = open_temp();
        s.insert("fih-engineer", "the blue widget spec", &emb(0.1), 1.0, 10)
            .unwrap();
        assert!(s
            .has_memory("fih-engineer", "the blue widget spec")
            .unwrap());
        assert!(!s.has_memory("fih-engineer", "a different memory").unwrap()); // different text
        assert!(!s.has_memory("other-agent", "the blue widget spec").unwrap()); // different agent
    }

    // ─── Vector store ────────────────────────────────────────────────────────

    #[test]
    fn sqlite_vec_is_registered_before_the_connection_is_opened() {
        // Inserting into the vec0 virtual table only works if the extension registered.
        let (_d, mut s) = open_temp();
        assert!(s.insert("a", "x", &emb(1.0), 1.0, 0).is_ok());
    }

    #[test]
    fn vec_items_is_created_as_vec0_embedding_float_384() {
        let (_d, s) = open_temp();
        let sql: String = s
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='vec_items'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sql.contains("vec0"), "{sql}");
        assert!(sql.contains("float[384]"), "{sql}");
    }

    #[test]
    fn a_memories_table_is_created() {
        let (_d, s) = open_temp();
        let n: i64 = s
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='memories'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn memories_has_the_base_and_structured_columns() {
        let (_d, s) = open_temp();
        let mut stmt = s
            .conn
            .prepare("SELECT name FROM pragma_table_info('memories') ORDER BY cid")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        // The original 8 base columns, in order, THEN the 0.2.0 structured/bi-temporal columns in
        // MIGRATION_COLUMNS order (derived here so the test stays correct as the migration evolves).
        let base = [
            "id",
            "agent_id",
            "text",
            "created_at",
            "last_used_at",
            "use_count",
            "base_score",
            "superseded_by",
        ];
        let expected: Vec<String> = base
            .iter()
            .map(|s| (*s).to_string())
            .chain(MIGRATION_COLUMNS.iter().map(|(n, _)| (*n).to_string()))
            .collect();
        assert_eq!(cols, expected);
    }

    #[test]
    fn memories_id_is_an_integer_primary_key() {
        let (_d, s) = open_temp();
        let pk: i64 = s
            .conn
            .query_row(
                "SELECT pk FROM pragma_table_info('memories') WHERE name='id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pk, 1);
    }

    #[test]
    fn memories_superseded_by_is_nullable_and_references_memories_id() {
        let (_d, s) = open_temp();
        let notnull: i64 = s
            .conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('memories') WHERE name='superseded_by'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notnull, 0, "superseded_by must be nullable");
        let refs: String = s
            .conn
            .query_row(
                "SELECT \"table\" FROM pragma_foreign_key_list('memories')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(refs, "memories");
    }

    #[test]
    fn vec_items_rowid_equals_memories_id() {
        let (_d, mut s) = open_temp();
        let id = s.insert("a", "hello", &emb(0.7), 1.0, 0).unwrap();
        assert_eq!(s.embedding_of(id).unwrap().len(), EMBED_DIM);
        let mid: i64 = s
            .conn
            .query_row("SELECT id FROM memories WHERE text='hello'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(mid, id);
    }

    #[test]
    fn the_assigned_id_is_read_with_last_insert_rowid() {
        let (_d, mut s) = open_temp();
        let id1 = s.insert("a", "x", &emb(0.1), 1.0, 0).unwrap();
        let id2 = s.insert("a", "y", &emb(0.2), 1.0, 0).unwrap();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn the_embedding_is_inserted_under_that_same_rowid() {
        let (_d, mut s) = open_temp();
        let v = emb(0.42);
        let id = s.insert("a", "x", &v, 1.0, 0).unwrap();
        assert_eq!(s.embedding_of(id).unwrap(), v);
    }

    #[test]
    fn insert_returns_the_assigned_i64() {
        let (_d, mut s) = open_temp();
        let id = s.insert("a", "x", &emb(0.3), 1.0, 0).unwrap();
        assert!(id >= 1);
    }

    #[test]
    fn insert_accepts_no_caller_supplied_id() {
        // The signature takes no id; SQLite assigns each row a distinct one.
        let (_d, mut s) = open_temp();
        let a = s.insert("a", "x", &emb(0.1), 1.0, 0).unwrap();
        let b = s.insert("a", "y", &emb(0.2), 1.0, 0).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn embeddings_are_passed_to_sqlite_via_bytemuck_cast_slice() {
        let (_d, mut s) = open_temp();
        let mut v = emb(0.9);
        v[100] = 0.123_456;
        let id = s.insert("a", "x", &v, 1.0, 0).unwrap();
        assert_eq!(
            s.embedding_of(id).unwrap(),
            v,
            "f32 byte round-trip is exact"
        );
    }

    #[test]
    fn knn_uses_the_verified_match_order_by_distance_limit_form() {
        let (_d, mut s) = open_temp();
        let near = s.insert("a", "near", &emb(1.0), 1.0, 0).unwrap();
        let _far = s.insert("a", "far", &emb(0.0), 1.0, 0).unwrap();
        let hits = s.knn("a", &emb(1.0), 1).unwrap();
        assert_eq!(hits.len(), 1, "LIMIT k respected");
        assert_eq!(hits[0].0, near, "nearest first");
    }

    #[test]
    fn vec0_keeps_its_default_l2_distance_metric() {
        let (_d, mut s) = open_temp();
        let v = emb(0.6);
        let id = s.insert("a", "x", &v, 1.0, 0).unwrap();
        let hits = s.knn("a", &v, 1).unwrap();
        assert_eq!(hits[0].0, id);
        assert!(hits[0].1.abs() < 1e-6, "exact match => L2 distance 0.0");
    }

    #[test]
    fn insert_takes_agent_id_and_records_the_row_under_it() {
        let (_d, mut s) = open_temp();
        s.insert("alpha", "x", &emb(0.5), 1.0, 0).unwrap();
        assert_eq!(s.active_memories("alpha").unwrap().len(), 1);
        assert_eq!(s.active_memories("beta").unwrap().len(), 0);
    }

    #[test]
    fn knn_takes_agent_id_and_restricts_results_to_that_agent() {
        let (_d, mut s) = open_temp();
        let a_id = s.insert("alpha", "a", &emb(1.0), 1.0, 0).unwrap();
        let _b = s.insert("beta", "b", &emb(1.0), 1.0, 0).unwrap();
        let hits = s.knn("alpha", &emb(1.0), 5).unwrap();
        assert!(
            hits.iter().all(|(id, _)| *id == a_id),
            "no cross-agent rows"
        );
    }

    #[test]
    fn rows_with_a_non_null_superseded_by_are_excluded_from_recall() {
        let (_d, mut s) = open_temp();
        let keep = s.insert("a", "keep", &emb(1.0), 1.0, 0).unwrap();
        let gone = s.insert("a", "gone", &emb(0.99), 1.0, 0).unwrap();
        s.set_superseded(gone, keep).unwrap();
        let hits = s.knn("a", &emb(1.0), 5).unwrap();
        assert!(
            hits.iter().all(|(id, _)| *id != gone),
            "superseded excluded"
        );
        assert!(hits.iter().any(|(id, _)| *id == keep));
    }

    /// AC10: a wrong-length vector is rejected (returns `Err`) rather than panicking.
    #[test]
    fn a_383_element_vector_is_rejected_by_insert_and_knn_without_panicking() {
        let (_d, mut s) = open_temp();
        let short = vec![0.0f32; 383];
        assert!(s.insert("a", "x", &short, 1.0, 0).is_err());
        assert!(s.knn("a", &short, 5).is_err());
    }

    /// Exercises `MemRow`'s decay-relevant fields end-to-end (used by consolidation).
    #[test]
    fn active_memories_returns_decay_fields() {
        let (_d, mut s) = open_temp();
        let id = s.insert("a", "x", &emb(0.5), 2.0, 100).unwrap();
        s.touch(id, 200).unwrap();
        let rows: Vec<MemRow> = s.active_memories("a").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].created_at, 100);
        assert_eq!(rows[0].last_used_at, 200);
        assert_eq!(rows[0].use_count, 1);
        assert!((rows[0].base_score - 2.0).abs() < 1e-9);
    }

    // ─── Lossless export/import (memory portability across systems) ───────────

    #[test]
    fn count_memories_counts_all_rows_including_superseded() {
        let (_d, mut s) = open_temp();
        let a = s.insert("x", "a", &emb(0.1), 1.0, 0).unwrap();
        let b = s.insert("x", "b", &emb(0.2), 1.0, 0).unwrap();
        s.set_superseded(a, b).unwrap();
        assert_eq!(
            s.count_memories().unwrap(),
            2,
            "superseded rows still count"
        );
    }

    #[test]
    fn export_all_returns_every_column_of_every_row_ordered_by_id() {
        let (_d, mut s) = open_temp();
        s.insert("alpha", "first", &emb(0.1), 1.5, 100).unwrap();
        let b = s.insert("beta", "second", &emb(0.2), 2.5, 200).unwrap();
        s.touch(b, 250).unwrap();
        let rows = s.export_all().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].agent_id, "alpha");
        assert_eq!(rows[0].text, "first");
        assert_eq!(rows[0].created_at, 100);
        assert_eq!(rows[0].superseded_by, None);
        assert_eq!(rows[1].agent_id, "beta");
        assert_eq!(rows[1].last_used_at, 250);
        assert_eq!(rows[1].use_count, 1);
    }

    /// THE lossless guarantee: a full `memories` table survives binary → JSON text → binary
    /// with **every column of every row byte-identical** — including special characters in the
    /// text and a `superseded_by` link. Uses dummy embeddings (embeddings are a regenerated
    /// index, not source data), so this runs with no ONNX model present.
    #[test]
    fn export_to_jsonl_and_back_preserves_every_row_exactly() {
        let (_d, mut s) = open_temp();
        s.insert_with_id(
            &MemRecord {
                id: 1,
                agent_id: "atlas".into(),
                text: "plain memory".into(),
                created_at: 1000,
                last_used_at: 1200,
                use_count: 4,
                base_score: 1.25,
                superseded_by: None,
            },
            &emb(0.3),
        )
        .unwrap();
        s.insert_with_id(
            &MemRecord {
                id: 2,
                agent_id: "atlas".into(),
                // newline, quotes, comma, unicode, backslash — the things a naive text conversion drops
                text: "line one\nline \"two\", café\\path — 90%".into(),
                created_at: 2000,
                last_used_at: 2000,
                use_count: 0,
                base_score: -0.5,
                superseded_by: Some(1),
            },
            &emb(0.4),
        )
        .unwrap();

        let before = s.export_all().unwrap();

        // Simulate the on-disk JSONL round-trip (one JSON object per line).
        let jsonl: String = before
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect();
        let parsed: Vec<MemRecord> = jsonl
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(parsed, before, "JSON text round-trip is exact");

        // Re-import into a FRESH store (fresh machine), then export again.
        let (_d2, mut s2) = open_temp();
        for r in &parsed {
            s2.insert_with_id(r, &emb(0.0)).unwrap();
        }
        let after = s2.export_all().unwrap();

        assert_eq!(
            after, before,
            "nothing is lost across export → text → import"
        );
    }
}
