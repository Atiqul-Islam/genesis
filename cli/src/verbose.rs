//! `genesis-cli verbose <on|off|status> [<agent>] [target_repo]` — the per-agent VERBOSE flag that decides
//! whether that agent's APPLIED-EXPERTISE declarations are DISPLAYED in prose (on) or recorded quietly to
//! `.genesis/applied-expertise.jsonl` (off, the default). Feature 2 — verbose-declarations; #5 adds `status`.
//!
//! The flag is the file `<repo>/.genesis/verbose/<agent>.json`: `on` writes `{"verbose":true}`, `off`
//! removes it, `status` reports it (single agent, or all agents with a flag). Enforcement/logging are
//! unaffected — only DISPLAY changes.

use crate::fsx;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Entry point for `genesis-cli verbose <on|off|status> [<agent>] [target_repo]`. Returns the exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let mode = args.first().map_or("", String::as_str);
    let agent = args.get(1).map_or("", String::as_str);
    if !matches!(mode, "on" | "off" | "status") || (agent.is_empty() && mode != "status") {
        fsx::fail("usage: genesis-cli verbose <on|off|status> [<agent>] [target_repo]");
    }
    let target = args.get(2).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );

    if mode == "status" {
        // `status <agent> [target]` reports one agent; `status --all [target]` (or `status` alone) lists all.
        let list_all = agent.is_empty() || agent == "--all";
        return status(&target, if list_all { "" } else { agent });
    }

    let flag = flag_path(&target, agent);
    let (state, note) = if mode == "on" {
        if let Err(e) = fsx::write_text(&flag, &fsx::json_pretty(&json!({ "verbose": true }))) {
            fsx::fail(&format!("could not write {}: {e}", flag.display()));
        }
        (
            "on",
            "declarations for this agent are now DISPLAYED in its replies",
        )
    } else {
        // off = remove the flag; absence is the default. Missing file is not an error (idempotent).
        if flag.exists() {
            if let Err(e) = std::fs::remove_file(&flag) {
                fsx::fail(&format!("could not remove {}: {e}", flag.display()));
            }
        }
        (
            "off",
            "declarations for this agent are now recorded quietly, not displayed",
        )
    };

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "agent": agent,
            "verbose": state,
            "flag": flag.to_string_lossy(),
            "note": note,
        }))
    );
    0
}

/// The per-agent flag file path.
fn flag_path(target: &Path, agent: &str) -> PathBuf {
    target
        .join(".genesis")
        .join("verbose")
        .join(format!("{agent}.json"))
}

/// Whether the per-agent verbose flag is ON (file present with `{"verbose":true}`). Absent/false = off.
#[must_use]
pub fn is_verbose(target: &Path, agent: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(flag_path(target, agent)) else {
        return false;
    };
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v.get("verbose").and_then(Value::as_bool))
        == Some(true)
}

/// `status`: report one agent's on/off, or (no agent) list every agent that has a flag file. Read-only.
fn status(target: &Path, agent: &str) -> i32 {
    if !agent.is_empty() {
        let on = is_verbose(target, agent);
        println!(
            "{}",
            fsx::json_pretty(&json!({
                "agent": agent,
                "verbose": if on { "on" } else { "off" },
                "flag": flag_path(target, agent).to_string_lossy(),
            }))
        );
        return 0;
    }
    // No agent: enumerate every `<agent>.json` under `.genesis/verbose/`.
    let dir = target.join(".genesis").join("verbose");
    let mut agents: Vec<(String, bool)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Some(name) = p.file_stem().and_then(|s| s.to_str()) {
                agents.push((name.to_string(), is_verbose(target, name)));
            }
        }
    }
    agents.sort();
    let list: Vec<Value> = agents
        .iter()
        .map(|(n, on)| json!({ "agent": n, "verbose": if *on { "on" } else { "off" } }))
        .collect();
    println!(
        "{}",
        fsx::json_pretty(&json!({
            "verbose_dir": dir.to_string_lossy(),
            "agents": list,
            "note": "agents with no flag file are verbose OFF (the default)",
        }))
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(mode: &str, agent: &str, target: &str) -> Vec<String> {
        let mut v = vec![mode.to_string()];
        if !agent.is_empty() {
            v.push(agent.to_string());
        }
        v.push(target.to_string());
        v
    }

    #[test]
    fn on_then_off_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        let target = td.path().to_string_lossy().into_owned();
        let flag = td.path().join(".genesis/verbose/method.json");

        assert_eq!(run(&a("on", "method", &target)), 0);
        assert!(flag.is_file(), "on creates the flag");
        let first = std::fs::read(&flag).unwrap();
        assert_eq!(run(&a("on", "method", &target)), 0);
        assert_eq!(
            std::fs::read(&flag).unwrap(),
            first,
            "a second `on` is byte-identical (idempotent)"
        );

        assert_eq!(run(&a("off", "method", &target)), 0);
        assert!(!flag.exists(), "off removes the flag");
        assert_eq!(
            run(&a("off", "method", &target)),
            0,
            "a second `off` is a no-op, not an error (idempotent)"
        );
    }

    #[test]
    fn flag_is_per_agent() {
        let td = tempfile::tempdir().unwrap();
        let target = td.path().to_string_lossy().into_owned();
        assert_eq!(run(&a("on", "sensei", &target)), 0);
        assert!(td.path().join(".genesis/verbose/sensei.json").is_file());
        assert!(
            !td.path().join(".genesis/verbose/method.json").exists(),
            "turning verbose on for one agent never touches another"
        );
    }

    #[test]
    fn status_reports_single_agent_on_off() {
        let td = tempfile::tempdir().unwrap();
        let target = td.path();
        assert!(!is_verbose(target, "method"), "absent flag => off");
        std::fs::create_dir_all(td.path().join(".genesis/verbose")).unwrap();
        std::fs::write(
            td.path().join(".genesis/verbose/method.json"),
            r#"{"verbose":true}"#,
        )
        .unwrap();
        assert!(is_verbose(target, "method"), "present flag => on");
        // status runs cleanly for both states
        let t = target.to_string_lossy().into_owned();
        assert_eq!(run(&a("status", "method", &t)), 0);
        assert_eq!(run(&a("status", "sensei", &t)), 0);
    }

    #[test]
    fn status_no_agent_lists_all() {
        let td = tempfile::tempdir().unwrap();
        let vdir = td.path().join(".genesis/verbose");
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(vdir.join("a.json"), r#"{"verbose":true}"#).unwrap();
        std::fs::write(vdir.join("b.json"), r#"{"verbose":false}"#).unwrap();
        // list-all status form is valid and exits 0
        let t = td.path().to_string_lossy().into_owned();
        assert_eq!(run(&a("status", "--all", &t)), 0);
    }
}
