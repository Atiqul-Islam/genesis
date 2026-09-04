//! REFLECT-SURFACE hook (UserPromptSubmit) — Feature 2, Phase B.
//!
//! Mneme's reflection loop runs as a `type: agent` Stop hook; when it proposes a durable rule autonomously
//! it queues it to `.genesis/mneme/proposals/pending.jsonl` (Mneme has NO SendMessage, so it cannot ask the
//! user directly). This hook runs at the next UserPromptSubmit and injects those pending proposals so the
//! MAIN agent presents them for the user's decision (approve / specialize / replace / reject). It only
//! SURFACES — it never writes or clears the queue; the Stop-side reflection applies the user's decision via
//! `genesis-cli expertise-learn` once the user answers. Non-blocking, fail-open.

use crate::{agent, cli, io};
use serde_json::{json, Value};
use std::path::Path;

/// Entry point for `genesis-hook reflect-surface [--main-agent <name>]`.
pub fn run(args: &[String]) {
    let (main_agent, _rest) = cli::take_option(args, "--main-agent");
    let ev = io::parse_event(&io::read_stdin());
    // DORMANCY GUARD: only for an active genesis agent (payload agent_type or the --main-agent fallback).
    let active = agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or(""));
    if active.is_empty() {
        std::process::exit(0);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let ctx = build_pending(&cwd);
    if ctx.is_empty() {
        std::process::exit(0);
    }
    io::emit(&json!({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": ctx,
        }
    }));
    std::process::exit(0);
}

/// The context block listing pending Mneme proposals, or empty when the queue is missing/empty. Each row
/// of `<cwd>/.genesis/mneme/proposals/pending.jsonl` is a JSON object; malformed rows are skipped.
fn build_pending(cwd: &Path) -> String {
    let path = cwd
        .join(".genesis")
        .join("mneme")
        .join("proposals")
        .join("pending.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let items: Vec<Value> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    if items.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    for it in &items {
        let exp = it.get("expertise").and_then(Value::as_str).unwrap_or("?");
        let id = it.get("id").and_then(Value::as_str).unwrap_or("");
        let text = it.get("text").and_then(Value::as_str).unwrap_or("").trim();
        let conflict = it.get("conflict").and_then(Value::as_str);
        let tag = if id.is_empty() {
            format!("[{exp}]")
        } else {
            format!("[{exp}#{id}]")
        };
        lines.push(format!("- {tag} {text}"));
        if let Some(c) = conflict {
            if !c.is_empty() {
                lines.push(format!("    ↳ conflict: {c}"));
            }
        }
    }
    format!(
        "## Pending Mneme learning proposals ({} awaiting your decision)\nMneme (the memory agent) drafted \
         these durable rules from recent turns. Present each to the user in plain English and ask: APPROVE \
         (enforce it), SPECIALIZE (scope it to a task/feature), REPLACE an existing rule, or REJECT. On the \
         user's answer, apply it with `genesis-cli expertise-learn` (approve → `--status active`; reject → \
         `set-status ... rejected`). Do NOT enforce any of these until the user approves:\n{}\n",
        items.len(),
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::build_pending;
    use std::path::Path;

    fn write_pending(cwd: &Path, body: &str) {
        let dir = cwd.join(".genesis").join("mneme").join("proposals");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pending.jsonl"), body).unwrap();
    }

    #[test]
    fn surfaces_pending_proposals() {
        let td = tempfile::tempdir().unwrap();
        let cwd = td.path();
        // missing queue -> nothing
        assert!(build_pending(cwd).is_empty());
        write_pending(
            cwd,
            "{\"expertise\":\"test-driven-determinism\",\"id\":\"tdd-40\",\"text\":\"Always run fmt before a diff.\"}\n\
             {\"expertise\":\"code-review\",\"text\":\"Check error paths.\",\"conflict\":\"clashes with cr-2\"}\n",
        );
        let out = build_pending(cwd);
        assert!(out.contains("awaiting your decision"));
        assert!(out.contains("[test-driven-determinism#tdd-40] Always run fmt before a diff."));
        assert!(out.contains("[code-review] Check error paths."));
        assert!(out.contains("conflict: clashes with cr-2"));
        assert!(out.to_lowercase().contains("approve") && out.to_lowercase().contains("reject"));
    }

    #[test]
    fn empty_queue_surfaces_nothing() {
        let td = tempfile::tempdir().unwrap();
        let cwd = td.path();
        write_pending(cwd, "   \n\n");
        assert!(build_pending(cwd).is_empty());
    }
}
