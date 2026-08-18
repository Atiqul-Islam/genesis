//! SESSION-POINTER hook (SessionStart | UserPromptSubmit | Stop) — records the LIVE session's id so
//! "copy my current session into an agent" knows WHICH session to capture.
//!
//! Faithful port of `hooks/session_pointer.js`. Writes `<cwd>/.genesis/current-session.json` every
//! turn. Fail-open + silent: a pointer-write failure must never disrupt the session.

use crate::io;
use serde_json::{json, Value};

/// Entry point for `genesis-hook session-pointer`.
pub fn run(_args: &[String]) {
    let ev = io::parse_event(&io::read_stdin());
    let sid = ev
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| ev.get("sessionId").and_then(Value::as_str))
        .unwrap_or("");
    let tpath = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");

    if sid.is_empty() {
        std::process::exit(0); // nothing to record
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let dir = cwd.join(".genesis");
    if std::fs::create_dir_all(&dir).is_ok() {
        let rec = json!({ "session_id": sid, "transcript_path": tpath, "ts": io::now_iso() });
        let _ = std::fs::write(dir.join("current-session.json"), rec.to_string());
    }
    std::process::exit(0);
}
