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
    let stem = db
        .file_stem()
        .map_or_else(|| "genesis-memory".to_string(), |s| s.to_string_lossy().into_owned());
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
pub fn import_jsonl(store: &mut VectorStore, embedder: &mut Embedder, path: &Path) -> Result<usize> {
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

/// On startup, restores memory from the committed JSONL export when the database is empty.
///
/// If the store already has rows (existing local DB) this is a no-op. If the store is empty
/// and no export file exists (brand-new agent) it is also a no-op. Only when the store is
/// empty AND an export exists — the fresh-clone case — is the embedder loaded and the export
/// re-imported. Returns the number of rows restored (0 for the no-op cases).
///
/// # Errors
/// Returns an error if counting, loading the embedder, or importing fails.
pub fn rebuild_if_needed(store: &mut VectorStore, model_dir: &Path, export: &Path) -> Result<usize> {
    if store.count_memories()? > 0 {
        return Ok(0);
    }
    if !export.exists() {
        return Ok(0);
    }
    let model = model_dir.join("onnx/model.onnx");
    let tokenizer = model_dir.join("tokenizer.json");
    let mut embedder = Embedder::load(&model.to_string_lossy(), &tokenizer.to_string_lossy())
        .context("loading embedder to rebuild memory from JSONL export")?;
    import_jsonl(store, &mut embedder, export)
}

#[cfg(test)]
mod tests {
    use super::{export_jsonl, resolve_export_path};
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
        assert!(!path.with_extension("jsonl.tmp").exists(), "no temp left behind");
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
}
