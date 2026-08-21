//! `genesis-cli promote <name> [target_repo] [genesis_home]` — promote an existing built agent into the
//! folder's MAIN Claude. Port of promote.js.
//!
//! Reads `<target>/.claude/agents/<name>.md`, takes its persona body, and installs it as the folder's main
//! Claude via `render::install_as_main` (CLAUDE.md managed block + main-thread hooks with `--main-agent`).
//! Non-destructive: the subagent `.md` is kept. Idempotent.

use crate::{fsx, render};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Return an assembled agent `.md`'s persona body (everything after its YAML frontmatter block).
#[must_use]
pub fn strip_frontmatter(md: &str) -> String {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(idx) = rest.find("\n---\n") {
            return rest[idx + "\n---\n".len()..]
                .trim_start_matches('\n')
                .trim_end()
                .to_string();
        }
    }
    md.trim_end().to_string()
}

/// The agent's required expertise as registered at build time.
fn required_for(gh: &Path, name: &str) -> Vec<String> {
    fsx::read_json(&gh.join("expertise").join("required.json"))
        .and_then(|d| {
            d.get(name).and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(ToString::to_string))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// The actionable error when `<name>` is not staged in the repo. A Genesis CORE agent (sensei/method,
/// per `render::builtin_meta`) is provisioned by BOOTSTRAPPING the folder — which stages its persona,
/// per-agent memory, and enforcement — so point there; any other name must be BUILT first via
/// `/genesis:new`.
fn missing_agent_message(name: &str, agent_md: &Path) -> String {
    if render::builtin_meta(name).is_some() {
        format!(
            "'{name}' is a Genesis core agent and is not set up in this folder yet ({} is absent).\n\
             Bootstrap the folder first — that stages its persona, per-agent memory, and enforcement — \
             then promote:\n  \
             node \"<plugin>/bin/genesis-memory.js\" --run-cli bootstrap . \"<plugin>\"\n  \
             then: genesis-cli promote {name}\n\
             The /genesis:promote command does both steps for you.",
            agent_md.display()
        )
    } else {
        format!(
            "agent not found: {}\nBuild '{name}' first (/genesis:new), or pass the target repo that contains it.",
            agent_md.display()
        )
    }
}

/// Entry point. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let Some(name) = args.first().map(String::as_str) else {
        fsx::fail("usage: genesis-cli promote <name> [target_repo] [genesis_home]");
    };
    let target = args.get(1).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let gh = args
        .get(2)
        .map_or_else(|| target.join(".genesis"), PathBuf::from);

    let agent_md = target
        .join(".claude")
        .join("agents")
        .join(format!("{name}.md"));
    if !agent_md.is_file() {
        fsx::fail(&missing_agent_message(name, &agent_md));
    }
    let body = strip_frontmatter(&fsx::read_text(&agent_md).unwrap_or_default());
    let expertise = required_for(&gh, name);
    let home = render::portable_home(&target, &gh);
    let warning = if gh.join("bin").is_dir() {
        Value::Null
    } else {
        json!(format!(
            "{}/bin not found — run bootstrap in the target so the genesis-hook binary is staged, else the \
             main-thread enforcement stays dormant.",
            gh.display()
        ))
    };
    let (claude_md, settings) = render::install_as_main(&target, name, &home, &expertise, &body);
    println!(
        "{}",
        fsx::json_pretty(&json!({
            "promoted": name, "kind": "main", "target": target.to_string_lossy(),
            "claude_md": claude_md.to_string_lossy(), "settings": settings.to_string_lossy(),
            "expertise": expertise, "subagent_kept": agent_md.to_string_lossy(), "warning": warning,
            "next": format!("Open Claude Code in {} — the main Claude is now '{name}'.", target.display()),
        }))
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_frontmatter_returns_body() {
        let md = "---\nname: bot\ntools: Read\n---\n\n# Bot\n\nBody.\n";
        assert_eq!(strip_frontmatter(md), "# Bot\n\nBody.");
    }

    #[test]
    fn missing_agent_message_guides_core_vs_custom() {
        // A CORE agent (sensei/method) is provisioned by bootstrap, not built — point there.
        let sensei = missing_agent_message("sensei", Path::new("/repo/.claude/agents/sensei.md"));
        assert!(
            sensei.contains("bootstrap"),
            "core agent points at bootstrap"
        );
        assert!(sensei.contains("core agent"));
        // A custom agent that was never built -> build-first, and NOT bootstrap.
        let custom =
            missing_agent_message("custombot", Path::new("/repo/.claude/agents/custombot.md"));
        assert!(custom.contains("Build 'custombot' first"));
        assert!(!custom.contains("bootstrap"));
    }
}
