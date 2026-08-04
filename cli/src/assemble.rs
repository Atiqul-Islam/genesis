//! `genesis-cli assemble <src> <name> <target> <gh> [--main]` — port of assemble.js.
//!
//! Assembles a member's source (persona.md/behavior.md/skills) into either a SUBAGENT
//! (`<target>/.claude/agents/<name>.md`, default) or the folder's MAIN Claude (`--main`:
//! CLAUDE.md managed block + main-thread hooks in .claude/settings.json). Registers the agent's
//! required expertise and installs its skills either way. `assemble_one` is reused by install/bootstrap.

use crate::fsx;
use crate::render::{self, Meta};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Resolve an agent's metadata: the built-in table (sensei/method) or the BUILT agent's meta.json.
fn load_meta(name: &str, src: &Path) -> Result<Meta, String> {
    if let Some(m) = render::builtin_meta(name) {
        return Ok(m);
    }
    let mp = src.join("meta.json");
    let v = fsx::read_json(&mp).ok_or_else(|| {
        format!(
            "no metadata for agent '{name}' — provide {} with 'description' and 'tools'.",
            mp.display()
        )
    })?;
    let to_strings = |a: &Vec<Value>| {
        a.iter()
            .filter_map(|x| x.as_str().map(ToString::to_string))
            .collect::<Vec<_>>()
    };
    let (Some(description), Some(tools)) = (
        v.get("description").and_then(Value::as_str),
        v.get("tools").and_then(Value::as_array),
    ) else {
        return Err(format!(
            "{} must contain 'description' and 'tools'.",
            mp.display()
        ));
    };
    let expertise = v
        .get("expertise")
        .or_else(|| v.get("required_expertise"))
        .and_then(Value::as_array)
        .map_or_else(Vec::new, to_strings);
    Ok(Meta {
        description: description.to_string(),
        tools: to_strings(tools),
        expertise,
        skills: Vec::new(),
    })
}

/// Assemble one agent; returns a JSON report. Shared by the `assemble` subcommand + install/bootstrap.
///
/// # Errors
/// Returns an error string if metadata is missing/invalid or persona/behavior can't be read.
pub fn assemble_one(
    src: &Path,
    name: &str,
    target: &Path,
    gh: &Path,
    as_main: bool,
) -> Result<Value, String> {
    let mut meta = load_meta(name, src)?;
    // Every assembled agent gets per-agent memory tools.
    meta.tools
        .extend(render::MEMORY_TOOLS.iter().map(|s| (*s).to_string()));
    let expertise = meta.expertise.clone();
    render::register_required(gh, name, &expertise);

    // Skills: built-ins source their NAMED skills from <gh>/skills; built agents from <src>/skills.
    let skills = match render::builtin_meta(name) {
        Some(b) => render::install_named_skills(gh, &b.skills, target),
        None => render::install_skills(src, target),
    };

    let body = render::body(src)?;
    let home = render::portable_home(target, gh);

    if as_main {
        let (claude_md, settings) = render::install_as_main(target, name, &home, &expertise, &body);
        Ok(json!({
            "agent": name, "kind": "main",
            "claude_md": claude_md.to_string_lossy(), "settings": settings.to_string_lossy(),
            "tools": meta.tools, "skills_installed": skills, "expertise": expertise,
            "body_lines": body.lines().count(),
        }))
    } else {
        let out_path: PathBuf = target
            .join(".claude")
            .join("agents")
            .join(format!("{name}.md"));
        let content = format!(
            "{}\n{body}\n",
            render::frontmatter(name, &meta, &home, &skills, &expertise)
        );
        fsx::write_text(&out_path, &content)
            .map_err(|e| format!("failed to write {}: {e}", out_path.display()))?;
        Ok(json!({
            "agent": name, "kind": "subagent", "written": out_path.to_string_lossy(),
            "tools": meta.tools, "skills_installed": skills, "body_lines": body.lines().count(),
        }))
    }
}

/// `genesis-cli assemble` entry point. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let as_main = args.iter().any(|a| a == "--main");
    let pos: Vec<&String> = args.iter().filter(|a| a.as_str() != "--main").collect();
    if pos.len() < 4 {
        fsx::fail("usage: genesis-cli assemble <source_member_dir> <name> <target_repo> <genesis_home> [--main]");
    }
    match assemble_one(
        Path::new(pos[0]),
        pos[1],
        Path::new(pos[2]),
        Path::new(pos[3]),
        as_main,
    ) {
        Ok(report) => {
            println!("{report}");
            0
        }
        Err(e) => fsx::fail(&e),
    }
}
