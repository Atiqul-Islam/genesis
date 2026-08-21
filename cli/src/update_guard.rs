//! `genesis-cli update-guard <agent> <source.json> [target_repo]` — the coordinator flow that creates
//! or upgrades ANY agent's guard (Feature 1 — agent-scoped-guards). Reads a candidate guard from
//! `source.json`, VALIDATES it, and only then writes `<repo>/.genesis/team/<agent>/guard.json`. A
//! malformed candidate is rejected (exit 2), never written — so the gate never loads a broken guard.
//!
//! The guard schema is the existing one: `{ "self_protect": [paths], "invariants": [ {id, files,
//! must_match?, must_not_match?, why} ] }`.

use crate::fsx;
use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Entry point for `genesis-cli update-guard <agent> <source.json> [target_repo]`. Returns the exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let agent = args.first().map_or("", String::as_str);
    let source = args.get(1).map_or("", String::as_str);
    if agent.is_empty() || source.is_empty() {
        fsx::fail("usage: genesis-cli update-guard <agent> <source.json> [target_repo]");
    }
    let target = args.get(2).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );

    let Some(candidate) = fsx::read_json(std::path::Path::new(source)) else {
        eprintln!("update-guard: {source} is not readable JSON — nothing written");
        return 2;
    };
    if let Err(reason) = validate_guard(&candidate) {
        eprintln!("update-guard: rejected — {reason}. Nothing written.");
        return 2;
    }
    let dest = target
        .join(".genesis")
        .join("team")
        .join(agent)
        .join("guard.json");
    if let Err(e) = fsx::write_text(&dest, &fsx::json_pretty(&candidate)) {
        fsx::fail(&format!("could not write {}: {e}", dest.display()));
    }
    println!(
        "{}",
        fsx::json_pretty(&json!({
            "agent": agent,
            "guard": dest.to_string_lossy(),
            "invariants": candidate.get("invariants").and_then(Value::as_array).map_or(0, Vec::len),
            "note": "guard validated and installed for this agent",
        }))
    );
    0
}

/// Validate a candidate guard. `Ok(())` if well-formed; `Err(reason)` otherwise (AC6): a top-level
/// object with a `self_protect` array and an `invariants` array, where every invariant carries a
/// non-empty `id`, a `files` array, at least one of `must_match`/`must_not_match`, and every provided
/// pattern is a compilable regex.
fn validate_guard(v: &Value) -> Result<(), String> {
    if !v.is_object() {
        return Err("top level must be a JSON object".to_string());
    }
    if v.get("self_protect").and_then(Value::as_array).is_none() {
        return Err("missing a `self_protect` array (may be empty)".to_string());
    }
    let Some(invs) = v.get("invariants").and_then(Value::as_array) else {
        return Err("missing an `invariants` array".to_string());
    };
    for (i, inv) in invs.iter().enumerate() {
        let id = inv.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            return Err(format!("invariant #{i} has no `id`"));
        }
        if inv.get("files").and_then(Value::as_array).is_none() {
            return Err(format!("invariant {id} has no `files` array"));
        }
        let mm = inv.get("must_match").and_then(Value::as_str);
        let mn = inv.get("must_not_match").and_then(Value::as_str);
        if mm.filter(|s| !s.is_empty()).is_none() && mn.filter(|s| !s.is_empty()).is_none() {
            return Err(format!(
                "invariant {id} needs at least one of `must_match` / `must_not_match`"
            ));
        }
        for pat in [mm, mn].into_iter().flatten() {
            if !pat.is_empty() && Regex::new(pat).is_err() {
                return Err(format!("invariant {id} has an uncompilable regex: /{pat}/"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Value {
        json!({
            "self_protect": [".genesis/team/atlas/guard.json"],
            "invariants": [{"id":"c1","files":["persona.md"],"must_match":"per-action approval","why":"keep it"}]
        })
    }

    #[test]
    fn accepts_a_well_formed_guard() {
        assert!(validate_guard(&valid()).is_ok());
    }

    #[test]
    fn rejects_missing_self_protect() {
        assert!(validate_guard(&json!({"invariants":[]})).is_err());
    }

    #[test]
    fn rejects_invariant_without_id_or_pattern() {
        assert!(
            validate_guard(
                &json!({"self_protect":[],"invariants":[{"files":["p.md"],"must_match":"x"}]})
            )
            .is_err(),
            "an invariant with no id is rejected"
        );
        assert!(
            validate_guard(
                &json!({"self_protect":[],"invariants":[{"id":"c1","files":["p.md"],"why":"y"}]})
            )
            .is_err(),
            "an invariant with neither must_match nor must_not_match is rejected"
        );
    }

    #[test]
    fn rejects_uncompilable_regex() {
        assert!(validate_guard(
            &json!({"self_protect":[],"invariants":[{"id":"c1","files":["p.md"],"must_match":"("}]})
        )
        .is_err());
    }

    #[test]
    fn writes_only_when_valid_and_is_idempotent() {
        let td = tempfile::tempdir().unwrap();
        let target = td.path().to_string_lossy().into_owned();
        let src = td.path().join("candidate.json");
        std::fs::write(&src, valid().to_string()).unwrap();
        let dest = td.path().join(".genesis/team/atlas/guard.json");

        let args = vec![
            "atlas".to_string(),
            src.to_string_lossy().into_owned(),
            target,
        ];
        assert_eq!(run(&args), 0);
        assert!(dest.is_file(), "a valid guard is written");
        let first = std::fs::read(&dest).unwrap();
        assert_eq!(run(&args), 0);
        assert_eq!(
            std::fs::read(&dest).unwrap(),
            first,
            "re-running with the same source is byte-identical (idempotent)"
        );

        // A malformed candidate is rejected and never written.
        let bad = td.path().join("bad.json");
        std::fs::write(&bad, json!({"invariants":[]}).to_string()).unwrap();
        let dest2 = td.path().join(".genesis/team/ghost/guard.json");
        let bad_args = vec![
            "ghost".to_string(),
            bad.to_string_lossy().into_owned(),
            td.path().to_string_lossy().into_owned(),
        ];
        assert_eq!(run(&bad_args), 2, "malformed guard is rejected with exit 2");
        assert!(!dest2.exists(), "a rejected guard is never written");
    }
}
