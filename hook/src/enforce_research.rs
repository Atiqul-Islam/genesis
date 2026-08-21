//! ENFORCE-RESEARCH hook (PreToolUse: Bash) — Sensei-scoped.
//!
//! Faithful port of `hooks/enforce_research.js`. BLOCKS the assembler Bash call that builds a
//! NON-builtin agent unless the session transcript shows the `research-expertise` Skill was invoked
//! this session. Self-guards: no-op unless a genesis agent is active.
//!
//! Decision:
//!   * command is not an `assemble.js <src> <name> ...` invocation -> allow (not our concern)
//!   * the assembled agent name is a built-in (sensei/method)      -> allow
//!   * a durable per-agent research marker exists in `.genesis/`  -> allow (RESUME-SAFE)
//!   * the current OR any sibling transcript shows the skill      -> allow (+persist the marker)
//!   * otherwise                                                  -> DENY (fail-closed)
//!
//! RESUME SAFETY: a resume/compact rotates the active transcript file, which would otherwise hide
//! research done before the session change and wrongly re-block an in-progress build. We defend two ways:
//! scan every transcript in the session's project dir (the prior one stays a sibling), and once research
//! is confirmed write a durable marker under `.genesis/` so a later session change, compaction, or
//! transcript pruning can never undo it.

use crate::{agent, cli, io};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const SKILL_NAME: &str = "research-expertise";
const ASSEMBLER: &str = "assemble.js"; // the assembler script basename the enforcer keys on

/// Entry point for `genesis-hook enforce-research`.
pub fn run(args: &[String]) {
    // --main-agent lets the gate fire for a promoted main thread (which carries no payload agent_type).
    let (main_agent, _rest) = cli::take_option(args, "--main-agent");
    let ev = io::parse_event(&io::read_stdin());

    // DORMANCY GUARD: no-op unless a genesis agent is active (payload agent_type, or --main-agent fallback).
    let active = agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or(""));
    if active.is_empty() {
        std::process::exit(0);
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let runtime = agent::runtime_dir(&cwd);
    let log = runtime.join("hook-decisions.log");

    let command = ev
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");

    // no-grep-guard: an agent whose discipline forbids grep (it carries no Grep tool) must not grep a
    // FILE to skip reading it — deny and tell it to Read. Piping command OUTPUT to grep is unaffected.
    if NO_GREP_AGENTS.contains(&active.as_str()) && grep_reads_a_file(command) {
        deny(
            &log,
            &active,
            "You have no Grep tool and must READ files in full — never grep a file to skip a read. Use \
             the Read tool on the file instead. (Piping command OUTPUT to grep is fine; grepping a FILE \
             is not.)",
        );
    }

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
            // RESUME-SAFE: a durable marker survives session changes/compaction, so an in-progress
            // build is never re-blocked once its research has been confirmed once.
            let marker = research_marker(&runtime, &name);
            if marker.is_file() {
                allow(&log, &name, "research confirmed (persisted marker)");
            }
            let transcript = ev
                .get("transcript_path")
                .and_then(Value::as_str)
                .unwrap_or("");
            if research_skill_used(transcript) {
                write_marker(&marker); // remember it so a later resume/compact can't undo it
                allow(&log, &name, "research-expertise skill confirmed");
            }
            deny(
                &log,
                &name,
                &format!(
                    "You must run the `research-expertise` skill to select and research \
                     '{name}'s expertise (with the user) BEFORE assembling it. Invoke the \
                     research-expertise skill, complete the process, then assemble again."
                ),
            );
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

/// True if the `research-expertise` Skill tool_use appears in the current transcript OR any sibling
/// transcript in the same project directory. Scanning siblings is what makes this RESUME-SAFE: a
/// resume/compact rotates the active transcript file but leaves the prior one — which holds the skill
/// invocation — in the same `~/.claude/projects/<enc>/` dir.
fn research_skill_used(transcript_path: &str) -> bool {
    if transcript_path.is_empty() {
        return false;
    }
    let cur = Path::new(transcript_path);
    let mut files: Vec<PathBuf> = Vec::new();
    if cur.is_file() {
        files.push(cur.to_path_buf());
    }
    if let Some(dir) = cur.parent() {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.as_path() != cur && p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                    files.push(p);
                }
            }
        }
    }
    files
        .iter()
        .any(|f| std::fs::read_to_string(f).is_ok_and(|t| transcript_has_research(&t)))
}

/// Does one transcript's text contain a `research-expertise` Skill tool_use?
fn transcript_has_research(text: &str) -> bool {
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
                    return true;
                }
            }
        }
    }
    false
}

/// Durable per-agent "research confirmed" marker under the repo's `.genesis` runtime dir. The agent
/// name is sanitized to a safe filename (no path separators / traversal).
fn research_marker(runtime: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    runtime.join("research-done").join(safe)
}

/// Best-effort persist of the research marker (non-fatal — the transcript scan is the fallback).
fn write_marker(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(marker, b"research-expertise confirmed\n");
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

/// Agents whose discipline forbids grep entirely — they carry NO Grep tool and must read files in full.
/// The grep-guard denies a file-grep only for these; agents that legitimately hold the Grep tool
/// (sensei/method/mneme, per `render::builtin_meta`) are never affected. Extend if more are added.
const NO_GREP_AGENTS: [&str; 1] = ["genesis-engineer"];

/// Grep-family content-search commands the guard recognizes.
const GREP_CMDS: [&str; 5] = ["grep", "egrep", "fgrep", "rg", "ag"];

/// A top-level shell control operator token (`shlex_split` yields these as standalone tokens when
/// space-separated).
fn is_shell_operator(tok: &str) -> bool {
    matches!(tok, "|" | "||" | "&&" | ";" | "(" | ")" | "|&" | "&")
}

/// The command basename of a token (`/usr/bin/grep` -> `grep`), slash-normalized.
fn cmd_basename(tok: &str) -> String {
    tok.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// A grep flag that makes grep read files recursively (`-r`, `-R`, `--recursive`, or a combined short
/// flag containing `r`/`R` such as `-rn`).
fn is_recursive_flag(tok: &str) -> bool {
    tok == "--recursive"
        || (tok.starts_with('-')
            && !tok.starts_with("--")
            && (tok.contains('r') || tok.contains('R')))
}

/// True if `command` runs a grep-family tool AGAINST A FILE (the "grep-to-skip-a-read" pattern). Piped
/// greps (`… | grep`) read stdin and are allowed; a bare `grep PATTERN` (stdin) is allowed; `grep PATTERN
/// <file>`, `grep -r …`, and `rg`/`ag` at a command position are file reads. Purely structural (no
/// filesystem lookup) so it is deterministic.
fn grep_reads_a_file(command: &str) -> bool {
    let toks = shlex_split(command).unwrap_or_else(|()| {
        command
            .split_whitespace()
            .map(ToString::to_string)
            .collect()
    });
    for (i, t) in toks.iter().enumerate() {
        let cmd = cmd_basename(t);
        if !GREP_CMDS.contains(&cmd.as_str()) {
            continue;
        }
        // Is this grep a COMMAND (vs an argument to something else)? Command position = index 0, or the
        // previous token is a shell operator. Immediately after a single `|` it reads stdin (allowed).
        let prev = i.checked_sub(1).map(|j| toks[j].as_str());
        let at_command_pos = i == 0 || prev.is_some_and(is_shell_operator);
        if !at_command_pos || prev == Some("|") {
            continue;
        }
        // rg / ag search files by default, even with no path argument.
        if cmd == "rg" || cmd == "ag" {
            return true;
        }
        // grep family: reads a file if it recurses, or has a file arg beyond the pattern.
        let seg: Vec<&String> = toks[i + 1..]
            .iter()
            .take_while(|a| !is_shell_operator(a))
            .collect();
        if seg.iter().any(|a| is_recursive_flag(a)) {
            return true;
        }
        if seg.iter().filter(|a| !a.starts_with('-')).count() >= 2 {
            return true; // pattern + at least one file
        }
    }
    false
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
    fn grep_guard_detects_file_reads_but_allows_piped_and_stdin() {
        // no-grep-guard: a grep-family tool run AGAINST A FILE is a "grep-to-skip-a-read".
        assert!(grep_reads_a_file("grep foo src/x.rs"), "pattern + file");
        assert!(grep_reads_a_file("grep -r foo ."), "recursive over a dir");
        assert!(
            grep_reads_a_file("grep -rn foo src"),
            "combined recursive flag"
        );
        assert!(grep_reads_a_file("rg foo"), "rg searches files by default");
        assert!(grep_reads_a_file("ag foo"), "ag searches files by default");
        assert!(
            grep_reads_a_file("cat a && grep foo b.rs"),
            "grep after && is a fresh command reading a file"
        );
        // Allowed: piped stdin, bare stdin grep, grep as a mere argument, non-grep commands.
        assert!(
            !grep_reads_a_file("cargo test | grep result"),
            "piped -> stdin"
        );
        assert!(!grep_reads_a_file("cat x | rg foo"), "rg piped -> stdin");
        assert!(!grep_reads_a_file("grep foo"), "bare grep reads stdin");
        assert!(
            !grep_reads_a_file("echo grep foo.rs"),
            "grep is an echo arg, not a command"
        );
        assert!(!grep_reads_a_file("ls -la"), "not a grep at all");
        assert!(!grep_reads_a_file("rustc --explain E0425"), "no grep");
    }

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

    fn assistant_skill_line(skill: &str) -> String {
        json!({
            "type": "assistant",
            "message": {"content": [{"type": "tool_use", "name": "Skill", "input": {"skill": skill}}]}
        })
        .to_string()
    }

    #[test]
    fn transcript_has_research_detects_the_skill() {
        assert!(transcript_has_research(&assistant_skill_line(
            "research-expertise"
        )));
        assert!(!transcript_has_research(&assistant_skill_line(
            "some-other-skill"
        )));
        assert!(!transcript_has_research("not json\n{}"));
    }

    #[test]
    fn research_marker_sanitizes_the_agent_name() {
        let m = research_marker(Path::new("/repo/.genesis"), "../evil/name");
        assert_eq!(m, Path::new("/repo/.genesis/research-done/___evil_name"));
        assert!(!m.to_string_lossy().contains(".."));
    }

    // The core fix: research recorded in a PRIOR (sibling) transcript is still found after a resume
    // rotates the active transcript file.
    #[test]
    fn research_found_in_sibling_transcript_survives_resume() {
        let dir = tempfile::tempdir().unwrap();
        // the "old" (pre-resume) transcript holds the research-expertise invocation
        std::fs::write(
            dir.path().join("old-session.jsonl"),
            assistant_skill_line("research-expertise"),
        )
        .unwrap();
        // the "current" (post-resume) transcript is fresh and does NOT contain it
        let current = dir.path().join("new-session.jsonl");
        std::fs::write(&current, assistant_skill_line("something-else")).unwrap();

        assert!(
            research_skill_used(current.to_str().unwrap()),
            "research in a sibling transcript must still count after a resume"
        );
    }

    #[test]
    fn research_absent_everywhere_is_false() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("s.jsonl");
        std::fs::write(&current, assistant_skill_line("nope")).unwrap();
        assert!(!research_skill_used(current.to_str().unwrap()));
        assert!(!research_skill_used(""));
    }
}
