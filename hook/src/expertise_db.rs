//! Read-only queries against the derived `expertise.db` (Feature 2, Phase A).
//!
//! This is the DB-first SOURCE for the enforcement readers (validate/gate/inject). Every query returns
//! `None` on ANY absence or error, so the caller falls back to the committed JSON/MD files — a missing or
//! corrupt DB can NEVER brick the guard stack. The DB is built by `genesis-cli migrate-expertise`; the
//! hooks only ever read it (read-only open).
//!
//! Behind the default-on `expertise-db` cargo feature. With the feature OFF, every query is `None` (a pure
//! file-fallback build, e.g. for a size-sensitive target that can't take bundled SQLite).
//!
//! Parity is the contract: for a bucket that exists, each query returns the SAME logical set the file
//! reader would — ids lowercased on the validate path, RAW on the gate path, `checkable` filtered, file
//! order preserved by `ordinal`. The `expertise_db_parity_*` tests assert this against a fixture store.

use std::path::Path;

/// The derived DB path for an expertise `root` (sibling of the committed manifests). Only referenced by
/// the SQLite backend (and its tests); absent from a file-only (`expertise-db` off) build.
#[cfg(feature = "expertise-db")]
#[must_use]
pub(crate) fn db_file(root: &Path) -> std::path::PathBuf {
    root.join("expertise.db")
}

#[cfg(feature = "expertise-db")]
mod backend {
    use super::{db_file, Path};
    use rusqlite::{Connection, OpenFlags};

    /// Open `<root>/expertise.db` read-only, or `None` if it is absent/unopenable (→ file fallback).
    fn open(root: &Path) -> Option<Connection> {
        let p = db_file(root);
        if !p.is_file() {
            return None;
        }
        Connection::open_with_flags(&p, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
    }

    /// Agent → required expertise names (array order), mirroring `required_for` / `required_list`.
    pub(crate) fn required(root: &Path, agent: &str) -> Option<Vec<String>> {
        let c = open(root)?;
        let mut stmt = c
            .prepare("SELECT expertise FROM required WHERE agent=?1 ORDER BY ordinal")
            .ok()?;
        let rows = stmt.query_map([agent], |r| r.get::<_, String>(0)).ok()?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.ok()?);
        }
        Some(v)
    }

    /// Checkable active rules as `(RAW id, raw text)` in file order (`ordinal`) — mirrors gate's
    /// `top_rules` source (gate keeps ids RAW, so no lowercasing here). `None` if the DB is absent.
    pub(crate) fn top_checkable(root: &Path, name: &str) -> Option<Vec<(String, String)>> {
        let c = open(root)?;
        let mut stmt = c
            .prepare(
                "SELECT id, text FROM rules WHERE expertise=?1 AND type='checkable' AND status='active' ORDER BY ordinal",
            )
            .ok()?;
        let rows = stmt
            .query_map([name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .ok()?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.ok()?);
        }
        Some(v)
    }
}

#[cfg(not(feature = "expertise-db"))]
mod backend {
    use super::Path;

    pub(crate) fn required(_root: &Path, _agent: &str) -> Option<Vec<String>> {
        None
    }
    pub(crate) fn top_checkable(_root: &Path, _name: &str) -> Option<Vec<(String, String)>> {
        None
    }
}

pub(crate) use backend::{required, top_checkable};

#[cfg(all(test, feature = "expertise-db"))]
mod tests {
    use super::{db_file, required, top_checkable};
    use rusqlite::Connection;
    use std::path::Path;

    /// A fixture expertise root holding BOTH sources for the same logical content: `expertise.db` (the DB
    /// reader's source) and the matching manifest JSON + required.json (the file reader's source). The
    /// parity tests then assert the two readers agree — the anti-divergence gate (risk: DB vs file drift).
    fn fixture(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(root.join("manifests"))?;
        std::fs::write(
            root.join("manifests/tdd.json"),
            r#"{"expertise":"tdd","rules":[
                {"id":"TDD-1","type":"checkable","text":"Alpha rule text long enough to quote."},
                {"id":"tdd-2","type":"judgment","text":"Beta rule text long enough to quote."},
                {"id":"tdd-3","type":"checkable","text":""}
            ]}"#,
        )?;
        std::fs::write(
            root.join("required.json"),
            r#"{"_doc":"x","bot":["tdd","mm"]}"#,
        )?;
        let c = Connection::open(db_file(root))?;
        c.execute_batch(
            "CREATE TABLE expertise(name TEXT PRIMARY KEY, source TEXT, note TEXT, origin TEXT);\
             CREATE TABLE rules(expertise TEXT, id TEXT, section TEXT, text TEXT, type TEXT, predicate TEXT, reviewer_criterion TEXT, origin TEXT, status TEXT, ordinal INTEGER, PRIMARY KEY(expertise,id));\
             CREATE TABLE required(agent TEXT, expertise TEXT, ordinal INTEGER, PRIMARY KEY(agent,expertise));\
             INSERT INTO expertise(name,origin) VALUES('tdd','migrated');\
             INSERT INTO rules(expertise,id,text,type,status,ordinal) VALUES \
               ('tdd','TDD-1','Alpha rule text long enough to quote.','checkable','active',0),\
               ('tdd','tdd-2','Beta rule text long enough to quote.','judgment','active',1),\
               ('tdd','tdd-3','','checkable','active',2);\
             INSERT INTO required(agent,expertise,ordinal) VALUES('bot','tdd',0),('bot','mm',1);",
        )?;
        Ok(())
    }

    #[test]
    fn required_db_reads_agent_expertise() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root).unwrap();
        let db = required(root, "bot").unwrap();
        assert_eq!(db, vec!["tdd".to_string(), "mm".to_string()]);
    }

    #[test]
    fn top_checkable_raw_id_file_order() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root).unwrap();
        let pairs = top_checkable(root, "tdd").unwrap();
        let ids: Vec<&str> = pairs.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(
            ids,
            vec!["TDD-1", "tdd-3"],
            "RAW id (gate path), file order, checkable only"
        );
    }

    #[test]
    fn absent_db_returns_none_for_every_reader() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path(); // no expertise.db written -> every reader None -> caller falls back to files
        assert!(required(root, "bot").is_none());
        assert!(top_checkable(root, "tdd").is_none());
    }
}
