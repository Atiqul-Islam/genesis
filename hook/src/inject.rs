//! INJECT hook (SessionStart / SubagentStart) — deterministic DELIVERY of the house rules +
//! expertise pointers.
//!
//! Faithful port of `hooks/inject.js`. Delivers (1) the checkable house rules and (2) pointers to
//! the decoupled expertise store; and, for a session-copy agent, its carried-over digest. Stays
//! under the 10,000-char hook-output cap.
//!
//! argv: `<expertise_dir> [<agent>] [--main-agent <name>]`.

use crate::{agent, cli, io};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const RULES: &str =
    "Genesis house rules (enforced by hooks — the gate blocks, the validator refuses to finish):\n\
- Never write \"chain-of-thought\"; use \"structured reasoning\" / \"step-by-step reasoning\".\n\
- Never write a credential value; reference it as \"credential present at <path>\".\n\
- Keep persona.md / behavior.md / CLAUDE.md at or under 200 lines each.\n\
- These are checkable and enforced deterministically; do not rely on memory to honor them.";

const SUMMARY_CAP: usize = 7000; // chars of the session-copy digest to inject; the rest is recall-only
const CTX_CAP: usize = 9500; // stay under the 10,000-char hook-output cap

/// Entry point for `genesis-hook inject <expertise_dir> [<agent>] [--main-agent <name>]`.
pub fn run(args: &[String]) {
    let (main_agent, pos) = take_main_agent(args);
    let exp_dir = pos.first().cloned().unwrap_or_default();
    let argv_agent = pos.get(1).cloned().unwrap_or_default();

    let ev = io::parse_event(&io::read_stdin());
    let active = agent::resolve_agent(&ev, &argv_agent, &main_agent);
    let cwd = std::env::current_dir().unwrap_or_default();

    let pointers = build_pointers(&exp_dir);
    let required = build_required(&exp_dir, &active);
    let summary_block = build_summary(&exp_dir, &active, &cwd);

    let mut ctx = format!("{RULES}{required}{pointers}{summary_block}");
    if ctx.chars().count() > CTX_CAP {
        ctx = format!("{}\n…(truncated)", cli::take_chars(&ctx, CTX_CAP));
    }
    io::emit(&json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": ctx,
        }
    }));
    std::process::exit(0);
}

/// Split `--main-agent <name>` out of argv, returning `(main_agent, positionals)`.
fn take_main_agent(args: &[String]) -> (String, Vec<String>) {
    let mut main_agent = String::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--main-agent" && i + 1 < args.len() {
            main_agent.clone_from(&args[i + 1]);
            i += 2;
            continue;
        }
        pos.push(args[i].clone());
        i += 1;
    }
    (main_agent, pos)
}

// Node lists `*.md` with a case-sensitive `endsWith(".md")`; keep that exact semantics.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn build_pointers(exp_dir: &str) -> String {
    if exp_dir.is_empty() || !Path::new(exp_dir).is_dir() {
        return String::new();
    }
    let Ok(entries) = std::fs::read_dir(exp_dir) else {
        return String::new();
    };
    let mut files: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    files.sort();
    if files.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = files
        .iter()
        .map(|f| {
            let stem = f.strip_suffix(".md").unwrap_or(f);
            let full = Path::new(exp_dir).join(f);
            format!("- {stem}: {}", full.to_string_lossy())
        })
        .collect();
    format!(
        "\nYour expertise store (decoupled, authoritative — read the file your behavior names, on \
         demand, before deep work):\n{}",
        lines.join("\n")
    )
}

fn build_required(exp_dir: &str, agent: &str) -> String {
    if agent.is_empty() || exp_dir.is_empty() {
        return String::new();
    }
    let req = required_list(exp_dir, agent);
    if req.is_empty() {
        return String::new();
    }
    format!(
        "\nYou are '{agent}'. Every task, load and apply these REQUIRED expertise: {}. Each has a \
         rule manifest at expertise/manifests/<name>.json (stable rule-ids + predicates). Before \
         finishing, declare the governing rules you actually applied — ONE LINE PER RULE, carrying \
         evidence:\n  APPLIED-EXPERTISE: <name>#<rule-id> — <evidence>\nwhere <evidence> is the file \
         the rule is embodied in (e.g. release-manager/CLAUDE.md) or a short verbatim quote from your \
         output. The Stop hook (validate) verifies each citation: a bare `APPLIED-EXPERTISE: <name>` \
         with no rule-id, a rule-id not in the manifest, or evidence pointing to a nonexistent file / \
         a quote absent from your work all BLOCK finishing. Cite at least the rules you truly used (a \
         token gesture fails).",
        req.join(", ")
    )
}

fn required_list(exp_dir: &str, agent: &str) -> Vec<String> {
    let path = Path::new(exp_dir).join("required.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    data.get(agent)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn build_summary(exp_dir: &str, agent: &str, cwd: &Path) -> String {
    let Some(sp) = find_summary(exp_dir, agent, cwd) else {
        return String::new();
    };
    let Ok(raw) = std::fs::read_to_string(&sp) else {
        return String::new();
    };
    let body = raw.trim();
    if body.is_empty() {
        return String::new();
    }
    let body = if body.chars().count() > SUMMARY_CAP {
        format!(
            "{}\n…(digest truncated — recall the rest via your memory tools)",
            cli::take_chars(body, SUMMARY_CAP)
        )
    } else {
        body.to_string()
    };
    format!(
        "\n\n## Your carried-over session memory (you were created by copying a prior Claude Code \
         session)\n{body}\nThe full prior conversation history + memory is stored under your agent id \
         — recall any specific detail with your memory tools (recall)."
    )
}

/// A session-copy agent's carried-over digest, if present.
fn find_summary(exp_dir: &str, agent: &str, cwd: &Path) -> Option<PathBuf> {
    if agent.is_empty() {
        return None;
    }
    let mut cands = Vec::new();
    if !exp_dir.is_empty() {
        let resolved = if Path::new(exp_dir).is_absolute() {
            PathBuf::from(exp_dir)
        } else {
            cwd.join(exp_dir)
        };
        if let Some(parent) = resolved.parent() {
            cands.push(parent.join("agents").join(agent).join("summary.md"));
        }
    }
    cands.push(
        cwd.join(".genesis")
            .join("agents")
            .join(agent)
            .join("summary.md"),
    );
    cands.into_iter().find(|c| c.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_list_reads_agent_entry() {
        let td = tempfile::tempdir().unwrap();
        std::fs::write(
            td.path().join("required.json"),
            r#"{"method":["persona-creation","prompt-engineering"]}"#,
        )
        .unwrap();
        let req = required_list(td.path().to_str().unwrap(), "method");
        assert_eq!(req, vec!["persona-creation", "prompt-engineering"]);
        assert!(required_list(td.path().to_str().unwrap(), "unknown").is_empty());
    }

    #[test]
    fn pointers_list_md_files() {
        let td = tempfile::tempdir().unwrap();
        std::fs::write(td.path().join("persona-creation.md"), "x").unwrap();
        std::fs::write(td.path().join("prompt-engineering.md"), "y").unwrap();
        std::fs::write(td.path().join("notes.txt"), "z").unwrap();
        let p = build_pointers(td.path().to_str().unwrap());
        assert!(p.contains("- persona-creation: "));
        assert!(p.contains("- prompt-engineering: "));
        assert!(!p.contains("notes"));
    }

    #[test]
    fn take_main_agent_splits() {
        let a: Vec<String> = ["/exp", "method", "--main-agent", "sensei"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let (m, pos) = take_main_agent(&a);
        assert_eq!(m, "sensei");
        assert_eq!(pos, vec!["/exp", "method"]);
    }
}
