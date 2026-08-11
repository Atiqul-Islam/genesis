//! Shared agent rendering — the persona/frontmatter/install-as-main logic ported from assemble.js.
//!
//! Used by `assemble`, `promote`, and `build-plugin-agents`. Output is byte-parity with the Node
//! installer for the agent `.md` frontmatter (those files are committed + drift-checked).

use crate::fsx;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Metadata for an agent: description + tool list (+ required expertise + preloaded skills).
pub struct Meta {
    /// Human-readable one-line description (goes into frontmatter, JSON-quoted).
    pub description: String,
    /// Frontmatter `tools:` list (memory tools appended by the caller).
    pub tools: Vec<String>,
    /// Required expertise (drives required.json + the review hook).
    pub expertise: Vec<String>,
    /// Skills to preload / install.
    pub skills: Vec<String>,
}

/// The per-agent semantic-memory MCP tools every assembled agent receives.
pub const MEMORY_TOOLS: [&str; 3] = [
    "mcp__genesis-memory__store",
    "mcp__genesis-memory__recall",
    "mcp__genesis-memory__consolidate",
];

const EXPERTISE_NOTE: &str = "## Your expertise\n\
- A SessionStart hook injects the house rules and pointers to your decoupled, versioned expertise store.\n\
- Read the expertise file your behavior names, on demand, before deep work. It is authoritative.\n\
- The hard, checkable rules are also enforced by gate/validate hooks — you cannot violate them.";

const MEMORY_NOTE: &str = "## Your memory (per-agent, durable across sessions)\n\
- The `genesis-memory` MCP server gives you your own semantic memory: `store`, `recall`, `consolidate`.\n\
- ALWAYS pass your own agent name as `agent_id` — the store is scoped by it, so you only see your own memories.\n\
- `store` a durable fact/decision; `recall` before deep work to retrieve what you learned before; \
`consolidate` to dedup. This is separate from the transient session context.";

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

/// Built-in team metadata (sensei/method), or `None` for a BUILT agent (which supplies its own meta.json).
#[must_use]
pub fn builtin_meta(name: &str) -> Option<Meta> {
    match name {
        "sensei" => Some(Meta {
            description:
                "Genesis coordinator - the user talks to Sensei; it verifies requirements, plans, delegates \
                 authoring to Method, then assembles, wires, installs, and delivers."
                    .to_string(),
            tools: strs(&["Read", "Write", "Edit", "Bash", "Glob", "Grep", "Agent", "SendMessage"]),
            expertise: strs(&["agent-building", "agentic-teams", "expertise-application"]),
            skills: strs(&["build-agent", "research-expertise"]),
        }),
        "method" => Some(Meta {
            description:
                "Genesis craftsman - authors and tests each agent's persona, behavior, and skills. Writes \
                 tests first; ships nothing untested; never orchestrates."
                    .to_string(),
            tools: strs(&["Read", "Write", "Edit", "Bash", "Glob", "Grep", "SendMessage"]),
            expertise: strs(&["persona-creation", "prompt-engineering", "expertise-application"]),
            skills: strs(&[]),
        }),
        _ => None,
    }
}

/// The persona body common to every assembled agent: persona + behavior + the expertise/memory notes.
///
/// # Errors
/// Returns an error string if persona.md or behavior.md can't be read.
pub fn body(src: &Path) -> Result<String, String> {
    let persona = fsx::read_rstrip(&src.join("persona.md"))
        .ok_or_else(|| format!("cannot read {}", src.join("persona.md").display()))?;
    let behavior = fsx::read_rstrip(&src.join("behavior.md"))
        .ok_or_else(|| format!("cannot read {}", src.join("behavior.md").display()))?;
    Ok([
        persona,
        behavior,
        EXPERTISE_NOTE.to_string(),
        MEMORY_NOTE.to_string(),
    ]
    .join("\n\n"))
}

/// Double-quote a path arg for a generated shell command (handles spaces; native separators kept).
#[must_use]
pub fn q(p: &str) -> String {
    format!("\"{p}\"")
}

/// Portable genesis-home for baking into hooks: `${CLAUDE_PROJECT_DIR}/<rel>` when `gh` is under `target`,
/// else the absolute path (parity with the Node relative/fallback logic).
#[must_use]
pub fn portable_home(target: &Path, gh: &Path) -> String {
    if let Ok(rel) = gh.strip_prefix(target) {
        let r = rel.to_string_lossy().replace('\\', "/");
        if !r.is_empty() && !r.starts_with("..") {
            return format!("${{CLAUDE_PROJECT_DIR}}/{r}");
        }
    }
    gh.to_string_lossy().replace('\\', "/")
}

/// A frontmatter hook `command:` YAML line: a single-quoted scalar (only `'` doubles).
fn yaml_cmd(shell: &str) -> String {
    format!("          command: '{}'\n", shell.replace('\'', "''"))
}

/// Pretty JSON (2-space) with `ensure_ascii` (non-ASCII escaped as `\uXXXX`) + trailing newline —
/// byte-parity with the Node `jsonDumpAscii` used for required.json.
#[must_use]
pub fn json_dump_ascii(v: &Value) -> String {
    let s = serde_json::to_string_pretty(v).unwrap_or_default();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if (c as u32) >= 0x80 {
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push(c);
        }
    }
    out.push('\n');
    out
}

/// Upsert this agent's required expertise into `<gh>/expertise/required.json` (the validate hook reads it).
pub fn register_required(gh: &Path, name: &str, expertise: &[String]) {
    let p = gh.join("expertise").join("required.json");
    let mut data = fsx::read_json(&p).filter(Value::is_object).unwrap_or_else(|| {
        json!({
            "_doc": "Per-agent REQUIRED expertise (auto-registered by assemble); the validate Stop hook \
                     blocks finishing until each is declared via APPLIED-EXPERTISE."
        })
    });
    if let Some(obj) = data.as_object_mut() {
        obj.insert(name.to_string(), json!(expertise));
    }
    let _ = fsx::write_text(&p, &json_dump_ascii(&data));
}

/// Copy one skill dir (must contain SKILL.md) into `dest_root`; returns its name if copied.
fn copy_skill(skill_dir: &Path, dest_root: &Path) -> Option<String> {
    if !skill_dir.is_dir() || !skill_dir.join("SKILL.md").is_file() {
        return None;
    }
    let name = skill_dir.file_name()?.to_string_lossy().into_owned();
    let dest = dest_root.join(&name);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    fsx::copy_tree(skill_dir, &dest).ok()?;
    Some(name)
}

/// BUILT agents: copy each skill under `<src>/skills/` into `<target>/.claude/skills/`; return their names.
#[must_use]
pub fn install_skills(src: &Path, target: &Path) -> Vec<String> {
    let src_skills = src.join("skills");
    if !src_skills.is_dir() {
        return Vec::new();
    }
    let dest_root = target.join(".claude").join("skills");
    let _ = std::fs::create_dir_all(&dest_root);
    let mut entries: Vec<_> = std::fs::read_dir(&src_skills)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries
        .iter()
        .filter_map(|p| copy_skill(p, &dest_root))
        .collect()
}

/// BUILT-IN agents (sensei/method): copy the NAMED skills from `<gh>/skills/` into the repo's skills dir.
#[must_use]
pub fn install_named_skills(gh: &Path, names: &[String], target: &Path) -> Vec<String> {
    if names.is_empty() {
        return Vec::new();
    }
    let src_root = gh.join("skills");
    let dest_root = target.join(".claude").join("skills");
    let _ = std::fs::create_dir_all(&dest_root);
    names
        .iter()
        .filter_map(|n| copy_skill(&src_root.join(n), &dest_root))
        .collect()
}

/// The independent semantic reviewer's prompt (baked into the built-in `agent` review hook).
#[must_use]
pub fn review_prompt_text(name: &str, expertise: &[String]) -> String {
    format!(
        "You are an INDEPENDENT semantic reviewer for the Genesis agent '{name}', which just finished. \
         You did NOT write its work; judge it skeptically. Read the artifacts it produced under the current \
         project (files matching *persona.md, *behavior.md, .claude/agents/*.md, *persona.spec.json, \
         *.tests.json); if there are none, respond {{\"ok\":true}}. Its REQUIRED expertise: {list}. The \
         expertise store is at .genesis/expertise under the project root. For each required expertise \
         <name>, read .genesis/expertise/manifests/<name>.json and check every rule of type \"judgment\" (and \
         non-mechanical \"checkable\" rules) against the artifacts — SKIP mechanical predicate kinds \
         (regex/line_count/declaration), which are enforced deterministically elsewhere. Be skeptical: if an \
         artifact does not clearly satisfy a rule, treat it as FAIL. Respond with ONLY a JSON object: \
         {{\"ok\":true}} if every checked rule is satisfied, or {{\"ok\":false,\"reason\":\"<name>#<rule-id>: what \
         is missing (one per line)\"}}. $ARGUMENTS",
        name = name,
        list = expertise.join(", ")
    )
}

/// The native `genesis-hook` binary path under `home` (`.exe` on Windows).
fn hook_bin(home: &str) -> String {
    let ext = if cfg!(windows) { ".exe" } else { "" };
    format!("{home}/bin/genesis-hook{ext}")
}

/// Render a SUBAGENT's `.claude/agents/<name>.md` frontmatter (byte-parity with the Node assembler).
#[must_use]
pub fn frontmatter(
    name: &str,
    meta: &Meta,
    home: &str,
    skills: &[String],
    expertise: &[String],
) -> String {
    let exp_dir = format!("{home}/expertise");
    let bin = q(&hook_bin(home));
    let inject = format!("{bin} inject {} {name}", q(&exp_dir));
    let gate = format!("{bin} gate --expertise {}", q(&exp_dir));
    let stop = format!("{bin} validate . {name} --expertise {}", q(&exp_dir));
    let skills_line = if skills.is_empty() {
        String::new()
    } else {
        format!("skills: {}\n", skills.join(", "))
    };

    let mut pretooluse = format!(
        "  PreToolUse:\n    - matcher: \"Write|Edit\"\n      hooks:\n        - type: command\n{}",
        yaml_cmd(&gate)
    );
    if name == "sensei" {
        let enforce = format!("{bin} enforce-research");
        let _ = write!(
            pretooluse,
            "    - matcher: \"Bash\"\n      hooks:\n        - type: command\n{}",
            yaml_cmd(&enforce)
        );
    }

    let mut stop_hooks = format!(
        "  Stop:\n    - hooks:\n        - type: command\n{}",
        yaml_cmd(&stop)
    );
    if !expertise.is_empty() {
        stop_hooks.push_str("        - type: agent\n          model: 'claude-haiku-4-5-20251001'\n          timeout: 120\n");
        let _ = writeln!(
            stop_hooks,
            "          prompt: '{}'",
            review_prompt_text(name, expertise).replace('\'', "''")
        );
    }

    format!(
        "---\nname: {name}\ndescription: {}\ntools: {}\n{skills_line}hooks:\n  SessionStart:\n    - matcher: \"startup|resume|compact\"\n      hooks:\n        - type: command\n{}{pretooluse}{stop_hooks}---\n",
        Value::String(meta.description.clone()),
        meta.tools.join(", "),
        yaml_cmd(&inject),
    )
}

// ── install-as-main: make the agent the folder's primary Claude ───────────────────────────────────

/// Render the agent's persona into `<target>/CLAUDE.md` as a managed block (preserving the user's content;
/// replacing only this agent's block on re-run). Returns the CLAUDE.md path.
#[must_use]
pub fn main_claude_md(target: &Path, name: &str, body: &str) -> PathBuf {
    let p = target.join("CLAUDE.md");
    let start = format!(
        "# >>> genesis agent: {name} (managed — content between the sentinels is overwritten) >>>"
    );
    let end = format!("# <<< genesis agent: {name} <<<");
    let block = format!("{start}\n\n{body}\n\n{end}\n");
    let existing = fsx::read_text(&p).unwrap_or_default();
    let merged = match (existing.find(&start), existing.find(&end)) {
        (Some(si), Some(ei)) if ei > si => {
            let pre = &existing[..si];
            let post = &existing[ei + end.len()..];
            let pre_t = pre.trim_end_matches('\n');
            let sep = if pre.trim().is_empty() { "" } else { "\n\n" };
            format!("{pre_t}{sep}{block}{}", post.trim_start_matches('\n'))
        }
        _ => {
            let sep = if existing.is_empty() || existing.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let gap = if existing.trim().is_empty() { "" } else { "\n" };
            format!("{existing}{sep}{gap}{block}")
        }
    };
    let _ = fsx::write_text(&p, &merged);
    p
}

/// The MAIN-THREAD hook config (JSON) for a promoted agent — inject/gate/validate + review, each carrying
/// `--main-agent <name>` so the hooks fire for the main thread (which has no payload agent_type).
#[must_use]
pub fn main_thread_hooks(name: &str, home: &str, expertise: &[String]) -> Value {
    let exp_dir = format!("{home}/expertise");
    let bin = q(&hook_bin(home));
    let inject = format!("{bin} inject {} {name} --main-agent {name}", q(&exp_dir));
    let gate = format!("{bin} gate --expertise {} --main-agent {name}", q(&exp_dir));
    let stop = format!(
        "{bin} validate . {name} --expertise {} --main-agent {name}",
        q(&exp_dir)
    );
    let mut stop_hooks = vec![json!({ "type": "command", "command": stop })];
    if !expertise.is_empty() {
        stop_hooks.push(json!({
            "type": "agent",
            "model": "claude-haiku-4-5-20251001",
            "timeout": 120,
            "prompt": review_prompt_text(name, expertise),
        }));
    }
    json!({
        "SessionStart": [{ "matcher": "startup|resume|compact", "hooks": [{ "type": "command", "command": inject }] }],
        "PreToolUse": [{ "matcher": "Write|Edit", "hooks": [{ "type": "command", "command": gate }] }],
        "Stop": [{ "hooks": stop_hooks }],
    })
}

/// Merge this agent's main-thread hooks into `<target>/.claude/settings.json` — preserving the user's own
/// hooks + other agents' entries, replacing only THIS agent's entries on re-run. Returns the settings path.
pub fn main_settings(target: &Path, name: &str, home: &str, expertise: &[String]) -> PathBuf {
    let p = target.join(".claude").join("settings.json");
    let mut s = fsx::read_json(&p)
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if !s.get("hooks").is_some_and(Value::is_object) {
        if let Some(o) = s.as_object_mut() {
            o.insert("hooks".to_string(), json!({}));
        }
    }
    let fresh = main_thread_hooks(name, home, expertise);
    let cmd_mark = format!("--main-agent {name}");
    let prompt_mark = format!("reviewer for the Genesis agent '{name}'");
    let is_this_agent = |blk: &Value| -> bool {
        blk.get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hs| {
                hs.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|c| c.contains(&cmd_mark))
                        || (h.get("type").and_then(Value::as_str) == Some("agent")
                            && h.get("prompt")
                                .and_then(Value::as_str)
                                .is_some_and(|p| p.contains(&prompt_mark)))
                })
            })
    };
    if let (Some(hooks), Some(fresh_obj)) = (
        s.get_mut("hooks").and_then(Value::as_object_mut),
        fresh.as_object(),
    ) {
        for (ev, fresh_entries) in fresh_obj {
            let mut kept: Vec<Value> = hooks
                .get(ev)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter(|b| !is_this_agent(b)).cloned().collect())
                .unwrap_or_default();
            if let Some(fe) = fresh_entries.as_array() {
                kept.extend(fe.iter().cloned());
            }
            hooks.insert(ev.clone(), json!(kept));
        }
    }
    let _ = fsx::write_text(&p, &fsx::json_pretty(&s));
    p
}

/// Install `name` as the folder's MAIN Claude: persona -> CLAUDE.md block, enforcement -> settings.json.
#[must_use]
pub fn install_as_main(
    target: &Path,
    name: &str,
    home: &str,
    expertise: &[String],
    body: &str,
) -> (PathBuf, PathBuf) {
    (
        main_claude_md(target, name, body),
        main_settings(target, name, home, expertise),
    )
}

/// Remove any genesis managed persona block from `<target>/CLAUDE.md` (name-agnostic — matched by the
/// sentinels, since only one agent is ever main). Returns the demoted agent's name if a block was present.
/// All other CLAUDE.md content is preserved.
#[must_use]
pub fn demote_claude_md(target: &Path) -> Option<String> {
    let p = target.join("CLAUDE.md");
    let existing = fsx::read_text(&p)?;
    let start_prefix = "# >>> genesis agent: ";
    let end_prefix = "# <<< genesis agent: ";
    let si = existing.find(start_prefix)?;
    let ei_start = si + existing[si..].find(end_prefix)?;
    let end_line_end = existing[ei_start..]
        .find('\n')
        .map_or(existing.len(), |n| ei_start + n + 1);
    // pull the name out of the start sentinel line (after the prefix, up to " (")
    let start_line_end = existing[si..].find('\n').map_or(existing.len(), |n| si + n);
    let name = existing[si..start_line_end]
        .strip_prefix(start_prefix)
        .and_then(|s| s.split(" (").next())
        .unwrap_or("")
        .trim()
        .to_string();
    let pre_t = existing[..si].trim_end_matches('\n');
    let post_t = existing[end_line_end..].trim_start_matches('\n');
    let merged = if pre_t.is_empty() {
        post_t.to_string()
    } else if post_t.is_empty() {
        format!("{pre_t}\n")
    } else {
        format!("{pre_t}\n\n{post_t}")
    };
    let _ = fsx::write_text(&p, &merged);
    Some(name)
}

/// True if a hook block is a genesis main-thread entry (carries `--main-agent`, or the review agent's prompt).
fn is_genesis_main_block(blk: &Value) -> bool {
    blk.get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hs| {
            hs.iter().any(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.contains("--main-agent"))
                    || (h.get("type").and_then(Value::as_str) == Some("agent")
                        && h.get("prompt")
                            .and_then(Value::as_str)
                            .is_some_and(|pr| pr.contains("reviewer for the Genesis agent")))
            })
        })
}

/// Remove genesis main-thread hook entries from `<target>/.claude/settings.json`, preserving the user's own
/// hooks. Hook events emptied by the removal are dropped; an emptied `hooks` object is dropped too.
pub fn demote_settings(target: &Path) {
    let p = target.join(".claude").join("settings.json");
    let Some(mut s) = fsx::read_json(&p).filter(Value::is_object) else {
        return;
    };
    if let Some(hooks) = s.get_mut("hooks").and_then(Value::as_object_mut) {
        let events: Vec<String> = hooks.keys().cloned().collect();
        for ev in events {
            let kept: Option<Vec<Value>> = hooks.get(&ev).and_then(Value::as_array).map(|arr| {
                arr.iter()
                    .filter(|b| !is_genesis_main_block(b))
                    .cloned()
                    .collect()
            });
            if let Some(kept) = kept {
                if kept.is_empty() {
                    hooks.remove(&ev);
                } else {
                    hooks.insert(ev, json!(kept));
                }
            }
        }
    }
    if s.get("hooks")
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        if let Some(o) = s.as_object_mut() {
            o.remove("hooks");
        }
    }
    let _ = fsx::write_text(&p, &fsx::json_pretty(&s));
}

/// Un-install the folder's MAIN Claude (inverse of `install_as_main`): strip the CLAUDE.md managed block +
/// the main-thread hooks. Returns the demoted agent name if one was promoted. The agent stays a subagent.
#[must_use]
pub fn uninstall_main(target: &Path) -> Option<String> {
    let name = demote_claude_md(target);
    demote_settings(target);
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_shape() {
        let meta = builtin_meta("method").unwrap();
        let fm = frontmatter(
            "method",
            &meta,
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &[],
            &meta.expertise,
        );
        assert!(fm.contains("name: method"));
        assert!(fm.contains("\"${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook\" gate --expertise"));
        assert!(fm.contains("- type: agent")); // method has expertise -> review hook
        assert!(!fm.contains("matcher: \"Bash\"")); // method is not sensei
    }

    #[test]
    fn sensei_gets_bash_enforce() {
        let meta = builtin_meta("sensei").unwrap();
        let fm = frontmatter(
            "sensei",
            &meta,
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &[],
            &meta.expertise,
        );
        assert!(fm.contains("matcher: \"Bash\""));
        assert!(fm.contains("enforce-research"));
    }

    #[test]
    fn claude_md_merges_and_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        std::fs::write(td.path().join("CLAUDE.md"), "# Mine\n\nkeep me\n").unwrap();
        let _ = main_claude_md(td.path(), "bot", "PERSONA v1");
        let _ = main_claude_md(td.path(), "bot", "PERSONA v2");
        let md = std::fs::read_to_string(td.path().join("CLAUDE.md")).unwrap();
        assert!(md.contains("keep me"));
        assert!(md.contains("PERSONA v2") && !md.contains("PERSONA v1"));
        assert_eq!(md.matches(">>> genesis agent: bot").count(), 1);
    }

    #[test]
    fn main_settings_carry_main_agent_and_preserve_user() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        std::fs::write(
            td.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"user-hook"}]}]}}"#,
        )
        .unwrap();
        main_settings(
            td.path(),
            "bot",
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &["persona-creation".to_string()],
        );
        main_settings(
            td.path(),
            "bot",
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &["persona-creation".to_string()],
        ); // re-run
        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let flat = s["hooks"].to_string();
        assert!(flat.contains("user-hook")); // preserved
        assert!(flat.contains("--main-agent bot"));
        let gate_count = s["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b.to_string().contains("--main-agent bot"))
            .count();
        assert_eq!(gate_count, 1); // idempotent
    }

    #[test]
    fn json_dump_ascii_escapes_nonascii() {
        let v = json!({"_doc": "em—dash"});
        assert!(json_dump_ascii(&v).contains("em\\u2014dash"));
    }

    // demote is the exact inverse of install-as-main: it removes ONLY genesis's block + main-thread hooks,
    // preserving the user's CLAUDE.md content and their own hooks.
    #[test]
    fn demote_reverses_install_as_main_and_preserves_user_content() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        std::fs::write(td.path().join("CLAUDE.md"), "# Mine\n\nkeep me\n").unwrap();
        std::fs::write(
            td.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"user-hook"}]}]}}"#,
        )
        .unwrap();
        let _ = install_as_main(
            td.path(),
            "bot",
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &["persona-creation".to_string()],
            "PERSONA",
        );

        let demoted = uninstall_main(td.path());
        assert_eq!(demoted.as_deref(), Some("bot"));

        let md = std::fs::read_to_string(td.path().join("CLAUDE.md")).unwrap();
        assert!(md.contains("keep me"), "user CLAUDE.md content preserved");
        assert!(!md.contains("genesis agent: bot"), "managed block removed");
        assert!(!md.contains("PERSONA"), "persona removed");

        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let flat = s["hooks"].to_string();
        assert!(flat.contains("user-hook"), "user hook preserved");
        assert!(!flat.contains("--main-agent"), "main-thread hooks removed");
        // SessionStart/Stop existed ONLY for genesis -> those events are dropped entirely
        assert!(s["hooks"].get("SessionStart").is_none());
        assert!(s["hooks"].get("Stop").is_none());

        // idempotent + safe when nothing is promoted
        assert_eq!(uninstall_main(td.path()), None);
    }
}
