//! `genesis-cli demote [target_repo]` — the inverse of promote/install-as-main. Removes the folder's MAIN
//! Claude: strips the genesis managed persona block from CLAUDE.md and the `--main-agent` enforcement hooks
//! from `.claude/settings.json`, preserving everything else. The agent stays available as a subagent.
//!
//! No agent name is taken — only one agent is ever the main Claude, so there is nothing to disambiguate.

use crate::{fsx, render};
use serde_json::json;
use std::path::PathBuf;

/// Entry point for `genesis-cli demote`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let target = args.first().map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    if !target.is_dir() {
        fsx::fail(&format!("target repo not found: {}", target.display()));
    }
    let demoted = render::uninstall_main(&target);
    let claude_md = target.join("CLAUDE.md").to_string_lossy().into_owned();
    let settings = target
        .join(".claude")
        .join("settings.json")
        .to_string_lossy()
        .into_owned();
    let out = match demoted {
        Some(name) => json!({
            "demoted": name,
            "target_repo": target.to_string_lossy(),
            "claude_md": claude_md,
            "settings": settings,
            "note": "removed the managed persona block + main-thread enforcement hooks; the agent remains a subagent",
        }),
        None => json!({
            "demoted": null,
            "target_repo": target.to_string_lossy(),
            "note": "no promoted main agent found — nothing to demote",
        }),
    };
    println!("{}", fsx::json_pretty(&out));
    0
}
