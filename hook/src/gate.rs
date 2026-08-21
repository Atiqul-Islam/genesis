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
    let active = agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or(""));
    if active.is_empty() {
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

    // ---- Layer 1b: per-agent guard (Feature 1 — agent-scoped-guards) ----
    // Reuses the existing guard shape (self_protect + must_match/must_not_match invariants), scoped to
    // the ACTIVE agent. Fail-open: a missing/malformed guard never blocks a session.
    if let Some(guard) = load_guard(&cwd, &active) {
        let after = proposed_content(&ti, p);
        if let Some((reason, rule)) = guard_violation(&guard, p, &after) {
            deny(&log, &reason, p, &rule);
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

/// Load the ACTIVE agent's guard `<cwd>/.genesis/team/<agent>/guard.json`, or `None` when it is
/// absent or malformed (fail-open — a broken guard never blocks a session). Feature 1 —
/// agent-scoped-guards; same schema as the existing `protected_core.json` guard.
fn load_guard(cwd: &Path, agent: &str) -> Option<Value> {
    if agent.is_empty() {
        return None;
    }
    let path = cwd
        .join(".genesis")
        .join("team")
        .join(agent)
        .join("guard.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}

/// Does `write_path` target the guard-entry path `entry`? Suffix match on `/`-normalized paths so a
/// repo-relative entry (`persona.md`, `.genesis/team/atlas/guard.json`) matches the absolute or
/// relative `file_path` a tool passes.
fn path_targets(write_path: &str, entry: &str) -> bool {
    let w = write_path.replace('\\', "/");
    let e = entry.trim_start_matches("./").replace('\\', "/");
    !e.is_empty() && (w == e || w.ends_with(&format!("/{e}")))
}

/// The content the file WILL have after this tool call: `content` for a Write; for an Edit, the current
/// file with the first `old_string` → `new_string` applied (falling back to the fragment if the file
/// can't be read). Used to evaluate guard invariants against the real post-write state.
fn proposed_content(ti: &Value, file_path: &str) -> String {
    if let Some(c) = ti.get("content").and_then(Value::as_str) {
        return c.to_string();
    }
    let new_s = ti.get("new_string").and_then(Value::as_str).unwrap_or("");
    let old_s = ti.get("old_string").and_then(Value::as_str).unwrap_or("");
    match std::fs::read_to_string(file_path) {
        Ok(cur) if !old_s.is_empty() => cur.replacen(old_s, new_s, 1),
        Ok(cur) => cur,
        Err(_) => new_s.to_string(),
    }
}

/// Evaluate the ACTIVE agent's guard against a proposed write. Returns `Some((reason, rule))` if the
/// write would break the guard (a `self_protect` path, or a targeted invariant's `must_match` missing /
/// `must_not_match` present), else `None`. An unparseable invariant regex is skipped (fail-open).
fn guard_violation(guard: &Value, write_path: &str, content: &str) -> Option<(String, String)> {
    if let Some(paths) = guard.get("self_protect").and_then(Value::as_array) {
        for entry in paths.iter().filter_map(Value::as_str) {
            if path_targets(write_path, entry) {
                return Some((
                    format!(
                        "Guard: {write_path} is a protected guard file for this agent — an agent \
                         cannot edit its own guard. Use the coordinator flow /genesis:update_guard."
                    ),
                    "guard-self-protect".to_string(),
                ));
            }
        }
    }
    let invs = guard.get("invariants").and_then(Value::as_array)?;
    for inv in invs {
        let targeted = inv
            .get("files")
            .and_then(Value::as_array)
            .is_some_and(|fs| {
                fs.iter()
                    .filter_map(Value::as_str)
                    .any(|f| path_targets(write_path, f))
            });
        if !targeted {
            continue;
        }
        let id = inv.get("id").and_then(Value::as_str).unwrap_or("?");
        let why = inv.get("why").and_then(Value::as_str).unwrap_or("");
        if let Some(mm) = inv.get("must_match").and_then(Value::as_str) {
            if Regex::new(mm).is_ok_and(|re| !re.is_match(content)) {
                return Some((
                    format!(
                        "Guard invariant {id} would be broken: {write_path} must still satisfy \
                         /{mm}/ ({why}) — restore it before writing."
                    ),
                    format!("guard-{id}"),
                ));
            }
        }
        if let Some(mn) = inv.get("must_not_match").and_then(Value::as_str) {
            if Regex::new(mn).is_ok_and(|re| re.is_match(content)) {
                return Some((
                    format!(
                        "Guard invariant {id} forbids this content in {write_path}: it must NOT match \
                         /{mn}/ ({why})."
                    ),
                    format!("guard-{id}"),
                ));
            }
        }
    }
    None
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
    fn load_guard_reads_per_agent_and_fails_open() {
        // Feature 1 (agent-scoped-guards): the guard is per-agent, and a missing/malformed guard
        // fails OPEN (None) so a broken guard never blocks a session.
        let td = tempfile::tempdir().unwrap();
        let cwd = td.path();
        assert!(load_guard(cwd, "atlas").is_none(), "absent guard => None");
        let gdir = cwd.join(".genesis/team/atlas");
        std::fs::create_dir_all(&gdir).unwrap();
        std::fs::write(
            gdir.join("guard.json"),
            r#"{"self_protect":[],"invariants":[]}"#,
        )
        .unwrap();
        assert!(load_guard(cwd, "atlas").is_some(), "present guard => Some");
        std::fs::write(gdir.join("guard.json"), "not json").unwrap();
        assert!(
            load_guard(cwd, "atlas").is_none(),
            "malformed => None (fail-open)"
        );
    }

    #[test]
    fn guard_self_protect_blocks_the_guard_file() {
        let guard = json!({"self_protect":[".genesis/team/atlas/guard.json"],"invariants":[]});
        assert!(guard_violation(&guard, "/r/.genesis/team/atlas/guard.json", "anything").is_some());
        assert!(guard_violation(&guard, "/r/persona.md", "anything").is_none());
    }

    #[test]
    fn guard_must_match_blocks_when_invariant_dropped() {
        let guard = json!({"self_protect":[],"invariants":[
            {"id":"c1","files":["persona.md"],"must_match":"(?is)per-action\\s+approval","why":"x"}]});
        assert!(
            guard_violation(&guard, "/r/team/atlas/persona.md", "no phrase here").is_some(),
            "dropping the required invariant is a violation"
        );
        assert!(
            guard_violation(
                &guard,
                "/r/team/atlas/persona.md",
                "needs per-action approval"
            )
            .is_none(),
            "keeping the invariant passes"
        );
        assert!(
            guard_violation(&guard, "/r/other.md", "no phrase here").is_none(),
            "a file the invariant does not name is not constrained"
        );
    }

    #[test]
    fn guard_must_not_match_blocks_forbidden_text() {
        let guard = json!({"self_protect":[],"invariants":[
            {"id":"c2","files":["persona.md"],"must_not_match":"(?i)act as anyone","why":"x"}]});
        assert!(guard_violation(&guard, "/r/persona.md", "may act as anyone").is_some());
        assert!(guard_violation(&guard, "/r/persona.md", "acts only as Atiqul").is_none());
    }

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
