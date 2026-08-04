//! GATE hook (PreToolUse: Write|Edit) — deterministic enforcement + just-in-time rule SURFACING.
//!
//! Faithful port of `hooks/gate.js`. Two jobs at the moment a file is about to be written:
//! (1) BLOCK a Write/Edit whose content violates a checkable house rule (banned phrase / credential
//! value / line budget); (2) SURFACE the governing rules for a substantial authoring artifact
//! (non-blocking `additionalContext`). Self-guards: no-op unless a genesis agent is active.

use crate::{agent, cli, io};
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;

const LINE_BUDGET: usize = 200;
const SURFACE_MIN: usize = 300; // only surface for a substantial authoring write
const SURFACE_N: usize = 5; // top-N governing rules to re-assert
const CAP: usize = 9500; // additionalContext (and all hook output) is capped at 10,000 chars

/// Entry point for `genesis-hook gate [--expertise <root>]`.
pub fn run(args: &[String]) {
    let (expertise, rest) = cli::take_option(args, "--expertise");
    // --main-agent lets the gate fire for a PROMOTED main thread (which carries no payload agent_type).
    let (main_agent, _rest) = cli::take_option(&rest, "--main-agent");
    let ev = io::parse_event(&io::read_stdin());

    // DORMANCY GUARD: no-op unless a genesis agent is active (payload agent_type, or the --main-agent fallback).
    if agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or("")).is_empty() {
        std::process::exit(0);
    }

    let ti = ev.get("tool_input").cloned().unwrap_or(Value::Null);
    let p = ti.get("file_path").and_then(Value::as_str).unwrap_or("");
    let content = ti
        .get("content")
        .and_then(Value::as_str)
        .or_else(|| ti.get("new_string").and_then(Value::as_str))
        .unwrap_or("");

    let cwd = std::env::current_dir().unwrap_or_default();
    let log = agent::runtime_dir(&cwd).join("hook-decisions.log");

    // ---- Layer 1: blocking checks ----
    if let Ok(re) = Regex::new(r"(?i)chain[\s\-]?of[\s\-]?thought") {
        if re.is_match(content) {
            deny(
                &log,
                "Accuracy rule: do not write \"chain-of-thought\" — use \"structured reasoning\" / \
                 \"step-by-step reasoning\". Reword and retry.",
                p,
                "banned-phrase",
            );
        }
    }

    for (pat, what) in cred_patterns() {
        if let Ok(re) = Regex::new(pat) {
            if re.is_match(content) {
                deny(
                    &log,
                    &format!(
                        "Security rule: this looks like a committed {what}. Never write a credential \
                         value. Reference it as \"credential present at <path>\" instead."
                    ),
                    p,
                    "credential",
                );
            }
        }
    }

    if is_budgeted(p) {
        let n = content.split('\n').count();
        if n > LINE_BUDGET {
            deny(
                &log,
                &format!(
                    "Budget rule: {p} is {n} lines (>{LINE_BUDGET}). Keep persona/behavior/rules lean \
                     so adherence doesn't decay — trim to the smallest high-signal set, then retry."
                ),
                p,
                "line-budget",
            );
        }
    }

    // ---- Layer 2: rule surfacing (non-blocking) ----
    let ctx = surface(p, content, expertise.as_deref());
    if ctx.is_empty() {
        std::process::exit(0); // nothing to surface -> silent; let normal permissions decide
    }
    let surfaced = target_manifest(p).unwrap_or("");
    log_decision(&log, "allow", p, "", surfaced);
    io::emit(&json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": cli::take_chars(&ctx, CAP),
        }
    }));
    std::process::exit(0);
}

fn cred_patterns() -> [(&'static str, &'static str); 3] {
    [
        (r"AKIA[0-9A-Z]{16}", "AWS access key id"),
        (
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            "private key block",
        ),
        (
            r#"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['"]?[^\s'"]{6,}"#,
            "credential value",
        ),
    ]
}

/// `(persona|behavior).md` / `CLAUDE.md` are line-budgeted.
fn is_budgeted(p: &str) -> bool {
    Regex::new(r"(?i)(persona|behavior)\.md$|(^|/)CLAUDE\.md$")
        .map(|re| re.is_match(p))
        .unwrap_or(false)
}

/// Which expertise's top rules to re-assert for an artifact path, or `None`.
fn target_manifest(p: &str) -> Option<&'static str> {
    let authoring = r"(?i)(persona|behavior)\.md$|(^|/)CLAUDE\.md$|\.claude/agents/.*\.md$";
    let promptish = r"(?i)(prompt|tool)[A-Za-z0-9_\-]*\.(md|jsonc?|txt)$|\.prompt$";
    if Regex::new(authoring)
        .map(|r| r.is_match(p))
        .unwrap_or(false)
    {
        return Some("persona-creation");
    }
    if Regex::new(promptish)
        .map(|r| r.is_match(p))
        .unwrap_or(false)
    {
        return Some("prompt-engineering");
    }
    None
}

/// Text up to (not including) the first whitespace that follows a `.` or `:` (emulates the Node
/// lookbehind `/(?<=[.:])\s/`).
fn first_sentence(text: &str) -> &str {
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() && matches!(prev, Some('.' | ':')) {
            return &text[..i];
        }
        prev = Some(c);
    }
    text
}

/// Top-N checkable rules for `name`, as `"<id>: <first-sentence>"` lines, read from
/// `<expertise>/manifests/<name>.json`.
fn top_rules(expertise: Option<&str>, name: &str, n: usize) -> Vec<String> {
    let Some(root) = expertise else {
        return Vec::new();
    };
    let path = Path::new(root)
        .join("manifests")
        .join(format!("{name}.json"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(rules) = data.get("rules").and_then(Value::as_array) {
        for r in rules {
            if r.get("type").and_then(Value::as_str) != Some("checkable") {
                continue;
            }
            let raw = r.get("text").and_then(Value::as_str).unwrap_or("").trim();
            let first = first_sentence(raw);
            let id = r.get("id").and_then(Value::as_str).unwrap_or("");
            let clipped = cli::take_chars(first, 170);
            out.push(format!("{id}: {}", clipped.trim_end()));
            if out.len() >= n {
                break;
            }
        }
    }
    out
}

fn surface(p: &str, content: &str, expertise: Option<&str>) -> String {
    if content.chars().count() < SURFACE_MIN {
        return String::new();
    }
    let Some(name) = target_manifest(p) else {
        return String::new();
    };
    let rules = top_rules(expertise, name, SURFACE_N);
    if rules.is_empty() {
        return String::new();
    }
    format!(
        "Governing rules for this artifact — re-asserted before you write ({name}; full manifest at \
         expertise/manifests/{name}.json). Apply them and cite the ones you use in your \
         APPLIED-EXPERTISE lines:\n- {}",
        rules.join("\n- ")
    )
}

fn deny(log: &Path, reason: &str, p: &str, rule: &str) -> ! {
    log_decision(log, "deny", p, rule, "");
    io::emit(&json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }));
    std::process::exit(0);
}

fn log_decision(log: &Path, decision: &str, p: &str, rule: &str, surfaced: &str) {
    io::append_log(
        log,
        &json!({
            "ts": io::now_iso(),
            "hook": "gate",
            "decision": decision,
            "path": p,
            "rule": rule,
            "surfaced": surfaced,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgeted_paths() {
        assert!(is_budgeted("a/persona.md"));
        assert!(is_budgeted("x/behavior.md"));
        assert!(is_budgeted("CLAUDE.md"));
        assert!(is_budgeted("sub/CLAUDE.md"));
        assert!(!is_budgeted("notes.md"));
    }

    #[test]
    fn target_manifest_routing() {
        assert_eq!(
            target_manifest("agent/persona.md"),
            Some("persona-creation")
        );
        assert_eq!(
            target_manifest(".claude/agents/x.md"),
            Some("persona-creation")
        );
        assert_eq!(target_manifest("my_prompt.md"), Some("prompt-engineering"));
        assert_eq!(
            target_manifest("tool_defs.json"),
            Some("prompt-engineering")
        );
        assert_eq!(target_manifest("readme.txt"), None);
    }

    #[test]
    fn first_sentence_stops_after_dot_or_colon() {
        assert_eq!(first_sentence("Do X. Then Y"), "Do X.");
        assert_eq!(first_sentence("Rule: apply it now"), "Rule:");
        assert_eq!(first_sentence("no terminator here"), "no terminator here");
    }

    #[test]
    fn banned_phrase_variants_match() {
        let re = Regex::new(r"(?i)chain[\s\-]?of[\s\-]?thought").unwrap();
        assert!(re.is_match("Chain-of-Thought"));
        assert!(re.is_match("chain of thought"));
        assert!(re.is_match("chainofthought"));
        assert!(!re.is_match("structured reasoning"));
    }

    #[test]
    fn credential_shapes_match() {
        let pats = cred_patterns();
        let aws = Regex::new(pats[0].0).unwrap();
        assert!(aws.is_match("AKIAABCDEFGHIJKLMNOP"));
        let cred = Regex::new(pats[2].0).unwrap();
        assert!(cred.is_match("password = hunter2xy"));
        assert!(cred.is_match("api_key: 'abcdef12'"));
        assert!(!cred.is_match("password: short")); // < 6 non-space chars after
    }
}
