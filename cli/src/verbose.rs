//! `genesis-cli verbose <on|off> <agent> [target_repo]` — flip the per-agent VERBOSE flag that decides
//! whether that agent's APPLIED-EXPERTISE declarations are DISPLAYED in prose (on) or recorded quietly
//! to `.genesis/applied-expertise.jsonl` (off, the default). Feature 2 — verbose-declarations.
//!
//! The flag is the file `<repo>/.genesis/verbose/<agent>.json`: `on` writes `{"verbose":true}`, `off`
//! removes it. Both are idempotent. Enforcement/logging are unaffected — only the DISPLAY changes.

use crate::fsx;
use serde_json::json;
use std::path::PathBuf;

/// Entry point for `genesis-cli verbose <on|off> <agent> [target_repo]`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let mode = args.first().map_or("", String::as_str);
    let agent = args.get(1).map_or("", String::as_str);
    if agent.is_empty() || !matches!(mode, "on" | "off") {
        fsx::fail("usage: genesis-cli verbose <on|off> <agent> [target_repo]");
    }
    let target = args.get(2).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let flag = target
        .join(".genesis")
        .join("verbose")
        .join(format!("{agent}.json"));

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

#[cfg(test)]
mod tests {
    use super::*;

    fn a(mode: &str, agent: &str, target: &str) -> Vec<String> {
        vec![mode.to_string(), agent.to_string(), target.to_string()]
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
}
