//! `genesis-cli install <target_repo> [genesis_home]` — assemble Genesis's own team (sensei + method)
//! into a target repo's `.claude/agents/`. Port of install.js (the memory-server binary check is dropped;
//! that's the launcher's job now).

use crate::{assemble, fsx};
use serde_json::json;
use std::path::PathBuf;

/// Entry point. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let Some(target_arg) = args.first() else {
        fsx::fail(
            "usage: genesis-cli install <target_repo> [genesis_home]  (or set $GENESIS_HOME)",
        );
    };
    let target = PathBuf::from(target_arg);
    let Some(gh) = args
        .get(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("GENESIS_HOME").ok().map(PathBuf::from))
    else {
        fsx::fail("genesis_home required — pass it as the 2nd arg or set $GENESIS_HOME");
    };
    if !target.is_dir() {
        fsx::fail(&format!("target repo not found: {}", target.display()));
    }

    let mut agents = Vec::new();
    for name in ["sensei", "method"] {
        match assemble::assemble_one(&gh.join("team").join(name), name, &target, &gh, false) {
            Ok(r) => agents.push(r.get("agent").cloned().unwrap_or(json!(name))),
            Err(e) => fsx::fail(&format!("assemble {name} failed: {e}")),
        }
    }
    println!(
        "{}",
        fsx::json_pretty(&json!({
            "genesis_home": gh.to_string_lossy(),
            "target_repo": target.to_string_lossy(),
            "agents_installed": agents,
            "agents_dir": target.join(".claude").join("agents").to_string_lossy(),
            "next": "Open Claude Code in the target repo; talk to Sensei: \"build me an agent that …\".",
        }))
    );
    0
}
