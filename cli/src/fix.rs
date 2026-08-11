//! `genesis-cli fix [--into <repo>] [--root <dir>] [--archive]` — CONSOLIDATE scattered memory into the
//! repo's canonical store, losslessly and deterministically.
//!
//! It scans `<root>` (default: `<into>`) for stray memory databases, reads every one READ-ONLY, and folds
//! their memories — together with whatever is already in `<into>/.genesis/memory.db` and its JSONL — into
//! `<into>/.genesis/memory/memory.jsonl` via [`memfix::consolidate`] (UNION by `(agent_id, text)`, so
//! nothing is overwritten or lost). The server rebuilds/​unions the local DB from that JSONL on its next
//! start, so `memory.db` catches up automatically.
//!
//! **Zero footprint outside the target repo.** Strays are only ever READ; the sole write is the target
//! repo's JSONL. Because the union dedupes by content, `fix` is idempotent — re-running with the strays
//! still in place produces the same JSONL and no duplicates, so the strays are safe to leave (and safe to
//! delete manually afterwards). `--archive` additionally COPIES (never moves) each stray into
//! `<into>/.genesis/memory/archived-strays/` for safekeeping, still touching nothing outside `<into>`.

use crate::{fsx, memfix};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli fix`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let into = flag(args, "--into").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !into.is_dir() {
        fsx::fail(&format!("target repo not found: {}", into.display()));
    }
    let root = flag(args, "--root").map_or_else(|| into.clone(), PathBuf::from);
    let archive = args.iter().any(|a| a == "--archive");

    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&into);
    let canonical_db_c = memfix::canon(&canonical_db);

    // Priority order for the union (earlier wins duplicate metadata): the repo's committed JSONL, then its
    // local DB, then the strays in scan order. This guarantees repo-native memory keeps its own metadata.
    let mut sources: Vec<Vec<memfix::MemRecord>> = Vec::new();
    let existing_jsonl = memfix::read_jsonl(&canonical_jsonl);
    let before = existing_jsonl.len();
    sources.push(existing_jsonl);
    if let Ok(Some(rows)) = memfix::read_db_memories(&canonical_db) {
        sources.push(rows);
    }

    let mut consolidated_from: Vec<Value> = Vec::new();
    let mut archived: Vec<Value> = Vec::new();
    let mut stray_memories = 0usize;

    for db in memfix::scan_memory_dbs(&root) {
        if memfix::canon(&db) == canonical_db_c {
            continue; // the canonical DB is already folded in above
        }
        let rows = match memfix::read_db_memories(&db) {
            Ok(Some(rows)) => rows,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };
        if rows.is_empty() {
            continue;
        }
        stray_memories += rows.len();
        consolidated_from.push(json!({ "path": db.to_string_lossy(), "memories": rows.len() }));
        if archive {
            match archive_copy(&db, &into) {
                Ok(dest) => archived.push(json!(dest.to_string_lossy())),
                Err(e) => eprintln!("warning: archive {}: {e}", db.display()),
            }
        }
        sources.push(rows);
    }

    let merged = memfix::consolidate(&sources);
    let after = merged.len();
    if let Err(e) = memfix::write_jsonl_atomic(&canonical_jsonl, &merged) {
        fsx::fail(&format!("writing consolidated memory: {e}"));
    }

    let note = if consolidated_from.is_empty() {
        format!(
            "No stray memory found. The repo store at {} holds {after} memories.",
            canonical_jsonl.display()
        )
    } else {
        format!(
            "Consolidated {} stray database(s) ({stray_memories} memories) into {}. Union is lossless and \
             idempotent; the strays were only READ and are safe to remove. The local memory.db rebuilds \
             from this JSONL on the next server start in this repo.",
            consolidated_from.len(),
            canonical_jsonl.display()
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "into": into.to_string_lossy(),
            "canonical_jsonl": canonical_jsonl.to_string_lossy(),
            "records_before": before,
            "records_after": after,
            "added": after.saturating_sub(before),
            "consolidated_from": consolidated_from,
            "archived": archived,
            "note": note,
        }))
    );
    0
}

/// Copy a stray DB into `<into>/.genesis/memory/archived-strays/`, giving it a unique, path-derived name so
/// two strays with the same basename don't collide. Returns the destination path. Never moves the source.
fn archive_copy(src: &Path, into: &Path) -> Result<PathBuf, String> {
    let dir = into.join(".genesis").join("memory").join("archived-strays");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let slug: String = memfix::canon(src)
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let dest = dir.join(format!("{slug}.db"));
    std::fs::copy(src, &dest)
        .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dest.display()))?;
    Ok(dest)
}

/// Return the value following `flag` in `args`, if present.
fn flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memfix::MemRecord;
    use rusqlite::Connection;

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

    fn make_db(path: &Path, recs: &[MemRecord]) {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).unwrap();
        }
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
    fn fix_consolidates_strays_into_repo_jsonl_without_touching_strays() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        // a stray at the launch root + a nested stray, both OUTSIDE .genesis/
        make_db(
            &repo.join("genesis-memory.db"),
            &[rec(1, "fih", "stray root", 5)],
        );
        make_db(
            &repo.join("sub/genesis-memory.db"),
            &[rec(1, "fih", "stray nested", 6)],
        );
        // canonical DB already has one memory
        make_db(
            &repo.join(".genesis/memory.db"),
            &[rec(1, "fih", "already in repo", 1)],
        );

        let code = run(&["--into".into(), repo.to_string_lossy().into_owned()]);
        assert_eq!(code, 0);

        let (_db, jsonl) = memfix::canonical_paths(repo);
        let out = memfix::read_jsonl(&jsonl);
        let texts: Vec<&str> = out.iter().map(|r| r.text.as_str()).collect();
        assert!(texts.contains(&"already in repo"));
        assert!(texts.contains(&"stray root"));
        assert!(texts.contains(&"stray nested"));
        assert_eq!(out.len(), 3, "union of 3 distinct memories");

        // strays untouched (READ-ONLY)
        assert!(repo.join("genesis-memory.db").is_file());
        assert!(repo.join("sub/genesis-memory.db").is_file());
    }

    #[test]
    fn fix_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        make_db(
            &repo.join("genesis-memory.db"),
            &[rec(1, "a", "one", 1), rec(2, "a", "two", 2)],
        );
        let (_db, jsonl) = memfix::canonical_paths(repo);

        assert_eq!(
            run(&["--into".into(), repo.to_string_lossy().into_owned()]),
            0
        );
        let first = std::fs::read_to_string(&jsonl).unwrap();
        assert_eq!(
            run(&["--into".into(), repo.to_string_lossy().into_owned()]),
            0
        );
        let second = std::fs::read_to_string(&jsonl).unwrap();
        assert_eq!(first, second, "re-running fix yields byte-identical jsonl");
    }
}
