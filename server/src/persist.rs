//! Portable memory persistence — a lossless JSONL mirror of the `memories` table.
//!
//! The `sqlite-vec` database (`.genesis/memory.db`) is machine-local and not committed
//! (a binary blob with embeddings baked in). To let an agent's memory **travel across
//! systems via git**, the full `memories` table is mirrored to a diff-friendly JSONL file
//! (one [`MemRecord`] per line) that IS committed. On a fresh clone the database is
//! rebuilt from that file — every column restored byte-identically, and the 384-dim
//! embedding regenerated from `text` with the same ONNX model.
//!
//! Losslessness: [`MemRecord`] carries every `memories` column (id, agent_id, text,
//! created_at, last_used_at, use_count, base_score, superseded_by), so export → text →
//! import preserves the memory content exactly (proven by `store`'s round-trip test).
//! The embedding is a derived index, not source data, so regenerating it loses no memory.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::embed::Embedder;
use crate::store::VectorStore;

/// The env var naming the JSONL export path. When unset, [`resolve_export_path`] derives a
/// path from the database path.
pub const EXPORT_ENV: &str = "GENESIS_MEMORY_EXPORT";

/// Resolves the JSONL export path.
///
/// Uses `GENESIS_MEMORY_EXPORT` when set; otherwise derives `<db-dir>/memory/<db-stem>.jsonl`
/// (e.g. `.genesis/memory.db` → `.genesis/memory/memory.jsonl`). Pure over its inputs (the
/// env value is passed in) so it is testable without touching global process state.
#[must_use]
pub fn resolve_export_path(db_path: &str, env_value: Option<String>) -> PathBuf {
    if let Some(v) = env_value {
        return PathBuf::from(v);
    }
    let db = Path::new(db_path);
    let stem = db.file_stem().map_or_else(
        || "genesis-memory".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    let dir = db.parent().unwrap_or_else(|| Path::new(".")).join("memory");
    dir.join(format!("{stem}.jsonl"))
}

/// The export path from the environment (`GENESIS_MEMORY_EXPORT`), else derived from `db_path`.
#[must_use]
pub fn export_path_from_env(db_path: &str) -> PathBuf {
    resolve_export_path(db_path, std::env::var(EXPORT_ENV).ok())
}

/// Writes the entire `memories` table to `path` as JSONL (one [`crate::store::MemRecord`] per
/// line, ordered by id for stable diffs). Parent directories are created; the write is atomic
/// (temp file + rename) so a crash never leaves a half-written export.
///
/// # Errors
/// Returns an error if the export query, serialization, or file write fails.
pub fn export_jsonl(store: &VectorStore, path: &Path) -> Result<()> {
    let rows = store.export_all()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating memory export dir {}", dir.display()))?;
    }
    let mut body = String::new();
    for r in &rows {
        body.push_str(&serde_json::to_string(r)?);
        body.push('\n');
    }
    let tmp = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp, &body).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Rebuilds the store from a JSONL export: each line is parsed to a full record, its embedding
/// is regenerated from `text`, and the row is re-inserted under its original id. Returns the
/// number of rows imported.
///
/// # Errors
/// Returns an error if the file cannot be read, a line cannot be parsed, embedding fails, or
/// an insert fails.
pub fn import_jsonl(
    store: &mut VectorStore,
    embedder: &mut Embedder,
    path: &Path,
) -> Result<usize> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut imported = 0usize;
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rec: crate::store::MemRecord = serde_json::from_str(line)
            .with_context(|| format!("parsing {} line {}", path.display(), i + 1))?;
        let vec = embedder.embed(&rec.text)?;
        store.insert_with_id(&rec, &vec)?;
        imported += 1;
    }
    Ok(imported)
}

/// Records from a JSONL export that still need importing into `store`: ALL of them for a fresh
/// (empty) store, or only those whose `(agent_id, text)` isn't already present for a non-empty one
/// (a UNION merge — dedupe by content so nothing is ever overwritten or lost). Pure over the store's
/// `has_memory`, so the merge DECISION is testable without loading the ONNX embedder.
///
/// # Errors
/// Returns an error if a line can't be parsed or a `has_memory` lookup fails.
fn pending_import(
    store: &VectorStore,
    content: &str,
    empty: bool,
    src: &Path,
) -> Result<Vec<crate::store::MemRecord>> {
    let mut pending = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rec: crate::store::MemRecord = serde_json::from_str(line)
            .with_context(|| format!("parsing {} line {}", src.display(), i + 1))?;
        if empty || !store.has_memory(&rec.agent_id, &rec.text)? {
            pending.push(rec);
        }
    }
    Ok(pending)
}

/// On startup, SYNC the store with the committed JSONL export (cross-system portability):
/// * empty store + export present (fresh clone) → import every record, preserving original ids;
/// * non-empty store → UNION-MERGE: insert only records whose `(agent_id, text)` isn't already
///   present (fresh local ids), so a pulled export adds new memories and never loses local ones.
///
/// This is what makes memory travel with the repo: a pull brings a newer JSONL, and the next server
/// start folds it into the local DB without loss (the JSONL itself is line-diffable, so git merges
/// concurrent edits; a genuine same-(agent,text) divergence simply keeps both). The (costly) ONNX
/// embedder is loaded ONLY when there is actually something to import. Returns rows imported/merged.
///
/// # Errors
/// Returns an error if counting, reading/parsing the export, loading the embedder, or an insert fails.
pub fn rebuild_if_needed(
    store: &mut VectorStore,
    model_dir: &Path,
    export: &Path,
) -> Result<usize> {
    if !export.exists() {
        return Ok(0);
    }
    let empty = store.count_memories()? == 0;
    let content =
        std::fs::read_to_string(export).with_context(|| format!("reading {}", export.display()))?;
    let pending = pending_import(store, &content, empty, export)?;
    if pending.is_empty() {
        return Ok(0); // nothing new to bring in — skip the costly embedder load
    }
    let model = model_dir.join("onnx/model.onnx");
    let tokenizer = model_dir.join("tokenizer.json");
    let mut embedder = Embedder::load(&model.to_string_lossy(), &tokenizer.to_string_lossy())
        .context("loading embedder to sync memory from JSONL export")?;
    for rec in &pending {
        let vec = embedder.embed(&rec.text)?;
        if empty {
            store.insert_with_id(rec, &vec)?; // fresh clone: preserve original ids
        } else {
            // union-merge: fresh local id; content preserved (agent_id / text / score / created_at)
            store.insert(
                &rec.agent_id,
                &rec.text,
                &vec,
                rec.base_score,
                rec.created_at,
            )?;
        }
    }
    Ok(pending.len())
}

#[cfg(test)]
mod tests {
    use super::{export_jsonl, pending_import, resolve_export_path};
    use crate::embed::EMBED_DIM;
    use crate::store::{MemRecord, VectorStore};

    fn open_temp() -> (tempfile::TempDir, VectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open(dir.path().join("m.db").to_str().unwrap()).unwrap();
        (dir, store)
    }

    fn emb(seed: f32) -> Vec<f32> {
        let mut e = vec![0.0f32; EMBED_DIM];
        e[0] = seed;
        e
    }

    #[test]
    fn export_path_uses_the_env_var_when_set() {
        assert_eq!(
            resolve_export_path("/x/memory.db", Some("/custom/mem.jsonl".into())),
            std::path::PathBuf::from("/custom/mem.jsonl")
        );
    }

    #[test]
    fn export_path_defaults_to_a_memory_dir_sibling_of_the_db() {
        let p = resolve_export_path("/repo/.genesis/memory.db", None);
        assert!(p.ends_with("memory/memory.jsonl"), "{}", p.display());
        assert!(p.to_string_lossy().contains(".genesis"));
    }

    #[test]
    fn export_writes_one_json_line_per_row_ordered_by_id() {
        let (dir, mut s) = open_temp();
        s.insert("a", "one", &emb(0.1), 1.0, 10).unwrap();
        s.insert("a", "two", &emb(0.2), 1.0, 20).unwrap();
        let path = dir.path().join("out/mem.jsonl");
        export_jsonl(&s, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: MemRecord = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first.id, 1);
        assert_eq!(first.text, "one");
    }

    #[test]
    fn export_is_atomic_leaving_no_tmp_file() {
        let (dir, mut s) = open_temp();
        s.insert("a", "x", &emb(0.1), 1.0, 0).unwrap();
        let path = dir.path().join("mem.jsonl");
        export_jsonl(&s, &path).unwrap();
        assert!(path.exists());
        assert!(
            !path.with_extension("jsonl.tmp").exists(),
            "no temp left behind"
        );
    }

    /// Full file round-trip WITHOUT the ONNX model: export → read file → parse → re-insert
    /// (dummy embedding) → export again → identical. Proves the on-disk text form loses nothing.
    #[test]
    fn file_export_then_reimport_preserves_all_rows() {
        let (dir, mut s) = open_temp();
        s.insert_with_id(
            &MemRecord {
                id: 1,
                agent_id: "atlas".into(),
                text: "weird \"text\",\nsecond line".into(),
                created_at: 5,
                last_used_at: 9,
                use_count: 2,
                base_score: 3.5,
                superseded_by: None,
                ..Default::default()
            },
            &emb(0.5),
        )
        .unwrap();
        let path = dir.path().join("memory/atlas.jsonl");
        export_jsonl(&s, &path).unwrap();

        let (_d2, mut s2) = open_temp();
        let content = std::fs::read_to_string(&path).unwrap();
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let rec: MemRecord = serde_json::from_str(line).unwrap();
            s2.insert_with_id(&rec, &emb(0.0)).unwrap();
        }
        assert_eq!(s2.export_all().unwrap(), s.export_all().unwrap());
    }

    /// The merge DECISION (no ONNX): a non-empty store unions by content — an already-present
    /// (agent_id,text) is deduped, a new one is pending; an empty store takes everything.
    #[test]
    fn pending_import_unions_by_content() {
        let (dir, mut s) = open_temp();
        s.insert("a", "have this", &emb(0.1), 1.0, 10).unwrap();
        let rec = |id: i64, text: &str| MemRecord {
            id,
            agent_id: "a".into(),
            text: text.into(),
            created_at: id,
            last_used_at: id,
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
            ..Default::default()
        };
        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&rec(1, "have this")).unwrap(),
            serde_json::to_string(&rec(2, "brand NEW memory")).unwrap(),
        );
        let src = dir.path().join("memory.jsonl");

        // non-empty store: only the new record is pending (existing one deduped by content).
        let some = pending_import(&s, &content, false, &src).unwrap();
        assert_eq!(some.len(), 1);
        assert_eq!(some[0].text, "brand NEW memory");

        // empty store: take everything (fresh-clone restore).
        let all = pending_import(&s, &content, true, &src).unwrap();
        assert_eq!(all.len(), 2);
    }
}
