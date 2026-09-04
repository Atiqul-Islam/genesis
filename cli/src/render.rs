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
        "mneme" => Some(Meta {
            description:
                "Genesis memory specialist - structures each memory the moment it is written, keeps the store \
                 contradiction-free via deterministic bi-temporal supersession, and owns the memory suite \
                 (validate/serialize/deserialize/merge). Never orchestrates."
                    .to_string(),
            tools: strs(&["Read", "Write", "Edit", "Bash", "Glob", "Grep"]),
            expertise: strs(&["memory-management", "expertise-application"]),
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

/// The Mneme reflection prompt (Feature 2, Phase B), baked into the promoted-main Stop `type: agent` hook.
/// Runs Mneme after `name` finishes a turn: it reads the turn and, ONLY on an explicit user directive
/// ("memorize"/"remember"), writes a rule directly (enforced); otherwise it QUEUES a proposal for the user
/// to approve next turn. Never enforces autonomously (ea-6); never writes a credential value (mm-8). Uses
/// project-relative paths (Mneme's cwd is the project root). Carries "the Genesis agent '<name>'" so
/// `main_settings`/`demote` identify + replace/remove it exactly like the reviewer hook.
#[must_use]
pub fn reflect_prompt_text(name: &str) -> String {
    format!(
        "You are Mneme, the memory agent, running the reflection loop for the Genesis agent '{name}' right \
         after it answered the user. Read ONLY the most recent turn from the transcript (the user's last \
         message and '{name}'s response). NEVER write a credential value — reference it as \"credential \
         present at <path>\". Reason step by step: is there a DURABLE, generalizable rule (an always/never \
         that would prevent a repeat mistake), not a one-off fact? \
         (1) If the USER EXPLICITLY directed it (\"memorize\", \"remember this\", \"save this rule\", \"add a \
         rule\", \"for the record\"): the directive IS the approval — write it ENFORCED now by running (via \
         the Node launcher, so it works on macOS/Linux/Windows alike) `node .genesis/bin/genesis-memory.js \
         --run-cli expertise-learn .genesis/expertise add --expertise <bucket> --text \"<one-sentence \
         imperative rule>\" --status active --agents {name}` (reuse an existing bucket from \
         .genesis/expertise/manifests, or a short new one). \
         (2) Otherwise, if you noticed a durable lesson AUTONOMOUSLY: do NOT enforce it — append ONE JSON \
         line to .genesis/mneme/proposals/pending.jsonl (create the dir) of the form \
         {{\"expertise\":\"<bucket>\",\"text\":\"<one-sentence rule>\",\"mode\":\"autonomous\"}}; the next \
         UserPromptSubmit surfaces it for the user to approve. \
         (3) If there is no durable lesson, do NOTHING. Keep it minimal; you are non-blocking and must never \
         fail the turn. Reply with a one-line summary (or 'no lesson'). $ARGUMENTS"
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

    // genesis-default-ultracode: Mneme is a SUBAGENT, so it can't take session-level ultracode — pin it
    // to xhigh via the `effort:` frontmatter field. Other agents get ultracode at the repo (settings.json).
    let effort_line = if name == "mneme" {
        "effort: xhigh\n"
    } else {
        ""
    };
    format!(
        "---\nname: {name}\ndescription: {}\ntools: {}\n{effort_line}{skills_line}hooks:\n  SessionStart:\n    - matcher: \"startup|resume|compact\"\n      hooks:\n        - type: command\n{}{pretooluse}{stop_hooks}---\n",
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
    // issue #3: restage this repo's .genesis/bin to the current plugin version on session start, so a
    // promoted-main repo (which never spawns a core subagent) still picks up /plugin update. Runs via the
    // Node launcher (--sync is a launcher function); idempotent — gated by the .staged-version stamp. It
    // sits in the SAME SessionStart block as inject (which carries --main-agent), so main_settings treats
    // the whole block as this agent's and replaces it idempotently on re-promote.
    let sync = format!(
        "node {} --sync {}",
        q(&format!("{home}/bin/genesis-memory.js")),
        q(home)
    );
    let inject = format!("{bin} inject {} {name} --main-agent {name}", q(&exp_dir));
    let gate = format!("{bin} gate --expertise {} --main-agent {name}", q(&exp_dir));
    // enforce-research on Bash: research-before-assemble + the no-grep guard, scoped to this main agent.
    // A promoted main carries no payload agent_type, so it needs the explicit --main-agent to fire.
    let enforce = format!("{bin} enforce-research --main-agent {name}");
    // issue #1: capture a resume snapshot before a context compaction; inject restores it on the next
    // SessionStart (source compact/resume). Scoped to this main agent.
    let precompact = format!("{bin} precompact --main-agent {name}");
    // issue #9: capture the live transcript into the repo on Stop so it travels; inject restores it on the
    // target machine. Carries --main-agent so main_settings replaces + demote removes it idempotently.
    let capture = format!("{bin} capture-session --main-agent {name}");
    // vector-completeness-warn: the background skip-detector SPAWNER — reads the Stop event, detaches a
    // worker that embeds the response vs. the required rules (via the Node launcher) and writes
    // .genesis/expertise-warnings.md for the next SessionStart. Exits 0 immediately (never blocks Stop).
    // Carries --main-agent so it is co-marked with the block (idempotent replace + demote removal).
    let ewarn = format!(
        "{bin} expertise-warn --agent {name} --expertise {} --launcher {} --main-agent {name}",
        q(&exp_dir),
        q(&format!("{home}/bin/genesis-memory.js")),
    );
    let stop = format!(
        "{bin} validate . {name} --expertise {} --main-agent {name}",
        q(&exp_dir)
    );
    // Feature 2, Phase B: reflect-surface presents Mneme's pending learning proposals to the user at the
    // next prompt (Mneme has no SendMessage). Carries --main-agent (idempotent replace + demote removal).
    let reflect_surface = format!("{bin} reflect-surface --main-agent {name}");
    let mut stop_hooks = vec![
        json!({ "type": "command", "command": capture }),
        json!({ "type": "command", "command": ewarn }),
        json!({ "type": "command", "command": stop }),
    ];
    if !expertise.is_empty() {
        stop_hooks.push(json!({
            "type": "agent",
            "model": "claude-haiku-4-5-20251001",
            "timeout": 120,
            "prompt": review_prompt_text(name, expertise),
        }));
    }
    // Feature 2, Phase B: the Mneme reflection loop — after every turn Mneme judges whether a durable rule
    // should be learned (user-directed → enforced now; autonomous → queued for approval). Runs AFTER
    // validate so it never affects the block/allow decision; non-blocking + fail-open. The prompt marker
    // "the Genesis agent '<name>'" lets main_settings/demote replace + remove it like the reviewer hook.
    stop_hooks.push(json!({
        "type": "agent",
        "model": "claude-haiku-4-5-20251001",
        "timeout": 120,
        "prompt": reflect_prompt_text(name),
    }));
    json!({
        "SessionStart": [{ "matcher": "startup|resume|compact", "hooks": [
            { "type": "command", "command": sync },
            { "type": "command", "command": inject },
        ] }],
        "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": reflect_surface }] }],
        "PreToolUse": [
            { "matcher": "Write|Edit", "hooks": [{ "type": "command", "command": gate }] },
            { "matcher": "Bash", "hooks": [{ "type": "command", "command": enforce }] },
        ],
        "PreCompact": [{ "matcher": "manual|auto", "hooks": [{ "type": "command", "command": precompact }] }],
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
    // Per-agent marker shared by BOTH `type: agent` hooks (the reviewer and the Mneme reflection loop):
    // each prompt contains "the Genesis agent '<name>'", so a re-promote replaces both idempotently.
    let prompt_mark = format!("the Genesis agent '{name}'");
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
    // genesis-default-ultracode: a promoted main starts its sessions in ultracode (xhigh effort + workflow
    // planning). Claude Code READS this key but never writes it, so a user's in-session `/effort` override
    // survives. Takes precedence over effortLevel/modelSettings (docs: settings-reference#ultracode).
    if let Some(o) = s.as_object_mut() {
        o.insert("ultracode".to_string(), json!(true));
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
                            .is_some_and(|pr| pr.contains("the Genesis agent")))
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
    // genesis-default-ultracode: remove the managed `ultracode` key on demote (inverse of main_settings).
    if let Some(o) = s.as_object_mut() {
        o.remove("ultracode");
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

/// The promoted agent's name from `<target>/CLAUDE.md`'s managed sentinel, parsed READ-ONLY. `None` if
/// there is no managed block. Single source for "which agent is promoted here" (issue #12 update path).
#[must_use]
pub fn promoted_agent(target: &Path) -> Option<String> {
    let existing = fsx::read_text(&target.join("CLAUDE.md"))?;
    let start_prefix = "# >>> genesis agent: ";
    let si = existing.find(start_prefix)?;
    let line_end = existing[si..].find('\n').map_or(existing.len(), |n| si + n);
    let after = existing[si..line_end]
        .strip_prefix(start_prefix)
        .unwrap_or("");
    // the name ends at the first " (" (the "(managed …)" note) or the closing " >>>" sentinel marker
    let name = after
        .split(" (")
        .next()
        .unwrap_or(after)
        .trim_end_matches(">>>")
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

/// The agent's required expertise from `<gh>/expertise/required.json`; empty on a missing file/key.
#[must_use]
pub fn required_expertise(gh: &Path, name: &str) -> Vec<String> {
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

/// Refresh the promoted agent's main-thread hooks in `<target>/.claude/settings.json` to the CURRENT
/// template (issue #12: the update path must refresh hook WIRING, not just binaries). Detects the agent
/// from the managed CLAUDE.md block, reads its expertise, and re-runs the idempotent `main_settings`
/// merge. Returns the settings path, or `None` if no agent is promoted here (no-op).
#[must_use]
pub fn sync_settings(target: &Path) -> Option<PathBuf> {
    let name = promoted_agent(target)?;
    let gh = target.join(".genesis");
    let expertise = required_expertise(&gh, &name);
    let home = portable_home(target, &gh);
    Some(main_settings(target, &name, &home, &expertise))
}

/// `genesis-cli sync-settings <target_repo>` — refresh the promoted agent's hook wiring on update.
pub fn run_sync_settings(args: &[String]) -> i32 {
    let target = args.first().map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    match sync_settings(&target) {
        Some(p) => println!(
            "{}",
            fsx::json_pretty(&json!({"synced_settings": p.to_string_lossy()}))
        ),
        None => println!(
            "{}",
            fsx::json_pretty(&json!({"synced_settings": Value::Null}))
        ),
    }
    0
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
        // The hook binary carries a `.exe` suffix on Windows (see `hook_bin`), so the gate command is
        // `...genesis-hook.exe" gate` there — assert with the platform-correct extension.
        let ext = if cfg!(windows) { ".exe" } else { "" };
        assert!(fm.contains(&format!(
            "\"${{CLAUDE_PROJECT_DIR}}/.genesis/bin/genesis-hook{ext}\" gate --expertise"
        )));
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
        let pretool = s["hooks"]["PreToolUse"].as_array().unwrap();
        // idempotent: exactly one gate entry and one Bash enforce-research entry for this agent
        let gate_count = pretool
            .iter()
            .filter(|b| {
                let s = b.to_string();
                s.contains("gate --expertise") && s.contains("--main-agent bot")
            })
            .count();
        assert_eq!(gate_count, 1);
        let enforce_count = pretool
            .iter()
            .filter(|b| b.to_string().contains("enforce-research --main-agent bot"))
            .count();
        assert_eq!(enforce_count, 1);
        // issue #3 AC2: exactly one SessionStart --sync hook after two runs (idempotent, no duplicate).
        let sync_count = s["hooks"]["SessionStart"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|blk| blk["hooks"].as_array().cloned().unwrap_or_default())
            .filter(|h| h["command"].as_str().is_some_and(|c| c.contains("--sync")))
            .count();
        assert_eq!(
            sync_count, 1,
            "one --sync hook, not duplicated on re-promote"
        );
    }

    #[test]
    fn main_thread_hooks_wire_capture_session() {
        // issue #9: a promoted main captures its transcript on Stop so it travels for cross-system resume.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let stop = h["Stop"][0]["hooks"].as_array().unwrap();
        let cmds: Vec<&str> = stop.iter().filter_map(|x| x["command"].as_str()).collect();
        assert!(
            cmds.iter()
                .any(|c| c.contains("capture-session --main-agent bot")),
            "Stop must capture the session, got: {cmds:?}"
        );
        assert!(
            cmds.iter().any(|c| c.contains("validate . bot")),
            "validate still present"
        );
    }

    #[test]
    fn main_thread_hooks_wire_expertise_warn() {
        // vector-completeness-warn AC8: a promoted main wires the expertise-warn Stop command (the
        // background skip-detector spawner), carrying the launcher path + --main-agent so it detaches a
        // worker on Stop and is removed on demote with the rest of the block.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let stop = h["Stop"][0]["hooks"].as_array().unwrap();
        let cmds: Vec<&str> = stop.iter().filter_map(|x| x["command"].as_str()).collect();
        assert!(
            cmds.iter().any(|c| c.contains("expertise-warn")
                && c.contains("--launcher")
                && c.contains("bin/genesis-memory.js")
                && c.contains("--main-agent bot")),
            "Stop must wire expertise-warn with the launcher, got: {cmds:?}"
        );
        // still co-hosts validate + capture in the same Stop block
        assert!(cmds.iter().any(|c| c.contains("validate . bot")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("capture-session --main-agent bot")));
    }

    #[test]
    fn main_thread_hooks_wire_reflection_loop() {
        // Feature 2 Phase B: reflect-surface on UserPromptSubmit + the Mneme reflection type:agent on Stop.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let ups = h["UserPromptSubmit"][0]["hooks"].as_array().unwrap();
        assert!(
            ups.iter().any(|x| x["command"]
                .as_str()
                .is_some_and(|c| c.contains("reflect-surface --main-agent bot"))),
            "UserPromptSubmit wires reflect-surface"
        );
        let stop = h["Stop"][0]["hooks"].as_array().unwrap();
        assert!(
            stop.iter().any(|x| x["type"] == "agent"
                && x["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("reflection loop for the Genesis agent 'bot'"))),
            "Stop wires the Mneme reflection agent"
        );
    }

    #[test]
    fn reflection_loop_idempotent_and_demotable() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        std::fs::write(td.path().join("CLAUDE.md"), "# mine\n").unwrap();
        std::fs::write(td.path().join(".claude/settings.json"), "{}").unwrap();
        for _ in 0..2 {
            let _ = install_as_main(td.path(), "bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[], "P");
        }
        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let ups_count = s["hooks"]["UserPromptSubmit"].as_array().map_or(0, |a| {
            a.iter()
                .flat_map(|b| b["hooks"].as_array().cloned().unwrap_or_default())
                .filter(|h| {
                    h["command"]
                        .as_str()
                        .is_some_and(|c| c.contains("reflect-surface"))
                })
                .count()
        });
        assert_eq!(ups_count, 1, "one reflect-surface after two runs");
        let reflect_agents = s["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|b| b["hooks"].as_array().cloned().unwrap_or_default())
            .filter(|h| {
                h["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("reflection loop for the Genesis agent 'bot'"))
            })
            .count();
        assert_eq!(reflect_agents, 1, "one reflection agent after two runs");
        let _ = uninstall_main(td.path());
        let s2: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert!(
            s2["hooks"].get("UserPromptSubmit").is_none(),
            "demote removes reflect-surface (its only UserPromptSubmit hook)"
        );
        assert!(
            !s2["hooks"].to_string().contains("reflection loop"),
            "demote removes the Mneme reflection agent"
        );
    }

    #[test]
    fn main_thread_hooks_wire_precompact() {
        // issue #1: a promoted main captures a resume snapshot before compaction.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let pc = h["PreCompact"][0].clone();
        assert_eq!(pc["matcher"], "manual|auto");
        assert!(pc["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("precompact --main-agent bot"));
    }

    #[test]
    fn precompact_hook_is_idempotent_and_demotable() {
        // The PreCompact command carries --main-agent, so main_settings replaces it idempotently and
        // demote removes it (both key on --main-agent). Assert one PreCompact after two runs, gone after demote.
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        for _ in 0..2 {
            main_settings(td.path(), "bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        }
        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let pc = s["hooks"]["PreCompact"].as_array().map_or(0, Vec::len);
        assert_eq!(pc, 1, "one PreCompact block, not duplicated");
        let _ = uninstall_main(td.path());
        let s2: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert!(
            s2["hooks"].get("PreCompact").is_none(),
            "demote removes the PreCompact hook"
        );
    }

    #[test]
    fn main_thread_hooks_wire_sessionstart_sync() {
        // issue #3: a promoted main must restage its binary on session start (via the launcher --sync),
        // so /plugin update reaches promoted-main repos. The sync hook sits in the SessionStart block.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let ss = h["SessionStart"][0]["hooks"].as_array().unwrap();
        let cmds: Vec<&str> = ss.iter().filter_map(|x| x["command"].as_str()).collect();
        assert!(
            cmds.iter().any(|c| c.contains("--sync")
                && c.contains("bin/genesis-memory.js")
                && c.contains("${CLAUDE_PROJECT_DIR}/.genesis")),
            "SessionStart must run the launcher --sync on the repo's .genesis, got: {cmds:?}"
        );
        assert!(
            cmds.iter()
                .any(|c| c.contains("inject") && c.contains("--main-agent bot")),
            "SessionStart still injects for the agent"
        );
    }

    #[test]
    fn main_thread_hooks_wire_bash_enforce_research() {
        // no-grep-guard AC7: a promoted main gets a Bash -> enforce-research hook (it had none before),
        // so the grep-guard (and research-before-assemble) fire for it.
        let h = main_thread_hooks("bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        let pretool = h["PreToolUse"].as_array().unwrap();
        let bash = pretool
            .iter()
            .find(|e| e["matcher"] == "Bash")
            .expect("a Bash matcher entry is present");
        assert!(bash["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("enforce-research --main-agent bot"));
        assert!(
            pretool.iter().any(|e| e["matcher"] == "Write|Edit"),
            "the Write|Edit gate entry is still present"
        );
    }

    #[test]
    fn json_dump_ascii_escapes_nonascii() {
        let v = json!({"_doc": "em—dash"});
        assert!(json_dump_ascii(&v).contains("em\\u2014dash"));
    }

    // ---- genesis-default-ultracode (spec: test/specs/genesis-default-ultracode.md) --------------

    #[test]
    fn mneme_frontmatter_pins_xhigh_effort() {
        // Mneme is a SUBAGENT, so it can't get session-level ultracode; it is pinned to xhigh via the
        // `effort:` frontmatter field. Every OTHER agent gets ultracode at the repo level (settings.json)
        // and carries no effort pin.
        let m = builtin_meta("mneme").unwrap();
        let fm = frontmatter(
            "mneme",
            &m,
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &[],
            &m.expertise,
        );
        assert!(
            fm.contains("effort: xhigh"),
            "mneme frontmatter pins xhigh effort"
        );
        let mm = builtin_meta("method").unwrap();
        let fm2 = frontmatter(
            "method",
            &mm,
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &[],
            &mm.expertise,
        );
        assert!(
            !fm2.contains("effort:"),
            "non-mneme agents carry no effort pin"
        );
    }

    #[test]
    fn main_settings_sets_ultracode_default() {
        // A promoted Genesis main starts its sessions in ultracode: `{"ultracode": true}` at the top level
        // of the repo's settings.json (Claude Code reads but never writes it). Idempotent; preserves the
        // user's own keys.
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        std::fs::write(
            td.path().join(".claude/settings.json"),
            r#"{"env":{"FOO":"bar"}}"#,
        )
        .unwrap();
        for _ in 0..2 {
            main_settings(td.path(), "bot", "${CLAUDE_PROJECT_DIR}/.genesis", &[]);
        }
        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(s["ultracode"], Value::Bool(true), "ultracode:true set");
        assert_eq!(s["env"]["FOO"], "bar", "user's own settings preserved");
    }

    #[test]
    fn demote_removes_ultracode() {
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(td.path().join(".claude")).unwrap();
        std::fs::write(td.path().join("CLAUDE.md"), "# mine\n").unwrap();
        std::fs::write(td.path().join(".claude/settings.json"), "{}").unwrap();
        let _ = install_as_main(
            td.path(),
            "bot",
            "${CLAUDE_PROJECT_DIR}/.genesis",
            &[],
            "PERSONA",
        );
        let s1: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(s1["ultracode"], Value::Bool(true), "install sets ultracode");
        let _ = uninstall_main(td.path());
        let s2: Value = serde_json::from_str(
            &std::fs::read_to_string(td.path().join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        assert!(
            s2.get("ultracode").is_none(),
            "demote removes the ultracode key"
        );
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

    // ---- issue #12: sync-settings refreshes the hook WIRING on update ----------------------------

    fn write(p: std::path::PathBuf, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn sync_settings_refreshes_old_wiring_idempotently() {
        // A repo promoted with the OLD template (inject/gate/validate only) must, on sync-settings, gain
        // capture-session (Stop), precompact (PreCompact), and --sync (SessionStart) — the current set —
        // while preserving the user's own hook; and running it twice must be byte-identical.
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        write(
            root.join("CLAUDE.md"),
            "# Mine\n\n# >>> genesis agent: bot (managed) >>>\npersona\n# <<< genesis agent: bot <<<\n",
        );
        write(
            root.join(".genesis/expertise/required.json"),
            r#"{"bot":["persona-creation"]}"#,
        );
        write(
            root.join(".claude/settings.json"),
            r#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"x inject y bot --main-agent bot"}]}],"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"user-hook"}]}],"Stop":[{"hooks":[{"type":"command","command":"x validate . bot --main-agent bot"}]}]}}"#,
        );

        let p = sync_settings(root).expect("agent promoted -> Some");
        assert!(p.ends_with("settings.json"));
        let run1 = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let _ = sync_settings(root); // AC3/AC6: idempotent
        let run2 = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        assert_eq!(run1, run2, "sync-settings is idempotent (byte-identical)");

        let s: Value = serde_json::from_str(&run2).unwrap();
        let flat = s["hooks"].to_string();
        assert!(flat.contains("user-hook"), "user hook preserved");
        assert!(
            flat.contains("capture-session --main-agent bot"),
            "capture wired"
        );
        assert!(
            flat.contains("precompact --main-agent bot"),
            "precompact wired"
        );
        assert!(
            s["hooks"]["PreCompact"].is_array(),
            "PreCompact event present"
        );
        let sync_present = s["hooks"]["SessionStart"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|b| b["hooks"].as_array().cloned().unwrap_or_default())
            .any(|h| h["command"].as_str().is_some_and(|c| c.contains("--sync")));
        assert!(sync_present, "--sync added to SessionStart");
        // AC4: reviewer present because required expertise is non-empty
        let reviewer = s["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|b| b["hooks"].as_array().cloned().unwrap_or_default())
            .any(|h| h["type"] == "agent");
        assert!(reviewer, "reviewer hook present when expertise non-empty");
    }

    #[test]
    fn sync_settings_noop_when_not_promoted() {
        // AC5: no managed CLAUDE.md block -> no agent -> None, settings untouched.
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        write(root.join("CLAUDE.md"), "# just mine\n");
        write(
            root.join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"user-hook"}]}]}}"#,
        );
        let before = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        assert!(sync_settings(root).is_none(), "not promoted -> None");
        let after = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        assert_eq!(before, after, "settings untouched when not promoted");
    }

    #[test]
    fn sync_settings_detects_agent_and_omits_reviewer_when_no_expertise() {
        // AC4 (negative) + agent detection: no required.json -> empty expertise -> no reviewer hook.
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        write(
            root.join("CLAUDE.md"),
            "# >>> genesis agent: bot (managed) >>>\np\n# <<< genesis agent: bot <<<\n",
        );
        write(root.join(".claude/settings.json"), "{}");
        assert_eq!(promoted_agent(root).as_deref(), Some("bot"));
        let _ = sync_settings(root).expect("promoted -> Some");
        let s: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        // The REVIEWER (review-specific prompt) is omitted; the Mneme reflection agent is still present
        // (it is unconditional, independent of expertise).
        let stop_agents: Vec<String> = s["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|b| b["hooks"].as_array().cloned().unwrap_or_default())
            .filter(|h| h["type"] == "agent")
            .filter_map(|h| h["prompt"].as_str().map(ToString::to_string))
            .collect();
        assert!(
            !stop_agents
                .iter()
                .any(|p| p.contains("reviewer for the Genesis agent")),
            "no reviewer when expertise empty"
        );
        assert!(
            stop_agents
                .iter()
                .any(|p| p.contains("reflection loop for the Genesis agent 'bot'")),
            "the Mneme reflection agent is present regardless of expertise"
        );
    }
}
