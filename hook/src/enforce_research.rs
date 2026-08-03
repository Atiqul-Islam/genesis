//! ENFORCE-RESEARCH hook (PreToolUse: Bash) — Sensei-scoped.
//!
//! Faithful port of `hooks/enforce_research.js`. BLOCKS the assembler Bash call that builds a
//! NON-builtin agent unless the session transcript shows the `research-expertise` Skill was invoked
//! this session. Self-guards: no-op unless a genesis agent is active.
//!
//! Decision:
//!   * command is not an `assemble.js <src> <name> ...` invocation -> allow (not our concern)
//!   * the assembled agent name is a built-in (sensei/method)      -> allow
//!   * transcript shows a `research-expertise` Skill tool_use      -> allow
//!   * otherwise (incl. transcript missing/unreadable)            -> DENY (fail-closed)

use crate::{agent, io};
use serde_json::{json, Value};
use std::path::Path;

const SKILL_NAME: &str = "research-expertise";
const ASSEMBLER: &str = "assemble.js"; // the assembler script basename the enforcer keys on

/// Entry point for `genesis-hook enforce-research`.
pub fn run(_args: &[String]) {
    let ev = io::parse_event(&io::read_stdin());

    // DORMANCY GUARD: no-op unless a genesis agent is active.
    if agent::resolve_agent(&ev, "", "").is_empty() {
        std::process::exit(0);
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let log = agent::runtime_dir(&cwd).join("hook-decisions.log");

    let command = ev
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");

    match assembled_agent_name(command) {
        None => allow(&log, "", ""), // not an assembler invocation
        Some(name) if name == "sensei" || name == "method" => allow(&log, &name, "builtin-exempt"),
        Some(name) if name.is_empty() => deny(
            &log,
            "(unknown)",
            &format!(
                "{ASSEMBLER} invocation with no parseable agent name — cannot verify the \
                 research-expertise skill ran; refusing to build (fail-closed)."
            ),
        ),
        Some(name) => {
            let transcript = ev
                .get("transcript_path")
                .and_then(Value::as_str)
                .unwrap_or("");
            match research_skill_used(transcript) {
                Some(true) => allow(&log, &name, "research-expertise skill confirmed"),
                _ => deny(
                    &log,
                    &name,
                    &format!(
                        "You must run the `research-expertise` skill to select and research \
                         '{name}'s expertise (with the user) BEFORE assembling it. Invoke the \
                         research-expertise skill, complete the process, then assemble again."
                    ),
                ),
            }
        }
    }
}

/// If `command` is an `assemble.js <src> <name> <target> <gh>` invocation, return `Some(<name>)`
/// (`Some("")` if malformed with no name token), else `None`.
fn assembled_agent_name(command: &str) -> Option<String> {
    let toks = shlex_split(command).unwrap_or_else(|()| {
        command
            .split_whitespace()
            .map(ToString::to_string)
            .collect()
    });
    for (i, t) in toks.iter().enumerate() {
        let normalized = t.replace('\\', "/");
        if normalized.ends_with(ASSEMBLER) {
            // positional args after the script: src(i+1), name(i+2), target(i+3), gh(i+4)
            return Some(toks.get(i + 2).cloned().unwrap_or_default());
        }
    }
    None
}

/// `Some(true)`/`Some(false)` if the transcript shows / doesn't show a `research-expertise` Skill
/// tool_use; `None` if the path was given but unreadable (a real error -> fail closed).
fn research_skill_used(transcript_path: &str) -> Option<bool> {
    if transcript_path.is_empty() || !Path::new(transcript_path).is_file() {
        return Some(false);
    }
    let text = std::fs::read_to_string(transcript_path).ok()?;
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(blocks) = ev
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for b in blocks {
            if b.get("type").and_then(Value::as_str) == Some("tool_use")
                && b.get("name").and_then(Value::as_str) == Some("Skill")
            {
                let inp = b.get("input").cloned().unwrap_or(Value::Null);
                let skill_matches = inp.get("skill").and_then(Value::as_str) == Some(SKILL_NAME);
                if skill_matches || inp.to_string().contains(SKILL_NAME) {
                    return Some(true);
                }
            }
        }
    }
    Some(false)
}

/// POSIX-mode shlex split. `Err(())` on an unbalanced quote (parity with Python raising, which the
/// caller catches by falling back to a whitespace split).
fn shlex_split(input: &str) -> Result<Vec<String>, ()> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut i = 0;
    while i < len {
        let ch = chars[i];
        if ch == '\\' {
            in_token = true;
            i += 1;
            if i < len {
                cur.push(chars[i]);
                i += 1;
            } else {
                cur.push('\\');
            }
            continue;
        }
        if ch == '\'' {
            in_token = true;
            i += 1;
            let mut closed = false;
            while i < len {
                if chars[i] == '\'' {
                    closed = true;
                    i += 1;
                    break;
                }
                cur.push(chars[i]);
                i += 1;
            }
            if !closed {
                return Err(());
            }
            continue;
        }
        if ch == '"' {
            in_token = true;
            i += 1;
            let mut closed = false;
            while i < len {
                let qc = chars[i];
                if qc == '\\' {
                    i += 1;
                    if i < len {
                        let esc = chars[i];
                        if esc == '"' || esc == '\\' || esc == '$' || esc == '`' || esc == '\n' {
                            cur.push(esc);
                        } else {
                            cur.push('\\');
                            cur.push(esc);
                        }
                        i += 1;
                    } else {
                        cur.push('\\');
                    }
                } else if qc == '"' {
                    closed = true;
                    i += 1;
                    break;
                } else {
                    cur.push(qc);
                    i += 1;
                }
            }
            if !closed {
                return Err(());
            }
            continue;
        }
        if ch.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut cur));
                in_token = false;
            }
            i += 1;
            continue;
        }
        in_token = true;
        cur.push(ch);
        i += 1;
    }
    if in_token {
        tokens.push(cur);
    }
    Ok(tokens)
}

fn deny(log: &Path, agent_name: &str, reason: &str) -> ! {
    log_decision(log, "deny", agent_name, reason);
    io::emit(&json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }));
    std::process::exit(0);
}

fn allow(log: &Path, agent_name: &str, note: &str) -> ! {
    if !agent_name.is_empty() {
        log_decision(log, "allow", agent_name, note);
    }
    std::process::exit(0); // stay silent; let normal permissions decide
}

fn log_decision(log: &Path, decision: &str, agent_name: &str, reason: &str) {
    io::append_log(
        log,
        &json!({
            "ts": io::now_iso(),
            "hook": "enforce_research",
            "decision": decision,
            "agent": agent_name,
            "reason": reason,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_an_assembler_call_is_none() {
        assert_eq!(assembled_agent_name("ls -la"), None);
        assert_eq!(assembled_agent_name("node other.js a b c"), None);
    }

    #[test]
    fn parses_assembled_name() {
        assert_eq!(
            assembled_agent_name("node install/assemble.js src release-manager /repo /gh"),
            Some("release-manager".to_string())
        );
        // backslash path normalized
        assert_eq!(
            assembled_agent_name(r"node install\assemble.js src mybot /repo /gh"),
            Some("mybot".to_string())
        );
    }

    #[test]
    fn malformed_assembler_call_is_empty_name() {
        assert_eq!(
            assembled_agent_name("node assemble.js onlysrc"),
            Some(String::new())
        );
    }

    #[test]
    fn shlex_handles_quotes_and_unbalanced() {
        assert_eq!(
            shlex_split(r#"node assemble.js "my src" name"#).unwrap(),
            vec!["node", "assemble.js", "my src", "name"]
        );
        assert_eq!(shlex_split(r#"node "unbalanced"#), Err(()));
    }

    #[test]
    fn assembler_name_survives_quoted_paths() {
        assert_eq!(
            assembled_agent_name(r#"node "install/assemble.js" 'src dir' worker /r /g"#),
            Some("worker".to_string())
        );
    }
}
