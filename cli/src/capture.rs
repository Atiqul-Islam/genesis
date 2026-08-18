//! `genesis-cli capture` — session-copy Phase 1 (port of capture.js / capture.py).
//!
//! Extracts the readable CONTENT of every store Claude Code uses to hold a session's context/memory into one
//! normalized, credential-scrubbed record stream. We extract TEXT (not native plugin DBs) so the result is
//! portable and uniform for embedding. Stores: transcript jsonl, auto-memory *.md, context-mode `chunks`
//! DBs, claude-mem observer jsonl, the agent's genesis-memory DB, and the user-level config snapshot.
//!
//! Output: `<out>/records.jsonl` (one `{source,kind,ts,title,text}` per line) + `<out>/capture_manifest.json`.

// The `.db` / `.jsonl` / `.md` checks intentionally mirror Node's case-sensitive `String.endsWith`.
#![allow(clippy::case_sensitive_file_extension_comparisons)]

use crate::{fsx, scrub};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

// ── small fs/text helpers (utf8-lossy to mirror the Node errors="replace" decode) ──────────
fn read_lossy(p: &Path) -> Option<String> {
    std::fs::read(p)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Immediate non-hidden subdirectory names of `dir`, sorted (mirrors a `*/` glob).
fn list_dirs(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

/// `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
#[must_use]
pub fn claude_home() -> PathBuf {
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    Path::new(&home).join(".claude")
}

/// Locate `<session>.jsonl` under any `~/.claude/projects/*/` dir (robust to path-encoding differences).
#[must_use]
pub fn find_transcript(session_id: &str) -> Option<PathBuf> {
    let projects = claude_home().join("projects");
    for d in list_dirs(&projects) {
        let cand = projects.join(&d).join(format!("{session_id}.jsonl"));
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

// ── content flattening ─────────────────────────────────────────────────────────────────────
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Flatten an Anthropic message `content` (string | list of blocks) into readable text.
#[must_use]
pub fn text_from_content(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let Some(arr) = content.as_array() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for b in arr {
        if !b.is_object() {
            parts.push(js_string(b));
            continue;
        }
        match b.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => parts.push(
                b.get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ),
            "tool_use" => {
                let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                let input = b.get("input").cloned().unwrap_or_else(|| json!({}));
                parts.push(format!(
                    "[tool_use {name}] {}",
                    head(&compact(&input), 2000)
                ));
            }
            "tool_result" => {
                let c = b.get("content").cloned().unwrap_or_else(|| json!(""));
                let s = c
                    .as_str()
                    .map_or_else(|| text_from_content(&c), ToString::to_string);
                parts.push(format!("[tool_result] {}", head(&s, 2000)));
            }
            "thinking" => {
                let t = b.get("thinking").and_then(Value::as_str).unwrap_or("");
                parts.push(format!("[thinking] {}", head(t, 2000)));
            }
            _ => {}
        }
    }
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn iter_jsonl(p: &Path) -> Vec<Value> {
    read_lossy(p)
        .into_iter()
        .flat_map(|raw| {
            raw.split('\n')
                .filter_map(|l| {
                    let t = l.trim();
                    if t.is_empty() {
                        None
                    } else {
                        serde_json::from_str::<Value>(t).ok()
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

// ── record helper ───────────────────────────────────────────────────────────────────────────
fn rec(
    known: &[String],
    source: &str,
    kind: &str,
    ts: &str,
    title: &str,
    text: &str,
) -> (Value, usize) {
    let (scrubbed, n) = scrub::scrub_text(text, known);
    (
        json!({"source": source, "kind": kind, "ts": ts, "title": title, "text": scrubbed}),
        n,
    )
}

// ── extractors: each returns (records, n_redacted) ───────────────────────────────────────────
fn extract_transcript(known: &[String], transcript: Option<&Path>) -> (Vec<Value>, usize) {
    let (mut recs, mut red) = (Vec::new(), 0);
    let Some(p) = transcript.filter(|p| p.is_file()) else {
        return (recs, red);
    };
    let Some(raw) = read_lossy(p) else {
        return (recs, red);
    };
    for line in raw.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let typ = ev.get("type").and_then(Value::as_str).unwrap_or("");
        if !matches!(typ, "user" | "assistant" | "system") {
            continue;
        }
        let ts = ev.get("timestamp").and_then(Value::as_str).unwrap_or("");
        let text = if typ == "system" {
            let c = ev.get("content").cloned().unwrap_or_else(|| json!(""));
            c.as_str()
                .map_or_else(|| text_from_content(&c), ToString::to_string)
        } else {
            let content = ev
                .get("message")
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or_else(|| json!(""));
            text_from_content(&content)
        };
        if text.trim().is_empty() {
            continue;
        }
        let (r, n) = rec(known, "transcript", typ, ts, "", &text);
        recs.push(r);
        red += n;
    }
    (recs, red)
}

fn extract_auto_memory(known: &[String], project_dir: Option<&Path>) -> (Vec<Value>, usize) {
    let (mut recs, mut red) = (Vec::new(), 0);
    let Some(mem) = project_dir.map(|d| d.join("memory")).filter(|d| d.is_dir()) else {
        return (recs, red);
    };
    let mut names: Vec<String> = std::fs::read_dir(&mem)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        let p = mem.join(&name);
        if !(p.is_file() && name.ends_with(".md")) {
            continue;
        }
        let Some(txt) = read_lossy(&p) else { continue };
        let (r, n) = rec(known, "auto-memory", "memory-file", "", &name, &txt);
        recs.push(r);
        red += n;
    }
    (recs, red)
}

/// Open a SQLite DB read-only. `None` on any failure (fail-safe, matches the Node try/catch-skip).
fn open_ro(db: &Path) -> Option<Connection> {
    Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn contextmode_from_db(
    known: &[String],
    db: &Path,
    session_id: &str,
) -> Option<(Vec<Value>, usize)> {
    let c = open_ro(db)?;
    let has: bool = c
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='chunks'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !has {
        return Some((Vec::new(), 0));
    }
    let mut stmt = c
        .prepare("SELECT title, content, timestamp FROM chunks WHERE session_id=?")
        .ok()?;
    let rows = stmt
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .ok()?;
    let (mut recs, mut red) = (Vec::new(), 0);
    for row in rows.flatten() {
        let (title, content, ts) = row;
        let (r, n) = rec(known, "context-mode", "chunk", &ts, &title, &content);
        recs.push(r);
        red += n;
    }
    Some((recs, red))
}

fn extract_contextmode(known: &[String], session_id: &str, home: &Path) -> (Vec<Value>, usize) {
    let (mut recs, mut red) = (Vec::new(), 0);
    let content_dir = home.join("context-mode").join("content");
    let mut files: Vec<String> = std::fs::read_dir(&content_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|f| f.ends_with(".db") && !f.starts_with('.'))
        .collect();
    files.sort();
    for f in files {
        let db = content_dir.join(&f);
        if !db.is_file() {
            continue;
        }
        if let Some((r, n)) = contextmode_from_db(known, &db, session_id) {
            recs.extend(r);
            red += n;
        }
    }
    (recs, red)
}

fn extract_claude_mem(known: &[String], session_id: &str, home: &Path) -> (Vec<Value>, usize) {
    let (mut recs, mut red) = (Vec::new(), 0);
    let projects = home.join("projects");
    let mut files: Vec<PathBuf> = Vec::new();
    for d in list_dirs(&projects) {
        // fnmatch("*claude-mem*observer*"): "claude-mem" then "observer" after it.
        let Some(idx) = d.find("claude-mem") else {
            continue;
        };
        if !d[idx + "claude-mem".len()..].contains("observer") {
            continue;
        }
        let dir = projects.join(&d);
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if let Ok(name) = e.file_name().into_string() {
                if name.ends_with(".jsonl") && !name.starts_with('.') && dir.join(&name).is_file() {
                    files.push(dir.join(&name));
                }
            }
        }
    }
    files.sort();
    for f in files {
        for ev in iter_jsonl(&f) {
            let content = ev.get("content").and_then(Value::as_str).unwrap_or("");
            let sid = ev.get("sessionId").and_then(Value::as_str).unwrap_or("");
            if content.is_empty() || sid != session_id {
                continue;
            }
            let op = ev
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("observation");
            let ts = ev.get("timestamp").and_then(Value::as_str).unwrap_or("");
            let (r, n) = rec(known, "claude-mem", op, ts, "session-match", content);
            recs.push(r);
            red += n;
        }
    }
    (recs, red)
}

fn genesis_memory_from_db(
    known: &[String],
    db: &Path,
    agent_id: Option<&str>,
) -> Option<(Vec<Value>, usize)> {
    let c = open_ro(db)?;
    let tables: Vec<String> = {
        let mut stmt = c
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .ok()?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).ok()?;
        rows.flatten().collect()
    };
    let tbl = if tables.iter().any(|t| t == "memories") {
        "memories"
    } else if tables.iter().any(|t| t == "memory") {
        "memory"
    } else {
        return Some((Vec::new(), 0));
    };
    let cols: Vec<String> = {
        let mut stmt = c.prepare(&format!("PRAGMA table_info('{tbl}')")).ok()?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(1)).ok()?;
        rows.flatten().collect()
    };
    let Some(textcol) = ["text", "content", "body"]
        .into_iter()
        .find(|x| cols.iter().any(|c| c == x))
    else {
        return Some((Vec::new(), 0));
    };
    let idcol = ["agent_id", "agent"]
        .into_iter()
        .find(|x| cols.iter().any(|c| c == x));
    // Table/column names come from a fixed allowlist above — safe to interpolate; values are bound.
    let (mut recs, mut red) = (Vec::new(), 0);
    let mut push = |texts: Vec<String>| {
        for t in texts {
            let (r, n) = rec(known, "genesis-memory", "memory", "", "", &t);
            recs.push(r);
            red += n;
        }
    };
    let texts: Vec<String> = if let (Some(idc), Some(aid)) = (idcol, agent_id) {
        let mut stmt = c
            .prepare(&format!("SELECT {textcol} FROM {tbl} WHERE {idc}=?"))
            .ok()?;
        let rows = stmt
            .query_map([aid], |r| r.get::<_, Option<String>>(0))
            .ok()?;
        rows.flatten().map(Option::unwrap_or_default).collect()
    } else {
        let mut stmt = c.prepare(&format!("SELECT {textcol} FROM {tbl}")).ok()?;
        let rows = stmt.query_map([], |r| r.get::<_, Option<String>>(0)).ok()?;
        rows.flatten().map(Option::unwrap_or_default).collect()
    };
    push(texts);
    Some((recs, red))
}

fn extract_genesis_memory(
    known: &[String],
    db_path: Option<&Path>,
    agent_id: Option<&str>,
) -> (Vec<Value>, usize) {
    let Some(db) = db_path.filter(|p| p.is_file()) else {
        return (Vec::new(), 0);
    };
    genesis_memory_from_db(known, db, agent_id).unwrap_or((Vec::new(), 0))
}

/// Recursively collect files named `SKILL.md` under `root` (non-hidden), full paths sorted.
fn glob_skills(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let p = dir.join(&name);
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else if name == "SKILL.md" {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn extract_user_config(known: &[String], home: &Path) -> (Vec<Value>, usize) {
    let (mut recs, mut red) = (Vec::new(), 0);
    let claude_md = home.join("CLAUDE.md");
    if claude_md.is_file() {
        if let Some(txt) = read_lossy(&claude_md) {
            let (r, n) = rec(known, "user-config", "claude-md", "", "CLAUDE.md", &txt);
            recs.push(r);
            red += n;
        }
    }
    let sj = home.join("settings.json");
    if sj.is_file() {
        if let Some(raw) = read_lossy(&sj) {
            let (r, n) = rec(known, "user-config", "settings", "", "settings.json", &raw);
            recs.push(r);
            red += n;
        }
    }
    for skill in glob_skills(&home.join("skills")) {
        let Some(txt) = read_lossy(&skill) else {
            continue;
        };
        let rel = skill
            .strip_prefix(home)
            .unwrap_or(&skill)
            .to_string_lossy()
            .replace('\\', "/");
        let (r, n) = rec(known, "user-config", "skill", "", &rel, &txt);
        recs.push(r);
        red += n;
    }
    (recs, red)
}

/// Full capture. Writes `<out_dir>/records.jsonl` + `capture_manifest.json`; returns the manifest.
///
/// # Errors
/// Returns a message if the output dir or files cannot be written.
pub fn capture(
    session_id: &str,
    cwd: Option<&Path>,
    out_dir: &Path,
    genesis_db: Option<&Path>,
    agent_id: Option<&str>,
    include_user_config: bool,
    known: &[String],
) -> Result<Value, String> {
    let transcript = find_transcript(session_id);
    let proj = transcript
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    let home = claude_home();

    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    let mut per_source = Map::new();
    let mut total = 0usize;
    let mut total_red = 0usize;
    let mut lines = String::new();

    let mut run = |name: &str, recs: Vec<Value>, red: usize| {
        for r in &recs {
            lines.push_str(&compact(r));
            lines.push('\n');
        }
        per_source.insert(
            name.to_string(),
            json!({"records": recs.len(), "redactions": red}),
        );
        total += recs.len();
        total_red += red;
    };

    let (r, n) = extract_transcript(known, transcript.as_deref());
    run("transcript", r, n);
    let (r, n) = extract_auto_memory(known, proj.as_deref());
    run("auto-memory", r, n);
    let (r, n) = extract_contextmode(known, session_id, &home);
    run("context-mode", r, n);
    let (r, n) = extract_claude_mem(known, session_id, &home);
    run("claude-mem", r, n);
    let (r, n) = extract_genesis_memory(known, genesis_db, agent_id);
    run("genesis-memory", r, n);
    if include_user_config {
        let (r, n) = extract_user_config(known, &home);
        run("user-config", r, n);
    }

    let out_path = out_dir.join("records.jsonl");
    fsx::write_text(&out_path, &lines).map_err(|e| format!("write {}: {e}", out_path.display()))?;

    let manifest = json!({
        "session_id": session_id,
        "cwd": cwd.map(|c| std::fs::canonicalize(c).unwrap_or_else(|_| c.to_path_buf()).to_string_lossy().into_owned()),
        "transcript": transcript.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "project_dir": proj.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "records_file": out_path.to_string_lossy(),
        "total_records": total,
        "total_redactions": total_red,
        "by_source": Value::Object(per_source),
    });
    let mp = out_dir.join("capture_manifest.json");
    fsx::write_text(&mp, &fsx::json_pretty(&manifest))
        .map_err(|e| format!("write {}: {e}", mp.display()))?;
    Ok(manifest)
}

/// Entry point for `genesis-cli capture`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let (mut session, mut cwd, mut out, mut gdb, mut agent) = (None, None, None, None, None);
    let mut no_user_config = false;
    let mut known: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--session" => session = args.get(i + 1).cloned(),
            "--cwd" => cwd = args.get(i + 1).cloned(),
            "--out" => out = args.get(i + 1).cloned(),
            "--genesis-db" => gdb = args.get(i + 1).cloned(),
            "--agent-id" => agent = args.get(i + 1).cloned(),
            "--no-user-config" => {
                no_user_config = true;
                i += 1;
                continue;
            }
            "--known-secret" => {
                if let Some(v) = args.get(i + 1).cloned() {
                    known.push(v);
                }
            }
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    let (Some(session), Some(out)) = (session, out) else {
        fsx::fail("usage: genesis-cli capture --session <id> --out <dir> [--cwd <repo>] [--genesis-db <path>] [--agent-id <name>] [--no-user-config] [--known-secret V ...]");
    };
    let cwd_pb = cwd.map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    match capture(
        &session,
        Some(&cwd_pb),
        Path::new(&out),
        gdb.as_deref().map(Path::new),
        agent.as_deref(),
        !no_user_config,
        &known,
    ) {
        Ok(m) => {
            println!("{}", fsx::json_pretty(&m));
            0
        }
        Err(e) => fsx::fail(&e),
    }
}
