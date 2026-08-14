//! Shared memory core for `doctor` (diagnose), `fix` (consolidate strays), and `reconcile` (merge with a
//! remote export).
//!
//! **The `<repo>/.genesis/memory.db` vector store is the source of truth** — committed and travelling with
//! the repo, ready-to-use with its embeddings baked in. `<repo>/.genesis/memory/memory.jsonl` is its
//! derived, line-diffable mirror (the merge/diff substrate). Before the beta.9 fixes, a plugin-scoped
//! server with no db path defaulted to a bare `genesis-memory.db` in whatever directory Claude Code was
//! launched from, so memory **scattered** into stray root DBs that never reached the repo.
//!
//! This module reads those stray DBs (READ-ONLY — zero footprint outside the target repo) and lets `fix`
//! merge them straight into the canonical `.db`: each stray already holds its embeddings, so a memory and
//! its embedding blob are copied **verbatim** into the canonical store (dedupe by `(agent_id, text)` —
//! nothing overwritten or dropped). No ONNX re-embedding, so consolidation is a plain `genesis-cli`
//! subcommand — and the consolidated memory is recall-able immediately, with no server restart. After the
//! merge, `fix` re-exports the JSONL mirror from the `.db`.

use rusqlite::ffi::sqlite3_auto_extension;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sqlite_vec::sqlite3_vec_init;
use std::collections::{BTreeMap, HashSet};
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

/// A committed memory record. Field names, order, AND serde attributes are byte-compatible with the memory
/// server's JSONL export line (`genesis_memory::store::MemRecord`), so a JSONL written here imports cleanly
/// server-side and, crucially, `read_jsonl`/`db_records` here do NOT silently DROP the 0.2.0 structured +
/// bi-temporal fields when `fix`/`reconcile`/`sync` round-trips the mirror. Keep this in lock-step with the
/// server struct (the cli duplicates it because it does not depend on the server crate).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

/// The SOURCE columns that travel in the JSONL export, in the same order as
/// `genesis_memory::store`'s `EXPORT_COLUMNS` and [`MemRecord`]'s fields. Shared by [`db_records`]
/// and [`read_active_with_embeddings`] so the cli's read paths preserve structure.
const EXPORT_COLUMNS: &str =
    "id, agent_id, text, created_at, last_used_at, use_count, base_score, \
     superseded_by, type, subject, relation, object, valid_from, valid_to, ingested_at, \
     expired_at, source, principal, asserted_by, confidence, content_id";

/// Column name + DDL that bring a memory DB up to the 0.2.0 structured / bi-temporal schema. MUST stay
/// identical to `genesis_memory::store`'s `MIGRATION_COLUMNS` so the cli and server agree on the canonical
/// schema (duplicated because the cli does not depend on the server crate).
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

/// Read a full [`MemRecord`] from a query row selecting [`EXPORT_COLUMNS`] (0–20).
fn mem_record_from_row(r: &rusqlite::Row) -> rusqlite::Result<MemRecord> {
    Ok(MemRecord {
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

/// Where `doctor`/`fix` should look for scattered memory. There is NO guessed default: scatter can land in
/// any launch directory, so the breadth is the USER's decision (surfaced by the slash commands), never a
/// path heuristic. Resolved by [`resolve_scan_roots`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Everything under the OS user directory (`USERPROFILE`/`HOME`).
    User,
    /// Everything on the machine — every filesystem root (all drives on Windows, `/` on Unix).
    System,
}

impl Scope {
    /// Parse a `--scope` value.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

/// The OS user directory, from the environment the OS itself reports (`USERPROFILE` on Windows, else
/// `HOME`). `None` if neither is set. This is the standard OS home lookup — not a path heuristic.
#[must_use]
pub fn os_home() -> Option<PathBuf> {
    for key in ["USERPROFILE", "HOME"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    None
}

/// Whether this process is running under WSL. In WSL the OS home (`$HOME`, e.g. `/home/atiqul`) is the LINUX
/// home, but the user's repos usually live on the mounted Windows drive (`/mnt/c/Users/<name>`), so "user
/// scope" must cover the Windows profile too. Detected from the OS itself (kernel string / interop env),
/// never from a repo path.
#[must_use]
pub fn is_wsl() -> bool {
    if std::env::var_os("WSL_INTEROP").is_some() || std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return true;
    }
    for p in ["/proc/sys/kernel/osrelease", "/proc/version"] {
        if let Ok(s) = std::fs::read_to_string(p) {
            let s = s.to_ascii_lowercase();
            if s.contains("microsoft") || s.contains("wsl") {
                return true;
            }
        }
    }
    false
}

/// Under WSL, the Windows user profile as a WSL path (e.g. `/mnt/c/Users/iatiq`). Resolved from the OS —
/// the `USERPROFILE` value (env if shared, else asked of `cmd.exe`), converted with `wslpath` (falling back
/// to a manual `<drive>:\…` → `/mnt/<drive>/…` mapping). Never derived from a repo path. `None` if it can't
/// be determined or the resolved path doesn't exist.
#[must_use]
pub fn wsl_windows_home() -> Option<PathBuf> {
    let winpath = std::env::var("USERPROFILE")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("cmd.exe")
                .args(["/C", "echo %USERPROFILE%"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty() && s != "%USERPROFILE%")
        })?;
    let home = wsl_to_unix(&winpath)?;
    home.is_dir().then_some(home)
}

/// Convert a Windows path (`C:\Users\iatiq`) to its WSL path, preferring the `wslpath` tool (which respects
/// custom mount roots) and falling back to a manual `<drive>:\…` → `/mnt/<drive>/…` mapping.
fn wsl_to_unix(winpath: &str) -> Option<PathBuf> {
    if let Ok(o) = std::process::Command::new("wslpath")
        .args(["-u", winpath])
        .output()
    {
        if o.status.success() {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    win_to_wsl_manual(winpath)
}

/// Pure `<drive>:\rest` → `/mnt/<drive>/rest` mapping (the `wslpath` fallback). `None` if it isn't a
/// drive-letter path.
fn win_to_wsl_manual(winpath: &str) -> Option<PathBuf> {
    let b = winpath.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        let drive = b[0].to_ascii_lowercase() as char;
        let rest = winpath[2..].replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        return Some(if rest.is_empty() {
            PathBuf::from(format!("/mnt/{drive}"))
        } else {
            PathBuf::from(format!("/mnt/{drive}/{rest}"))
        });
    }
    None
}

/// Every filesystem root on this machine: the existing drive letters on Windows, `/` on Unix.
#[must_use]
pub fn filesystem_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        for c in b'A'..=b'Z' {
            let drive = format!("{}:\\", char::from(c));
            if Path::new(&drive).exists() {
                roots.push(PathBuf::from(drive));
            }
        }
        if roots.is_empty() {
            roots.push(PathBuf::from("C:\\"));
        }
        roots
    }
    #[cfg(not(windows))]
    {
        vec![PathBuf::from("/")]
    }
}

/// Resolve the concrete scan roots from a user-chosen `scope` plus any explicit `--root` paths. There is
/// deliberately NO fallback default: if the caller supplies neither, this errors, so `doctor`/`fix` never
/// silently guess where to look.
///
/// # Errors
/// Returns a message if `scope` is `User` but the OS home can't be determined and no explicit root was
/// given, or if nothing at all was specified.
pub fn resolve_scan_roots(
    scope: Option<Scope>,
    explicit: &[PathBuf],
    repo: &Path,
) -> Result<Vec<PathBuf>, String> {
    // The breadth is still the user's decision: an absent scope with no explicit root is a usage
    // error, exactly as before. Checked BEFORE the repo is added, so adding it cannot mask this.
    if scope.is_none() && explicit.is_empty() {
        return Err(
            "specify a scan scope: --scope user | --scope system  (or an explicit --root <path>)"
                .to_string(),
        );
    }
    let mut roots: Vec<PathBuf> = explicit.to_vec();
    // The repo itself is ALWAYS scanned, on every scope. Scatter most often lands in the launch
    // directory, which is usually the repo root — and `--repo`/`--into` is a known absolute path,
    // so including it needs no detection, no heuristic and no OS-specific branch. Scope decides how
    // far to look BEYOND the repo; it never decides whether the repo is looked at. Without this, a
    // stray sitting in the repo directory is missed and `healthy` is reported for a scattered store.
    roots.push(repo.to_path_buf());
    match scope {
        Some(Scope::User) => {
            let mut found = false;
            if let Some(home) = os_home() {
                roots.push(home);
                found = true;
            }
            // Under WSL, also scan the Windows user profile — that's where the repos actually live.
            if is_wsl() {
                if let Some(win) = wsl_windows_home() {
                    roots.push(win);
                    found = true;
                }
            }
            if !found && explicit.is_empty() {
                return Err(
                    "could not determine your user directory (USERPROFILE/HOME not set); \
                            re-run with --root <path>"
                        .to_string(),
                );
            }
        }
        Some(Scope::System) => roots.extend(filesystem_roots()),
        None => {}
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// Scan several roots for genesis memory DBs, deduping the results (roots may overlap).
#[must_use]
pub fn scan_memory_dbs_in(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for r in roots {
        out.extend(scan_memory_dbs(r));
    }
    out.sort();
    out.dedup();
    out
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
    // Structure-complete + schema-aware: a 0.2.0 canonical DB carries the structured/bi-temporal columns
    // (needed for contradiction detection); a legacy 8-column DB does not — reading them there would fail.
    let cols: HashSet<String> = {
        let mut s = conn
            .prepare("PRAGMA table_info(memories)")
            .map_err(|e| format!("read schema {}: {e}", path.display()))?;
        let names = s
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| format!("read cols {}: {e}", path.display()))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("collect cols {}: {e}", path.display()))?;
        names
    };
    let rows = if cols.contains("subject") {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {EXPORT_COLUMNS} FROM memories ORDER BY id"
            ))
            .map_err(|e| format!("prepare read {}: {e}", path.display()))?;
        let out = stmt
            .query_map([], mem_record_from_row)
            .map_err(|e| format!("query {}: {e}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("read rows {}: {e}", path.display()))?;
        out
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by \
                 FROM memories ORDER BY id",
            )
            .map_err(|e| format!("prepare read {}: {e}", path.display()))?;
        let out = stmt
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
                    ..Default::default()
                })
            })
            .map_err(|e| format!("query {}: {e}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("read rows {}: {e}", path.display()))?;
        out
    };
    Ok(Some(rows))
}

/// A memory plus its raw embedding blob, read straight out of a DB's `vec_items` — the exact bytes
/// `sqlite-vec` stored (native-endian `f32`s). Copied verbatim between DBs, so no decode and no ONNX
/// re-embedding is ever needed.
#[derive(Debug, Clone)]
pub struct EmbeddedRecord {
    /// The memory row (its `id`/`superseded_by` are not carried across a merge).
    pub rec: MemRecord,
    /// The raw `vec_items` embedding blob (384 `f32`s = 1536 bytes).
    pub embedding: Vec<u8>,
}

/// Register the `sqlite-vec` extension for every subsequent connection in this process. Mirrors the memory
/// server's registration (`store.rs`); calling it per-open is the server's own idempotent pattern.
#[allow(unsafe_code)]
fn register_vec_extension() {
    // The sole `unsafe` in the crate: the verified `sqlite3_auto_extension(sqlite3_vec_init)` registration,
    // identical to the memory server. There is no safe wrapper.
    #[allow(clippy::missing_transmute_annotations)]
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }
}

/// Open (creating if missing) a genesis memory DB with `sqlite-vec` registered and the canonical schema
/// ensured — for READ/WRITE of memories AND their embeddings. This is the store `fix` merges into.
///
/// # Errors
/// Returns a message if the parent dir, the connection, or the schema DDL fails.
pub fn open_memory_db(path: &Path) -> Result<Connection, String> {
    register_vec_extension();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
    }
    let conn = Connection::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
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
    )
    .map_err(|e| format!("ensure schema {}: {e}", path.display()))?;
    migrate_memories(&conn)?;
    Ok(conn)
}

/// Idempotently bring the `memories` table up to the 0.2.0 structured / bi-temporal schema: add ONLY the
/// missing columns (checked via `PRAGMA table_info`), then backfill `valid_from`/`ingested_at` from
/// `created_at`. Mirrors the server's `migrate` so the cli's read/write paths (`db_records`,
/// `insert_embedded`) can carry the structured fields without stripping them. The FTS index is left to the
/// server, which rebuilds/backfills it on its next open.
///
/// # Errors
/// Returns a message if the pragma read or any `ALTER`/`UPDATE` fails.
fn migrate_memories(conn: &Connection) -> Result<(), String> {
    let existing: HashSet<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(memories)")
            .map_err(|e| format!("read schema: {e}"))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| format!("read schema cols: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("collect schema cols: {e}"))?;
        cols
    };
    for (name, decl) in MIGRATION_COLUMNS {
        if !existing.contains(*name) {
            // Column names/decls are compile-time constants, never user input — no injection surface.
            conn.execute_batch(&format!("ALTER TABLE memories ADD COLUMN {name} {decl};"))
                .map_err(|e| format!("add column {name}: {e}"))?;
        }
    }
    conn.execute_batch(
        "UPDATE memories SET valid_from = created_at WHERE valid_from IS NULL;
         UPDATE memories SET ingested_at = created_at WHERE ingested_at IS NULL;",
    )
    .map_err(|e| format!("backfill bi-temporal defaults: {e}"))?;
    Ok(())
}

/// Read a DB's ACTIVE memories (`superseded_by IS NULL`) together with their embedding blobs, READ-ONLY.
/// Superseded rows were deliberately retired, so they are not carried into a consolidation. Returns
/// `Ok(None)` if the file has no `memories` table. A memory with no `vec_items` row (no embedding) is
/// skipped — it can't be recalled without re-embedding.
///
/// # Errors
/// Returns a message if the DB can't be opened read-only or the query fails.
pub fn read_active_with_embeddings(path: &Path) -> Result<Option<Vec<EmbeddedRecord>>, String> {
    register_vec_extension();
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
    // A stray can be legacy (8-col) or already 0.2.0 (structured). Detect the schema so a 0.2.0 stray's
    // structure is carried into the consolidation losslessly, while a legacy stray still reads (its
    // structured fields simply come back `None`). Reading structured columns from a legacy stray would fail.
    let cols: HashSet<String> = {
        let mut s = conn
            .prepare("PRAGMA table_info(memories)")
            .map_err(|e| format!("read stray schema {}: {e}", path.display()))?;
        let names = s
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| format!("read stray cols {}: {e}", path.display()))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("collect stray cols {}: {e}", path.display()))?;
        names
    };
    let rows = if cols.contains("subject") {
        // Structured stray: carry every source field + the bi-temporal ACTIVE filter.
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {EXPORT_COLUMNS}, v.embedding \
                 FROM memories JOIN vec_items v ON v.rowid = memories.id \
                 WHERE superseded_by IS NULL AND valid_to IS NULL AND expired_at IS NULL \
                 ORDER BY id",
            ))
            .map_err(|e| format!("prepare read {}: {e}", path.display()))?;
        let out = stmt
            .query_map([], |r| {
                Ok(EmbeddedRecord {
                    rec: mem_record_from_row(r)?,
                    embedding: r.get(21)?,
                })
            })
            .map_err(|e| format!("query {}: {e}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("read rows {}: {e}", path.display()))?;
        out
    } else {
        // Legacy 8-column stray: base fields only, structured fields default to None.
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.agent_id, m.text, m.created_at, m.last_used_at, m.use_count, m.base_score, \
                 v.embedding \
                 FROM memories m JOIN vec_items v ON v.rowid = m.id \
                 WHERE m.superseded_by IS NULL ORDER BY m.id",
            )
            .map_err(|e| format!("prepare read {}: {e}", path.display()))?;
        let out = stmt
            .query_map([], |r| {
                Ok(EmbeddedRecord {
                    rec: MemRecord {
                        id: r.get(0)?,
                        agent_id: r.get(1)?,
                        text: r.get(2)?,
                        created_at: r.get(3)?,
                        last_used_at: r.get(4)?,
                        use_count: r.get(5)?,
                        base_score: r.get(6)?,
                        superseded_by: None,
                        ..Default::default()
                    },
                    embedding: r.get(7)?,
                })
            })
            .map_err(|e| format!("query {}: {e}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("read rows {}: {e}", path.display()))?;
        out
    };
    Ok(Some(rows))
}

/// The set of `(agent_id, text)` keys already present in an open memory DB (for dedup during a merge).
///
/// # Errors
/// Returns a message if the query fails.
pub fn existing_keys(
    conn: &Connection,
) -> Result<std::collections::HashSet<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT agent_id, text FROM memories")
        .map_err(|e| format!("prepare keys: {e}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| format!("query keys: {e}"))?
        .collect::<Result<std::collections::HashSet<_>, _>>()
        .map_err(|e| format!("read keys: {e}"))?;
    Ok(rows)
}

/// Insert one memory + its embedding blob into an open memory DB under a shared fresh rowid (same two-table
/// write the server does). The embedding bytes are copied verbatim — no decode, no re-embedding.
///
/// # Errors
/// Returns a message if either insert fails.
pub fn insert_embedded(conn: &Connection, e: &EmbeddedRecord) -> Result<(), String> {
    // `type` is NOT NULL DEFAULT 'semantic'; a legacy record carries None -> use the default. Bi-temporal
    // invariants: a row must have valid_from/ingested_at, so backfill from created_at (mirrors the server).
    let mem_type = e
        .rec
        .mem_type
        .clone()
        .unwrap_or_else(|| "semantic".to_string());
    let valid_from = e.rec.valid_from.unwrap_or(e.rec.created_at);
    let ingested_at = e.rec.ingested_at.unwrap_or(e.rec.created_at);
    conn.execute(
        "INSERT INTO memories
             (agent_id, text, created_at, last_used_at, use_count, base_score, superseded_by,
              type, subject, relation, object, valid_from, valid_to, ingested_at, expired_at,
              source, principal, asserted_by, confidence, content_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            e.rec.agent_id,
            e.rec.text,
            e.rec.created_at,
            e.rec.last_used_at,
            e.rec.use_count,
            e.rec.base_score,
            e.rec.superseded_by,
            mem_type,
            e.rec.subject,
            e.rec.relation,
            e.rec.object,
            valid_from,
            e.rec.valid_to,
            ingested_at,
            e.rec.expired_at,
            e.rec.source,
            e.rec.principal,
            e.rec.asserted_by,
            e.rec.confidence,
            e.rec.content_id,
        ],
    )
    .map_err(|err| format!("insert memory: {err}"))?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO vec_items(rowid, embedding) VALUES (?1, ?2)",
        params![id, e.embedding],
    )
    .map_err(|err| format!("insert embedding: {err}"))?;
    Ok(())
}

/// Read every memory row from an open memory DB (for re-exporting the JSONL mirror after a merge).
///
/// # Errors
/// Returns a message if the query fails.
pub fn db_records(conn: &Connection) -> Result<Vec<MemRecord>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EXPORT_COLUMNS} FROM memories ORDER BY id"
        ))
        .map_err(|e| format!("prepare db_records: {e}"))?;
    let rows = stmt
        .query_map([], mem_record_from_row)
        .map_err(|e| format!("query db_records: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read db_records: {e}"))?;
    Ok(rows)
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

/// A semantic contradiction: one `(agent_id, subject, relation)` key asserted with 2+ distinct objects
/// among ACTIVE, fully-structured memories — "the ball is blue" vs "the ball is green". This is THE conflict
/// Mneme surfaces. Detection is deterministic (keyed on the identity triple), never similarity/LLM — cosine
/// similarity cannot tell a contradiction from agreement (MemStrata: AUROC ~= chance).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contradiction {
    /// The agent the contradicting memories belong to.
    pub agent_id: String,
    /// The shared subject.
    pub subject: String,
    /// The shared relation.
    pub relation: String,
    /// The distinct conflicting objects (sorted, deduped).
    pub objects: Vec<String>,
    /// The ids of the memories asserting them (sorted).
    pub ids: Vec<i64>,
}

/// Find semantic contradictions among `records`: ACTIVE, fully-structured memories that share an
/// `(agent_id, subject, relation)` key but assert DIFFERENT objects. Only records with non-empty
/// subject/relation/object AND active bi-temporal validity (`valid_to` and `expired_at` both null)
/// participate — a superseded fact is history, not a live contradiction. Deterministic and sorted for
/// stable output. No similarity, no LLM.
#[must_use]
pub fn contradictions(records: &[MemRecord]) -> Vec<Contradiction> {
    // (agent, subject, relation) -> object -> asserting ids. BTreeMap for deterministic ordering.
    let mut groups: BTreeMap<(String, String, String), BTreeMap<String, Vec<i64>>> =
        BTreeMap::new();
    for r in records {
        if r.valid_to.is_some() || r.expired_at.is_some() {
            continue; // not active — a retired fact cannot contradict the present
        }
        let (Some(s), Some(rel), Some(o)) = (
            r.subject.as_deref(),
            r.relation.as_deref(),
            r.object.as_deref(),
        ) else {
            continue; // unstructured — no identity triple to key on
        };
        if s.is_empty() || rel.is_empty() || o.is_empty() {
            continue;
        }
        groups
            .entry((r.agent_id.clone(), s.to_string(), rel.to_string()))
            .or_default()
            .entry(o.to_string())
            .or_default()
            .push(r.id);
    }
    let mut out = Vec::new();
    for ((agent_id, subject, relation), objs) in groups {
        if objs.len() < 2 {
            continue; // one object (even asserted many times) agrees with itself — not a contradiction
        }
        let objects: Vec<String> = objs.keys().cloned().collect();
        let mut ids: Vec<i64> = objs.values().flatten().copied().collect();
        ids.sort_unstable();
        out.push(Contradiction {
            agent_id,
            subject,
            relation,
            objects,
            ids,
        });
    }
    out
}

/// Minimal HTML-escape for text placed in element content / attribute values.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a self-contained (inline-CSS) HTML report of the semantic contradictions the user must resolve.
/// Deterministic (no timestamp) so it is testable. Lists each conflict's agent, `subject relation`, the
/// competing objects, and the memory ids — the substrate for Mneme's resolution conversation.
#[must_use]
pub fn contradictions_html(conflicts: &[Contradiction], repo: &Path) -> String {
    use std::fmt::Write as _;
    let mut rows = String::new();
    for (i, c) in conflicts.iter().enumerate() {
        let mut opts = String::new();
        for o in &c.objects {
            let _ = write!(opts, "<li><code>{}</code></li>", html_escape(o));
        }
        let ids = c
            .ids
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            rows,
            "<tr><td>{}</td><td>{}</td><td><code>{}</code> <code>{}</code></td><td><ul>{opts}</ul></td><td>{ids}</td></tr>",
            i + 1,
            html_escape(&c.agent_id),
            html_escape(&c.subject),
            html_escape(&c.relation),
        );
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>Genesis memory — contradictions to resolve</title>\n\
         <style>body{{font:15px/1.5 system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#111}}\
         h1{{font-size:1.4rem}}table{{border-collapse:collapse;width:100%}}\
         th,td{{border:1px solid #ccc;padding:.5rem .6rem;text-align:left;vertical-align:top}}\
         th{{background:#f4f4f4}}code{{background:#f0f0f0;padding:.05rem .3rem;border-radius:3px}}\
         ul{{margin:0;padding-left:1.1rem}}.hint{{color:#555}}</style></head>\n<body>\n\
         <h1>Memory contradictions to resolve</h1>\n\
         <p class=\"hint\">Repo: <code>{}</code> — {} semantic contradiction(s). For each, the same \
         <code>subject relation</code> asserts more than one object. Tell Mneme which object is correct for \
         each; it will keep that one and supersede the rest (kept as history, never deleted).</p>\n\
         <table><thead><tr><th>#</th><th>agent</th><th>subject / relation</th><th>competing objects</th>\
         <th>memory ids</th></tr></thead><tbody>\n{rows}</tbody></table>\n</body></html>\n",
        html_escape(&repo.to_string_lossy()),
        conflicts.len(),
    )
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
            ..Default::default()
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

    /// P1b: a structured, bi-temporal memory written via `insert_embedded` must come back with EVERY
    /// structured/provenance field intact through `db_records` — the path `fix`/`sync` uses to re-export the
    /// JSONL. If any field were dropped here, Mneme's structuring would silently die on the next round-trip.
    #[test]
    fn insert_embedded_and_db_records_round_trip_structured_fields() {
        let td = tempfile::tempdir().unwrap();
        let db = td.path().join(".genesis/memory.db");
        let conn = open_memory_db(&db).unwrap();
        let rec = MemRecord {
            id: 0,
            agent_id: "atlas".into(),
            text: "the ball is blue".into(),
            created_at: 100,
            last_used_at: 100,
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
            mem_type: Some("semantic".into()),
            subject: Some("ball".into()),
            relation: Some("color".into()),
            object: Some("blue".into()),
            valid_from: Some(100),
            valid_to: None,
            ingested_at: Some(100),
            expired_at: None,
            source: Some("session-1".into()),
            principal: Some("atiqul".into()),
            asserted_by: Some("atlas".into()),
            confidence: Some(0.8),
            content_id: Some("abc123".into()),
        };
        insert_embedded(
            &conn,
            &EmbeddedRecord {
                rec,
                embedding: vec![0u8; 384 * 4], // dummy 384-dim f32 blob
            },
        )
        .unwrap();

        let back = db_records(&conn).unwrap();
        assert_eq!(back.len(), 1);
        let g = &back[0];
        assert_eq!(g.mem_type.as_deref(), Some("semantic"));
        assert_eq!(g.subject.as_deref(), Some("ball"));
        assert_eq!(g.relation.as_deref(), Some("color"));
        assert_eq!(g.object.as_deref(), Some("blue"));
        assert_eq!(g.valid_from, Some(100));
        assert_eq!(g.ingested_at, Some(100));
        assert_eq!(g.source.as_deref(), Some("session-1"));
        assert_eq!(g.principal.as_deref(), Some("atiqul"));
        assert_eq!(g.asserted_by.as_deref(), Some("atlas"));
        assert_eq!(g.confidence, Some(0.8));
        assert_eq!(g.content_id.as_deref(), Some("abc123"));
    }

    /// P1b: opening a pre-0.2.0 (8-column) DB must migrate it in place — add the structured/bi-temporal
    /// columns and backfill `valid_from`/`ingested_at` from `created_at` — so the 21-column `db_records`
    /// read then succeeds instead of failing on the missing columns.
    #[test]
    fn open_memory_db_migrates_a_legacy_eight_column_db_and_backfills_bitemporal() {
        let td = tempfile::tempdir().unwrap();
        let db = td.path().join("legacy.db");
        make_db(&db, &[rec(1, "a", "old memory", 50)]); // legacy 8-column schema
        let conn = open_memory_db(&db).unwrap(); // must ALTER-add the 0.2.0 columns + backfill
        let recs = db_records(&conn).unwrap(); // 21-column SELECT now works
        assert_eq!(recs.len(), 1);
        assert_eq!(
            recs[0].valid_from,
            Some(50),
            "valid_from backfilled from created_at"
        );
        assert_eq!(
            recs[0].ingested_at,
            Some(50),
            "ingested_at backfilled from created_at"
        );
        assert_eq!(
            recs[0].mem_type.as_deref(),
            Some("semantic"),
            "type column defaults to 'semantic'"
        );
    }

    #[test]
    fn contradictions_flags_same_key_different_object_active_only() {
        let mk = |id: i64, agent: &str, subj: &str, rel: &str, obj: &str, valid_to: Option<i64>| {
            MemRecord {
                id,
                agent_id: agent.into(),
                text: format!("{subj} {rel} {obj}"),
                created_at: id,
                last_used_at: id,
                use_count: 0,
                base_score: 1.0,
                superseded_by: None,
                mem_type: Some("semantic".into()),
                subject: Some(subj.into()),
                relation: Some(rel.into()),
                object: Some(obj.into()),
                valid_from: Some(id),
                valid_to,
                ingested_at: Some(id),
                ..Default::default()
            }
        };
        let recs = vec![
            mk(1, "a", "ball", "color", "blue", None),
            mk(2, "a", "ball", "color", "green", None), // contradicts #1
            mk(3, "a", "sky", "color", "blue", None),   // different subject — no conflict
            mk(4, "a", "ball", "color", "red", Some(50)), // SUPERSEDED — excluded
            mk(5, "b", "ball", "color", "green", None), // different agent — separate scope
        ];
        let c = contradictions(&recs);
        assert_eq!(c.len(), 1, "exactly one ACTIVE contradiction");
        assert_eq!(c[0].agent_id, "a");
        assert_eq!(c[0].subject, "ball");
        assert_eq!(c[0].relation, "color");
        assert_eq!(c[0].objects, vec!["blue".to_string(), "green".to_string()]);
        assert_eq!(c[0].ids, vec![1, 2]);
    }

    #[test]
    fn contradictions_ignores_agreement_and_unstructured() {
        let base = |id: i64, subj: Option<&str>, rel: Option<&str>, obj: Option<&str>| MemRecord {
            id,
            agent_id: "a".into(),
            text: "t".into(),
            created_at: id,
            last_used_at: id,
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
            subject: subj.map(Into::into),
            relation: rel.map(Into::into),
            object: obj.map(Into::into),
            valid_from: Some(id),
            ingested_at: Some(id),
            ..Default::default()
        };
        let recs = vec![
            base(1, Some("ball"), Some("color"), Some("blue")),
            base(2, Some("ball"), Some("color"), Some("blue")), // same object — agreement
            base(3, None, None, None),                          // unstructured — ignored
            base(4, Some("ball"), Some("color"), None),         // partial — ignored
        ];
        assert!(contradictions(&recs).is_empty());
    }

    #[test]
    fn contradictions_html_is_self_contained_and_escaped() {
        let c = vec![Contradiction {
            agent_id: "a".into(),
            subject: "ball".into(),
            relation: "color".into(),
            objects: vec!["blue".into(), "gr<een>".into()],
            ids: vec![1, 2],
        }];
        let html = contradictions_html(&c, Path::new("/repo"));
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("gr&lt;een&gt;"), "objects are HTML-escaped");
        assert!(html.contains("/repo"));
        assert!(html.contains("1 semantic contradiction"));
    }

    #[test]
    fn resolve_scan_roots_requires_a_scope_or_explicit_root() {
        let repo = PathBuf::from("/some/repo");
        // No scope, no root → error (never silently guess a path). The repo being an implicit root
        // must NOT satisfy this: the breadth beyond the repo is still the user's decision.
        assert!(resolve_scan_roots(None, &[], &repo).is_err());
        // Explicit root alone is fine — and the repo comes along with it.
        let r = resolve_scan_roots(None, &[PathBuf::from("/some/dir")], &repo).unwrap();
        assert!(r.contains(&PathBuf::from("/some/dir")));
        assert!(r.contains(&repo));
        // System scope resolves to the filesystem roots, plus the repo.
        let r = resolve_scan_roots(Some(Scope::System), &[], &repo).unwrap();
        for fs_root in filesystem_roots() {
            assert!(r.contains(&fs_root));
        }
        assert!(r.contains(&repo));
        // Explicit roots are unioned + deduped with scope roots.
        assert!(
            resolve_scan_roots(Some(Scope::System), &[PathBuf::from("/some/dir")], &repo)
                .unwrap()
                .contains(&PathBuf::from("/some/dir"))
        );
    }

    #[test]
    fn resolve_scan_roots_always_includes_the_repo() {
        // The regression this guards: a stray database sitting in the repo directory was missed
        // because scope resolution never included the repo, so `doctor` reported `healthy: true`
        // for a scattered store. The repo is a known absolute path — it is always scanned.
        let repo = PathBuf::from("/some/repo");
        for scope in [Some(Scope::User), Some(Scope::System)] {
            let roots = resolve_scan_roots(scope, &[], &repo).unwrap();
            assert!(
                roots.contains(&repo),
                "repo must be a scan root for scope {scope:?}"
            );
        }
        // Deduped when the caller also passes it explicitly.
        let roots =
            resolve_scan_roots(Some(Scope::System), std::slice::from_ref(&repo), &repo).unwrap();
        assert_eq!(roots.iter().filter(|p| **p == repo).count(), 1);
    }

    #[test]
    fn scope_parses_only_user_and_system() {
        assert_eq!(Scope::parse("user"), Some(Scope::User));
        assert_eq!(Scope::parse("system"), Some(Scope::System));
        assert_eq!(Scope::parse("home"), None);
        assert_eq!(Scope::parse(""), None);
    }

    #[test]
    fn win_to_wsl_manual_maps_drive_letters() {
        assert_eq!(
            win_to_wsl_manual(r"C:\Users\iatiq"),
            Some(PathBuf::from("/mnt/c/Users/iatiq"))
        );
        assert_eq!(
            win_to_wsl_manual(r"D:\Repositories\x"),
            Some(PathBuf::from("/mnt/d/Repositories/x"))
        );
        assert_eq!(win_to_wsl_manual(r"C:\"), Some(PathBuf::from("/mnt/c")));
        assert_eq!(win_to_wsl_manual("/home/atiqul"), None); // not a drive path
        assert_eq!(win_to_wsl_manual("%USERPROFILE%"), None);
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
