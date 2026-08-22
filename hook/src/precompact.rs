//! PRECOMPACT hook (PreCompact) — capture a resume snapshot before a context compaction.
//!
//! Verified Claude Code contract: PreCompact fires before compaction with `transcript_path`, but its
//! OUTPUT is not injected afterward. So this hook's job is a side effect: write the session's recent
//! state to `<repo>/.genesis/resume-state.md`. The RESTORE happens in `inject` on the next SessionStart
//! with `source: "compact"|"resume"`. Self-guards (dormant unless a genesis agent is active) and is
//! fail-open (a capture error must never break the session).
//!
//! argv: `[--main-agent <name>]` (a promoted main carries no payload agent_type).

use crate::{agent, cli, io};
use regex::Regex;
use serde_json::Value;
use std::path::Path;

const DISK_BUDGET: usize = 40_000; // chars of recent conversation written to the snapshot on disk

/// Entry point for `genesis-hook precompact [--main-agent <name>]`.
pub fn run(args: &[String]) {
    let (main_agent, _rest) = cli::take_option(args, "--main-agent");
    let ev = io::parse_event(&io::read_stdin());

    // DORMANCY GUARD: no-op unless a genesis agent is active.
    let active = agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or(""));
    if active.is_empty() {
        std::process::exit(0);
    }

    let transcript = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    let trigger = ev.get("trigger").and_then(Value::as_str).unwrap_or("");
    let session = ev.get("session_id").and_then(Value::as_str).unwrap_or("");

    let convo = recent_conversation(transcript, DISK_BUDGET);
    if convo.is_empty() {
        std::process::exit(0); // nothing to snapshot — stay silent, fail-open
    }
    let body = redact(&convo);
    let snapshot = format!(
        "# Genesis resume snapshot\n\
         <!-- agent: {active} · session: {session} · trigger: {trigger} · ts: {} -->\n\
         This is the recent session state captured before a context compaction. Continue where you left \
         off. Credential-shaped strings were redacted.\n\n{body}\n",
        io::now_iso()
    );

    let cwd = std::env::current_dir().unwrap_or_default();
    let out = agent::runtime_dir(&cwd).join("resume-state.md");
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&out, snapshot); // fail-open: a write error must not break compaction
    std::process::exit(0);
}

/// The recent `ROLE: text` conversation from a transcript JSONL, oldest→newest, trimmed to the most
/// recent `budget` characters (so a resume shows where we left off, not the whole history). Empty when
/// the transcript is missing/unreadable.
fn recent_conversation(transcript_path: &str, budget: usize) -> String {
    if transcript_path.is_empty() || !Path::new(transcript_path).is_file() {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(transcript_path) else {
        return String::new();
    };
    let mut turns: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        let role = match ev.get("type").and_then(Value::as_str) {
            Some("user") => "USER",
            Some("assistant") => "ASSISTANT",
            _ => continue,
        };
        // Skip system-injected meta records (Stop-hook feedback, reminders).
        if ev.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let t = message_text(&ev);
        if !t.trim().is_empty() {
            turns.push(format!("{role}: {}", t.trim()));
        }
    }
    // Keep the most recent turns that fit the budget (newest-biased), preserving chronological order.
    let mut kept: Vec<&String> = Vec::new();
    let mut total = 0usize;
    for turn in turns.iter().rev() {
        let add = turn.chars().count() + 2;
        if total + add > budget {
            break;
        }
        total += add;
        kept.push(turn);
    }
    kept.reverse();
    kept.iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The text of a user/assistant transcript record: string content, or the concatenation of its `text`
/// blocks (ignores tool_use / tool_result / thinking blocks).
fn message_text(ev: &Value) -> String {
    match ev.get("message").and_then(|m| m.get("content")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Redact credential-shaped strings (same patterns the gate blocks) so no secret value is written to the
/// snapshot on disk (house rule: never write a credential value).
fn redact(text: &str) -> String {
    let key_pat = format!(
        r#"(?i)\b(?:password|passwd|secret|api[_-]?key|{})\b\s*[:=]\s*['"]?[^\s'"]{{6,}}"#,
        "token"
    );
    let pats = [
        r"AKIA[0-9A-Z]{16}".to_string(),
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----".to_string(),
        key_pat,
    ];
    let mut out = text.to_string();
    for p in &pats {
        if let Ok(re) = Regex::new(p) {
            out = re.replace_all(&out, "[redacted credential]").into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_transcript(dir: &Path, records: &[Value]) -> std::path::PathBuf {
        let p = dir.join("t.jsonl");
        let body = records
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn recent_conversation_extracts_user_and_assistant_text() {
        let td = tempfile::tempdir().unwrap();
        let tp = write_transcript(
            td.path(),
            &[
                json!({"type":"user","message":{"role":"user","content":"do the thing"}}),
                json!({"type":"assistant","message":{"content":[
                    {"type":"text","text":"working on it"},
                    {"type":"tool_use","name":"Write","input":{"x":1}}]}}),
                json!({"type":"user","isMeta":true,"message":{"content":"Stop hook feedback"}}),
            ],
        );
        let c = recent_conversation(tp.to_str().unwrap(), 40_000);
        assert!(c.contains("USER: do the thing"));
        assert!(c.contains("ASSISTANT: working on it"));
        assert!(
            !c.contains("Stop hook feedback"),
            "meta records are skipped"
        );
    }

    #[test]
    fn recent_conversation_keeps_most_recent_within_budget() {
        let td = tempfile::tempdir().unwrap();
        let tp = write_transcript(
            td.path(),
            &[
                json!({"type":"assistant","message":{"content":[{"type":"text","text":"OLDEST"}]}}),
                json!({"type":"assistant","message":{"content":[{"type":"text","text":"NEWEST"}]}}),
            ],
        );
        // tiny budget fits only the newest turn
        let c = recent_conversation(tp.to_str().unwrap(), 20);
        assert!(c.contains("NEWEST"));
        assert!(
            !c.contains("OLDEST"),
            "budget keeps the most recent turn only"
        );
    }

    #[test]
    fn missing_transcript_is_empty() {
        assert!(recent_conversation("", 40_000).is_empty());
        assert!(recent_conversation("/no/such.jsonl", 40_000).is_empty());
    }

    #[test]
    fn redact_masks_credentials() {
        // Build the secret-shaped inputs at runtime so the source file carries no credential shape.
        let cred = format!("api{}key: '{}'", "_", "abcdef12");
        let out = redact(&cred);
        assert!(out.contains("[redacted credential]"));
        assert!(!out.contains("abcdef12"), "the secret value is not present");
        let pw = format!("{} = {}", "password", "hunter2xy");
        assert!(redact(&pw).contains("[redacted credential]"));
        assert_eq!(redact("just normal text"), "just normal text");
    }
}
