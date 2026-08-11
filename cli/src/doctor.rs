//! `genesis-cli doctor [--root <dir>]` — READ-ONLY diagnosis of where an agent's memory actually lives.
//!
//! Scans `<root>` (default: the current directory) for every genesis memory database — the canonical
//! `<root>/.genesis/memory.db` plus any **stray** `genesis-memory.db` / nested `memory.db` left behind by
//! a mis-anchored server (Issue 2). It reports, per database, how many memories each agent has, and flags
//! memory that is NOT in the repo's `.genesis/` store. It changes NOTHING — run `genesis-cli fix` (or
//! `/genesis:fix`) to consolidate.

use crate::{fsx, memfix};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Entry point for `genesis-cli doctor`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let root = flag(args, "--root").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !root.is_dir() {
        fsx::fail(&format!("root not found: {}", root.display()));
    }

    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&root);
    let canonical_db_c = memfix::canon(&canonical_db);

    let mut canonical = json!({
        "db": canonical_db.to_string_lossy(),
        "jsonl": canonical_jsonl.to_string_lossy(),
        "db_exists": canonical_db.is_file(),
        "jsonl_exists": canonical_jsonl.is_file(),
        "total": 0,
        "by_agent": {},
    });
    let mut strays: Vec<Value> = Vec::new();
    let mut scattered_total = 0usize;

    for db in memfix::scan_memory_dbs(&root) {
        let rows = match memfix::read_db_memories(&db) {
            Ok(Some(rows)) => rows,
            Ok(None) => continue, // not a genesis memory DB
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };
        let by_agent = memfix::agent_counts(&rows);
        let entry = json!({
            "path": db.to_string_lossy(),
            "total": rows.len(),
            "by_agent": by_agent,
        });
        if memfix::canon(&db) == canonical_db_c {
            canonical = json!({
                "db": canonical_db.to_string_lossy(),
                "jsonl": canonical_jsonl.to_string_lossy(),
                "db_exists": true,
                "jsonl_exists": canonical_jsonl.is_file(),
                "total": rows.len(),
                "by_agent": by_agent,
            });
        } else {
            scattered_total += rows.len();
            strays.push(entry);
        }
    }

    // The committed JSONL may hold memories the local DB doesn't (fresh clone before first server start).
    let jsonl_total = memfix::read_jsonl(&canonical_jsonl).len();

    let healthy = strays.is_empty();
    let note = if healthy {
        "All memory is in the repo's .genesis/ store — nothing to consolidate.".to_string()
    } else {
        format!(
            "{} stray memory database(s) hold {} memories OUTSIDE the repo's .genesis/ store. Run \
             `genesis-cli fix` (or /genesis:fix) to consolidate them into {}.",
            strays.len(),
            scattered_total,
            canonical_jsonl.display()
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "root": root.to_string_lossy(),
            "healthy": healthy,
            "canonical": canonical,
            "canonical_jsonl_records": jsonl_total,
            "strays": strays,
            "scattered_total": scattered_total,
            "note": note,
        }))
    );
    0
}

/// Return the value following `flag` in `args`, if present.
fn flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
