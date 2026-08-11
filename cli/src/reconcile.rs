//! `genesis-cli reconcile --repo <r> --incoming <jsonl>` — the deterministic, lossless merge step of the
//! "update remote" procedure. Given the repo's local memory and an INCOMING (remote) JSONL export, it
//! decides — using the JSONL, before touching the `.db` — whether adopting the incoming set would lose
//! local context, and writes the reconciled canonical JSONL accordingly:
//!
//! * **add-only** (`local ⊆ incoming`) → adopting incoming loses nothing; the `.db` may simply be replaced.
//! * **already-synced** → identical sets; nothing changes.
//! * **merged** → local holds memories incoming lacks; the two are UNION-merged so nothing is lost.
//!
//! The written JSONL is always the lossless UNION (== incoming in the add-only case). The local `.db` then
//! catches up from this JSONL via the memory server's union-on-start. A genuine can't-decide *conflict* is
//! never produced here (the union is always safe) — that is a git-level concern the `/genesis:sync` skill
//! escalates to the user. READ-ONLY except for the one write to `<repo>/.genesis/memory/memory.jsonl`.

use crate::{fsx, memfix};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli reconcile`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let repo = flag(args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let Some(incoming_arg) = flag(args, "--incoming") else {
        fsx::fail(
            "usage: genesis-cli reconcile --repo <repo> --incoming <remote-memory.jsonl>  (the incoming \
             export, e.g. `git show <upstream>:.genesis/memory/memory.jsonl`)",
        );
    };
    if !repo.is_dir() {
        fsx::fail(&format!("repo not found: {}", repo.display()));
    }

    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&repo);

    // Local truth = the committed JSONL UNIONed with the live local DB (so unexported local memory is never
    // lost even if the server hasn't snapshotted yet). Both read-only.
    let mut local_sources = vec![memfix::read_jsonl(&canonical_jsonl)];
    if let Ok(Some(rows)) = memfix::read_db_memories(&canonical_db) {
        local_sources.push(rows);
    }
    let local = memfix::consolidate(&local_sources);
    let incoming = memfix::read_jsonl(Path::new(&incoming_arg));

    let recon = memfix::reconcile(&local, &incoming);
    if let Err(e) = memfix::write_jsonl_atomic(&canonical_jsonl, &recon.merged) {
        fsx::fail(&format!("writing reconciled memory: {e}"));
    }

    let note = match recon.status {
        memfix::ReconcileStatus::AlreadySynced => {
            "Local and remote memory are identical — nothing to merge.".to_string()
        }
        memfix::ReconcileStatus::AddOnly => format!(
            "Remote is add-only ({} new memories, no local context lost). Adopted it; the .db replace is \
             safe. Commit .genesis/memory.db + memory.jsonl and push.",
            recon.added_from_incoming
        ),
        memfix::ReconcileStatus::Merged => format!(
            "Merged deterministically (lossless UNION): brought in {} remote memories and PRESERVED {} \
             local-only memories a plain replace would have lost. The .db rebuilds from this JSONL on the \
             next server start. Commit .genesis/memory.db + memory.jsonl and push.",
            recon.added_from_incoming, recon.kept_local_only
        ),
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "repo": repo.to_string_lossy(),
            "canonical_jsonl": canonical_jsonl.to_string_lossy(),
            "status": recon.status.as_str(),
            "local": recon.local,
            "incoming": recon.incoming,
            "added_from_incoming": recon.added_from_incoming,
            "kept_local_only": recon.kept_local_only,
            "records_after": recon.merged.len(),
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
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
        }
    }

    #[test]
    fn reconcile_writes_union_and_preserves_local_only() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (_db, jsonl) = memfix::canonical_paths(repo);
        write_jsonl(
            &jsonl,
            &[rec(1, "a", "local only", 1), rec(2, "a", "shared", 2)],
        );

        let incoming = repo.join("incoming.jsonl");
        write_jsonl(
            &incoming,
            &[rec(9, "a", "shared", 2), rec(8, "a", "remote only", 3)],
        );

        let code = run(&[
            "--repo".into(),
            repo.to_string_lossy().into_owned(),
            "--incoming".into(),
            incoming.to_string_lossy().into_owned(),
        ]);
        assert_eq!(code, 0);

        let out = memfix::read_jsonl(&jsonl);
        let texts: Vec<&str> = out.iter().map(|r| r.text.as_str()).collect();
        assert!(
            texts.contains(&"local only"),
            "local-only memory must survive"
        );
        assert!(texts.contains(&"shared"));
        assert!(texts.contains(&"remote only"));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn reconcile_also_unions_the_live_db() {
        // A memory only in the local .db (not yet exported to JSONL) must not be lost.
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        let (db, jsonl) = memfix::canonical_paths(repo);
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (id INTEGER PRIMARY KEY, agent_id TEXT, text TEXT, created_at INTEGER, \
             last_used_at INTEGER, use_count INTEGER, base_score REAL, superseded_by INTEGER);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories VALUES (1,'a','db only memory',1,1,0,1.0,NULL)",
            [],
        )
        .unwrap();
        drop(conn);

        let incoming = repo.join("incoming.jsonl");
        write_jsonl(&incoming, &[rec(1, "a", "remote memory", 2)]);

        assert_eq!(
            run(&[
                "--repo".into(),
                repo.to_string_lossy().into_owned(),
                "--incoming".into(),
                incoming.to_string_lossy().into_owned(),
            ]),
            0
        );
        let texts: Vec<String> = memfix::read_jsonl(&jsonl)
            .into_iter()
            .map(|r| r.text)
            .collect();
        assert!(
            texts.iter().any(|t| t == "db only memory"),
            "unexported local db memory must survive"
        );
        assert!(texts.iter().any(|t| t == "remote memory"));
    }
}
