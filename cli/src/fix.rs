//! `genesis-cli fix --into <repo> (--scope user|system | --root <dir>...) [--agent <name>] [--all-agents]
//! [--archive]` — CONSOLIDATE scattered memory INTO the repo's canonical `.db` (the source of truth).
//!
//! Scatter lands in whatever directory Claude Code was launched from — a SIBLING of the repo, NOT inside it,
//! and it can be anywhere — so the scan area is the USER's choice (`--scope user|system` or explicit `--root`
//! path(s)), never a guessed default. The repo itself is always scanned (via `resolve_scan_roots`).
//!
//! **The `.db` is the source of truth.** Stray DBs already hold their embeddings, so `fix` copies each
//! memory AND its embedding blob straight into `<repo>/.genesis/memory.db` — no re-embedding, no ONNX, and
//! the consolidated memory is **recall-able immediately, with no server restart**. The `.jsonl` is then
//! re-exported from the `.db` as its derived merge/diff mirror. Dedup is by `(agent_id, text)` — nothing is
//! overwritten or dropped. To avoid a broad scan pulling OTHER repos' memory in, an external stray only
//! contributes memories whose `agent_id` is one of THIS repo's custom agents (`.claude/agents/*.md`, minus
//! the shared `sensei`/`method`); a stray physically INSIDE the repo contributes all of its memories.
//! `--agent <name>` targets one agent; `--all-agents` takes every agent found.
//!
//! **Zero footprint** outside the target repo: strays are only READ; the sole writes are the repo's own
//! `.db` + `.jsonl`. Idempotent. `--archive` COPIES (never moves) contributing strays into
//! `<repo>/.genesis/memory/archived-strays/`.

use crate::{fsx, memfix};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli fix`. Returns the process exit code.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn run(args: &[String]) -> i32 {
    let into = flag(args, "--into").map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !into.is_dir() {
        fsx::fail(&format!("target repo not found: {}", into.display()));
    }
    let scan_scope = scope_of(args);
    let explicit: Vec<PathBuf> = flag_values(args, "--root")
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let roots = match memfix::resolve_scan_roots(scan_scope, &explicit, &into) {
        Ok(r) => r,
        Err(e) => fsx::fail(&e),
    };
    let roots_display = roots
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let archive = args.iter().any(|a| a == "--archive");

    // Which external agents' memories may be pulled in: an explicit --agent, everything (--all-agents), or
    // (default) this repo's custom agents. In-repo strays ignore this filter (they are unambiguously ours).
    let filter: Option<HashSet<String>> = if let Some(a) = flag(args, "--agent") {
        Some([a].into_iter().collect())
    } else if args.iter().any(|a| a == "--all-agents") {
        None
    } else {
        Some(memfix::repo_custom_agents(&into).into_iter().collect())
    };
    let agent_scope = describe_scope(filter.as_ref());

    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&into);
    let canonical_db_c = memfix::canon(&canonical_db);

    // Open the canonical .db — the source of truth we merge into (created with schema if missing).
    let conn = match memfix::open_memory_db(&canonical_db) {
        Ok(c) => c,
        Err(e) => fsx::fail(&e),
    };
    let before = memfix::db_records(&conn).map(|r| r.len()).unwrap_or(0);
    let mut seen = match memfix::existing_keys(&conn) {
        Ok(s) => s,
        Err(e) => fsx::fail(&e),
    };

    // Collect the memories (with embeddings) to bring in, deduping by (agent_id, text) against the canonical
    // store and within this run. Strays are READ-ONLY.
    let mut to_insert: Vec<memfix::EmbeddedRecord> = Vec::new();
    let mut consolidated_from: Vec<Value> = Vec::new();
    let mut archived: Vec<Value> = Vec::new();

    for db in memfix::scan_memory_dbs_in(&roots) {
        if memfix::canon(&db) == canonical_db_c {
            continue; // the canonical DB is the target, not a source
        }
        let recs = match memfix::read_active_with_embeddings(&db) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("warning: {e}");
                continue;
            }
        };
        let inside = memfix::is_inside(&db, &into);
        let selected: Vec<memfix::EmbeddedRecord> = recs
            .into_iter()
            .filter(|e| inside || filter.as_ref().is_none_or(|f| f.contains(&e.rec.agent_id)))
            .filter(|e| seen.insert((e.rec.agent_id.clone(), e.rec.text.clone())))
            .collect();
        if selected.is_empty() {
            continue;
        }
        let mut agents: Vec<String> = selected.iter().map(|e| e.rec.agent_id.clone()).collect();
        agents.sort();
        agents.dedup();
        consolidated_from.push(
            json!({ "path": db.to_string_lossy(), "memories": selected.len(), "agents": agents }),
        );
        if archive {
            match archive_copy(&db, &into) {
                Ok(dest) => archived.push(json!(dest.to_string_lossy())),
                Err(e) => eprintln!("warning: archive {}: {e}", db.display()),
            }
        }
        to_insert.extend(selected);
    }

    // Write the new memories + their embeddings into the .db in one transaction (recall-able immediately).
    let added = to_insert.len();
    if added > 0 {
        let tx = match conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => fsx::fail(&format!("begin merge transaction: {e}")),
        };
        for e in &to_insert {
            if let Err(err) = memfix::insert_embedded(&conn, e) {
                fsx::fail(&format!("merging memory into the .db: {err}"));
            }
        }
        if let Err(e) = tx.commit() {
            fsx::fail(&format!("commit merge: {e}"));
        }
    }

    // Re-export the JSONL mirror from the now-updated .db (the derived merge/diff substrate).
    let after_records = match memfix::db_records(&conn) {
        Ok(r) => r,
        Err(e) => fsx::fail(&e),
    };
    let after = after_records.len();
    if let Err(e) = memfix::write_jsonl_atomic(&canonical_jsonl, &after_records) {
        fsx::fail(&format!("re-exporting the JSONL mirror: {e}"));
    }

    let note = if consolidated_from.is_empty() {
        format!(
            "No stray memory for this repo found in the {roots_display} scan (agent scope: {agent_scope}). \
             The canonical store {} holds {after} memories.",
            canonical_db.display()
        )
    } else {
        format!(
            "Consolidated {added} memories from {} stray database(s) (agent scope: {agent_scope}) straight \
             into {} — embeddings copied, so they are recall-able NOW with no server restart. The JSONL \
             mirror at {} was re-exported. Strays were only READ.",
            consolidated_from.len(),
            canonical_db.display(),
            canonical_jsonl.display()
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "into": into.to_string_lossy(),
            "scan_roots": roots.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "canonical_db": canonical_db.to_string_lossy(),
            "canonical_jsonl": canonical_jsonl.to_string_lossy(),
            "records_before": before,
            "records_after": after,
            "added": added,
            "consolidated_from": consolidated_from,
            "archived": archived,
            "note": note,
        }))
    );
    0
}

/// A short human description of which agents' external memories are eligible.
fn describe_scope(filter: Option<&HashSet<String>>) -> String {
    match filter {
        None => "all agents".to_string(),
        Some(set) if set.is_empty() => {
            "in-repo strays only (no custom agents installed)".to_string()
        }
        Some(set) => {
            let mut v: Vec<&str> = set.iter().map(String::as_str).collect();
            v.sort_unstable();
            format!("this repo's agents [{}] + any in-repo strays", v.join(", "))
        }
    }
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

/// Return every value following an occurrence of `name` (so `--root` can be repeated).
fn flag_values(args: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            if let Some(v) = args.get(i + 1) {
                out.push(v.clone());
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Parse `--scope user|system` (absent → `None`); a present-but-invalid value is a fatal usage error.
fn scope_of(args: &[String]) -> Option<memfix::Scope> {
    flag(args, "--scope").map(|s| {
        memfix::Scope::parse(&s)
            .unwrap_or_else(|| fsx::fail(&format!("invalid --scope {s:?} (expected user|system)")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memfix::{EmbeddedRecord, MemRecord};

    /// A valid 384-dim embedding blob (1536 bytes) with `seed` as its first component.
    fn emb(seed: f32) -> Vec<u8> {
        let mut b = vec![0u8; 384 * 4];
        b[0..4].copy_from_slice(&seed.to_ne_bytes());
        b
    }

    fn rec(agent: &str, text: &str) -> MemRecord {
        MemRecord {
            id: 0,
            agent_id: agent.into(),
            text: text.into(),
            created_at: 5,
            last_used_at: 5,
            use_count: 0,
            base_score: 1.0,
            superseded_by: None,
            ..Default::default()
        }
    }

    /// Build a stray genesis memory DB (memories + vec_items) with real embeddings.
    fn make_db(path: &Path, items: &[(MemRecord, Vec<u8>)]) {
        let conn = memfix::open_memory_db(path).unwrap();
        for (r, e) in items {
            memfix::insert_embedded(
                &conn,
                &EmbeddedRecord {
                    rec: r.clone(),
                    embedding: e.clone(),
                },
            )
            .unwrap();
        }
    }

    fn install_agent(repo: &Path, name: &str) {
        let p = repo
            .join(".claude")
            .join("agents")
            .join(format!("{name}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "---\nname: x\n---\n").unwrap();
    }

    fn vec_count(db: &Path) -> i64 {
        let conn = memfix::open_memory_db(db).unwrap();
        conn.query_row("SELECT COUNT(*) FROM vec_items", [], |r| r.get(0))
            .unwrap()
    }

    /// The core new behavior: fix copies memories AND their embeddings into the canonical .db, so it is
    /// immediately recall-able (vec_items populated) — no restart — and the JSONL mirror is re-exported.
    #[test]
    fn fix_merges_embeddings_into_the_db_no_restart_needed() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        install_agent(repo, "fih-engineer");
        // an in-repo stray with an embedded memory
        make_db(
            &repo.join("genesis-memory.db"),
            &[(rec("fih-engineer", "the spec"), emb(0.7))],
        );

        let code = run(&[
            "--into".into(),
            repo.to_string_lossy().into_owned(),
            "--root".into(),
            repo.to_string_lossy().into_owned(),
        ]);
        assert_eq!(code, 0);

        let (db, jsonl) = memfix::canonical_paths(repo);
        // The canonical .db has the memory AND its embedding row — recall-able immediately.
        let recs = memfix::db_records(&memfix::open_memory_db(&db).unwrap()).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].text, "the spec");
        assert_eq!(vec_count(&db), 1, "embedding copied into vec_items");
        // The JSONL mirror was re-exported from the .db.
        assert_eq!(memfix::read_jsonl(&jsonl).len(), 1);
    }

    /// Sibling stray for THIS repo's custom agent is pulled; a foreign repo's memory is not.
    fn foreign_and_own(archive: bool) {
        let td = tempfile::tempdir().unwrap();
        let parent = td.path();
        let repo = parent.join("ifs-repo");
        std::fs::create_dir_all(&repo).unwrap();
        install_agent(&repo, "fih-engineer");
        make_db(
            &parent.join("launch/genesis-memory.db"),
            &[(rec("fih-engineer", "mine"), emb(0.1))],
        );
        make_db(
            &parent.join("other/genesis-memory.db"),
            &[(rec("sensei", "theirs"), emb(0.2))],
        );

        let mut args = vec![
            "--into".to_string(),
            repo.to_string_lossy().into_owned(),
            "--root".into(),
            parent.to_string_lossy().into_owned(),
        ];
        if archive {
            args.push("--archive".into());
        }
        assert_eq!(run(&args), 0);

        let (db, _jsonl) = memfix::canonical_paths(&repo);
        let texts: Vec<String> = memfix::db_records(&memfix::open_memory_db(&db).unwrap())
            .unwrap()
            .into_iter()
            .map(|r| r.text)
            .collect();
        assert!(
            texts.contains(&"mine".to_string()),
            "own agent's sibling memory pulled"
        );
        assert!(
            !texts.contains(&"theirs".to_string()),
            "foreign memory NOT pulled"
        );
        assert_eq!(vec_count(&db), 1);
    }

    #[test]
    fn fix_pulls_own_agent_not_foreign() {
        foreign_and_own(false);
    }

    #[test]
    fn fix_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        install_agent(repo, "fih-engineer");
        make_db(
            &repo.join("genesis-memory.db"),
            &[
                (rec("fih-engineer", "one"), emb(0.1)),
                (rec("fih-engineer", "two"), emb(0.2)),
            ],
        );
        let (db, _jsonl) = memfix::canonical_paths(repo);
        let args = [
            "--into".to_string(),
            repo.to_string_lossy().into_owned(),
            "--root".into(),
            repo.to_string_lossy().into_owned(),
        ];
        assert_eq!(run(&args), 0);
        let first = vec_count(&db);
        assert_eq!(run(&args), 0);
        let second = vec_count(&db);
        assert_eq!(first, 2);
        assert_eq!(
            first, second,
            "re-running fix adds nothing (dedup by agent_id+text)"
        );
    }
}
