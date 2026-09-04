//! `genesis-cli migrate-expertise <root> [--dump]` — build the DERIVED `expertise.db` read model the
//! hooks query, from the committed substrate under `<root>`: `manifests/*.json` + `required.json` +
//! `*.md` guides + (optional) `learned.jsonl`.
//!
//! Design (Feature 2, Phase A — spec `test/specs/expertise-sqlite-migration.md`):
//! - The committed text stays the SOURCE OF TRUTH; `expertise.db` is a gitignored, regenerable cache.
//!   `build` NEVER rewrites the committed files (existing byte-parity drift tests are unaffected).
//! - Idempotent: a `meta.source_sha` fingerprint of the substrate makes a re-run a no-op.
//! - Atomic: build into `expertise.db.tmp-<pid>` then rename over the target so a reader never sees a
//!   partial file.
//! - `--dump` prints a canonical LOGICAL dump (stable text) — the idempotence/no-drift test surface
//!   (raw SQLite bytes are non-deterministic, so tests never compare the `.db` bytes).
//!
//! WRITER ONLY. The hooks read this DB read-only (`hook/src/expertise_db.rs`); this is the single writer.

use crate::fsx;
use rusqlite::Connection;
use serde_json::Value;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "1";

const DDL: &str = "\
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);\
CREATE TABLE expertise(name TEXT PRIMARY KEY, source TEXT, note TEXT, origin TEXT NOT NULL DEFAULT 'migrated');\
CREATE TABLE rules(expertise TEXT NOT NULL, id TEXT NOT NULL, section TEXT, text TEXT NOT NULL, \
type TEXT NOT NULL, predicate TEXT, reviewer_criterion TEXT, origin TEXT NOT NULL DEFAULT 'migrated', \
status TEXT NOT NULL DEFAULT 'active', ordinal INTEGER NOT NULL, PRIMARY KEY(expertise,id));\
CREATE INDEX idx_rules_exp_ord ON rules(expertise,ordinal);\
CREATE TABLE required(agent TEXT NOT NULL, expertise TEXT NOT NULL, ordinal INTEGER NOT NULL, PRIMARY KEY(agent,expertise));\
CREATE TABLE guides(stem TEXT PRIMARY KEY, rel_path TEXT NOT NULL, content_hash TEXT NOT NULL, body TEXT);";

/// The derived DB path for an expertise `root`.
#[must_use]
pub fn db_path(root: &Path) -> PathBuf {
    root.join("expertise.db")
}

/// `genesis-cli migrate-expertise <root> [--dump]`. Exit 0 on success/no-op; 1 on error. Fail-LOUD (the
/// caller asked explicitly); the launcher's `syncRepo` invokes it with stdio ignored so it stays fail-open
/// at the propagation layer.
pub fn run(args: &[String]) -> i32 {
    let dump = args.iter().any(|a| a == "--dump");
    let root = args.iter().find(|a| !a.starts_with("--")).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if dump {
        return match dump_logical(&root) {
            Ok(s) => {
                print!("{s}");
                0
            }
            Err(e) => {
                eprintln!("migrate-expertise --dump: {e}");
                1
            }
        };
    }
    match build(&root) {
        Ok(rebuilt) => {
            println!(
                "migrate-expertise: {} {}",
                if rebuilt {
                    "built"
                } else {
                    "up-to-date (no-op)"
                },
                db_path(&root).display()
            );
            0
        }
        Err(e) => {
            eprintln!("migrate-expertise: {e}");
            1
        }
    }
}

/// Build `<root>/expertise.db` from the committed substrate. Returns `Ok(true)` if it (re)built, `Ok(false)`
/// if the DB was already current (`source_sha` match). Atomic + idempotent.
///
/// # Errors
/// Any IO / SQLite / substrate-parse failure.
pub fn build(root: &Path) -> Result<bool, String> {
    if !root.is_dir() {
        return Err(format!("expertise root not found: {}", root.display()));
    }
    let sha = source_sha(root);
    let target = db_path(root);
    if current_source_sha(&target).as_deref() == Some(sha.as_str()) {
        return Ok(false); // already current — no-op (idempotent re-run)
    }

    let tmp = root.join(format!("expertise.db.tmp-{}", std::process::id()));
    remove_db_family(&tmp);
    {
        let mut con = Connection::open(&tmp).map_err(|e| format!("open {}: {e}", tmp.display()))?;
        con.execute_batch(DDL)
            .map_err(|e| format!("create schema: {e}"))?;
        let tx = con.transaction().map_err(|e| format!("begin: {e}"))?;
        insert_manifests(&tx, root)?;
        insert_required(&tx, root)?;
        insert_guides(&tx, root)?;
        insert_learned(&tx, root)?;
        tx.execute(
            "INSERT INTO meta(key,value) VALUES ('schema_version',?1),('source_sha',?2)",
            rusqlite::params![SCHEMA_VERSION, sha],
        )
        .map_err(|e| format!("write meta: {e}"))?;
        tx.commit().map_err(|e| format!("commit: {e}"))?;
    }
    remove_db_family(&target);
    std::fs::rename(&tmp, &target)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), target.display()))?;
    Ok(true)
}

/// Delete a SQLite DB and its WAL/SHM/journal siblings (safe if absent).
fn remove_db_family(db: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut s = db.as_os_str().to_os_string();
        s.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(s));
    }
}

/// The `meta.source_sha` recorded in an existing DB (schema-version-gated), or `None`.
fn current_source_sha(db: &Path) -> Option<String> {
    if !db.is_file() {
        return None;
    }
    let con = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let ver: Option<String> = con
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    if ver.as_deref() != Some(SCHEMA_VERSION) {
        return None; // schema changed -> force rebuild
    }
    con.query_row("SELECT value FROM meta WHERE key='source_sha'", [], |r| {
        r.get(0)
    })
    .ok()
}

// ── substrate readers → rows ───────────────────────────────────────────────────────────────────

/// Insert every manifest bucket + its rules (file order preserved as `ordinal`).
fn insert_manifests(tx: &rusqlite::Transaction, root: &Path) -> Result<(), String> {
    for path in manifest_files(root) {
        let Some(data) = fsx::read_json(&path) else {
            return Err(format!("parse manifest {}", path.display()));
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = data
            .get("expertise")
            .and_then(Value::as_str)
            .unwrap_or(&stem)
            .to_string();
        tx.execute(
            "INSERT OR REPLACE INTO expertise(name,source,note,origin) VALUES (?1,?2,?3,'migrated')",
            rusqlite::params![
                name,
                data.get("source").and_then(Value::as_str),
                data.get("note").and_then(Value::as_str),
            ],
        )
        .map_err(|e| format!("insert expertise {name}: {e}"))?;
        if let Some(rules) = data.get("rules").and_then(Value::as_array) {
            for (i, r) in rules.iter().enumerate() {
                let id = r.get("id").and_then(Value::as_str).unwrap_or("");
                let text = r.get("text").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let ordinal = i64::try_from(i).unwrap_or(i64::MAX);
                let predicate = r.get("predicate").map(ToString::to_string);
                tx.execute(
                    "INSERT OR REPLACE INTO rules(expertise,id,section,text,type,predicate,reviewer_criterion,origin,status,ordinal) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,'migrated','active',?8)",
                    rusqlite::params![
                        name,
                        id,
                        r.get("section").and_then(Value::as_str),
                        text,
                        r.get("type").and_then(Value::as_str).unwrap_or("judgment"),
                        predicate,
                        r.get("reviewer_criterion").and_then(Value::as_str),
                        ordinal,
                    ],
                )
                .map_err(|e| format!("insert rule {name}#{id}: {e}"))?;
            }
        }
    }
    Ok(())
}

/// Insert the agent→expertise requirement links from `required.json` (array order preserved).
fn insert_required(tx: &rusqlite::Transaction, root: &Path) -> Result<(), String> {
    let path = root.join("required.json");
    let Some(data) = fsx::read_json(&path) else {
        return Ok(()); // no required.json -> no links (parity with the file reader's empty default)
    };
    let Some(obj) = data.as_object() else {
        return Ok(());
    };
    for (agent, exps) in obj {
        if agent == "_doc" {
            continue;
        }
        let Some(arr) = exps.as_array() else { continue };
        for (i, e) in arr.iter().enumerate() {
            let Some(exp) = e.as_str() else { continue };
            let ordinal = i64::try_from(i).unwrap_or(i64::MAX);
            tx.execute(
                "INSERT OR REPLACE INTO required(agent,expertise,ordinal) VALUES (?1,?2,?3)",
                rusqlite::params![agent, exp, ordinal],
            )
            .map_err(|e| format!("insert required {agent}/{exp}: {e}"))?;
        }
    }
    Ok(())
}

/// Insert the `*.md` guide bodies (stem-keyed) — the source for inject's guide pointers.
fn insert_guides(tx: &rusqlite::Transaction, root: &Path) -> Result<(), String> {
    for path in md_files(root) {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = name.strip_suffix(".md").unwrap_or(&name).to_string();
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        tx.execute(
            "INSERT OR REPLACE INTO guides(stem,rel_path,content_hash,body) VALUES (?1,?2,?3,?4)",
            rusqlite::params![stem, name, fnv1a_hex(body.as_bytes()), body],
        )
        .map_err(|e| format!("insert guide {stem}: {e}"))?;
    }
    Ok(())
}

/// Insert learned rules from `learned.jsonl` (one JSON object per line). Each row carries its own `status`
/// (`active` enforces; `proposed`/`rejected`/`retired` do not) and `origin='learned'`.
fn insert_learned(tx: &rusqlite::Transaction, root: &Path) -> Result<(), String> {
    let path = root.join("learned.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(()); // no learned rules yet
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(r) = serde_json::from_str::<Value>(line) else {
            continue; // tolerant: skip a malformed line
        };
        let exp = r.get("expertise").and_then(Value::as_str).unwrap_or("");
        let id = r.get("id").and_then(Value::as_str).unwrap_or("");
        let rtext = r.get("text").and_then(Value::as_str).unwrap_or("");
        if exp.is_empty() || id.is_empty() || rtext.is_empty() {
            continue;
        }
        // A learned rule may attach to a bucket that has no manifest — ensure an expertise row exists.
        tx.execute(
            "INSERT OR IGNORE INTO expertise(name,source,note,origin) VALUES (?1,NULL,NULL,'learned')",
            rusqlite::params![exp],
        )
        .map_err(|e| format!("ensure learned bucket {exp}: {e}"))?;
        tx.execute(
            "INSERT OR REPLACE INTO rules(expertise,id,section,text,type,predicate,reviewer_criterion,origin,status,ordinal) \
             VALUES (?1,?2,?3,?4,?5,NULL,NULL,'learned',?6,?7)",
            rusqlite::params![
                exp,
                id,
                r.get("section").and_then(Value::as_str),
                rtext,
                r.get("type").and_then(Value::as_str).unwrap_or("judgment"),
                r.get("status").and_then(Value::as_str).unwrap_or("proposed"),
                r.get("ordinal").and_then(Value::as_i64).unwrap_or(10_000),
            ],
        )
        .map_err(|e| format!("insert learned {exp}#{id}: {e}"))?;
        // optional per-agent attachment
        if let Some(agents) = r.get("agents").and_then(Value::as_array) {
            for a in agents {
                if let Some(agent) = a.as_str() {
                    tx.execute(
                        "INSERT OR IGNORE INTO required(agent,expertise,ordinal) VALUES (?1,?2,9999)",
                        rusqlite::params![agent, exp],
                    )
                    .map_err(|e| format!("attach learned {exp} to {agent}: {e}"))?;
                }
            }
        }
    }
    Ok(())
}

// ── canonical logical dump (idempotence/no-drift test surface) ──────────────────────────────────

/// A stable, human-readable dump of the DB's logical content — the test surface for idempotence and
/// build↔substrate fidelity. Deterministic ordering; never the raw `.db` bytes.
///
/// # Errors
/// If the DB is absent or unreadable.
pub fn dump_logical(root: &Path) -> Result<String, String> {
    let db = db_path(root);
    let con = Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("open {}: {e}", db.display()))?;
    let mut out = String::new();

    let mut q = con
        .prepare("SELECT name,COALESCE(origin,'') FROM expertise ORDER BY name")
        .map_err(|e| format!("prep expertise: {e}"))?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| format!("q expertise: {e}"))?;
    for row in rows {
        let (n, o) = row.map_err(|e| format!("expertise row: {e}"))?;
        let _ = writeln!(out, "expertise\t{n}\t{o}");
    }

    let mut q = con
        .prepare("SELECT expertise,id,type,status,origin,ordinal,text FROM rules ORDER BY expertise,ordinal,id")
        .map_err(|e| format!("prep rules: {e}"))?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("q rules: {e}"))?;
    for row in rows {
        let (e, id, ty, st, og, ord, tx) = row.map_err(|e| format!("rule row: {e}"))?;
        let _ = writeln!(
            out,
            "rule\t{e}\t{id}\t{ty}\t{st}\t{og}\t{ord}\t{}",
            tx.replace('\n', " ")
        );
    }

    let mut q = con
        .prepare("SELECT agent,expertise,ordinal FROM required ORDER BY agent,ordinal,expertise")
        .map_err(|e| format!("prep required: {e}"))?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("q required: {e}"))?;
    for row in rows {
        let (a, e, ord) = row.map_err(|e| format!("required row: {e}"))?;
        let _ = writeln!(out, "required\t{a}\t{e}\t{ord}");
    }

    let mut q = con
        .prepare("SELECT stem,rel_path FROM guides ORDER BY stem")
        .map_err(|e| format!("prep guides: {e}"))?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| format!("q guides: {e}"))?;
    for row in rows {
        let (s, p) = row.map_err(|e| format!("guide row: {e}"))?;
        let _ = writeln!(out, "guide\t{s}\t{p}");
    }
    Ok(out)
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────────────

/// Sorted `<root>/manifests/*.json` paths.
fn manifest_files(root: &Path) -> Vec<PathBuf> {
    sorted_by_name(&root.join("manifests"), ".json")
}

/// Sorted `<root>/*.md` paths.
fn md_files(root: &Path) -> Vec<PathBuf> {
    sorted_by_name(root, ".md")
}

/// Files in `dir` ending with `suffix`, sorted by file name (deterministic order).
fn sorted_by_name(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(suffix))
        })
        .collect();
    v.sort();
    v
}

/// A deterministic, dependency-free content fingerprint of the whole substrate (FNV-1a-64 hex). Any change
/// to a manifest / required.json / guide / learned.jsonl flips it, triggering a rebuild; identical content
/// keeps it stable so a re-run is a no-op.
fn source_sha(root: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut files: Vec<PathBuf> = manifest_files(root);
    files.push(root.join("required.json"));
    files.push(root.join("learned.jsonl"));
    files.extend(md_files(root));
    for p in files {
        feed(
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_bytes(),
        );
        feed(b"\0");
        if let Ok(bytes) = std::fs::read(&p) {
            feed(&bytes);
        }
        feed(b"\x1e");
    }
    format!("fnv1a64:{h:016x}")
}

/// FNV-1a-64 hex of a byte slice (per-guide content hash; change-detection only).
fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path) {
        std::fs::create_dir_all(root.join("manifests")).unwrap();
        std::fs::write(
            root.join("manifests/test-driven-determinism.json"),
            r#"{"expertise":"test-driven-determinism","source":"guide","rules":[
                {"id":"tdd-1","section":"§1","type":"checkable","text":"Write no code without a failing test."},
                {"id":"tdd-2","type":"judgment","text":"Verify RED before writing code."}
            ]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("required.json"),
            r#"{"_doc":"x","genesis-engineer":["test-driven-determinism"],"method":["test-driven-determinism"]}"#,
        )
        .unwrap();
        std::fs::write(root.join("test-driven-determinism.md"), "# TDD guide\nbody").unwrap();
    }

    #[test]
    fn build_populates_rules_required_guides() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        assert!(build(root).unwrap(), "first build rebuilds");
        let con = Connection::open(db_path(root)).unwrap();
        let n_rules: i64 = con
            .query_row("SELECT COUNT(*) FROM rules WHERE expertise='test-driven-determinism' AND status='active'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n_rules, 2);
        let checkable: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM rules WHERE type='checkable'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(checkable, 1, "tdd-1 is checkable, tdd-2 is judgment");
        let req: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM required WHERE agent='genesis-engineer'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(req, 1);
        let guide: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM guides WHERE stem='test-driven-determinism'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(guide, 1);
    }

    #[test]
    fn rerun_is_noop_and_dump_is_stable() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        assert!(build(root).unwrap(), "first build");
        let dump1 = dump_logical(root).unwrap();
        assert!(
            !build(root).unwrap(),
            "second build is a no-op (source_sha match)"
        );
        let dump2 = dump_logical(root).unwrap();
        assert_eq!(dump1, dump2, "canonical logical dump stable across runs");
        assert!(dump1.contains("rule\ttest-driven-determinism\ttdd-1"));
    }

    #[test]
    fn learned_active_enforced_proposed_not() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        std::fs::write(
            root.join("learned.jsonl"),
            "{\"expertise\":\"test-driven-determinism\",\"id\":\"tdd-100\",\"text\":\"Learned active rule here.\",\"status\":\"active\"}\n\
             {\"expertise\":\"test-driven-determinism\",\"id\":\"tdd-101\",\"text\":\"Learned proposed rule here.\",\"status\":\"proposed\"}\n",
        )
        .unwrap();
        build(root).unwrap();
        let con = Connection::open(db_path(root)).unwrap();
        let active: i64 = con
            .query_row("SELECT COUNT(*) FROM rules WHERE id='tdd-100' AND status='active' AND origin='learned'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active, 1, "active learned rule present + active");
        let proposed_active: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM rules WHERE id='tdd-101' AND status='active'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(proposed_active, 0, "proposed learned rule is NOT active");
    }

    #[test]
    fn dump_errors_when_db_absent() {
        let td = tempfile::tempdir().unwrap();
        assert!(dump_logical(td.path()).is_err());
    }
}
