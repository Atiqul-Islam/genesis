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
const RESUME_CAP: usize = 6000; // chars of the compaction resume snapshot to inject inline (rest on disk)
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
    // issue #1: on a post-compaction / resume start, surface the snapshot written by the precompact hook
    // so the agent continues where it left off. Placed high so it survives the CTX cap.
    let resume = build_resume(&ev, &cwd);
    // issue #9: on a startup/resume start, restore any committed transcript(s) into this machine's Claude
    // Code store so native `claude -c` / `--resume` works, and notify the user with the exact command.
    let session_restore = build_session_restore(&ev, &cwd);
    let summary_block = build_summary(&exp_dir, &active, &cwd);

    let mut ctx = format!("{RULES}{required}{resume}{session_restore}{pointers}{summary_block}");
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
    expertise_instruction(agent, &req.join(", "))
}

/// Tell the agent which REQUIRED expertise to load for every task (pointers to its decoupled store).
/// The APPLIED-EXPERTISE declaration mandate was removed (#39) — the validator no longer blocks on it.
fn expertise_instruction(agent: &str, req: &str) -> String {
    format!(
        "\nYou are '{agent}'. Every task, load and apply these REQUIRED expertise: {req}. Each has a \
         rule manifest at expertise/manifests/<name>.json (stable rule-ids + predicates); read the \
         ones your task needs before deep work."
    )
}

fn required_list(exp_dir: &str, agent: &str) -> Vec<String> {
    // Feature 2: DB-first (expertise.db under exp_dir); file fallback below.
    if let Some(v) = crate::expertise_db::required(Path::new(exp_dir), agent) {
        return v;
    }
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

/// The compaction resume block (issue #1): on a SessionStart whose `source` is `compact` or `resume`,
/// read `<cwd>/.genesis/resume-state.md` (written by the precompact hook) and return it, capped, with a
/// pointer to the full file on disk. Empty on any other source, or when no snapshot exists (fail-open).
fn build_resume(ev: &Value, cwd: &Path) -> String {
    let source = ev.get("source").and_then(Value::as_str).unwrap_or("");
    if source != "compact" && source != "resume" {
        return String::new();
    }
    let path = cwd.join(".genesis").join("resume-state.md");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let body = raw.trim();
    if body.is_empty() {
        return String::new();
    }
    let disp = path.to_string_lossy();
    let shown = if body.chars().count() > RESUME_CAP {
        format!(
            "{}\n…(truncated — read the full snapshot at {disp})",
            cli::take_chars(body, RESUME_CAP)
        )
    } else {
        body.to_string()
    };
    format!(
        "\n\n## Resume — recent session state recovered after compaction\n{shown}\nThe full snapshot is \
         on disk at {disp} — read it to recover anything not shown above."
    )
}

/// The cross-system resume block (issue #9): on a SessionStart whose `source` is `startup` or `resume`,
/// restore committed transcripts from `<cwd>/.genesis/sessions/` into this machine's Claude Code store,
/// and return a notice naming the exact `claude --resume <id>`. Empty on other sources or when nothing
/// was restored (fail-open).
fn build_session_restore(ev: &Value, cwd: &Path) -> String {
    let source = ev.get("source").and_then(Value::as_str).unwrap_or("");
    if source != "startup" && source != "resume" {
        return String::new();
    }
    let Some(home) = home_dir() else {
        return String::new();
    };
    let cwd_str = cwd.to_string_lossy();
    let ids = crate::session_transfer::restore(cwd, &home, &cwd_str);
    crate::session_transfer::resume_notice(&ids)
}

/// The user's home directory (`HOME`, or `USERPROFILE` on Windows). `None` if neither is set.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
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
    fn expertise_instruction_lists_required_without_declaration_mandate() {
        let out = expertise_instruction("method", "persona-creation, prompt-engineering");
        assert!(out.contains("REQUIRED expertise: persona-creation, prompt-engineering"));
        // #39: no APPLIED-EXPERTISE recording mandate, no applied-expertise.jsonl, no block note.
        assert!(!out.contains("APPLIED-EXPERTISE"));
        assert!(!out.contains("applied-expertise.jsonl"));
    }

    #[test]
    fn build_resume_only_on_compact_or_resume() {
        use serde_json::json;
        let td = tempfile::tempdir().unwrap();
        let cwd = td.path();
        std::fs::create_dir_all(cwd.join(".genesis")).unwrap();
        std::fs::write(
            cwd.join(".genesis").join("resume-state.md"),
            "# Genesis resume snapshot\n\nUSER: pick up here",
        )
        .unwrap();
        // compact -> injected, with a disk pointer
        let r = build_resume(&json!({"source":"compact"}), cwd);
        assert!(r.contains("Resume — recent session state"));
        assert!(r.contains("pick up here"));
        assert!(r.contains("resume-state.md"));
        // resume -> injected
        assert!(build_resume(&json!({"source":"resume"}), cwd).contains("pick up here"));
        // startup -> NOT injected (avoid stale re-injection)
        assert!(build_resume(&json!({"source":"startup"}), cwd).is_empty());
        // missing snapshot -> empty even on compact (fail-open)
        let empty = tempfile::tempdir().unwrap();
        assert!(build_resume(&json!({"source":"compact"}), empty.path()).is_empty());
    }

    #[test]
    fn build_session_restore_only_on_startup_or_resume() {
        use serde_json::json;
        let td = tempfile::tempdir().unwrap();
        let repo = td.path().join("repo");
        let home = td.path().join("home");
        std::fs::create_dir_all(repo.join(".genesis/sessions")).unwrap();
        std::fs::write(repo.join(".genesis/sessions/sess-9.jsonl"), "x").unwrap();
        // point home_dir() at our temp home for the duration of this test
        std::env::set_var("HOME", &home);

        let cwd_str = repo.to_string_lossy().to_string();
        let enc = crate::session_transfer::encode_project_dir(&cwd_str);
        let placed = home
            .join(".claude/projects")
            .join(&enc)
            .join("sess-9.jsonl");

        // startup -> restores + notice with the resume command
        let out = build_session_restore(&json!({"source":"startup"}), &repo);
        assert!(
            out.contains("claude --resume sess-9"),
            "notice names the resume command"
        );
        assert!(
            placed.is_file(),
            "transcript placed into the machine's project dir"
        );
        // a non-startup/resume source -> nothing
        let td2 = tempfile::tempdir().unwrap();
        let repo2 = td2.path().join("r");
        std::fs::create_dir_all(repo2.join(".genesis/sessions")).unwrap();
        std::fs::write(repo2.join(".genesis/sessions/s.jsonl"), "x").unwrap();
        assert!(build_session_restore(&json!({"source":"compact"}), &repo2).is_empty());
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
