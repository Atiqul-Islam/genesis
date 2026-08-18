//! `genesis-cli store` — session-copy Phase 2 (port of store.js / store.py).
//!
//! Takes the scrubbed `records.jsonl` from capture and produces the new agent's PORTABLE bundle under
//! `<out>/`: `history.sqlite` (the records in a clean, queryable table that travels with the repo) and
//! `summary.md` (a DETERMINISTIC start-time digest). Pure + deterministic; embedding is a separate step.

use crate::fsx;
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::path::Path;

/// Parse `records.jsonl` (skipping blank / unparseable lines).
#[must_use]
pub fn load_records(records_path: &Path) -> Vec<Value> {
    let Some(raw) = std::fs::read(records_path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
    else {
        return Vec::new();
    };
    raw.split('\n')
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() {
                None
            } else {
                serde_json::from_str::<Value>(t).ok()
            }
        })
        .collect()
}

fn field<'a>(r: &'a Value, k: &str) -> &'a str {
    r.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Write the portable history DB: one row per record. Overwrites cleanly (drops any prior db + sidecars).
///
/// # Errors
/// Returns a message if the SQLite database cannot be created or written.
pub fn write_history_db(recs: &[Value], db_path: &Path) -> Result<(), String> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let p = if suffix.is_empty() {
            db_path.to_path_buf()
        } else {
            let mut s = db_path.as_os_str().to_os_string();
            s.push(suffix);
            std::path::PathBuf::from(s)
        };
        let _ = std::fs::remove_file(&p);
    }
    let mut con =
        Connection::open(db_path).map_err(|e| format!("open {}: {e}", db_path.display()))?;
    con.execute_batch(
        "CREATE TABLE records (seq INTEGER PRIMARY KEY, source TEXT, kind TEXT, ts TEXT, title TEXT, text TEXT);\
         CREATE INDEX idx_source ON records(source);",
    )
    .map_err(|e| format!("create schema: {e}"))?;
    let tx = con.transaction().map_err(|e| format!("begin: {e}"))?;
    {
        let mut ins = tx
            .prepare(
                "INSERT INTO records (seq, source, kind, ts, title, text) VALUES (?,?,?,?,?,?)",
            )
            .map_err(|e| format!("prepare insert: {e}"))?;
        for (i, r) in recs.iter().enumerate() {
            let seq = i64::try_from(i).unwrap_or(i64::MAX);
            ins.execute(rusqlite::params![
                seq,
                field(r, "source"),
                field(r, "kind"),
                field(r, "ts"),
                field(r, "title"),
                field(r, "text"),
            ])
            .map_err(|e| format!("insert row {i}: {e}"))?;
        }
    }
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    Ok(())
}

/// Count records per `source`, in first-seen order (backs both the summary and the manifest `by_source`).
#[must_use]
pub fn counts(recs: &[Value]) -> Map<String, Value> {
    let mut c = Map::new();
    for r in recs {
        let k = r
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let n = c.get(&k).and_then(Value::as_u64).unwrap_or(0) + 1;
        c.insert(k, json!(n));
    }
    c
}

fn head_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A deterministic, portable start-time digest: what was captured + the prior MEMORY.md + the recent turns.
#[must_use]
pub fn build_summary(recs: &[Value], agent_name: &str) -> String {
    const RECENT_TURNS: usize = 12;
    const MEMORY_CHARS: usize = 4000;
    let c = counts(recs);
    let mut lines: Vec<String> = vec![
        format!("# Session-copy memory — {agent_name}"),
        String::new(),
    ];
    lines.push(
        "You were created by copying a prior Claude Code session. Its full history + memory is in your \
         portable store (`history.sqlite`) and is semantically recallable via your memory tools. This \
         summary is the always-loaded digest; recall specifics on demand."
            .to_string(),
    );
    lines.push(String::new());
    lines.push("## What was carried over".to_string());
    for src in [
        "transcript",
        "auto-memory",
        "context-mode",
        "claude-mem",
        "genesis-memory",
        "user-config",
    ] {
        if let Some(n) = c.get(src).and_then(Value::as_u64) {
            lines.push(format!("- {src}: {n} records"));
        }
    }
    lines.push(String::new());

    // The prior session's index memory (MEMORY.md), if captured — the highest-signal standing context.
    if let Some(mem) = recs
        .iter()
        .find(|r| field(r, "source") == "auto-memory" && field(r, "title") == "MEMORY.md")
    {
        let body = field(mem, "text").trim();
        lines.push("## Standing memory (from the prior session's MEMORY.md)".to_string());
        let truncated = if char_len(body) > MEMORY_CHARS {
            "\n…(truncated — recall the rest)"
        } else {
            ""
        };
        lines.push(format!("{}{truncated}", head_chars(body, MEMORY_CHARS)));
        lines.push(String::new());
    }

    // The tail of the conversation — most recent turns, compact.
    let turns: Vec<&Value> = recs
        .iter()
        .filter(|r| {
            field(r, "source") == "transcript" && matches!(field(r, "kind"), "user" | "assistant")
        })
        .collect();
    if !turns.is_empty() {
        lines.push(format!(
            "## Most recent {} turns (tail of the prior conversation)",
            RECENT_TURNS.min(turns.len())
        ));
        let start = turns.len().saturating_sub(RECENT_TURNS);
        for r in &turns[start..] {
            let who = if field(r, "kind") == "assistant" {
                "You"
            } else {
                "User"
            };
            let snippet = head_chars(&collapse_ws(field(r, "text")), 240);
            lines.push(format!("- **{who}:** {snippet}"));
        }
        lines.push(String::new());
    }
    lines.push(
        "_Recall any detail from the full history with your memory tools — it is all stored._"
            .to_string(),
    );
    lines.join("\n")
}

/// Build the full bundle (history.sqlite + summary.md + store_manifest.json). Returns the manifest.
///
/// # Errors
/// Returns a message if the output dir or any bundle file cannot be written.
pub fn build_bundle(
    records_path: &Path,
    out_dir: &Path,
    agent_name: Option<&str>,
) -> Result<Value, String> {
    let name = agent_name
        .map(ToString::to_string)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            out_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "agent".to_string());
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let recs = load_records(records_path);
    let db_path = out_dir.join("history.sqlite");
    write_history_db(&recs, &db_path)?;
    let summary = build_summary(&recs, &name);
    let summary_path = out_dir.join("summary.md");
    fsx::write_text(&summary_path, &format!("{summary}\n"))
        .map_err(|e| format!("write summary: {e}"))?;
    let manifest = json!({
        "agent": name,
        "records": recs.len(),
        "by_source": Value::Object(counts(&recs)),
        "history_db": db_path.to_string_lossy(),
        "summary": summary_path.to_string_lossy(),
    });
    let mp = out_dir.join("store_manifest.json");
    fsx::write_text(&mp, &fsx::json_pretty(&manifest))
        .map_err(|e| format!("write manifest: {e}"))?;
    Ok(manifest)
}

/// Entry point for `genesis-cli store`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let (mut records, mut out, mut name) = (None, None, None);
    let mut i = 0;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--records" => records = args.get(i + 1).cloned(),
            "--out" => out = args.get(i + 1).cloned(),
            "--name" => name = args.get(i + 1).cloned(),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    let (Some(records), Some(out)) = (records, out) else {
        fsx::fail("usage: genesis-cli store --records <dir>/records.jsonl --out <repo>/.genesis/agents/<name> [--name <name>]");
    };
    match build_bundle(Path::new(&records), Path::new(&out), name.as_deref()) {
        Ok(m) => {
            println!("{}", fsx::json_pretty(&m));
            0
        }
        Err(e) => fsx::fail(&e),
    }
}
