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

/// A COMPLETE `memories` row — every SOURCE column, so it round-trips losslessly through the
/// JSONL export/import (see [`crate::persist`]). The embedding and its metadata
/// (`embedding_model`/`version`/`dim`/`metric`/`normalized`) are deliberately absent: they
/// are a derived index regenerated from `text` with the CURRENT model on import, not source
/// data. Every other column — including the structured `(type, subject, relation, object)`
/// and the bi-temporal validity/provenance fields — travels, so Mneme's structuring and the
/// supersede-don't-delete history survive a rebuild-from-JSONL.
///
/// The 13 structured/bi-temporal fields all carry `#[serde(default)]`, so a pre-0.2.0 JSONL
/// export (which lacks them) still parses — they simply come back as `None`, and
/// [`VectorStore::insert_with_id`] backfills `valid_from`/`ingested_at` from `created_at`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The memory type from the taxonomy (semantic/episodic/procedural/...); set by Mneme.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub mem_type: Option<String>,
    /// Structured subject of the fact `(subject, relation, object)`; set by Mneme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Structured relation of the fact; set by Mneme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    /// Structured object of the fact; set by Mneme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Bi-temporal: unix seconds the fact became valid in the world (defaults to `created_at`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<i64>,
    /// Bi-temporal: unix seconds the fact stopped being valid (null = still active/current).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<i64>,
    /// Bi-temporal: unix seconds the fact was ingested into the store (defaults to `created_at`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingested_at: Option<i64>,
    /// Bi-temporal: unix seconds the fact was retracted/expired (null = not expired).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<i64>,
    /// Provenance: where the fact came from (e.g. a session id, a document).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Provenance: the principal the fact is about/for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// Provenance: who asserted the fact (the writing agent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asserted_by: Option<String>,
    /// Confidence in the fact, 0.0–1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Content-addressed id: `sha256(normalized_text \x1e type \x1e agent_id)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

impl MemRecord {
    /// Reads a full `MemRecord` from a query row selecting all columns in [`EXPORT_COLUMNS`] order:
    /// the 8 base columns (0–7) then the 13 structured/bi-temporal columns (8–20).
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
            mem_type: r.get(8)?,
            subject: r.get(9)?,
            relation: r.get(10)?,
            object: r.get(11)?,
            valid_from: r.get(12)?,
            valid_to: r.get(13)?,
            ingested_at: r.get(14)?,
            expired_at: r.get(15)?,
            source: r.get(16)?,
            principal: r.get(17)?,
            asserted_by: r.get(18)?,
            confidence: r.get(19)?,
            content_id: r.get(20)?,
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

/// The SOURCE columns that travel in the JSONL export, in [`MemRecord::from_row`] index order:
/// the 8 base columns then the 13 structured/bi-temporal columns. The 5 embedding-metadata
/// columns (`embedding_model`/`version`/`dim`/`metric`/`normalized`) are intentionally excluded —
/// they describe the derived embedding index, which is regenerated with the CURRENT model on
/// import, so they are not source data. `export_all` and `insert_with_id` share this list so a
/// round-trip stays lossless and column-aligned.
const EXPORT_COLUMNS: &str =
    "id, agent_id, text, created_at, last_used_at, use_count, base_score, \
     superseded_by, type, subject, relation, object, valid_from, valid_to, ingested_at, \
     expired_at, source, principal, asserted_by, confidence, content_id";

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
    // Full-text (BM25) index for the lexical leg of hybrid retrieval. A standalone FTS5 table keyed to the
    // memory id: `text` is immutable after insert and rows are superseded (never hard-deleted), so a
    // per-insert populate plus a one-time backfill of any rows missing from the index keeps it in sync
    // without update/delete triggers. Backfill is idempotent (only inserts ids not already indexed).
    conn.execute_batch("CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(text);")?;
    conn.execute_batch(
        "INSERT INTO memories_fts(rowid, text)
           SELECT id, text FROM memories WHERE id NOT IN (SELECT rowid FROM memories_fts);",
    )?;
    Ok(())
}

/// Build a safe FTS5 MATCH query from arbitrary text: keep alphanumeric tokens, double-quote each (so FTS5
/// special characters can never break the query syntax), and OR them — BM25 then ranks by term overlap.
/// Returns `None` for empty / symbol-only text (the BM25 leg simply contributes nothing).
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.to_lowercase()))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
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

/// Hybrid-retrieval tuning — research defaults, to be calibrated on real data. RRF constant (Cormack 2009);
/// per-hour recency decay on last access (Generative Agents); MMR diversity λ (Carbonell 1998); and the blend
/// weights for the fused-relevance / recency / importance composite.
const RRF_K: f64 = 60.0;
const RECENCY_DECAY_PER_HOUR: f64 = 0.995;
const MMR_LAMBDA: f64 = 0.7;
const W_RELEVANCE: f64 = 1.0;
const W_RECENCY: f64 = 0.3;
const W_IMPORTANCE: f64 = 0.3;

/// Dot product of two equal-length vectors. Embeddings are stored L2-normalized, so dot == cosine.
fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum()
}

/// Min-max normalize scores to [0,1]; a flat set maps to all-0.5 (neutral, avoids divide-by-zero).
fn min_max(v: &[f64]) -> Vec<f64> {
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        });
    let span = hi - lo;
    if span <= f64::EPSILON {
        vec![0.5; v.len()]
    } else {
        v.iter().map(|&x| (x - lo) / span).collect()
    }
}

/// A `usize` rank / count as `f64` without a lossy `as` cast (ranks are tiny, so the clamp never triggers).
fn as_f64(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
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
        // WAL + a busy timeout so the live MCP server and the `structure`/`fix` CLI can hold concurrent
        // connections to the same db: the PostToolUse structuring hook writes structure via the CLI while
        // the server is running. WAL lets readers and a single writer proceed without blocking each other,
        // and the timeout makes a second writer WAIT for the lock instead of failing with SQLITE_BUSY.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
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
        tx.execute(
            "INSERT INTO memories_fts(rowid, text) VALUES (?1, ?2)",
            params![id, text],
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

    /// The `valid_from` of a memory row (unix seconds), or an error if the row is missing.
    ///
    /// # Errors
    /// Returns an error on SQL failure or if no row has `id`.
    pub fn valid_from_of(&self, id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT valid_from FROM memories WHERE id = ?1",
            params![id],
            |r| r.get::<_, i64>(0),
        )?)
    }

    /// Structure a stored memory (Mneme's write-back) and deterministically supersede prior ACTIVE facts
    /// for its key. Sets `(type, subject, relation, object)` on `id`; then — only when BOTH `subject` and
    /// `relation` are present — retires older active facts with the same `(agent_id, subject, relation)` by
    /// setting their `valid_to` to this memory's `valid_from` (supersede-don't-delete; see
    /// [`Self::supersede_by_key`]). An unstructurable memory (no subject/relation) is typed but supersedes
    /// nothing. Returns the number of prior facts retired.
    ///
    /// This is the operation the PostToolUse structuring hook drives: a raw `store` lands the text
    /// instantly, then Mneme calls this to add the structure and retire the fact it replaces — no
    /// similarity, no LLM in the store (staleness is undetectable by embedding similarity — MemStrata).
    ///
    /// # Errors
    /// Returns an error if the structure write, the `valid_from` lookup, or the supersession fails.
    pub fn structure_memory(
        &self,
        agent_id: &str,
        id: i64,
        mem_type: &str,
        subject: Option<&str>,
        relation: Option<&str>,
        object: Option<&str>,
    ) -> Result<usize> {
        self.set_structure(id, mem_type, subject, relation, object)?;
        match (subject, relation) {
            (Some(s), Some(r)) if !s.is_empty() && !r.is_empty() => {
                let vf = self.valid_from_of(id)?;
                self.supersede_by_key(agent_id, s, r, vf)
            }
            _ => Ok(0),
        }
    }

    /// ACTIVE memories Mneme has NOT yet structured (no `subject`), as `(id, agent_id, text)` — the input to
    /// the `/genesis:memory migrate` pass that backfills structure onto pre-0.2.0 flat memories. Optionally
    /// scoped to one agent. Ordered by id.
    ///
    /// # Errors
    /// Returns an error on any SQL failure.
    pub fn unstructured(&self, agent: Option<&str>) -> Result<Vec<(i64, String, String)>> {
        let base = "SELECT id, agent_id, text FROM memories \
             WHERE (subject IS NULL OR subject = '') AND valid_to IS NULL AND expired_at IS NULL";
        let map = |r: &rusqlite::Row| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        };
        let rows = if let Some(a) = agent {
            let mut stmt = self
                .conn
                .prepare(&format!("{base} AND agent_id = ?1 ORDER BY id"))?;
            let out = stmt
                .query_map(params![a], map)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            out
        } else {
            let mut stmt = self.conn.prepare(&format!("{base} ORDER BY id"))?;
            let out = stmt
                .query_map([], map)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            out
        };
        Ok(rows)
    }

    /// BM25 full-text recall for `agent_id` — the lexical leg of hybrid retrieval. Returns `(id, bm25_rank)`
    /// (lower rank = better), ACTIVE memories only (same agent / `superseded_by` / bi-temporal filter as
    /// [`Self::knn`]). Empty or symbol-only queries return `[]`.
    ///
    /// # Errors
    /// Returns an error on any SQL failure.
    pub fn bm25(&self, agent_id: &str, query_text: &str, k: usize) -> Result<Vec<(i64, f64)>> {
        let Some(q) = fts_query(query_text) else {
            return Ok(Vec::new());
        };
        let k_i64 = i64::try_from(k).unwrap_or(i64::MAX);
        let pool = self.count_memories()?.max(k_i64).max(1);
        let mut stmt = self.conn.prepare(
            "SELECT m.id, f.rank
               FROM ( SELECT rowid, rank FROM memories_fts
                      WHERE memories_fts MATCH ?1 ORDER BY rank LIMIT ?2 ) AS f
               JOIN memories m ON m.id = f.rowid
              WHERE m.agent_id = ?3 AND m.superseded_by IS NULL
                AND m.valid_to IS NULL AND m.expired_at IS NULL
              ORDER BY f.rank
              LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![q, pool, agent_id, k_i64], id_distance)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Full hybrid recall: fuse the vector (semantic) + BM25 (lexical) legs with Reciprocal Rank Fusion,
    /// blend in recency + importance (min-max normalized), then MMR-diversify to the top `k`. Returns
    /// `(id, composite_score)` best-first; ACTIVE memories only. `now_unix` drives recency decay.
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn hybrid_recall(
        &self,
        agent_id: &str,
        query_text: &str,
        query_emb: &[f32],
        k: usize,
        now_unix: i64,
    ) -> Result<Vec<(i64, f64)>> {
        Self::check_dim(query_emb)?;
        let pool = (k * 5).max(20);
        let vec_hits = self.knn(agent_id, query_emb, pool)?;
        let bm_hits = self.bm25(agent_id, query_text, pool)?;

        // Reciprocal Rank Fusion by rank position — scale-free, no cross-leg score calibration needed.
        let mut fused: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        for (rank, (id, _)) in vec_hits.iter().enumerate() {
            *fused.entry(*id).or_insert(0.0) += 1.0 / (RRF_K + as_f64(rank) + 1.0);
        }
        for (rank, (id, _)) in bm_hits.iter().enumerate() {
            *fused.entry(*id).or_insert(0.0) += 1.0 / (RRF_K + as_f64(rank) + 1.0);
        }
        if fused.is_empty() {
            return Ok(Vec::new());
        }

        // Candidate metadata (recency ← last_used_at, importance ← base_score) + embeddings (for MMR).
        let cand: Vec<i64> = fused.keys().copied().collect();
        let fused_scores: Vec<f64> = cand.iter().map(|id| fused[id]).collect();
        let mut recency = Vec::with_capacity(cand.len());
        let mut importance = Vec::with_capacity(cand.len());
        let mut embs: Vec<Vec<f32>> = Vec::with_capacity(cand.len());
        for id in &cand {
            let (last_used, base): (i64, f64) = self.conn.query_row(
                "SELECT last_used_at, base_score FROM memories WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let secs = i32::try_from((now_unix - last_used).max(0)).unwrap_or(i32::MAX);
            recency.push(RECENCY_DECAY_PER_HOUR.powf(f64::from(secs) / 3600.0));
            importance.push(base);
            embs.push(self.embedding_of(*id)?);
        }
        let nf = min_max(&fused_scores);
        let ni = min_max(&importance);
        let relevance: Vec<f64> = (0..cand.len())
            .map(|i| W_RELEVANCE * nf[i] + W_RECENCY * recency[i] + W_IMPORTANCE * ni[i])
            .collect();

        // MMR: greedily pick, trading relevance against similarity to what's already chosen (diversity).
        let want = k.min(cand.len());
        let mut selected: Vec<usize> = Vec::with_capacity(want);
        while selected.len() < want {
            let mut best: Option<(usize, f64)> = None;
            for i in 0..cand.len() {
                if selected.contains(&i) {
                    continue;
                }
                let max_sim = selected
                    .iter()
                    .map(|&s| dot(&embs[i], &embs[s]))
                    .fold(0.0_f64, f64::max);
                let mmr = MMR_LAMBDA.mul_add(relevance[i], -(1.0 - MMR_LAMBDA) * max_sim);
                if best.is_none_or(|(_, b)| mmr > b) {
                    best = Some((i, mmr));
                }
            }
            let Some((i, _)) = best else { break };
            selected.push(i);
        }
        Ok(selected
            .into_iter()
            .map(|i| (cand[i], relevance[i]))
            .collect())
    }

    /// Returns the `k` nearest non-superseded memories for `agent_id`, nearest first.
    ///
    /// The verified `vec0` KNN (`MATCH ... ORDER BY distance LIMIT`) runs as an inner
    /// subquery over a candidate pool sized to the whole table, so the outer agent /
    /// `superseded_by` / bi-temporal-validity filter never under-returns. Recall returns only ACTIVE
    /// memories — those neither `superseded_by`-linked (legacy) NOR bi-temporally retired
    /// (`valid_to`/`expired_at` set by [`Self::supersede_by_key`]).
    ///
    /// # Errors
    /// Returns an error on dimension mismatch or any SQL failure.
    pub fn knn(&self, agent_id: &str, query: &[f32], k: usize) -> Result<Vec<(i64, f64)>> {
        Self::check_dim(query)?;
        let k_i64 = i64::try_from(k).unwrap_or(i64::MAX);
        // Candidate pool >= all vec rows so the outer agent/validity filter never under-returns.
        let pool = self.count_vectors()?.max(k_i64).max(1);
        let mut stmt = self.conn.prepare(
            "SELECT m.id, k.distance
               FROM ( SELECT rowid, distance FROM vec_items
                      WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2 ) AS k
               JOIN memories m ON m.id = k.rowid
              WHERE m.agent_id = ?3 AND m.superseded_by IS NULL
                AND m.valid_to IS NULL AND m.expired_at IS NULL
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
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {EXPORT_COLUMNS} FROM memories ORDER BY id"
        ))?;
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
        // `type` is NOT NULL DEFAULT 'semantic'; a pre-0.2.0 record carries None -> use the default.
        let mem_type = rec
            .mem_type
            .clone()
            .unwrap_or_else(|| "semantic".to_string());
        // Bi-temporal invariants: an active row must have a valid_from/ingested_at. A pre-0.2.0
        // record lacks them, so backfill from created_at (same rule as `migrate`).
        let valid_from = rec.valid_from.unwrap_or(rec.created_at);
        let ingested_at = rec.ingested_at.unwrap_or(rec.created_at);
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO memories
                 (id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by,
                  type, subject, relation, object, valid_from, valid_to, ingested_at, expired_at,
                  source, principal, asserted_by, confidence, content_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                rec.id,
                rec.agent_id,
                rec.text,
                rec.created_at,
                rec.last_used_at,
                rec.use_count,
                rec.base_score,
                rec.superseded_by,
                mem_type,
                rec.subject,
                rec.relation,
                rec.object,
                valid_from,
                rec.valid_to,
                ingested_at,
                rec.expired_at,
                rec.source,
                rec.principal,
                rec.asserted_by,
                rec.confidence,
                rec.content_id,
            ],
        )?;
        tx.execute(
            "INSERT INTO vec_items(rowid, embedding) VALUES (?1, ?2)",
            params![rec.id, bytemuck::cast_slice::<f32, u8>(embedding)],
        )?;
        // Keep the FTS index in step so lexical (BM25) recall works after a rebuild-from-JSONL,
        // not just after live inserts.
        tx.execute(
            "INSERT INTO memories_fts(rowid, text) VALUES (?1, ?2)",
            params![rec.id, rec.text],
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

    #[test]
    fn structure_memory_sets_structure_then_supersedes_the_older_same_key_fact() {
        let (_d, mut s) = open_temp();
        let blue = s
            .insert("mneme", "ball is blue", &emb(0.1), 1.0, 100)
            .unwrap();
        let green = s
            .insert("mneme", "ball is green", &emb(0.2), 1.0, 200)
            .unwrap();

        // Structuring the older fact first supersedes nothing (no prior same-key fact).
        assert_eq!(
            s.structure_memory(
                "mneme",
                blue,
                "semantic",
                Some("ball"),
                Some("color"),
                Some("blue")
            )
            .unwrap(),
            0,
        );
        // Structuring the newer fact retires exactly the older 'blue' — via its OWN valid_from (200).
        assert_eq!(
            s.structure_memory(
                "mneme",
                green,
                "semantic",
                Some("ball"),
                Some("color"),
                Some("green")
            )
            .unwrap(),
            1,
        );
        assert_eq!(
            s.active_object("mneme", "ball", "color")
                .unwrap()
                .as_deref(),
            Some("green"),
        );
        assert_eq!(s.count_memories().unwrap(), 2, "the retired row is kept");

        // An unstructurable memory (no subject/relation) is typed but supersedes nothing.
        let vague = s
            .insert("mneme", "hmm, unclear note", &emb(0.3), 1.0, 300)
            .unwrap();
        assert_eq!(
            s.structure_memory("mneme", vague, "episodic", None, None, None)
                .unwrap(),
            0,
        );
    }

    #[test]
    fn unstructured_lists_active_unstructured_memories_scoped_by_agent() {
        let (_d, mut s) = open_temp();
        let flat = s.insert("a", "a flat memory", &emb(0.1), 1.0, 10).unwrap();
        let done = s
            .insert("a", "the ball is blue", &emb(0.2), 1.0, 20)
            .unwrap();
        s.structure_memory(
            "a",
            done,
            "semantic",
            Some("ball"),
            Some("color"),
            Some("blue"),
        )
        .unwrap();
        s.insert("b", "other agent flat", &emb(0.3), 1.0, 30)
            .unwrap();

        let a_flat = s.unstructured(Some("a")).unwrap();
        assert_eq!(a_flat.len(), 1, "only the still-unstructured 'a' memory");
        assert_eq!(a_flat[0].0, flat);
        assert_eq!(a_flat[0].2, "a flat memory");

        assert_eq!(
            s.unstructured(None).unwrap().len(),
            2,
            "both agents' unstructured memories, all-agent scope"
        );
    }

    #[test]
    fn knn_excludes_bitemporally_superseded_facts() {
        let (_d, mut s) = open_temp();
        let old = s.insert("a", "ball is blue", &emb(0.5), 1.0, 100).unwrap();
        s.set_structure(old, "semantic", Some("ball"), Some("color"), Some("blue"))
            .unwrap();
        let new = s.insert("a", "ball is green", &emb(0.5), 1.0, 200).unwrap();
        assert_eq!(s.supersede_by_key("a", "ball", "color", 200).unwrap(), 1);
        let ids: Vec<i64> = s
            .knn("a", &emb(0.5), 10)
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(ids.contains(&new), "the active fact is recalled");
        assert!(
            !ids.contains(&old),
            "a bi-temporally superseded fact must NOT be recalled"
        );
    }

    #[test]
    fn bm25_finds_by_keyword_scoped_to_agent_and_active() {
        let (_d, mut s) = open_temp();
        let blue = s
            .insert("a", "the sky is blue today", &emb(0.1), 1.0, 1)
            .unwrap();
        let _green = s.insert("a", "grass is green", &emb(0.2), 1.0, 2).unwrap();
        let other = s
            .insert("b", "the blue whale swims", &emb(0.3), 1.0, 3)
            .unwrap();

        let ids: Vec<i64> = s
            .bm25("a", "blue sky", 10)
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(ids.contains(&blue), "BM25 finds the keyword match");
        assert!(
            !ids.contains(&other),
            "BM25 is scoped to the querying agent"
        );
        assert!(
            s.bm25("a", "!!! ???", 10).unwrap().is_empty(),
            "symbol-only query yields no results, never an FTS5 syntax error"
        );
    }

    #[test]
    fn hybrid_recall_fuses_legs_ranks_relevant_first_and_excludes_superseded() {
        let (_d, mut s) = open_temp();
        let sky = s
            .insert("a", "the sky is blue", &emb(0.9), 1.0, 100)
            .unwrap();
        let grass = s
            .insert("a", "grass is green", &emb(0.1), 1.0, 100)
            .unwrap();
        let old = s
            .insert("a", "sky colour note", &emb(0.9), 1.0, 50)
            .unwrap();
        s.set_structure(old, "semantic", Some("sky"), Some("color"), Some("grey"))
            .unwrap();
        s.supersede_by_key("a", "sky", "color", 100).unwrap();

        let hits = s.hybrid_recall("a", "blue sky", &emb(0.9), 5, 100).unwrap();
        let ids: Vec<i64> = hits.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&sky), "the relevant active memory is recalled");
        assert!(
            !ids.contains(&old),
            "a superseded memory is excluded from hybrid recall"
        );
        let pos = |x: i64| ids.iter().position(|id| *id == x);
        if let (Some(ps), Some(pg)) = (pos(sky), pos(grass)) {
            assert!(ps < pg, "the more relevant memory ranks higher");
        }
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
                ..Default::default()
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
                // The 0.2.0 structured + bi-temporal fields must travel too — a superseded fact
                // with a full (subject, relation, object) triple, closed validity, and provenance.
                mem_type: Some("semantic".into()),
                subject: Some("ball".into()),
                relation: Some("color".into()),
                object: Some("blue".into()),
                valid_from: Some(1500),
                valid_to: Some(1800),
                ingested_at: Some(2000),
                expired_at: None,
                source: Some("session-42".into()),
                principal: Some("atiqul".into()),
                asserted_by: Some("atlas".into()),
                confidence: Some(0.9),
                content_id: Some("deadbeef".into()),
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
