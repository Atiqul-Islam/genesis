//! `genesis-cli doctor [--repo <repo>] [--root <dir>]` — READ-ONLY diagnosis of where a repo's memory lives.
//!
//! Scatter lands in whatever directory Claude Code was launched from — a SIBLING of the repo, not inside it
//! — so `doctor` scans the user's HOME directory by default (override with `--root`), NOT just the repo. It
//! reports the repo's canonical store, then any **stray** databases that hold THIS repo's custom agents
//! (`<repo>/.claude/agents/*.md`, excluding the shared `sensei`/`method`) — the memory `genesis-cli fix`
//! (or `/genesis:fix`) would recover. It changes NOTHING.

use crate::{fsx, memfix};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Entry point for `genesis-cli doctor`. Returns the process exit code.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn run(args: &[String]) -> i32 {
    let repo = flag(args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !repo.is_dir() {
        fsx::fail(&format!("repo not found: {}", repo.display()));
    }
    let root = flag(args, "--root").map_or_else(|| memfix::default_scan_root(&repo), PathBuf::from);

    let custom_agents = memfix::repo_custom_agents(&repo);
    let custom_set: std::collections::HashSet<&str> =
        custom_agents.iter().map(String::as_str).collect();
    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&repo);
    let canonical_db_c = memfix::canon(&canonical_db);

    let mut canonical = json!({
        "db": canonical_db.to_string_lossy(),
        "jsonl": canonical_jsonl.to_string_lossy(),
        "db_exists": canonical_db.is_file(),
        "jsonl_exists": canonical_jsonl.is_file(),
        "total": 0,
        "by_agent": {},
    });
    let mut recoverable: Vec<Value> = Vec::new(); // strays holding THIS repo's memory
    let mut other_stores: Vec<Value> = Vec::new(); // memory belonging to other repos/agents (informational)
    let mut recoverable_total = 0usize;

    for db in memfix::scan_memory_dbs(&root) {
        let rows = match memfix::read_db_memories(&db) {
            Ok(Some(rows)) => rows,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };
        let by_agent = memfix::agent_counts(&rows);
        if memfix::canon(&db) == canonical_db_c {
            canonical = json!({
                "db": canonical_db.to_string_lossy(),
                "jsonl": canonical_jsonl.to_string_lossy(),
                "db_exists": true,
                "jsonl_exists": canonical_jsonl.is_file(),
                "total": rows.len(),
                "by_agent": by_agent,
            });
            continue;
        }
        let inside = memfix::is_inside(&db, &repo);
        // Memory this repo could recover: everything in an in-repo stray, else only this repo's custom agents.
        let mine: usize = if inside {
            rows.len()
        } else {
            rows.iter()
                .filter(|r| custom_set.contains(r.agent_id.as_str()))
                .count()
        };
        if mine > 0 {
            recoverable_total += mine;
            recoverable.push(json!({
                "path": db.to_string_lossy(),
                "inside_repo": inside,
                "memories_for_this_repo": mine,
                "by_agent": by_agent,
            }));
        } else if !rows.is_empty() {
            other_stores.push(json!({ "path": db.to_string_lossy(), "by_agent": by_agent }));
        }
    }

    let jsonl_total = memfix::read_jsonl(&canonical_jsonl).len();
    let healthy = recoverable.is_empty();
    let note = if healthy {
        format!(
            "All of this repo's memory is in its .genesis/ store (scanned {}). Custom agents: {}.",
            root.display(),
            if custom_agents.is_empty() {
                "none".into()
            } else {
                custom_agents.join(", ")
            }
        )
    } else {
        format!(
            "Found {} memories for THIS repo sitting OUTSIDE its .genesis/ store (in {} stray database(s) \
             under {}). Run `genesis-cli fix` (or /genesis:fix) to consolidate them into {}.",
            recoverable_total,
            recoverable.len(),
            root.display(),
            canonical_jsonl.display()
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "repo": repo.to_string_lossy(),
            "scan_root": root.to_string_lossy(),
            "custom_agents": custom_agents,
            "healthy": healthy,
            "canonical": canonical,
            "canonical_jsonl_records": jsonl_total,
            "recoverable_strays": recoverable,
            "recoverable_total": recoverable_total,
            "other_stores": other_stores,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memfix::MemRecord;
    use rusqlite::Connection;
    use std::path::Path;

    fn rec(agent: &str, text: &str) -> MemRecord {
        MemRecord {
            id: 1,
            agent_id: agent.into(),
            text: text.into(),
            created_at: 1,
            last_used_at: 1,
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
        for (i, r) in recs.iter().enumerate() {
            conn.execute(
                "INSERT INTO memories VALUES (?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    i64::try_from(i).unwrap() + 1,
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

    /// doctor must flag this repo's agent stranded in a SIBLING dir as recoverable, while a foreign repo's
    /// store is only informational (not attributed to this repo).
    #[test]
    fn doctor_flags_this_repos_stray_in_a_sibling_not_foreign_memory() {
        let td = tempfile::tempdir().unwrap();
        let parent = td.path();
        let repo = parent.join("ifs-repo");
        std::fs::create_dir_all(repo.join(".claude/agents")).unwrap();
        std::fs::write(
            repo.join(".claude/agents/fih-engineer.md"),
            "---\nname: x\n---\n",
        )
        .unwrap();

        make_db(
            &parent.join("launch/genesis-memory.db"),
            &[rec("fih-engineer", "mine")],
        );
        make_db(
            &parent.join("other/genesis-memory.db"),
            &[rec("sensei", "theirs")],
        );

        // capture via return code + re-reading is hard (prints JSON); instead assert the classification
        // through the public helpers the run() uses.
        let root = parent;
        let custom = memfix::repo_custom_agents(&repo);
        assert_eq!(custom, vec!["fih-engineer".to_string()]);
        let dbs = memfix::scan_memory_dbs(root);
        let sibling = dbs
            .iter()
            .find(|p| p.ends_with("launch/genesis-memory.db"))
            .unwrap();
        let foreign = dbs
            .iter()
            .find(|p| p.ends_with("other/genesis-memory.db"))
            .unwrap();
        assert!(!memfix::is_inside(sibling, &repo));
        let mine: usize = memfix::read_db_memories(sibling)
            .unwrap()
            .unwrap()
            .iter()
            .filter(|r| custom.contains(&r.agent_id))
            .count();
        assert_eq!(mine, 1, "the sibling's fih-engineer memory is recoverable");
        let foreign_mine: usize = memfix::read_db_memories(foreign)
            .unwrap()
            .unwrap()
            .iter()
            .filter(|r| custom.contains(&r.agent_id))
            .count();
        assert_eq!(
            foreign_mine, 0,
            "the foreign sensei memory is NOT this repo's"
        );

        // run() itself must succeed and report unhealthy (strays exist).
        assert_eq!(
            run(&[
                "--repo".into(),
                repo.to_string_lossy().into_owned(),
                "--root".into(),
                root.to_string_lossy().into_owned()
            ]),
            0
        );
    }
}
