//! `genesis-cli merge --repo <r> --incoming <jsonl>` — the contradiction-aware merge step behind
//! `/genesis:memory`.
//!
//! It reconciles the repo's local memory (canonical JSONL UNIONed with the live `.db`) against an INCOMING
//! JSONL export using the same lossless UNION as `reconcile`, THEN checks the merged set for semantic
//! contradictions:
//!
//! * **clean** (no contradiction) → the merged UNION is written straight to the canonical JSONL and the
//!   status is the reconcile classification (`already-synced` | `add-only` | `merged`).
//! * **conflicts** → the canonical store is left UNTOUCHED. The merged UNION is written to a *staged* file
//!   and a self-contained HTML report of the contradictions is emitted, so the user (via Mneme) can decide
//!   which object is correct and `resolve` can finalize. Nothing is ever dropped in either case.

use crate::{fsx, memfix};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli merge`. Prints the JSON result and returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let repo = flag(args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let Some(incoming) = flag(args, "--incoming") else {
        fsx::fail(
            "usage: genesis-cli merge --repo <repo> --incoming <memory.jsonl>  (the incoming export to \
             merge into this repo's canonical memory)",
        );
    };
    if !repo.is_dir() {
        fsx::fail(&format!("repo not found: {}", repo.display()));
    }
    println!(
        "{}",
        fsx::json_pretty(&do_merge(&repo, Path::new(&incoming)))
    );
    0
}

/// Reconcile local memory against `incoming_path`, act on the outcome (write canonical OR stage +
/// report contradictions), and return the JSON result. Split from [`run`] so the merge logic — including
/// its file side effects — is directly testable without capturing stdout.
fn do_merge(repo: &Path, incoming_path: &Path) -> serde_json::Value {
    let (db, jsonl) = memfix::canonical_paths(repo);
    // Local truth = committed JSONL UNIONed with the live local `.db` (a missing/unreadable `.db` is an
    // empty contribution — same as `reconcile`), so unexported local memory is never lost.
    let local = memfix::consolidate(&[
        memfix::read_jsonl(&jsonl),
        memfix::read_db_memories(&db)
            .ok()
            .flatten()
            .unwrap_or_default(),
    ]);
    let incoming = memfix::read_jsonl(incoming_path);

    let recon = memfix::reconcile(&local, &incoming);
    let merged = recon.merged;
    let conflicts = memfix::contradictions(&merged);

    let staged = repo
        .join(".genesis")
        .join("memory")
        .join("merge.staged.jsonl");
    let html = repo
        .join(".genesis")
        .join("memory")
        .join("contradictions.html");

    if conflicts.is_empty() {
        // Safe to adopt: write the lossless UNION as the new canonical JSONL.
        if let Err(e) = memfix::write_jsonl_atomic(&jsonl, &merged) {
            fsx::fail(&format!("writing canonical memory: {e}"));
        }
        json!({
            "repo": repo.to_string_lossy(),
            "status": recon.status.as_str(),
            "incoming": recon.incoming,
            "added_from_incoming": recon.added_from_incoming,
            "kept_local_only": recon.kept_local_only,
            "records_after": merged.len(),
            "contradiction_count": 0,
            "canonical_jsonl": jsonl.to_string_lossy(),
        })
    } else {
        // Do NOT touch canonical: stage the merged set + emit the contradictions report for resolution.
        if let Err(e) = memfix::write_jsonl_atomic(&staged, &merged) {
            fsx::fail(&format!("writing staged merge: {e}"));
        }
        if let Some(d) = html.parent() {
            if let Err(e) = std::fs::create_dir_all(d) {
                fsx::fail(&format!("mkdir {}: {e}", d.display()));
            }
        }
        if let Err(e) = std::fs::write(&html, memfix::contradictions_html(&conflicts, repo)) {
            fsx::fail(&format!("writing {}: {e}", html.display()));
        }
        let contradictions_json =
            serde_json::to_value(&conflicts).unwrap_or(serde_json::Value::Null);
        json!({
            "repo": repo.to_string_lossy(),
            "status": "conflicts",
            "incoming": recon.incoming,
            "added_from_incoming": recon.added_from_incoming,
            "kept_local_only": recon.kept_local_only,
            "records_after": merged.len(),
            "contradiction_count": conflicts.len(),
            "contradictions": contradictions_json,
            "html": html.to_string_lossy(),
            "staged": staged.to_string_lossy(),
            "note": "Resolve the contradictions, then Mneme finalizes; canonical was NOT changed.",
        })
    }
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

    #[test]
    fn clean_union_writes_canonical_and_no_staged() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (_db, jsonl) = memfix::canonical_paths(repo);
        write_jsonl(&jsonl, &[rec(1, "a", "note one", 1)]);

        let incoming = repo.join("incoming.jsonl");
        write_jsonl(&incoming, &[rec(1, "a", "note two", 2)]);

        let report = do_merge(repo, &incoming);
        assert_ne!(
            report["status"].as_str(),
            Some("conflicts"),
            "a non-conflicting merge is never staged"
        );
        assert_eq!(report["contradiction_count"].as_u64(), Some(0));

        let staged = repo.join(".genesis/memory/merge.staged.jsonl");
        assert!(
            !staged.exists(),
            "staged must NOT be written on a clean merge"
        );
        let out = memfix::read_jsonl(&jsonl);
        assert_eq!(out.len(), 2, "canonical holds the lossless union");
    }

    #[test]
    fn conflicting_incoming_stages_and_leaves_canonical_untouched() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (_db, jsonl) = memfix::canonical_paths(repo);
        write_jsonl(&jsonl, &[srec(1, "a", "ball", "color", "blue", 1)]);

        let incoming = repo.join("incoming.jsonl");
        write_jsonl(&incoming, &[srec(1, "a", "ball", "color", "green", 2)]);

        let report = do_merge(repo, &incoming);
        assert_eq!(report["status"].as_str(), Some("conflicts"));
        assert!(report["contradiction_count"].as_u64().unwrap() >= 1);

        let staged = repo.join(".genesis/memory/merge.staged.jsonl");
        let html = repo.join(".genesis/memory/contradictions.html");
        assert!(staged.exists(), "staged UNION written on conflict");
        assert!(html.exists(), "contradictions report written on conflict");

        // Canonical must be untouched: still exactly the original single "blue" record.
        let out = memfix::read_jsonl(&jsonl);
        assert_eq!(out.len(), 1, "canonical must not change on conflict");
        assert_eq!(out[0].object.as_deref(), Some("blue"));
    }
}
