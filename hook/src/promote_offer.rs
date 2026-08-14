//! PROMOTE-OFFER hook (SessionStart) — repo-local, installed by `bootstrap` into a Genesis WORKSPACE's
//! `.claude/settings.json` (NOT the plugin — the plugin stays dormant-by-default, so a normal Claude Code
//! session anywhere is untouched). Once per session, if this repo is a Genesis workspace but no Genesis
//! agent is the folder's MAIN Claude yet, it offers promotion. It is SILENT (no-op) once promoted, or if
//! this is not a Genesis workspace. FAIL-OPEN — it can never break a session.

use crate::io;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Entry point for `genesis-hook promote-offer`.
pub fn run(_args: &[String]) {
    let _ = io::read_stdin(); // drain the event JSON so the pipe never blocks; the offer needs no fields
    if let Some(ctx) = offer(&project_root()) {
        io::emit(&json!({
            "hookSpecificOutput": { "hookEventName": "SessionStart", "additionalContext": ctx }
        }));
    }
    std::process::exit(0);
}

/// The offer text if `root` is an UN-promoted Genesis workspace, else `None` (stay silent).
fn offer(root: &Path) -> Option<String> {
    if !root.join(".genesis").is_dir() {
        return None; // not a Genesis workspace — nothing to offer
    }
    if is_promoted(root) {
        return None; // a Genesis agent is already the main Claude — nothing to offer
    }
    Some(
        "Genesis: this repo has Genesis agents but none is your MAIN Claude yet. To make Sensei the folder's \
         main agent (full persona + enforcement), run `/genesis:promote sensei`. Optional — ignore this to \
         keep using Genesis via /genesis commands."
            .to_string(),
    )
}

/// True if a Genesis agent is installed as the folder's main Claude — detected by the managed persona block
/// `render::persona_block` writes into `CLAUDE.md` (`# >>> genesis agent: <name> ...`).
fn is_promoted(root: &Path) -> bool {
    std::fs::read_to_string(root.join("CLAUDE.md"))
        .is_ok_and(|s| s.contains(">>> genesis agent:"))
}

/// The repo root: `CLAUDE_PROJECT_DIR` (Claude Code sets it for hooks), else the current dir.
fn project_root() -> PathBuf {
    std::env::var("CLAUDE_PROJECT_DIR").map_or_else(
        |_| std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    )
}

#[cfg(test)]
mod tests {
    use super::offer;

    #[test]
    fn silent_when_not_a_genesis_workspace() {
        let td = tempfile::tempdir().unwrap();
        assert!(offer(td.path()).is_none(), "no .genesis -> no offer");
    }

    #[test]
    fn offers_in_an_unpromoted_workspace() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".genesis")).unwrap();
        let ctx = offer(td.path()).expect("an un-promoted workspace gets the offer");
        assert!(ctx.contains("/genesis:promote sensei"));
    }

    #[test]
    fn silent_once_promoted() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".genesis")).unwrap();
        std::fs::write(
            td.path().join("CLAUDE.md"),
            "# >>> genesis agent: sensei (managed ...) >>>\nbody\n",
        )
        .unwrap();
        assert!(offer(td.path()).is_none(), "promoted -> silent");
    }
}
