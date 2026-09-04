//! VALIDATE hook (Stop / SubagentStop) — the loop-closer.
//!
//! Faithful port of `hooks/validate.js`. Refuses to FINISH while a checkable rule is still violated,
//! OR while the agent never *credibly* declared it applied its required expertise. Three enforcement
//! layers: (1) checkable-rule offenders in produced files; (2) declaration integrity — real rule-ids,
//! a coverage floor, and evidence that is a VERBATIM quote of the rule's own text (forces reading).
//! Fail-closed; guards an infinite block loop via `stop_hook_active`. Self-guards on the active agent.

use crate::{agent, cli, glob, io};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

const LINE_BUDGET: usize = 200;
const FLOOR: usize = 3; // min distinct real rule-ids per required expertise (or all checkable, if fewer)
const BULLET_WORD_LIMIT: usize = 20; // reply-format guard: max words per bullet/point
/// Agents whose replies must be tables/bullets with ≤20 words per point (reply-format guard).
const BULLET_FORMAT_AGENTS: [&str; 1] = ["genesis-engineer"];

/// Parsed APPLIED-EXPERTISE declarations: expertise-name -> [(rule-id, evidence)], plus the set of
/// bare (rule-id-less) declaration names.
type Declarations = (HashMap<String, Vec<(String, String)>>, HashSet<String>);

/// Entry point for `genesis-hook validate <root> [agent] [--expertise <root>] [--main-agent <name>]`.
pub fn run(args: &[String]) {
    let (expertise, rest) = cli::take_option(args, "--expertise");
    let ev = io::parse_event(&io::read_stdin());

    // Avoid an endless block loop: if we already blocked once this stop-chain, let it through.
    if ev.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
        std::process::exit(0);
    }

    let (argv_root, argv_agent, main_agent) = agent::split_args(&rest);
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = argv_root.map_or_else(|| cwd.clone(), std::path::PathBuf::from);
    let active = agent::resolve_agent(&ev, &argv_agent, &main_agent);
    let session = ev.get("session_id").and_then(Value::as_str).unwrap_or("");

    // DORMANCY GUARD: do nothing unless a genesis agent is active.
    if active.is_empty() {
        std::process::exit(0);
    }

    let log = agent::runtime_dir(&cwd).join("hook-decisions.log");
    let manifest_dir = expertise.as_ref().map(|r| Path::new(r).join("manifests"));
    let required_json = expertise
        .as_ref()
        .map(|r| Path::new(r).join("required.json"));

    let files = glob::produced_files(&root);
    let mut reasons = offenders(&files);

    // Reply-format guard: for agents on the format list, flag any over-long bullet in THIS turn's reply.
    let transcript = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    reasons.extend(format_reasons(
        &active,
        &current_turn_visible_text(transcript),
    ));

    let mut cited: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Accepted (name, rule-id, evidence) triples for the audit log written on a clean finish.
    let mut accepted: Vec<(String, String, String)> = Vec::new();
    let required = required_for(required_json.as_deref(), &active);
    if !required.is_empty() {
        match parse_declarations(transcript) {
            None => reasons.push(
                "Could not read the transcript to verify the expertise declaration — cannot finish."
                    .to_string(),
            ),
            Some((decls, bare)) => {
                for name in &required {
                    if let Some((all_ids, _)) = load_manifest(manifest_dir.as_deref(), name) {
                        let entries = decls.get(name);
                        let ids: Vec<String> = entries
                            .map(|entries| {
                                entries
                                    .iter()
                                    .filter(|(rid, _)| all_ids.contains(rid))
                                    .map(|(rid, _)| rid.clone())
                                    .collect::<BTreeSet<_>>()
                                    .into_iter()
                                    .collect()
                            })
                            .unwrap_or_default();
                        for rid in &ids {
                            let evid = entries
                                .and_then(|es| es.iter().find(|(r, _)| r == rid))
                                .map(|(_, e)| e.clone())
                                .unwrap_or_default();
                            accepted.push((name.clone(), rid.clone(), evid));
                        }
                        if !ids.is_empty() {
                            cited.insert(name.clone(), ids);
                        }
                    }
                }
                reasons.extend(verify_declaration(
                    manifest_dir.as_deref(),
                    &required,
                    &decls,
                    &bare,
                ));
            }
        }
    }

    if !reasons.is_empty() {
        log_decision(&log, &active, "block", &reasons, &cited, session);
        let shown: Vec<String> = reasons.iter().take(20).cloned().collect();
        io::emit(&json!({
            "decision": "block",
            "reason": format!("Cannot finish:\n- {}\nFix these, then stop again.", shown.join("\n- ")),
        }));
        std::process::exit(0);
    }
    // Feature 2 (verbose-declarations) AC3: on a clean finish, log every accepted citation to the
    // per-repo audit trail — this happens whether or not the declarations were displayed in prose.
    let audit = agent::runtime_dir(&cwd).join("applied-expertise.log.jsonl");
    for rec in applied_records(&active, session, &accepted) {
        io::append_log(&audit, &rec);
    }
    log_decision(&log, &active, "allow", &[], &cited, session);
    std::process::exit(0);
}

/// Build the audit records written to `applied-expertise.log.jsonl` on a clean finish (Feature 2 —
/// verbose-declarations): one JSONL record per accepted citation, so every applied rule is logged
/// whether or not it was displayed in visible prose.
fn applied_records(
    agent: &str,
    session: &str,
    accepted: &[(String, String, String)],
) -> Vec<Value> {
    let ts = io::now_iso();
    accepted
        .iter()
        .map(|(name, rid, evid)| {
            json!({
                "ts": ts,
                "agent": agent,
                "session": session,
                "name": name,
                "rule_id": rid,
                "evidence": evid,
            })
        })
        .collect()
}

/// Checkable-rule offenders (banned phrase / credential value / line budget) in produced files.
fn offenders(files: &[(String, String)]) -> Vec<String> {
    let banned = Regex::new(r"(?i)chain[\s\-]?of[\s\-]?thought").ok();
    let creds: Vec<Regex> = [
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        r#"(?i)\b(?:password|passwd|secret|api[_-]?key|token)\b\s*[:=]\s*['"]?[^\s'"]{6,}"#,
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect();
    let budgeted = Regex::new(r"(?i)(persona|behavior)\.md$|(^|/)CLAUDE\.md$").ok();

    let mut out = Vec::new();
    for (f, txt) in files {
        if banned.as_ref().is_some_and(|re| re.is_match(txt)) {
            out.push(format!(
                "{f}: contains \"chain-of-thought\" — use \"structured reasoning\"."
            ));
        }
        if creds.iter().any(|re| re.is_match(txt)) {
            out.push(format!(
                "{f}: looks like a committed credential value — remove it."
            ));
        }
        if budgeted.as_ref().is_some_and(|re| re.is_match(f)) {
            let n = txt.split('\n').count();
            if n > LINE_BUDGET {
                out.push(format!("{f}: {n} lines (>{LINE_BUDGET} budget) — trim."));
            }
        }
    }
    out
}

fn required_for(required_json: Option<&Path>, agent: &str) -> Vec<String> {
    if agent.is_empty() {
        return Vec::new();
    }
    let Some(path) = required_json else {
        return Vec::new();
    };
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

/// `(all_rule_ids, checkable_rule_ids)` or `None` if the manifest can't be read.
fn load_manifest(
    manifest_dir: Option<&Path>,
    name: &str,
) -> Option<(HashSet<String>, HashSet<String>)> {
    let path = manifest_dir?.join(format!("{name}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let data = serde_json::from_str::<Value>(&text).ok()?;
    let mut ids = HashSet::new();
    let mut checkable = HashSet::new();
    if let Some(rules) = data.get("rules").and_then(Value::as_array) {
        for r in rules {
            let rid = r
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            if rid.is_empty() {
                continue;
            }
            ids.insert(rid.clone());
            if r.get("type").and_then(Value::as_str) == Some("checkable") {
                checkable.insert(rid);
            }
        }
    }
    Some((ids, checkable))
}

/// `(declarations, bare-set)` or `None` if the transcript existed but was unreadable (fail closed).
/// A missing/empty transcript yields empty declarations (not `None`) — parity with the Node hook.
fn parse_declarations(p: &str) -> Option<Declarations> {
    if p.is_empty() || !Path::new(p).is_file() {
        return Some((HashMap::new(), HashSet::new()));
    }
    let text = std::fs::read_to_string(p).ok()?;
    let decl = Regex::new(
        r"(?im)APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*#\s*([a-z]+-[0-9]+)\b[ \t]*(?:[—:\-]+[ \t]*(.*?))?[ \t]*$",
    )
    .ok()?;
    let bare_re =
        Regex::new(r"(?im)APPLIED-EXPERTISE:\s*([a-z0-9\-]+)\s*(?:$|[^#A-Za-z0-9_])").ok()?;

    let mut decls: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut bare: HashSet<String> = HashSet::new();

    // D4 fix — turn-scoping. Only APPLIED-EXPERTISE declarations from the CURRENT turn count:
    // scan only the transcript records AFTER the last genuine user (human) message. Without this the
    // validator re-checked every declaration ever emitted this session against files produced in the
    // current turn — unsatisfiable on a long session. Tool-result records are also `type:"user"`, so
    // a real human turn is one whose content is a string or carries a `text` block (never solely a
    // tool_result). A transcript with no human record leaves `start = 0` (parses all — prior parity).
    let lines: Vec<&str> = text.split('\n').collect();
    let start = turn_start(&lines);

    for raw in &lines[start..] {
        let line = raw.trim_end_matches('\r');
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        // Visible prose (verbose display) AND the quiet record channel (Feature 2) both count: a
        // quiet agent records APPLIED-EXPERTISE by writing them to `applied-expertise.jsonl` instead
        // of printing them, and the validator enforces either source identically.
        let mut texts = assistant_texts(&ev);
        texts.extend(record_channel_texts(&ev));
        for t in &texts {
            for caps in decl.captures_iter(t) {
                let name = caps.get(1).map_or("", |m| m.as_str()).to_lowercase();
                let rid = caps.get(2).map_or("", |m| m.as_str()).to_lowercase();
                let evid = caps.get(3).map_or("", |m| m.as_str()).trim().to_string();
                decls.entry(name).or_default().push((rid, evid));
            }
            for caps in bare_re.captures_iter(t) {
                bare.insert(caps.get(1).map_or("", |m| m.as_str()).to_lowercase());
            }
        }
    }
    Some((decls, bare))
}

/// The text of every assistant text block in a transcript record (string content is treated as a
/// single text block, matching the Node port).
fn assistant_texts(ev: &Value) -> Vec<String> {
    let content = ev.get("message").and_then(|m| m.get("content"));
    match content {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| {
                b.get("text")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Declaration text recorded via the QUIET record channel (Feature 2 — verbose-declarations): the
/// `content` (Write) or `new_string` (Edit) of a tool_use whose target file is
/// `applied-expertise.jsonl`. This lets an agent record its APPLIED-EXPERTISE lines without printing
/// them in visible prose, while the validator enforces them identically. A `\`-style path is
/// normalized so a Windows `file_path` still matches.
fn record_channel_texts(ev: &Value) -> Vec<String> {
    let Some(Value::Array(blocks)) = ev.get("message").and_then(|m| m.get("content")) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter(|b| {
            matches!(
                b.get("name").and_then(Value::as_str),
                Some("Write" | "Edit")
            )
        })
        .filter_map(|b| {
            let input = b.get("input")?;
            let path = input.get("file_path").and_then(Value::as_str)?;
            if !path.replace('\\', "/").ends_with("applied-expertise.jsonl") {
                return None;
            }
            input
                .get("content")
                .and_then(Value::as_str)
                .or_else(|| input.get("new_string").and_then(Value::as_str))
                .map(ToString::to_string)
        })
        .collect()
}

/// The index of the first transcript line AFTER the last genuine human message — the current turn's start.
/// Tool-result records are also `type:"user"`; a real human turn has string content or a `text` block, and
/// never `isMeta:true` (Stop-hook feedback / system reminders are meta). No human record -> 0 (parse all).
fn turn_start(lines: &[&str]) -> usize {
    let mut start = 0usize;
    for (i, raw) in lines.iter().enumerate() {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let is_meta = ev.get("isMeta").and_then(Value::as_bool) == Some(true);
        let is_human = !is_meta
            && match ev.get("message").and_then(|m| m.get("content")) {
                Some(Value::String(_)) => true,
                Some(Value::Array(blocks)) => blocks
                    .iter()
                    .any(|b| b.get("type").and_then(Value::as_str) == Some("text")),
                _ => false,
            };
        if is_human {
            start = i + 1;
        }
    }
    start
}

/// The concatenated VISIBLE assistant text (text blocks only, not the record channel) of the current turn.
/// Fail-open: empty string on a missing/unreadable transcript.
fn current_turn_visible_text(p: &str) -> String {
    if p.is_empty() || !Path::new(p).is_file() {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(p) else {
        return String::new();
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let start = turn_start(&lines);
    let mut out: Vec<String> = Vec::new();
    for raw in &lines[start..] {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) == Some("assistant") {
            out.extend(assistant_texts(&ev));
        }
    }
    out.join("\n")
}

/// The text after a markdown bullet marker (`- `/`* `/`+ `/`N. `/`N) `), or `None` if not a bullet line.
fn bullet_body(trimmed: &str) -> Option<&str> {
    for m in ["- ", "* ", "+ "] {
        if let Some(b) = trimmed.strip_prefix(m) {
            return Some(b);
        }
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &trimmed[digits..];
        return rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "));
    }
    None
}

/// Markdown bullet lines in `text` that exceed the 20-word limit (reply-format guard). CONSERVATIVE: skips
/// fenced code blocks and `APPLIED-EXPERTISE` lines, and only checks markdown bullets — table rows,
/// headers, and prose are never flagged, so it cannot false-block on those.
fn overlong_bullets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for raw in text.split('\n') {
        let trimmed = raw.trim_end_matches('\r').trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || trimmed.starts_with("APPLIED-EXPERTISE:") {
            continue;
        }
        let Some(body) = bullet_body(trimmed) else {
            continue;
        };
        let words = body.split_whitespace().count();
        if words > BULLET_WORD_LIMIT {
            let clip: String = body.chars().take(60).collect();
            out.push(format!("{words} words: \"{clip}…\""));
        }
    }
    out
}

/// Reply-format reasons for an agent on `BULLET_FORMAT_AGENTS`: one per over-long bullet in `text`.
fn format_reasons(active: &str, text: &str) -> Vec<String> {
    if !BULLET_FORMAT_AGENTS.contains(&active) {
        return Vec::new();
    }
    overlong_bullets(text)
        .into_iter()
        .map(|b| {
            format!(
                "Reply-format rule: a bullet exceeds {BULLET_WORD_LIMIT} words ({b}). Split it into \
                 shorter points."
            )
        })
        .collect()
}

fn quote_norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Rule-id -> rule `text`, from `expertise/manifests/<name>.json` (for the verbatim-quote evidence check).
fn manifest_rule_texts(manifest_dir: Option<&Path>, name: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(dir) = manifest_dir else {
        return out;
    };
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.json"))) else {
        return out;
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return out;
    };
    if let Some(rules) = data.get("rules").and_then(Value::as_array) {
        for r in rules {
            let id = r
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let t = r.get("text").and_then(Value::as_str).unwrap_or("");
            if !id.is_empty() && !t.is_empty() {
                out.insert(id, t.to_string());
            }
        }
    }
    out
}

/// Minimum normalized length of a rule quote that counts as evidence (blocks trivial matches).
const MIN_RULE_QUOTE: usize = 20;

/// True if `evid` contains a verbatim (normalized, >= `MIN_RULE_QUOTE` chars) snippet of `rule_text`.
/// Forces the agent to have READ the rule — you cannot quote what you did not open. Deterministic, no LLM.
fn quote_is_from_rule(evid: &str, rule_text: &str) -> bool {
    let stripped = evid
        .trim()
        .trim_matches(|c| c == '"' || c == '`' || c == '\'');
    let cand = quote_norm(stripped);
    cand.chars().count() >= MIN_RULE_QUOTE && quote_norm(rule_text).contains(&cand)
}

fn verify_declaration(
    manifest_dir: Option<&Path>,
    required: &[String],
    decls: &HashMap<String, Vec<(String, String)>>,
    bare: &HashSet<String>,
) -> Vec<String> {
    let empty: Vec<(String, String)> = Vec::new();

    let mut reasons = Vec::new();
    for name in required {
        let Some((all_ids, checkable)) = load_manifest(manifest_dir, name) else {
            reasons.push(format!(
                "Could not read the manifest for '{name}' to verify your citations — cannot finish."
            ));
            continue;
        };
        let entries = decls.get(name).unwrap_or(&empty);
        if entries.is_empty() {
            let hint = if bare.contains(name) {
                format!(" You wrote a bare `APPLIED-EXPERTISE: {name}` with no rule-ids — that no longer counts.")
            } else {
                String::new()
            };
            reasons.push(format!(
                "You did not credibly declare applying '{name}'.{hint} Re-read expertise/{name}.md, \
                 then emit one line per governing rule you applied: `APPLIED-EXPERTISE: {name}#<rule-id> \
                 — <evidence>` (evidence = a VERBATIM quote of that rule's own text from the manifest)."
            ));
            continue;
        }
        // (a) citation integrity — every cited id must be real
        let bad: Vec<String> = entries
            .iter()
            .filter(|(rid, _)| !all_ids.contains(rid))
            .map(|(rid, _)| rid.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !bad.is_empty() {
            reasons.push(format!(
                "'{name}': these cited rule-ids are not in the manifest (fabricated?): {}. Cite real \
                 ids from expertise/manifests/{name}.json.",
                bad.join(", ")
            ));
        }
        // (b) coverage floor
        let good: HashSet<&String> = entries
            .iter()
            .filter(|(rid, _)| all_ids.contains(rid))
            .map(|(rid, _)| rid)
            .collect();
        let floor = FLOOR.min(if checkable.is_empty() {
            FLOOR
        } else {
            checkable.len()
        });
        if good.len() < floor {
            reasons.push(format!(
                "'{name}': only {} valid rule(s) cited; cite at least {floor} of the governing rules \
                 you actually applied (not a token gesture).",
                good.len()
            ));
        }
        // (c) evidence = a VERBATIM quote of the rule's own text (forces reading; deterministic, no LLM).
        let texts = manifest_rule_texts(manifest_dir, name);
        for (rid, evid) in entries {
            if !all_ids.contains(rid) {
                continue;
            }
            let Some(rule_text) = texts.get(rid) else {
                continue; // tolerant: a rule with no text can't be quote-checked
            };
            if !quote_is_from_rule(evid, rule_text) {
                reasons.push(format!(
                    "'{name}#{rid}': evidence must be a VERBATIM quote (>= {MIN_RULE_QUOTE} chars) from the \
                     rule's text — read expertise/manifests/{name}.json and quote rule {rid}."
                ));
            }
        }
    }
    reasons
}

fn log_decision(
    log: &Path,
    agent_name: &str,
    decision: &str,
    reasons: &[String],
    cited: &BTreeMap<String, Vec<String>>,
    session: &str,
) {
    io::append_log(
        log,
        &json!({
            "ts": io::now_iso(),
            "hook": "validate",
            "agent": agent_name,
            "session": session,
            "decision": decision,
            "reasons": reasons,
            "cited": cited,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offenders_flags_banned_and_budget() {
        let big = format!("CLAUDE.md content\n{}", "x\n".repeat(250));
        let files = vec![
            (
                "a/persona.md".to_string(),
                "uses chain-of-thought here".to_string(),
            ),
            ("CLAUDE.md".to_string(), big),
            ("ok.md".to_string(), "fine".to_string()),
        ];
        let out = offenders(&files);
        assert!(out.iter().any(|r| r.contains("chain-of-thought")));
        assert!(out.iter().any(|r| r.contains("budget")));
    }

    #[test]
    fn parse_decls_extracts_rule_ids_and_bare() {
        // Simulate a transcript JSONL: one assistant text block with declarations.
        let td = tempfile::tempdir().unwrap();
        let tp = td.path().join("t.jsonl");
        let rec = json!({
            "type": "assistant",
            "message": {"content": [{"type":"text","text":
                "APPLIED-EXPERTISE: persona-creation#pc-30 — release-manager/CLAUDE.md\n\
                 APPLIED-EXPERTISE: prompt-engineering"}]}
        });
        std::fs::write(&tp, format!("{rec}\n")).unwrap();
        let (decls, bare) = parse_declarations(tp.to_str().unwrap()).unwrap();
        assert_eq!(decls["persona-creation"][0].0, "pc-30");
        assert_eq!(decls["persona-creation"][0].1, "release-manager/CLAUDE.md");
        assert!(bare.contains("prompt-engineering"));
    }

    #[test]
    fn parse_decls_is_turn_scoped_and_ignores_tool_results() {
        // D4 regression: only declarations AFTER the last genuine human message count, and a
        // trailing tool_result (also `type:"user"`) must NOT be treated as a turn boundary.
        let td = tempfile::tempdir().unwrap();
        let tp = td.path().join("t.jsonl");
        let stale = json!({"type":"assistant","message":{"content":[{"type":"text",
            "text":"APPLIED-EXPERTISE: agent-building#ab-1 — stale/old.md"}]}});
        let human = json!({"type":"user","message":{"role":"user","content":"do the next thing"}});
        let current = json!({"type":"assistant","message":{"content":[{"type":"text",
            "text":"APPLIED-EXPERTISE: expertise-application#ea-3 — fresh/now.md"}]}});
        let tool_result =
            json!({"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}});
        // A Stop-hook-feedback record (type:user, isMeta:true) must NOT count as a turn boundary —
        // otherwise it would push past and drop `current` (which precedes it).
        let stop_feedback = json!({"type":"user","isMeta":true,
            "message":{"role":"user","content":"Stop hook feedback: Cannot finish"}});
        std::fs::write(
            &tp,
            format!("{stale}\n{human}\n{current}\n{tool_result}\n{stop_feedback}\n"),
        )
        .unwrap();
        let (decls, _bare) = parse_declarations(tp.to_str().unwrap()).unwrap();
        assert!(
            decls.contains_key("expertise-application"),
            "current-turn decl kept"
        );
        assert!(
            !decls.contains_key("agent-building"),
            "stale pre-user decl must be dropped (turn-scoping)"
        );
    }

    #[test]
    fn reply_format_flags_overlong_bullets_only() {
        assert!(overlong_bullets("- one two three").is_empty());
        let long = format!("- {}", "word ".repeat(25));
        assert_eq!(overlong_bullets(&long).len(), 1, "25-word bullet flagged");
        let twenty = format!("- {}", vec!["w"; 20].join(" "));
        assert!(overlong_bullets(&twenty).is_empty(), "exactly 20 allowed");
        let fenced = format!("```\n- {}\n```", "word ".repeat(25));
        assert!(overlong_bullets(&fenced).is_empty(), "code fence exempt");
        assert!(
            overlong_bullets(
                "APPLIED-EXPERTISE: a#a-1 — evidence string that keeps going and going and going \
                 and going and going and going and going"
            )
            .is_empty(),
            "APPLIED-EXPERTISE line exempt"
        );
        let trow = format!("| {} |", "cell ".repeat(25));
        assert!(overlong_bullets(&trow).is_empty(), "table row exempt");
    }

    #[test]
    fn format_reasons_scoped_to_the_list() {
        let long = format!("- {}", "word ".repeat(25));
        assert_eq!(format_reasons("genesis-engineer", &long).len(), 1);
        assert!(
            format_reasons("method", &long).is_empty(),
            "agents not on the list are unaffected"
        );
    }

    #[test]
    fn applied_records_builds_one_per_citation_with_evidence() {
        // Feature 2 (verbose-declarations) AC3: on a clean finish, every accepted citation is written
        // to the audit log — displayed or not. This builds those records deterministically.
        let accepted = vec![
            (
                "expertise-application".to_string(),
                "ea-3".to_string(),
                "foo.md".to_string(),
            ),
            (
                "test-driven-determinism".to_string(),
                "tdd-1".to_string(),
                "bar.rs".to_string(),
            ),
        ];
        let recs = applied_records("method", "sess-1", &accepted);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0]["agent"], "method");
        assert_eq!(recs[0]["session"], "sess-1");
        assert_eq!(recs[0]["name"], "expertise-application");
        assert_eq!(recs[0]["rule_id"], "ea-3");
        assert_eq!(recs[0]["evidence"], "foo.md");
        assert!(
            recs[0].get("ts").is_some(),
            "each record carries a timestamp"
        );
    }

    #[test]
    fn parse_decls_reads_record_channel_tool_use() {
        // Feature 2 (verbose-declarations): a QUIET agent records its declarations by writing them to
        // `.genesis/applied-expertise.jsonl` via a Write tool call instead of printing prose. The
        // validator must extract APPLIED-EXPERTISE from that current-turn tool_use input, so the SAME
        // enforcement holds without the declarations ever appearing in visible text.
        let td = tempfile::tempdir().unwrap();
        let tp = td.path().join("t.jsonl");
        let human = json!({"type":"user","message":{"role":"user","content":"do the work"}});
        // No text block carries the declaration — only the Write tool_use to the record file does.
        let record = json!({"type":"assistant","message":{"content":[
            {"type":"text","text":"Done — recorded my declarations."},
            {"type":"tool_use","name":"Write","input":{
                "file_path":"/proj/.genesis/applied-expertise.jsonl",
                "content":"APPLIED-EXPERTISE: expertise-application#ea-3 — .genesis/expertise/expertise-application.md"
            }}
        ]}});
        std::fs::write(&tp, format!("{human}\n{record}\n")).unwrap();
        let (decls, _bare) = parse_declarations(tp.to_str().unwrap()).unwrap();
        assert_eq!(
            decls.get("expertise-application").map(|e| e[0].0.as_str()),
            Some("ea-3"),
            "declaration recorded via the applied-expertise.jsonl write channel must be parsed"
        );
    }

    #[test]
    fn missing_transcript_is_empty_not_none() {
        let (decls, bare) = parse_declarations("").unwrap();
        assert!(decls.is_empty() && bare.is_empty());
        let (d2, _) = parse_declarations("/no/such/file.jsonl").unwrap();
        assert!(d2.is_empty());
    }

    // ---- verbatim rule-quote evidence (spec: test/specs/verbatim-rule-quote-evidence.md) ----------

    #[test]
    fn quote_is_from_rule_matches_only_real_substrings() {
        let rule = "always read every relevant file fully before acting";
        assert!(
            quote_is_from_rule("read every relevant file fully", rule),
            "a real >=20-char substring passes"
        );
        assert!(
            quote_is_from_rule("`read every relevant file fully`", rule),
            "surrounding backticks are ignored"
        );
        assert!(
            !quote_is_from_rule("this snippet is nowhere in that rule", rule),
            "text not in the rule is rejected"
        );
        assert!(
            !quote_is_from_rule("read", rule),
            "too-short snippet rejected"
        );
        assert!(
            !quote_is_from_rule("/x/expertise/foo.md", rule),
            "a file path is rejected (fixes the trivial-pass gap)"
        );
    }

    #[test]
    fn manifest_rule_texts_maps_id_to_text() {
        let td = tempfile::tempdir().unwrap();
        let mdir = td.path().join("manifests");
        std::fs::create_dir_all(&mdir).unwrap();
        std::fs::write(
            mdir.join("foo.json"),
            r#"{"rules":[{"id":"foo-1","text":"quote me exactly from the rule text"},{"id":"foo-2","text":"another rule statement"}]}"#,
        )
        .unwrap();
        let m = manifest_rule_texts(Some(&mdir), "foo");
        assert_eq!(
            m.get("foo-1").map(String::as_str),
            Some("quote me exactly from the rule text")
        );
        assert_eq!(
            m.get("foo-2").map(String::as_str),
            Some("another rule statement")
        );
    }
}
