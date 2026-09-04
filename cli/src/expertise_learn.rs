//! `genesis-cli expertise-learn <root> <add|set-status> …` — the ONLY writer of `<root>/learned.jsonl`,
//! the committed substrate for LEARNED expertise rules (Feature 2, Phase B). Deterministic; never runs an
//! LLM. After every write it re-migrates `expertise.db` so enforcement reflects the change immediately
//! (Phase A already enforces `learned.jsonl` rows whose `status='active'`).
//!
//! - `add` appends (or updates in place, keyed on (expertise,id) or same-text) a learned row; without
//!   `--id` it allocates the next id in the bucket (`<prefix>-<1+max numeric suffix>`, `[a-z]+-[0-9]+`).
//! - `set-status` flips an existing row's status (approve `proposed`->`active`, `reject`, `retire`).
//!
//! Idempotent: a repeat of the same logical write updates in place, never double-appends.

use crate::{expertise_migrate, fsx};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// `genesis-cli expertise-learn <root> <op> [--flags]`. Exit 0 on success; 1 on error. Fail-LOUD (an
/// explicit request); Mneme's autonomous path invokes it and treats a non-zero as "proposal not recorded".
pub fn run(args: &[String]) -> i32 {
    // Positionals are `<root> <op>`; every flag takes a value, so skip flag+value pairs when scanning.
    let mut positionals: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2;
        } else {
            positionals.push(&args[i]);
            i += 1;
        }
    }
    let root = positionals.first().map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let op = positionals.get(1).map(ToString::to_string);
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let result = match op.as_deref() {
        Some("add") => {
            let (Some(exp), Some(text)) = (get("--expertise"), get("--text")) else {
                return usage("add requires --expertise and --text");
            };
            let agents: Vec<String> = get("--agents")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            add(
                &root,
                &exp,
                &text,
                get("--id").as_deref(),
                &get("--type").unwrap_or_else(|| "judgment".to_string()),
                &get("--status").unwrap_or_else(|| "proposed".to_string()),
                &get("--scope").unwrap_or_else(|| "global".to_string()),
                &agents,
            )
        }
        Some("set-status") => {
            let (Some(exp), Some(id), Some(status)) =
                (get("--expertise"), get("--id"), get("--status"))
            else {
                return usage("set-status requires --expertise, --id, --status");
            };
            set_status(&root, &exp, &id, &status)
        }
        _ => return usage("expected `add` or `set-status`"),
    };
    match result {
        Ok(id) => {
            println!("{}", fsx::json_pretty(&json!({ "learned": id })));
            0
        }
        Err(e) => {
            eprintln!("expertise-learn: {e}");
            1
        }
    }
}

fn usage(msg: &str) -> i32 {
    eprintln!("expertise-learn: {msg}");
    2
}

/// Append (or update in place) a learned rule, then re-migrate. Returns the rule-id written.
///
/// # Errors
/// IO / migrate failure.
#[allow(clippy::too_many_arguments)]
pub fn add(
    root: &Path,
    exp: &str,
    text: &str,
    id: Option<&str>,
    rule_type: &str,
    status: &str,
    scope: &str,
    agents: &[String],
) -> Result<String, String> {
    let mut rows = read_learned(root);
    // Match an existing row: by explicit id, else by same (expertise, normalized text) — so a repeat of the
    // same autonomous proposal updates in place instead of allocating a new id (idempotence, B3/B7).
    let existing = rows.iter().position(|r| {
        row_str(r, "expertise") == exp
            && match id {
                Some(want) => row_str(r, "id") == want,
                None => norm(&row_str(r, "text")) == norm(text),
            }
    });
    let rid = match existing {
        Some(i) => row_str(&rows[i], "id"),
        None => id.map_or_else(|| next_rule_id(root, exp), ToString::to_string),
    };
    let row = json!({
        "expertise": exp,
        "id": rid,
        "text": text,
        "type": rule_type,
        "status": status,
        "scope": scope,
        "agents": agents,
        "origin": "learned",
    });
    match existing {
        Some(i) => rows[i] = row,
        None => rows.push(row),
    }
    write_learned(root, &rows)?;
    expertise_migrate::build(root)?;
    Ok(rid)
}

/// Flip an existing learned row's status (`active`|`proposed`|`rejected`|`retired`), then re-migrate.
///
/// # Errors
/// If no learned row matches `(exp, id)`, or on IO / migrate failure.
pub fn set_status(root: &Path, exp: &str, id: &str, status: &str) -> Result<String, String> {
    let mut rows = read_learned(root);
    let idx = rows
        .iter()
        .position(|r| row_str(r, "expertise") == exp && row_str(r, "id") == id)
        .ok_or_else(|| format!("no learned rule {exp}#{id}"))?;
    if let Some(obj) = rows[idx].as_object_mut() {
        obj.insert("status".to_string(), json!(status));
    }
    write_learned(root, &rows)?;
    expertise_migrate::build(root)?;
    Ok(id.to_string())
}

/// The next unused rule-id for `bucket`: `<prefix>-<1 + max numeric suffix>` over ALL rows (manifest +
/// learned), so it never collides and never reuses a retired id. Prefix = the bucket's existing id prefix,
/// else the initials of the bucket name (`test-driven-determinism` → `tdd`).
#[must_use]
pub fn next_rule_id(root: &Path, bucket: &str) -> String {
    let ids = all_bucket_ids(root, bucket);
    let prefix = ids
        .iter()
        .find_map(|id| alpha_prefix(id))
        .unwrap_or_else(|| initials(bucket));
    let maxn = ids
        .iter()
        .filter_map(|id| numeric_suffix(id))
        .max()
        .unwrap_or(0);
    format!("{prefix}-{}", maxn + 1)
}

// ── helpers ────────────────────────────────────────────────────────────────────────────────────

fn learned_path(root: &Path) -> PathBuf {
    root.join("learned.jsonl")
}

/// Parse `<root>/learned.jsonl` into rows (tolerant: blank/malformed lines skipped).
fn read_learned(root: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(learned_path(root)) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// Write rows back as one compact JSON object per line.
fn write_learned(root: &Path, rows: &[Value]) -> Result<(), String> {
    let body: String = rows
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let out = if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    };
    let p = learned_path(root);
    fsx::write_text(&p, &out).map_err(|e| format!("write {}: {e}", p.display()))
}

/// Every rule-id in `bucket`, from the manifest (`manifests/<bucket>.json`) and `learned.jsonl`.
fn all_bucket_ids(root: &Path, bucket: &str) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(data) = fsx::read_json(&root.join("manifests").join(format!("{bucket}.json"))) {
        if let Some(rules) = data.get("rules").and_then(Value::as_array) {
            for r in rules {
                if let Some(id) = r.get("id").and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
        }
    }
    for r in read_learned(root) {
        if row_str(&r, "expertise") == bucket {
            let id = row_str(&r, "id");
            if !id.is_empty() {
                ids.push(id);
            }
        }
    }
    ids
}

fn row_str(r: &Value, key: &str) -> String {
    r.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The leading `[a-z]+` of an id like `tdd-12` → `Some("tdd")`; `None` if it doesn't match the shape.
fn alpha_prefix(id: &str) -> Option<String> {
    let (alpha, rest) = id.split_once('-')?;
    if !alpha.is_empty()
        && alpha.chars().all(|c| c.is_ascii_lowercase())
        && rest.chars().all(|c| c.is_ascii_digit())
        && !rest.is_empty()
    {
        Some(alpha.to_string())
    } else {
        None
    }
}

/// The numeric suffix of an id like `tdd-12` → `Some(12)`.
fn numeric_suffix(id: &str) -> Option<u64> {
    id.rsplit_once('-').and_then(|(_, n)| n.parse().ok())
}

/// Initials of a hyphen/space/underscore-separated name, lowercased (`test-driven-determinism` → `tdd`).
fn initials(name: &str) -> String {
    let s: String = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|w| w.chars().next())
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if s.is_empty() {
        "x".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fixture(root: &Path) {
        std::fs::create_dir_all(root.join("manifests")).unwrap();
        std::fs::write(
            root.join("manifests/test-driven-determinism.json"),
            r#"{"expertise":"test-driven-determinism","rules":[
                {"id":"tdd-1","type":"checkable","text":"Write no code without a failing test."},
                {"id":"tdd-2","type":"judgment","text":"Verify RED before writing code."}
            ]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("required.json"),
            r#"{"_doc":"x","bot":["test-driven-determinism"]}"#,
        )
        .unwrap();
    }

    fn active_ids(root: &Path, exp: &str) -> Vec<String> {
        let c = Connection::open(expertise_migrate::db_path(root)).unwrap();
        let mut stmt = c
            .prepare("SELECT id FROM rules WHERE expertise=?1 AND status='active' ORDER BY id")
            .unwrap();
        let rows = stmt.query_map([exp], |r| r.get::<_, String>(0)).unwrap();
        rows.filter_map(Result::ok).collect()
    }

    #[test]
    fn add_active_is_enforced_proposed_is_not() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        let id_a = add(
            root,
            "test-driven-determinism",
            "Always run fmt before a diff.",
            None,
            "judgment",
            "active",
            "global",
            &[],
        )
        .unwrap();
        let id_p = add(
            root,
            "test-driven-determinism",
            "Prefer small commits always.",
            None,
            "judgment",
            "proposed",
            "global",
            &[],
        )
        .unwrap();
        let active = active_ids(root, "test-driven-determinism");
        assert!(active.contains(&id_a), "active learned rule enforced");
        assert!(
            !active.contains(&id_p),
            "proposed learned rule NOT enforced"
        );
    }

    #[test]
    fn next_id_allocates_above_max_and_is_idempotent_on_text() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        // manifest has tdd-1, tdd-2 -> first learned id is tdd-3
        let id1 = add(
            root,
            "test-driven-determinism",
            "Learned rule alpha.",
            None,
            "judgment",
            "proposed",
            "global",
            &[],
        )
        .unwrap();
        assert_eq!(id1, "tdd-3");
        assert!(super::alpha_prefix(&id1).is_some(), "matches [a-z]+-[0-9]+");
        // re-add same TEXT -> same id, no duplicate row
        let id1b = add(
            root,
            "test-driven-determinism",
            "Learned rule alpha.",
            None,
            "judgment",
            "proposed",
            "global",
            &[],
        )
        .unwrap();
        assert_eq!(id1b, "tdd-3", "same text reuses the id (idempotent)");
        assert_eq!(read_learned(root).len(), 1, "no duplicate learned row");
        // a different text -> next id tdd-4
        let id2 = add(
            root,
            "test-driven-determinism",
            "Learned rule beta.",
            None,
            "judgment",
            "proposed",
            "global",
            &[],
        )
        .unwrap();
        assert_eq!(id2, "tdd-4");
    }

    #[test]
    fn set_status_approve_then_retire() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        let id = add(
            root,
            "test-driven-determinism",
            "Learned rule to approve.",
            None,
            "judgment",
            "proposed",
            "global",
            &[],
        )
        .unwrap();
        assert!(!active_ids(root, "test-driven-determinism").contains(&id));
        set_status(root, "test-driven-determinism", &id, "active").unwrap();
        assert!(
            active_ids(root, "test-driven-determinism").contains(&id),
            "approved -> enforced"
        );
        set_status(root, "test-driven-determinism", &id, "retired").unwrap();
        assert!(
            !active_ids(root, "test-driven-determinism").contains(&id),
            "retired -> not enforced"
        );
    }

    #[test]
    fn agents_attachment_adds_requirement() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fixture(root);
        add(
            root,
            "code-review",
            "Learned review rule for the bot.",
            None,
            "judgment",
            "active",
            "global",
            &["bot".to_string()],
        )
        .unwrap();
        // new bucket 'code-review' -> initials prefix 'cr'
        let c = Connection::open(expertise_migrate::db_path(root)).unwrap();
        let attached: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM required WHERE agent='bot' AND expertise='code-review'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            attached, 1,
            "learned bucket attached to the agent's required set"
        );
    }

    #[test]
    fn initials_and_prefix_helpers() {
        assert_eq!(initials("memory-management"), "mm");
        assert_eq!(initials("test-driven-determinism"), "tdd");
        assert_eq!(alpha_prefix("tdd-12").as_deref(), Some("tdd"));
        assert_eq!(numeric_suffix("tdd-12"), Some(12));
        assert!(alpha_prefix("not-an-id-x").is_none());
    }
}
