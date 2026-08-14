//! `genesis-cli resolve --repo <r> --staged <path> --retire <id> [--retire <id> ...] [--at <unix>]` —
//! the finalize step behind `/genesis:memory`, run after `merge` has staged a set with contradictions.
//!
//! For each `--retire <id>` it SUPERSEDES that memory rather than deleting it: the row is kept but stamped
//! `valid_to = <at>` (so it becomes history and drops out of the ACTIVE contradiction scope). The retirement
//! time `at` is either `--at` or a deterministic `max(valid_from|created_at) + 1` over the staged set, so a
//! given staged file always resolves identically. Then it re-checks:
//!
//! * all contradictions cleared → the finalized set is written to the canonical JSONL and the staged file is
//!   removed (`status: "resolved"`).
//! * some contradiction remains → the retirements are persisted back to the staged file and the remaining
//!   conflicts are returned so the user can retire more (`status: "remaining"`). Canonical is not touched.

use crate::{fsx, memfix};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli resolve`. Prints the JSON result and returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let repo = flag(args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let Some(staged) = flag(args, "--staged") else {
        fsx::fail(
            "usage: genesis-cli resolve --repo <repo> --staged <merge.staged.jsonl> --retire <id> \
             [--retire <id> ...] [--at <unix>]",
        );
    };
    if !repo.is_dir() {
        fsx::fail(&format!("repo not found: {}", repo.display()));
    }
    let retire_ids = match parse_retire_ids(args) {
        Ok(v) => v,
        Err(e) => fsx::fail(&e),
    };
    let at_flag = match flag(args, "--at") {
        None => None,
        Some(s) => match s.parse::<i64>() {
            Ok(v) => Some(v),
            Err(_) => fsx::fail(&format!(
                "--at expects a unix timestamp (integer), got {s:?}"
            )),
        },
    };
    println!(
        "{}",
        fsx::json_pretty(&do_resolve(&repo, Path::new(&staged), &retire_ids, at_flag))
    );
    0
}

/// Retire the requested ids in the staged set, re-check for contradictions, and either finalize to the
/// canonical JSONL or persist the retirements back to staging. Split from [`run`] so the resolution logic —
/// including its file side effects — is directly testable without capturing stdout.
fn do_resolve(
    repo: &Path,
    staged: &Path,
    retire_ids: &[i64],
    at_flag: Option<i64>,
) -> serde_json::Value {
    let mut records = memfix::read_jsonl(staged);
    // Deterministic retirement time: newest world-validity in the set, plus one — so the same staged file
    // always resolves to byte-identical output. `--at` overrides for callers that need an explicit stamp.
    let at = at_flag.unwrap_or_else(|| {
        records
            .iter()
            .map(|r| r.valid_from.unwrap_or(r.created_at))
            .max()
            .unwrap_or(0)
            + 1
    });

    let mut retired: Vec<i64> = Vec::new();
    let mut missing: Vec<i64> = Vec::new();
    for &id in retire_ids {
        if let Some(rec) = records.iter_mut().find(|r| r.id == id) {
            // Supersede, don't delete: stamp valid_to (only if still active) and keep the row as history.
            if rec.valid_to.is_none() {
                rec.valid_to = Some(at);
            }
            retired.push(id);
        } else {
            missing.push(id);
        }
    }

    let conflicts = memfix::contradictions(&records);
    let (_db, jsonl) = memfix::canonical_paths(repo);
    let retired_json = serde_json::to_value(&retired).unwrap_or(serde_json::Value::Null);
    let missing_json = serde_json::to_value(&missing).unwrap_or(serde_json::Value::Null);

    if conflicts.is_empty() {
        if let Err(e) = memfix::write_jsonl_atomic(&jsonl, &records) {
            fsx::fail(&format!("writing canonical memory: {e}"));
        }
        // Best-effort: the staged file has served its purpose once canonical is finalized.
        let _ = std::fs::remove_file(staged);
        json!({
            "repo": repo.to_string_lossy(),
            "status": "resolved",
            "retired": retired_json,
            "missing": missing_json,
            "canonical_jsonl": jsonl.to_string_lossy(),
        })
    } else {
        // Persist the retirements so far, but leave canonical alone until every contradiction is cleared.
        if let Err(e) = memfix::write_jsonl_atomic(staged, &records) {
            fsx::fail(&format!("writing staged merge: {e}"));
        }
        let remaining = serde_json::to_value(&conflicts).unwrap_or(serde_json::Value::Null);
        json!({
            "repo": repo.to_string_lossy(),
            "status": "remaining",
            "retired": retired_json,
            "missing": missing_json,
            "remaining_conflicts": remaining,
            "staged": staged.to_string_lossy(),
        })
    }
}

/// Parse every `--retire <id>` pair from `args` (there may be several), preserving order.
///
/// # Errors
/// Returns a message if a `--retire` has no following value or its value is not a valid `i64`.
fn parse_retire_ids(args: &[String]) -> Result<Vec<i64>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let Some(a) = args.get(i) else { break };
        if a == "--retire" {
            let Some(v) = args.get(i + 1) else {
                return Err("--retire requires an id".to_string());
            };
            let id = v
                .parse::<i64>()
                .map_err(|_| format!("--retire expects an integer id, got {v:?}"))?;
            out.push(id);
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(out)
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

    #[test]
    fn retiring_the_conflicting_record_resolves_and_removes_staged() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let staged = repo.join(".genesis/memory/merge.staged.jsonl");
        // blue (id 1) vs green (id 2): one active contradiction.
        write_jsonl(
            &staged,
            &[
                srec(1, "a", "ball", "color", "blue", 100),
                srec(2, "a", "ball", "color", "green", 200),
            ],
        );

        // Retire the blue assertion; 99 does not exist → tracked as missing.
        let report = do_resolve(repo, &staged, &[1, 99], None);
        assert_eq!(report["status"].as_str(), Some("resolved"));

        let retired: Vec<i64> = serde_json::from_value(report["retired"].clone()).unwrap();
        let missing: Vec<i64> = serde_json::from_value(report["missing"].clone()).unwrap();
        assert!(retired.contains(&1), "id 1 retired");
        assert!(missing.contains(&99), "id 99 reported missing");

        assert!(!staged.exists(), "staged removed once resolved");

        let (_db, jsonl) = memfix::canonical_paths(repo);
        let out = memfix::read_jsonl(&jsonl);
        let r1 = out.iter().find(|r| r.id == 1).unwrap();
        assert_eq!(
            r1.valid_to,
            Some(201),
            "retired row is superseded at max(valid_from|created)+1, not deleted"
        );
        assert!(
            memfix::contradictions(&out).is_empty(),
            "no contradiction remains in canonical"
        );
    }

    #[test]
    fn remaining_when_a_conflict_persists() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let staged = repo.join(".genesis/memory/merge.staged.jsonl");
        // Three competing objects: retiring one still leaves two → still a contradiction.
        write_jsonl(
            &staged,
            &[
                srec(1, "a", "ball", "color", "blue", 1),
                srec(2, "a", "ball", "color", "green", 2),
                srec(3, "a", "ball", "color", "red", 3),
            ],
        );

        let report = do_resolve(repo, &staged, &[1], None);
        assert_eq!(report["status"].as_str(), Some("remaining"));
        let remaining = report["remaining_conflicts"].as_array().unwrap();
        assert!(!remaining.is_empty(), "a contradiction still stands");

        assert!(
            staged.exists(),
            "staged persisted with the partial retirement"
        );
        let after = memfix::read_jsonl(&staged);
        assert_eq!(
            after.iter().find(|r| r.id == 1).unwrap().valid_to,
            Some(4),
            "the retirement is persisted back to staging (at = max created + 1)"
        );
    }
}
