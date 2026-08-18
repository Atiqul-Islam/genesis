//! VALIDATE hook (Stop / SubagentStop) — the loop-closer.
//!
//! Faithful port of `hooks/validate.js`. Refuses to FINISH while a checkable rule is still violated,
//! OR while the agent never *credibly* declared it applied its required expertise. Three enforcement
//! layers: (1) checkable-rule offenders in produced files; (2) evidence-carrying declaration
//! integrity (real rule-ids, coverage floor, evidence spot-check); (3) artifact spot-check.
//! Fail-closed; guards an infinite block loop via `stop_hook_active`. Self-guards on the active agent.

use crate::{agent, cli, glob, io};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

const LINE_BUDGET: usize = 200;
const FLOOR: usize = 3; // min distinct real rule-ids per required expertise (or all checkable, if fewer)

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

    let mut cited: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let required = required_for(required_json.as_deref(), &active);
    if !required.is_empty() {
        let transcript = ev
            .get("transcript_path")
            .and_then(Value::as_str)
            .unwrap_or("");
        match parse_declarations(transcript) {
            None => reasons.push(
                "Could not read the transcript to verify the expertise declaration — cannot finish."
                    .to_string(),
            ),
            Some((decls, bare)) => {
                for name in &required {
                    if let Some((all_ids, _)) = load_manifest(manifest_dir.as_deref(), name) {
                        let ids: Vec<String> = decls
                            .get(name)
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
                    &files,
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
    log_decision(&log, &active, "allow", &[], &cited, session);
    std::process::exit(0);
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
    let mut start = 0usize;
    for (i, raw) in lines.iter().enumerate() {
        let Ok(ev) = serde_json::from_str::<Value>(raw.trim_end_matches('\r')) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        // Exclude system-injected user records — Stop-hook feedback, system reminders — which are
        // `type:"user"` with `isMeta:true`. A genuine human prompt has no `isMeta` (and carries
        // `promptSource`). Without this, every "Stop hook feedback:" block became a false turn boundary
        // that pushed past — and dropped — the very declarations being validated (self-defeating loop).
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

    for raw in &lines[start..] {
        let line = raw.trim_end_matches('\r');
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if ev.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        for t in assistant_texts(&ev) {
            for caps in decl.captures_iter(&t) {
                let name = caps.get(1).map_or("", |m| m.as_str()).to_lowercase();
                let rid = caps.get(2).map_or("", |m| m.as_str()).to_lowercase();
                let evid = caps.get(3).map_or("", |m| m.as_str()).trim().to_string();
                decls.entry(name).or_default().push((rid, evid));
            }
            for caps in bare_re.captures_iter(&t) {
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

fn quote_norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// Spot-check one evidence string. Returns a violation string if verifiably fabricated, else `None`.
fn check_evidence(
    evid: &str,
    keys: &[String],
    blob_norm: &str,
    pathish: &Regex,
    quoted: &Regex,
) -> Option<String> {
    if let Some(m) = pathish.find(evid) {
        let tok = m.as_str().replace('\\', "/");
        let base = basename(&tok);
        let ok = Path::new(&tok).is_file()
            || keys.iter().any(|k| {
                let kk = k.replace('\\', "/");
                kk.ends_with(&tok) || basename(&kk) == base
            });
        if !ok {
            return Some(format!(
                "evidence points to \"{tok}\" but no such produced file exists"
            ));
        }
        return None;
    }
    if let Some(caps) = quoted.captures(evid) {
        let g = (1..=3)
            .filter_map(|i| caps.get(i))
            .map(|m| m.as_str())
            .find(|s| !s.is_empty());
        if let Some(g) = g {
            let q = quote_norm(g);
            if !q.is_empty() && !blob_norm.contains(&q) {
                let clip: String = q.chars().take(40).collect();
                return Some(format!(
                    "quoted evidence \"{clip}…\" does not appear in any produced artifact"
                ));
            }
        }
    }
    None
}

fn verify_declaration(
    manifest_dir: Option<&Path>,
    required: &[String],
    decls: &HashMap<String, Vec<(String, String)>>,
    bare: &HashSet<String>,
    files: &[(String, String)],
) -> Vec<String> {
    let pathish =
        Regex::new(r"(?i)[A-Za-z0-9_./\\-]+\.(?:md|jsonc?|toml|ya?ml|txt|py|js|ts|rs|cfg|ini)");
    let quoted = Regex::new(r#""([^"]{8,})"|`([^`]{8,})`|'([^']{8,})'"#);
    let keys: Vec<String> = files.iter().map(|(k, _)| k.clone()).collect();
    let blob_norm = quote_norm(
        &files
            .iter()
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
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
                 — <evidence>` (evidence = the file it's embodied in, or a short quote from it)."
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
        // (c) evidence spot-check
        if let (Ok(pathish), Ok(quoted)) = (&pathish, &quoted) {
            for (rid, evid) in entries {
                if !all_ids.contains(rid) {
                    continue;
                }
                if let Some(v) = check_evidence(evid, &keys, &blob_norm, pathish, quoted) {
                    reasons.push(format!("'{name}#{rid}': {v}."));
                }
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
    fn missing_transcript_is_empty_not_none() {
        let (decls, bare) = parse_declarations("").unwrap();
        assert!(decls.is_empty() && bare.is_empty());
        let (d2, _) = parse_declarations("/no/such/file.jsonl").unwrap();
        assert!(d2.is_empty());
    }

    #[test]
    fn check_evidence_detects_fabricated_path_and_quote() {
        let pathish =
            Regex::new(r"(?i)[A-Za-z0-9_./\\-]+\.(?:md|jsonc?|toml|ya?ml|txt|py|js|ts|rs|cfg|ini)")
                .unwrap();
        let quoted = Regex::new(r#""([^"]{8,})"|`([^`]{8,})`|'([^']{8,})'"#).unwrap();
        let keys = vec!["release-manager/CLAUDE.md".to_string()];
        let blob = quote_norm("the persona says be terse and skeptical always");
        // real path -> ok
        assert!(
            check_evidence("release-manager/CLAUDE.md", &keys, &blob, &pathish, &quoted).is_none()
        );
        // fabricated path -> violation
        assert!(check_evidence("ghost/missing.md", &keys, &blob, &pathish, &quoted).is_some());
        // present quote -> ok
        assert!(check_evidence(
            r#""be terse and skeptical""#,
            &keys,
            &blob,
            &pathish,
            &quoted
        )
        .is_none());
        // absent quote -> violation
        assert!(
            check_evidence(r#""this text is nowhere""#, &keys, &blob, &pathish, &quoted).is_some()
        );
    }
}
