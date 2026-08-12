//! `genesis-cli fix --into <repo> (--scope user|system | --root <dir>...) [--agent <name>] [--all-agents]
//! [--archive]` — CONSOLIDATE scattered memory into the repo's canonical store, losslessly + deterministically.
//!
//! Scatter lands in whatever directory Claude Code was launched from — a SIBLING of the repo, NOT inside it,
//! and it can be anywhere — so the scan area is the USER's choice (`--scope user|system` or explicit `--root`
//! path(s)), never a guessed default. To avoid a broad scan pulling OTHER repos' memory in, an external stray
//! only contributes memories whose
//! `agent_id` is one of THIS repo's custom agents (`<repo>/.claude/agents/*.md`, excluding the shared
//! `sensei`/`method` builder team). A stray physically INSIDE the repo is unambiguously the repo's, so all
//! of its memories are taken regardless of agent. `--agent <name>` targets one agent; `--all-agents` takes
//! every agent found (the old broad behavior).
//!
//! Selected memories are UNION-merged (by `(agent_id, text)`) with the repo's existing JSONL + local DB into
//! `<repo>/.genesis/memory/memory.jsonl` — nothing is overwritten or lost. The local `.db` catches up from
//! the JSONL on the next server start. **Zero footprint:** strays are only READ; the sole write is the
//! target repo's JSONL. Idempotent. `--archive` COPIES (never moves) contributing strays into
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

    let (canonical_db, canonical_jsonl) = memfix::canonical_paths(&into);
    let canonical_db_c = memfix::canon(&canonical_db);

    // Priority order for the union (earlier wins duplicate metadata): the repo's committed JSONL, then its
    // local DB, then the selected strays in scan order — so repo-native memory keeps its own metadata.
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

    for db in memfix::scan_memory_dbs_in(&roots) {
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
        let selected = select(rows, &db, &into, filter.as_ref());
        if selected.is_empty() {
            continue; // holds nothing that belongs to this repo
        }
        stray_memories += selected.len();
        let agents: Vec<String> = {
            let mut a: Vec<String> = selected.iter().map(|r| r.agent_id.clone()).collect();
            a.sort();
            a.dedup();
            a
        };
        consolidated_from.push(
            json!({ "path": db.to_string_lossy(), "memories": selected.len(), "agents": agents }),
        );
        if archive {
            match archive_copy(&db, &into) {
                Ok(dest) => archived.push(json!(dest.to_string_lossy())),
                Err(e) => eprintln!("warning: archive {}: {e}", db.display()),
            }
        }
        sources.push(selected);
    }

    let merged = memfix::consolidate(&sources);
    let after = merged.len();
    if let Err(e) = memfix::write_jsonl_atomic(&canonical_jsonl, &merged) {
        fsx::fail(&format!("writing consolidated memory: {e}"));
    }

    let scope = match &filter {
        None => "all agents".to_string(),
        Some(set) if set.is_empty() => {
            "in-repo strays only (no custom agents installed)".to_string()
        }
        Some(set) => {
            let mut v: Vec<&str> = set.iter().map(String::as_str).collect();
            v.sort_unstable();
            format!("this repo's agents [{}] + any in-repo strays", v.join(", "))
        }
    };
    let note = if consolidated_from.is_empty() {
        format!(
            "No stray memory for this repo found in the {roots_display} scan. Agent scope: {scope}. The repo \
             store at {} holds {after} memories.",
            canonical_jsonl.display()
        )
    } else {
        format!(
            "Consolidated {} stray database(s) ({stray_memories} memories; scope: {scope}) into {}. Union is \
             lossless + idempotent; strays were only READ. The local memory.db rebuilds from this JSONL on \
             the next server start in this repo.",
            consolidated_from.len(),
            canonical_jsonl.display()
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "into": into.to_string_lossy(),
            "scan_roots": roots.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
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

/// Pick which of a stray DB's rows belong to `into`: everything if the DB is inside the repo tree, else
/// only rows whose `agent_id` passes `filter` (`None` = keep all — the `--all-agents` case).
fn select(
    rows: Vec<memfix::MemRecord>,
    db: &Path,
    into: &Path,
    filter: Option<&HashSet<String>>,
) -> Vec<memfix::MemRecord> {
    if memfix::is_inside(db, into) {
        return rows;
    }
    rows.into_iter()
        .filter(|r| filter.is_none_or(|f| f.contains(&r.agent_id)))
        .collect()
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

    fn install_agent(repo: &Path, name: &str) {
        let p = repo
            .join(".claude")
            .join("agents")
            .join(format!("{name}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "---\nname: x\n---\n").unwrap();
    }

    /// The bug this fixes: fih-engineer's memory is in a SIBLING launch dir, and OTHER repos' memory sits
    /// next to it. `fix --into repo --root <parent>` must pull the repo's custom agent from the sibling but
    /// NOT drag the other repo's sensei memory in.
    #[test]
    fn fix_pulls_the_repos_custom_agent_from_a_sibling_but_not_foreign_memory() {
        let td = tempfile::tempdir().unwrap();
        let parent = td.path();
        let repo = parent.join("ifs-repo");
        std::fs::create_dir_all(&repo).unwrap();
        install_agent(&repo, "fih-engineer"); // this repo's custom agent

        // sibling launch dir holds the scattered fih-engineer memory + an unrelated sensei store
        make_db(
            &parent.join("launch-dir/genesis-memory.db"),
            &[rec(1, "fih-engineer", "the fih memory", 5)],
        );
        make_db(
            &parent.join("other-repo/genesis-memory.db"),
            &[rec(1, "sensei", "someone else's memory", 6)],
        );

        let code = run(&[
            "--into".into(),
            repo.to_string_lossy().into_owned(),
            "--root".into(),
            parent.to_string_lossy().into_owned(),
        ]);
        assert_eq!(code, 0);

        let (_db, jsonl) = memfix::canonical_paths(&repo);
        let texts: Vec<String> = memfix::read_jsonl(&jsonl)
            .into_iter()
            .map(|r| r.text)
            .collect();
        assert!(
            texts.iter().any(|t| t == "the fih memory"),
            "must recover the repo's own agent from the sibling"
        );
        assert!(
            !texts.iter().any(|t| t == "someone else's memory"),
            "must NOT pull a foreign repo's memory"
        );
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn fix_takes_all_agents_from_an_in_repo_stray() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        install_agent(repo, "fih-engineer");
        // an in-repo stray with a builder-team agent → taken despite not being a "custom" agent
        make_db(
            &repo.join("genesis-memory.db"),
            &[rec(1, "method", "in-repo builder note", 5)],
        );

        assert_eq!(
            run(&[
                "--into".into(),
                repo.to_string_lossy().into_owned(),
                "--root".into(),
                repo.to_string_lossy().into_owned()
            ]),
            0
        );
        let (_db, jsonl) = memfix::canonical_paths(repo);
        let texts: Vec<String> = memfix::read_jsonl(&jsonl)
            .into_iter()
            .map(|r| r.text)
            .collect();
        assert!(
            texts.iter().any(|t| t == "in-repo builder note"),
            "in-repo strays are taken regardless of agent"
        );
    }

    #[test]
    fn fix_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path();
        install_agent(repo, "fih-engineer");
        make_db(
            &repo.join("genesis-memory.db"),
            &[
                rec(1, "fih-engineer", "one", 1),
                rec(2, "fih-engineer", "two", 2),
            ],
        );
        let (_db, jsonl) = memfix::canonical_paths(repo);
        let args = [
            "--into".to_string(),
            repo.to_string_lossy().into_owned(),
            "--root".into(),
            repo.to_string_lossy().into_owned(),
        ];
        assert_eq!(run(&args), 0);
        let first = std::fs::read_to_string(&jsonl).unwrap();
        assert_eq!(run(&args), 0);
        let second = std::fs::read_to_string(&jsonl).unwrap();
        assert_eq!(first, second, "re-running fix yields byte-identical jsonl");
    }
}
