//! `genesis-cli build-plugin-agents [repo_root]` — regenerate the PLUGIN-shipped
//! `agents/{sensei,method,mneme}.md` from the team sources. Port of build_plugin_agents.js.
//!
//! Different from `assemble`: a plugin-shipped agent CANNOT declare `hooks`/`mcpServers`/`permissionMode`
//! in frontmatter (security-blocked), so these carry ONLY name/description/tools/skills, and their memory
//! tools use the plugin-scoped names `mcp__plugin_<plugin>_genesis-memory__{store,recall,consolidate}`.
//! Enforcement + memory come from the plugin level (hooks/hooks.json + the plugin-root .mcp.json).
//! Body composition matches `assemble` (persona + behavior + the notes), so the agents never drift.

use crate::{fsx, render};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn plugin_name(repo: &Path) -> Option<String> {
    fsx::read_json(&repo.join(".claude-plugin").join("plugin.json"))?
        .get("name")?
        .as_str()
        .map(String::from)
}

fn memory_tools(plugin: &str) -> Vec<String> {
    ["store", "recall", "consolidate"]
        .iter()
        .map(|t| format!("mcp__plugin_{plugin}_genesis-memory__{t}"))
        .collect()
}

/// Frontmatter for a plugin-shipped agent: name/description/tools/skills only (NO hooks).
fn plugin_frontmatter(
    name: &str,
    description: &str,
    tools: &[String],
    skills: &[String],
) -> String {
    let skills_line = if skills.is_empty() {
        String::new()
    } else {
        format!("skills: {}\n", skills.join(", "))
    };
    format!(
        "---\nname: {name}\ndescription: {}\ntools: {}\n{skills_line}---\n",
        Value::String(description.to_string()),
        tools.join(", "),
    )
}

/// Entry point. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let repo = args.first().map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let Some(plugin) = plugin_name(&repo) else {
        fsx::fail(&format!(
            "cannot read plugin name from {}",
            repo.join(".claude-plugin").join("plugin.json").display()
        ));
    };
    let mem = memory_tools(&plugin);

    let mut out = Vec::new();
    for name in ["sensei", "method", "mneme"] {
        let Some(meta) = render::builtin_meta(name) else {
            continue; // unreachable for the built-in team
        };
        let body = match render::body(&repo.join("team").join(name)) {
            Ok(b) => b,
            Err(e) => fsx::fail(&e),
        };
        let tools: Vec<String> = meta
            .tools
            .iter()
            .cloned()
            .chain(mem.iter().cloned())
            .collect();
        let skills: Vec<String> = meta
            .skills
            .iter()
            .filter(|s| repo.join("skills").join(s).is_dir())
            .cloned()
            .collect();
        let content = format!(
            "{}\n{body}\n",
            plugin_frontmatter(name, &meta.description, &tools, &skills)
        );
        let out_path = repo.join("agents").join(format!("{name}.md"));
        if let Err(e) = fsx::write_text(&out_path, &content) {
            fsx::fail(&format!("failed to write {}: {e}", out_path.display()));
        }
        out.push(json!({ "agent": name, "written": out_path.to_string_lossy(), "tools": tools, "skills": skills }));
    }
    println!(
        "{}",
        fsx::json_pretty(&json!({ "plugin_root": repo.to_string_lossy(), "agents": out }))
    );
    0
}
