//! Shared memory core for `doctor` (diagnose), `fix` (consolidate strays), and `reconcile` (merge with a
//! remote export).
//!
//! Genesis memory lives in ONE place per repo and is committed as BOTH `<repo>/.genesis/memory.db` (the
//! ready-to-use vector DB — git takes the latest on a sync) and `<repo>/.genesis/memory/memory.jsonl` (the
//! line-diffable merge substrate that guarantees nothing is lost). Before the beta.9 fixes, a plugin-scoped
//! server with no db path defaulted to a bare `genesis-memory.db` in whatever directory Claude Code was
//! launched from, so memory **scattered** into stray root DBs that never reached the repo and never
//! travelled.
//!
//! This module reads those stray DBs (READ-ONLY — zero footprint on anything outside the target repo)
//! and folds their memories into the target repo's JSONL via a **deterministic, lossless UNION merge**
//! (dedupe by `(agent_id, text)` — nothing is ever overwritten or dropped). The server's own
//! `rebuild_if_needed` performs the same union on its next start, so the DB catches up automatically.
//!
//! No ONNX embedder is needed here: the JSONL is text, and embeddings are a derived index the server
//! regenerates from `text` on import. That is why consolidation can be a plain `genesis-cli` subcommand.

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Directory names never worth descending into while scanning for stray memory DBs. Includes OS junk
/// (`AppData`, `Library`) so a home-rooted scan stays fast — memory DBs live in project dirs, not there.
const PRUNE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".next",
    "AppData",
    "Library",
    "Application Data",
    ARCHIVE_DIRNAME,
];

/// File names that denote a genesis memory database (the canonical `.genesis/memory.db` and the legacy
/// scattered `genesis-memory.db`).
const DB_FILENAMES: &[&str] = &["memory.db", "genesis-memory.db"];

/// The `.genesis/memory/` subdir where an optional archival copy of consolidated strays may be kept.
const ARCHIVE_DIRNAME: &str = "archived-strays";

/// Bound on scan recursion depth — a backstop against pathological or symlinked trees.
const MAX_SCAN_DEPTH: usize = 16;

/// A committed memory record. Field names AND order are byte-compatible with the memory server's JSONL
/// export line (`genesis_memory::store::MemRecord`), so a JSONL written here imports cleanly server-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemRecord {
    /// Row id. Rewritten to a stable 1..N on consolidation (see [`consolidate`]).
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
    /// The id of the memory that superseded this one, if any. Consolidation clears this (`null`): ids are
    /// rewritten across a cross-DB union so old links cannot be preserved unambiguously — matching the
    /// server's own non-empty union, which also drops the link. Nothing is lost: the memory text stays.
    pub superseded_by: Option<i64>,
}

/// Per-agent memory counts, for the diagnosis report.
pub type AgentCounts = BTreeMap<String, usize>;

/// Recursively find every genesis memory DB under `root` (pruning heavy/irrelevant dirs). Read-only;
/// returns paths sorted for deterministic output. Never descends past [`MAX_SCAN_DEPTH`].
#[must_use]
pub fn scan_memory_dbs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ft.is_dir() {
            if PRUNE_DIRS.contains(&name.as_ref()) || name.starts_with('.') && name != ".genesis" {
                continue;
            }
            walk(&path, depth + 1, out);
        } else if ft.is_file() && DB_FILENAMES.contains(&name.as_ref()) {
            out.push(path);
        }
    }
}

/// Canonicalize a path for identity comparison, falling back to the path itself when it does not exist.
#[must_use]
pub fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The default scan root for `doctor`/`fix`: the user's HOME directory (`USERPROFILE` on Windows, else
/// `HOME`), falling back to the current dir. Scatter lands in whatever directory Claude Code was launched
/// from — a SIBLING of the repo, not inside it — so the default must be broad enough to find it. Narrow it
/// with `--root` when you know where to look.
#[must_use]
pub fn default_scan_root() -> PathBuf {
    for key in ["USERPROFILE", "HOME"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    std::env::current_dir().unwrap_or_default()
}

/// Is `db` physically inside the `repo` tree? (An in-repo stray is unambiguously the repo's, so `fix`
/// consolidates it regardless of agent.)
#[must_use]
pub fn is_inside(db: &Path, repo: &Path) -> bool {
    canon(db).starts_with(canon(repo))
}

/// The repo's CUSTOM agents — the basenames of `<repo>/.claude/agents/*.md` EXCLUDING the shared builder
/// team (`sensei`, `method`). A custom agent's name is unique to the repo that built it, so it safely
/// identifies THIS repo's scattered memory across a broad scan; `sensei`/`method` are installed in every
/// Genesis repo, so their memory can't be attributed by name and is never cross-pulled.
#[must_use]
pub fn repo_custom_agents(repo: &Path) -> Vec<String> {
    let dir = repo.join(".claude").join("agents");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if stem != "sensei" && stem != "method" {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Read every row of a memory DB's `memories` table (READ-ONLY). Returns:
/// * `Ok(Some(rows))` for a genesis memory DB (possibly empty),
/// * `Ok(None)` if the file has no `memories` table (not a genesis memory DB — skip it),
/// * `Err(_)` on an actual I/O / SQL failure.
///
/// The `sqlite-vec` extension is deliberately NOT registered: `memories` is a plain table, so it reads
/// without the vec module (only the separate `vec_items` virtual table would need it).
///
/// # Errors
/// Returns a message if the database cannot be opened read-only or the row query fails.
pub fn read_db_memories(path: &Path) -> Result<Option<Vec<MemRecord>>, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open {} read-only: {e}", path.display()))?;
    let has_table: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='memories'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !has_table {
        return Ok(None);
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by \
             FROM memories ORDER BY id",
        )
        .map_err(|e| format!("prepare read {}: {e}", path.display()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(MemRecord {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                text: r.get(2)?,
                created_at: r.get(3)?,
                last_used_at: r.get(4)?,
                use_count: r.get(5)?,
                base_score: r.get(6)?,
                superseded_by: r.get(7)?,
            })
        })
        .map_err(|e| format!("query {}: {e}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read rows {}: {e}", path.display()))?;
    Ok(Some(rows))
}

/// Count memories per agent (for the diagnosis report), in sorted-agent order.
#[must_use]
pub fn agent_counts(recs: &[MemRecord]) -> AgentCounts {
    let mut c = AgentCounts::new();
    for r in recs {
        *c.entry(r.agent_id.clone()).or_insert(0) += 1;
    }
    c
}

/// Parse a JSONL memory export (one [`MemRecord`] per line), skipping blank / unparseable lines.
#[must_use]
pub fn read_jsonl(path: &Path) -> Vec<MemRecord> {
    let Some(text) = std::fs::read_to_string(path).ok() else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() {
                None
            } else {
                serde_json::from_str::<MemRecord>(t).ok()
            }
        })
        .collect()
}

/// Deterministic, lossless UNION of memory records, in priority order (earlier inputs win the metadata
/// for a duplicate). Dedupe key is `(agent_id, text)` — the same content is never stored twice, and no
/// record is ever dropped or overwritten. The result is sorted by `(agent_id, created_at, text)` and
/// re-ided `1..N` so the same content always yields byte-identical JSONL (stable git diffs, idempotent
/// repeated runs); `superseded_by` is cleared (see [`MemRecord::superseded_by`]).
#[must_use]
pub fn consolidate(sources: &[Vec<MemRecord>]) -> Vec<MemRecord> {
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut merged: Vec<MemRecord> = Vec::new();
    for src in sources {
        for r in src {
            let key = (r.agent_id.clone(), r.text.clone());
            if seen.insert(key) {
                merged.push(r.clone());
            }
        }
    }
    merged.sort_by(|a, b| {
        a.agent_id
            .cmp(&b.agent_id)
            .then(a.created_at.cmp(&b.created_at))
            .then_with(|| a.text.cmp(&b.text))
    });
    for (i, r) in merged.iter_mut().enumerate() {
        r.id = i64::try_from(i + 1).unwrap_or(i64::MAX);
        r.superseded_by = None;
    }
    merged
}

/// How the local store compares to an incoming (remote) export — the classification behind Atiqul's
/// reconcile procedure (use the JSONL to decide before touching the `.db`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileStatus {
    /// Local and incoming hold exactly the same memories — nothing to do.
    AlreadySynced,
    /// Incoming is a superset of local (local ⊆ incoming): adopting incoming loses NO local context, so
    /// the `.db` can simply be replaced with the incoming one.
    AddOnly,
    /// Local holds memories the incoming set is missing: a plain replace would lose context, so the two are
    /// UNION-merged (lose nothing).
    Merged,
}

impl ReconcileStatus {
    /// A stable lowercase tag for JSON output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlreadySynced => "already-synced",
            Self::AddOnly => "add-only",
            Self::Merged => "merged",
        }
    }
}

/// The result of reconciling local memory against an incoming (remote) export.
#[derive(Debug, Clone)]
pub struct Reconciliation {
    /// How local compared to incoming.
    pub status: ReconcileStatus,
    /// Distinct memories in local before reconciling.
    pub local: usize,
    /// Distinct memories in the incoming export.
    pub incoming: usize,
    /// Memories present in incoming but not local (brought in).
    pub added_from_incoming: usize,
    /// Memories present locally but missing from incoming (preserved — would have been lost by a replace).
    pub kept_local_only: usize,
    /// The reconciled canonical set to write (the lossless UNION), stably ordered and re-ided.
    pub merged: Vec<MemRecord>,
}

/// Deterministically reconcile `local` memory against an `incoming` (remote) export — the lossless core of
/// the "update remote" procedure. Never loses a memory:
/// * `local == incoming` (by `(agent_id, text)`) → [`ReconcileStatus::AlreadySynced`];
/// * `local ⊆ incoming` → [`ReconcileStatus::AddOnly`] (a plain `.db` replace is safe);
/// * otherwise → [`ReconcileStatus::Merged`] (UNION so local-only memories survive).
///
/// `merged` is always the UNION (== incoming in the AddOnly case), so the caller can unconditionally write
/// it as the new canonical JSONL. A genuine can't-decide *conflict* is not produced here (the union is
/// always safe); it is a git-level concern the orchestrating skill escalates to the user.
#[must_use]
pub fn reconcile(local: &[MemRecord], incoming: &[MemRecord]) -> Reconciliation {
    let key = |r: &MemRecord| (r.agent_id.clone(), r.text.clone());
    let local_keys: std::collections::HashSet<(String, String)> = local.iter().map(key).collect();
    let incoming_keys: std::collections::HashSet<(String, String)> =
        incoming.iter().map(key).collect();
    let added_from_incoming = incoming_keys.difference(&local_keys).count();
    let kept_local_only = local_keys.difference(&incoming_keys).count();
    let status = if added_from_incoming == 0 && kept_local_only == 0 {
        ReconcileStatus::AlreadySynced
    } else if kept_local_only == 0 {
        ReconcileStatus::AddOnly
    } else {
        ReconcileStatus::Merged
    };
    // Incoming first so remote metadata wins on a shared memory (git "takes the latest"); local-only records
    // are appended by the union so they are never dropped.
    let merged = consolidate(&[incoming.to_vec(), local.to_vec()]);
    Reconciliation {
        status,
        local: local_keys.len(),
        incoming: incoming_keys.len(),
        added_from_incoming,
        kept_local_only,
        merged,
    }
}

/// Serialize records to JSONL text (one line per record, trailing newline), matching the server's export.
#[must_use]
pub fn to_jsonl(recs: &[MemRecord]) -> String {
    let mut body = String::new();
    for r in recs {
        if let Ok(line) = serde_json::to_string(r) {
            body.push_str(&line);
            body.push('\n');
        }
    }
    body
}

/// Write JSONL atomically (temp file + rename), creating parent dirs — never leaves a half-written export.
///
/// # Errors
/// Returns a message if a parent dir, the temp write, or the rename fails.
pub fn write_jsonl_atomic(path: &Path, recs: &[MemRecord]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp, to_jsonl(recs)).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(())
}

/// The canonical memory paths for a repo: `<repo>/.genesis/memory.db` and its JSONL mirror.
#[must_use]
pub fn canonical_paths(repo: &Path) -> (PathBuf, PathBuf) {
    let db = repo.join(".genesis").join("memory.db");
    let jsonl = repo.join(".genesis").join("memory").join("memory.jsonl");
    (db, jsonl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: i64, agent: &str, text: &str, created: i64) -> MemRecord {
        MemRecord {
            id,
            agent_id: agent.into(),
            text: text.into(),
            created_at: created,
            last_used_at: created,
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
        }
    }

    /// Build a minimal genesis memory DB (memories table only, no vec extension) for read tests.
    fn make_db(path: &Path, recs: &[MemRecord]) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (id INTEGER PRIMARY KEY, agent_id TEXT, text TEXT, created_at INTEGER, \
             last_used_at INTEGER, use_count INTEGER, base_score REAL, superseded_by INTEGER);",
        )
        .unwrap();
        for r in recs {
            conn.execute(
                "INSERT INTO memories VALUES (?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    r.id,
                    r.agent_id,
                    r.text,
                    r.created_at,
                    r.last_used_at,
                    r.use_count,
                    r.base_score,
                    r.superseded_by
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn scan_finds_dbs_and_prunes_heavy_dirs() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        make_db(&root.join("genesis-memory.db"), &[]); // stray at launch root
        std::fs::create_dir_all(root.join(".genesis")).unwrap();
        make_db(&root.join(".genesis/memory.db"), &[]); // canonical
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        make_db(&root.join("node_modules/pkg/memory.db"), &[]); // must be pruned
        std::fs::create_dir_all(root.join("sub")).unwrap();
        make_db(&root.join("sub/genesis-memory.db"), &[]); // nested stray

        let found = scan_memory_dbs(root);
        assert!(found.iter().any(|p| p.ends_with("genesis-memory.db")));
        assert!(found.iter().any(|p| p.ends_with(".genesis/memory.db")));
        assert!(found.iter().any(|p| p.ends_with("sub/genesis-memory.db")));
        assert!(
            !found
                .iter()
                .any(|p| p.to_string_lossy().contains("node_modules")),
            "node_modules must be pruned"
        );
    }

    #[test]
    fn read_db_returns_rows_and_none_for_non_memory_db() {
        let td = tempfile::tempdir().unwrap();
        let db = td.path().join("genesis-memory.db");
        make_db(
            &db,
            &[
                rec(1, "fih-engineer", "the spec", 10),
                rec(2, "fih-engineer", "another", 20),
            ],
        );
        let rows = read_db_memories(&db).unwrap().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].agent_id, "fih-engineer");

        // a DB without a `memories` table → None (skipped, not an error)
        let other = td.path().join("other.db");
        let conn = Connection::open(&other).unwrap();
        conn.execute_batch("CREATE TABLE notes (x TEXT);").unwrap();
        assert!(read_db_memories(&other).unwrap().is_none());
    }

    #[test]
    fn consolidate_unions_dedupes_and_stably_reids() {
        // existing repo jsonl (wins metadata), plus two strays with an overlap.
        let existing = vec![rec(7, "a", "keep me", 100)];
        let stray1 = vec![rec(1, "a", "keep me", 100), rec(2, "a", "new one", 50)];
        let stray2 = vec![rec(9, "b", "b memory", 30), rec(2, "a", "new one", 50)];
        let out = consolidate(&[existing, stray1, stray2]);
        // union of distinct (agent,text): (a,"keep me"), (a,"new one"), (b,"b memory") = 3
        assert_eq!(out.len(), 3);
        // stable order by (agent_id, created_at, text): a/50 "new one", a/100 "keep me", b/30 "b memory"
        assert_eq!(
            (out[0].agent_id.as_str(), out[0].text.as_str()),
            ("a", "new one")
        );
        assert_eq!(
            (out[1].agent_id.as_str(), out[1].text.as_str()),
            ("a", "keep me")
        );
        assert_eq!(
            (out[2].agent_id.as_str(), out[2].text.as_str()),
            ("b", "b memory")
        );
        // re-ided 1..N, superseded_by cleared
        assert_eq!(out.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(out.iter().all(|r| r.superseded_by.is_none()));
    }

    #[test]
    fn consolidate_is_idempotent_on_the_same_content() {
        let src = vec![rec(1, "a", "x", 1), rec(2, "a", "y", 2)];
        let once = consolidate(std::slice::from_ref(&src));
        let twice = consolidate(std::slice::from_ref(&once));
        assert_eq!(
            to_jsonl(&once),
            to_jsonl(&twice),
            "same content -> byte-identical jsonl"
        );
    }

    #[test]
    fn reconcile_classifies_add_only_merged_and_synced_losslessly() {
        let local = vec![rec(1, "a", "shared", 1), rec(2, "a", "local only", 2)];
        let incoming = vec![rec(9, "a", "shared", 1), rec(8, "a", "remote only", 3)];

        // local has "local only" not in incoming -> Merged, and nothing is dropped.
        let m = reconcile(&local, &incoming);
        assert_eq!(m.status, ReconcileStatus::Merged);
        assert_eq!(m.added_from_incoming, 1); // "remote only"
        assert_eq!(m.kept_local_only, 1); // "local only"
        let texts: Vec<&str> = m.merged.iter().map(|r| r.text.as_str()).collect();
        assert!(
            texts.contains(&"shared")
                && texts.contains(&"local only")
                && texts.contains(&"remote only")
        );
        assert_eq!(m.merged.len(), 3, "union loses nothing");

        // incoming is a superset of local -> AddOnly (safe to just replace the .db).
        let sup = reconcile(
            &local,
            &[
                rec(1, "a", "shared", 1),
                rec(2, "a", "local only", 2),
                rec(3, "a", "extra", 4),
            ],
        );
        assert_eq!(sup.status, ReconcileStatus::AddOnly);
        assert_eq!(sup.kept_local_only, 0);

        // identical sets -> AlreadySynced.
        let same = reconcile(
            &local,
            &[rec(5, "a", "shared", 1), rec(6, "a", "local only", 2)],
        );
        assert_eq!(same.status, ReconcileStatus::AlreadySynced);
    }

    #[test]
    fn jsonl_roundtrips_through_write_and_read() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join(".genesis/memory/memory.jsonl");
        let recs = consolidate(&[vec![rec(1, "a", "hello", 1), rec(2, "a", "world", 2)]]);
        write_jsonl_atomic(&p, &recs).unwrap();
        assert!(p.exists());
        assert!(
            !p.with_extension("jsonl.tmp").exists(),
            "no temp left behind"
        );
        assert_eq!(read_jsonl(&p), recs);
    }
}
