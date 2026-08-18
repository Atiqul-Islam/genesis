//! `genesis-cli validate --repo <r>` — the READ-ONLY health check behind `/genesis:memory`.
//!
//! It compares the repo's two memory faces — the canonical `.db` vector store and its `memory.jsonl`
//! mirror — and reports whether they agree AND whether the store holds any semantic contradiction (one
//! `(agent, subject, relation)` asserted with different objects among ACTIVE, structured memories). It
//! never writes: `validate` diagnoses, `merge`/`resolve` are the write steps. The one fatal case is a bad
//! `--repo`; everything else is reported as data so Mneme can act on it.

use crate::{fsx, memfix};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli validate`. Prints the JSON report and returns the process exit code
/// (`0` — this command is read-only and always succeeds once `--repo` resolves to a directory).
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let repo = flag(args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !repo.is_dir() {
        fsx::fail(&format!("repo not found: {}", repo.display()));
    }
    println!("{}", fsx::json_pretty(&build_report(&repo)));
    0
}

/// Compute the read-only validation report for `repo` as a JSON value (pure — no writes), so the CLI
/// entry point only has to print it and the logic stays directly testable.
fn build_report(repo: &Path) -> serde_json::Value {
    let (db, jsonl) = memfix::canonical_paths(repo);
    // A missing / unreadable `.db` is treated as an empty store (Ok(None) or an open error), exactly as
    // `reconcile` does — the JSONL mirror is the diff substrate and the canonical store may not exist yet.
    let db_recs = memfix::read_db_memories(&db)
        .ok()
        .flatten()
        .unwrap_or_default();
    let jsonl_recs = memfix::read_jsonl(&jsonl);

    let db_count = db_recs.len();
    let jsonl_count = jsonl_recs.len();

    // Consistency is keyed on identity `(agent_id, text)`, not raw row count, so re-ids / metadata drift
    // never read as an inconsistency.
    let key = |r: &memfix::MemRecord| (r.agent_id.clone(), r.text.clone());
    let db_keys: HashSet<(String, String)> = db_recs.iter().map(key).collect();
    let jsonl_keys: HashSet<(String, String)> = jsonl_recs.iter().map(key).collect();
    let only_in_db = db_keys.difference(&jsonl_keys).count();
    let only_in_jsonl = jsonl_keys.difference(&db_keys).count();
    let consistent = only_in_db == 0 && only_in_jsonl == 0;

    // Contradictions are detected over the lossless UNION of both faces, so a conflict is caught whether
    // it lives in the `.db`, the JSONL, or is only visible once the two are combined.
    let conflicts = memfix::contradictions(&memfix::consolidate(&[db_recs, jsonl_recs]));
    let contradiction_count = conflicts.len();
    let healthy = consistent && contradiction_count == 0;
    let contradictions_json = serde_json::to_value(&conflicts).unwrap_or(serde_json::Value::Null);

    json!({
        "repo": repo.to_string_lossy(),
        "db": db.to_string_lossy(),
        "jsonl": jsonl.to_string_lossy(),
        "db_count": db_count,
        "jsonl_count": jsonl_count,
        "only_in_db": only_in_db,
        "only_in_jsonl": only_in_jsonl,
        "consistent": consistent,
        "contradiction_count": contradiction_count,
        "contradictions": contradictions_json,
        "healthy": healthy,
    })
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

    fn write_jsonl(path: &Path, recs: &[MemRecord]) {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(path, memfix::to_jsonl(recs)).unwrap();
    }

    fn rec(id: i64, agent: &str, text: &str, created: i64) -> MemRecord {
        MemRecord {
            id,
            agent_id: agent.into(),
            text: text.into(),
            created_at: created,
            last_used_at: created,
            base_score: 1.0,
            ..Default::default()
        }
    }

    /// A fully-structured, ACTIVE memory (participates in contradiction detection).
    fn srec(id: i64, agent: &str, subj: &str, rel: &str, obj: &str, created: i64) -> MemRecord {
        MemRecord {
            id,
            agent_id: agent.into(),
            text: format!("{subj} {rel} {obj}"),
            created_at: created,
            last_used_at: created,
            base_score: 1.0,
            mem_type: Some("semantic".into()),
            subject: Some(subj.into()),
            relation: Some(rel.into()),
            object: Some(obj.into()),
            valid_from: Some(created),
            ingested_at: Some(created),
            ..Default::default()
        }
    }

    /// Build a minimal (legacy 8-column) genesis memory DB so a real `.db` face can be compared.
    fn make_db(path: &Path, recs: &[MemRecord]) {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).unwrap();
        }
        let conn = rusqlite::Connection::open(path).unwrap();
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
    fn healthy_when_db_and_jsonl_agree_and_no_contradictions() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (db, jsonl) = memfix::canonical_paths(repo);
        let recs = [rec(1, "a", "one", 1), rec(2, "a", "two", 2)];
        make_db(&db, &recs);
        write_jsonl(&jsonl, &recs);

        let report = build_report(repo);
        assert_eq!(report["healthy"].as_bool(), Some(true));
        assert_eq!(report["consistent"].as_bool(), Some(true));
        assert_eq!(report["contradiction_count"].as_u64(), Some(0));
        assert_eq!(report["db_count"].as_u64(), Some(2));
        assert_eq!(report["jsonl_count"].as_u64(), Some(2));
        assert_eq!(report["only_in_db"].as_u64(), Some(0));
        assert_eq!(report["only_in_jsonl"].as_u64(), Some(0));
    }

    #[test]
    fn contradiction_in_jsonl_makes_it_unhealthy() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (_db, jsonl) = memfix::canonical_paths(repo);
        // "ball color blue" vs "ball color green" — both active, structured → one contradiction.
        write_jsonl(
            &jsonl,
            &[
                srec(1, "a", "ball", "color", "blue", 1),
                srec(2, "a", "ball", "color", "green", 2),
            ],
        );

        let report = build_report(repo);
        let cc = report["contradiction_count"].as_u64().unwrap();
        assert!(cc >= 1, "expected at least one contradiction, got {cc}");
        assert_eq!(report["healthy"].as_bool(), Some(false));
    }
}
