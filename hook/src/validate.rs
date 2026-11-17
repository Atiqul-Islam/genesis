//! VALIDATE hook (Stop / SubagentStop) — the loop-closer.
//!
//! Faithful port of `hooks/validate.js`. Refuses to FINISH while a checkable rule is still violated,
//! OR while the agent never *credibly* declared it applied its required expertise. Three enforcement
//! layers: (1) checkable-rule offenders in produced files; (2) declaration integrity — real rule-ids,
//! a coverage floor, and evidence that is a VERBATIM quote of the rule's own text (forces reading).
//! Fail-closed; guards an infinite block loop via `stop_hook_active`. Self-guards on the active agent.

use crate::{agent, cli, glob, io};
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;

const LINE_BUDGET: usize = 200;
const BULLET_WORD_LIMIT: usize = 20; // reply-format guard: max words per bullet/point
/// Agents whose replies must be tables/bullets with ≤20 words per point (reply-format guard).
const BULLET_FORMAT_AGENTS: [&str; 1] = ["genesis-engineer"];

/// Entry point for `genesis-hook validate <root> [agent] [--expertise <root>] [--main-agent <name>]`.
pub fn run(args: &[String]) {
    // Keep stripping --expertise (still passed by settings.json / hooks.json) so it can't be
    // misparsed as the positional agent; its value is now unused (declaration gate removed, #39).
    let (_expertise, rest) = cli::take_option(args, "--expertise");
    let ev = io::parse_event(&io::read_stdin());

    // Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
    if ev.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
        std::process::exit(0);
    }

    let (argv_root, argv_agent, main_agent) = agent::split_args(&rest);
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = argv_root.map_or_else(|| cwd.clone(), std::path::PathBuf::from);
    let active = agent::resolve_agent(&ev, &argv_agent, &main_agent);
    let session = ev.get("session_id").and_then(Value::as_str).unwrap_or("");

    // DORMANCY GUARD: do nothing unless a genesis agent is active.
    if active.is_empty() {
        std::process::exit(0);
    }

    let log = agent::runtime_dir(&cwd).join("hook-decisions.log");
    let files = glob::produced_files(&root);
    let mut reasons = offenders(&files);

    // Reply-format guard: for agents on the format list, flag any over-long bullet in THIS turn's reply.
    let transcript = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    reasons.extend(format_reasons(
        &active,
        &current_turn_visible_text(transcript),
    ));

    if !reasons.is_empty() {
        log_decision(&log, &active, "block", &reasons, session);
        let shown: Vec<String> = reasons.iter().take(20).cloned().collect();
        io::emit(&json!({
            "decision": "block",
            "reason": format!("Cannot finish:\n- {}\nFix these, then stop again.", shown.join("\n- ")),
        }));
        std::process::exit(0);
    }
    log_decision(&log, &active, "allow", &[], session);
    std::process::exit(0);
}

/// Checkable-rule offenders (banned phrase / credential value / line budget) in produced files.
fn offenders(files: &[(String, String)]) -> Vec<String> {
    let banned = Regex::new(r"(?i)chain[\s\-]?of[\s\-]?thought").ok();
    let creds: Vec<Regex> = [
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        r#"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['"]?[^\s'"]{6,}"#,
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect();
    let budgeted = Regex::new(r"(?i)(persona|behavior)\.md$|(^|/)CLAUDE\.md$").ok();

    let mut out = Vec::new();
    for (f, txt) in files {
        if banned.as_ref().is_some_and(|re| re.is_match(txt)) {
            out.push(format!(
                "{f}: contains \"chain-of-thought\" — use \"structured reasoning\"."
            ));
        }
        if creds.iter().any(|re| re.is_match(txt)) {
            out.push(format!(
                "{f}: looks like a committed credential value — remove it."
            ));
        }
        if budgeted.as_ref().is_some_and(|re| re.is_match(f)) {
            let n = txt.split('\n').count();
            if n > LINE_BUDGET {
                out.push(format!("{f}: {n} lines (>{LINE_BUDGET} budget) — trim."));
            }
        }
    }
    out
}

/// The text of every assistant text block in a transcript record (string content is treated as a
/// single text block, matching the Node port).
fn assistant_texts(ev: &Value) -> Vec<String> {
    let content = ev.get("message").and_then(|m| m.get("content"));
    match content {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| {
                b.get("text")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The index of the first transcript line AFTER the last genuine human message — the current turn's start.
/// Tool-result records are also `type:"user"`; a real human turn has string content or a `text` block, and
/// never `isMeta:true` (Stop-hook feedback / system reminders are meta). No human record -> 0 (parse all).
fn turn_start(lines: &[&str]) -> usize {
    let mut start = 0usize;
    for (i, raw) in lines.iter().enumerate() {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let is_meta = ev.get("isMeta").and_then(Value::as_bool) == Some(true);
        let is_human = !is_meta
            && match ev.get("message").and_then(|m| m.get("content")) {
                Some(Value::String(_)) => true,
                Some(Value::Array(blocks)) => blocks
                    .iter()
                    .any(|b| b.get("type").and_then(Value::as_str) == Some("text")),
                _ => false,
            };
        if is_human {
            start = i + 1;
        }
    }
    start
}

/// The concatenated VISIBLE assistant text (text blocks only, not the record channel) of the current turn.
/// Fail-open: empty string on a missing/unreadable transcript.
pub(crate) fn current_turn_visible_text(p: &str) -> String {
    if p.is_empty() || !Path::new(p).is_file() {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(p) else {
        return String::new();
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let start = turn_start(&lines);
    let mut out: Vec<String> = Vec::new();
    for raw in &lines[start..] {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) == Some("assistant") {
            out.extend(assistant_texts(&ev));
        }
    }
    out.join("\n")
}

/// The text after a markdown bullet marker (`- `/`* `/`+ `/`N. `/`N) `), or `None` if not a bullet line.
fn bullet_body(trimmed: &str) -> Option<&str> {
    for m in ["- ", "* ", "+ "] {
        if let Some(b) = trimmed.strip_prefix(m) {
            return Some(b);
        }
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &trimmed[digits..];
        return rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "));
    }
    None
}

/// Markdown bullet lines in `text` that exceed the 20-word limit (reply-format guard). CONSERVATIVE: skips
/// fenced code blocks and `APPLIED-EXPERTISE` lines, and only checks markdown bullets — table rows,
/// headers, and prose are never flagged, so it cannot false-block on those.
fn overlong_bullets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for raw in text.split('\n') {
        let trimmed = raw.trim_end_matches('\r').trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || trimmed.starts_with("APPLIED-EXPERTISE:") {
            continue;
        }
        let Some(body) = bullet_body(trimmed) else {
            continue;
        };
        let words = body.split_whitespace().count();
        if words > BULLET_WORD_LIMIT {
            let clip: String = body.chars().take(60).collect();
            out.push(format!("{words} words: \"{clip}…\""));
        }
    }
    out
}

/// Reply-format reasons for an agent on `BULLET_FORMAT_AGENTS`: one per over-long bullet in `text`.
fn format_reasons(active: &str, text: &str) -> Vec<String> {
    if !BULLET_FORMAT_AGENTS.contains(&active) {
        return Vec::new();
    }
    overlong_bullets(text)
        .into_iter()
        .map(|b| {
            format!(
                "Reply-format rule: a bullet exceeds {BULLET_WORD_LIMIT} words ({b}). Split it into \
                 shorter points."
            )
        })
        .collect()
}

fn log_decision(log: &Path, agent_name: &str, decision: &str, reasons: &[String], session: &str) {
    io::append_log(
        log,
        &json!({
            "ts": io::now_iso(),
            "hook": "validate",
            "agent": agent_name,
            "session": session,
            "decision": decision,
            "reasons": reasons,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offenders_flags_banned_and_budget() {
        let big = format!("CLAUDE.md content\n{}", "x\n".repeat(250));
        let files = vec![
            (
                "a/persona.md".to_string(),
                "uses chain-of-thought here".to_string(),
            ),
            ("CLAUDE.md".to_string(), big),
            ("ok.md".to_string(), "fine".to_string()),
        ];
        let out = offenders(&files);
        assert!(out.iter().any(|r| r.contains("chain-of-thought")));
        assert!(out.iter().any(|r| r.contains("budget")));
    }

    #[test]
    fn reply_format_flags_overlong_bullets_only() {
        assert!(overlong_bullets("- one two three").is_empty());
        let long = format!("- {}", "word ".repeat(25));
        assert_eq!(overlong_bullets(&long).len(), 1, "25-word bullet flagged");
        let twenty = format!("- {}", vec!["w"; 20].join(" "));
        assert!(overlong_bullets(&twenty).is_empty(), "exactly 20 allowed");
        let fenced = format!("```\n- {}\n```", "word ".repeat(25));
        assert!(overlong_bullets(&fenced).is_empty(), "code fence exempt");
        assert!(
            overlong_bullets(
                "APPLIED-EXPERTISE: a#a-1 — evidence string that keeps going and going and going \
                 and going and going and going and going"
            )
            .is_empty(),
            "APPLIED-EXPERTISE line exempt"
        );
        let trow = format!("| {} |", "cell ".repeat(25));
        assert!(overlong_bullets(&trow).is_empty(), "table row exempt");
    }

    #[test]
    fn format_reasons_scoped_to_the_list() {
        let long = format!("- {}", "word ".repeat(25));
        assert_eq!(format_reasons("genesis-engineer", &long).len(), 1);
        assert!(
            format_reasons("method", &long).is_empty(),
            "agents not on the list are unaffected"
        );
    }

    #[test]
    fn current_turn_visible_text_is_turn_scoped() {
        // Preserves the turn-scoping behavior that fed the (now-removed) declaration parser: only the
        // assistant text AFTER the last genuine human message is returned (used by the reply-format
        // guard + reflect_bg). Replaces the coverage lost when the declaration tests were deleted (#39).
        let td = tempfile::tempdir().unwrap();
        let tp = td.path().join("t.jsonl");
        let stale =
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"old reply"}]}});
        let human = json!({"type":"user","message":{"role":"user","content":"do the next thing"}});
        let current = json!({"type":"assistant","message":{"content":[{"type":"text","text":"fresh reply"}]}});
        std::fs::write(&tp, format!("{stale}\n{human}\n{current}\n")).unwrap();
        let out = current_turn_visible_text(tp.to_str().unwrap());
        assert!(out.contains("fresh reply"), "current-turn text kept");
        assert!(
            !out.contains("old reply"),
            "pre-user text dropped (turn-scoping)"
        );
        assert!(
            current_turn_visible_text("").is_empty(),
            "missing transcript -> empty"
        );
    }
}
