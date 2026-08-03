//! Resolve the ACTIVE AGENT + the PROJECT runtime dir for a hook.
//!
//! Faithful port of `hooks/agent_ident.js`. Agent identity derives from the hook PAYLOAD's
//! `agent_type`, with the same resolution precedence: explicit argv positional > payload
//! `agent_type` (scope-normalized) > `--main-agent <name>` fallback.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Remove leading & trailing characters that are in `chars` (emulates Python `str.strip(chars)`).
fn strip_charset<'a>(s: &'a str, chars: &str) -> &'a str {
    let s = s.trim_start_matches(|c| chars.contains(c));
    s.trim_end_matches(|c| chars.contains(c))
}

/// `genesis:method` / `^genesis:method$` / `method` -> `method`; `""` -> `""`.
#[must_use]
pub fn normalize(agent_type: &str) -> String {
    if agent_type.is_empty() {
        return String::new();
    }
    let a = agent_type.trim();
    let a = strip_charset(a, "^$").trim();
    // plugin-scoped "<plugin>:<name>" -> bare "<name>"
    let a = a.rfind(':').map_or(a, |idx| &a[idx + 1..]);
    a.trim().to_string()
}

/// The active agent name from a hook payload, or `""` if none. Reads `agent_type`
/// (camelCase `agentType` tolerated defensively), scope-normalized.
#[must_use]
pub fn agent_from_payload(ev: &Value) -> String {
    if !ev.is_object() {
        return String::new();
    }
    let at = ev
        .get("agent_type")
        .and_then(Value::as_str)
        .or_else(|| ev.get("agentType").and_then(Value::as_str))
        .unwrap_or("");
    normalize(at)
}

/// Active agent, precedence: explicit argv > payload `agent_type` > `--main-agent` fallback.
#[must_use]
pub fn resolve_agent(ev: &Value, argv_agent: &str, default_agent: &str) -> String {
    if !argv_agent.is_empty() {
        return argv_agent.to_string();
    }
    let a = agent_from_payload(ev);
    if !a.is_empty() {
        return a;
    }
    default_agent.to_string()
}

/// Parse a Stop/SubagentStop hook's argv into `(root, agent, main_agent)`.
///
/// Tolerates the historical positional form `<root> <agent>` MIXED with a `--main-agent <name>`
/// flag. `root` is `None` when no positional root was given.
#[must_use]
pub fn split_args(argv: &[String]) -> (Option<String>, String, String) {
    let mut main_agent = String::new();
    let mut pos: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        if argv[i] == "--main-agent" && i + 1 < argv.len() {
            main_agent.clone_from(&argv[i + 1]);
            i += 2;
            continue;
        }
        pos.push(&argv[i]);
        i += 1;
    }
    let root = pos.first().map(|s| (*s).clone());
    let agent = pos.get(1).map_or_else(String::new, |s| (*s).clone());
    (root, agent, main_agent)
}

/// The PROJECT's writable `.genesis/` runtime dir, derived from `cwd`. NEVER the plugin dir.
/// Guards against a nested `.genesis/.genesis` if `cwd` already IS a workspace runtime dir.
#[must_use]
pub fn runtime_dir(cwd: &Path) -> PathBuf {
    let s = cwd.to_string_lossy();
    let trimmed = s.trim_end_matches(['/', '\\']);
    let base = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    if base == ".genesis" {
        return cwd.to_path_buf();
    }
    cwd.join(".genesis")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_scopes_and_anchors() {
        assert_eq!(normalize("genesis:method"), "method");
        assert_eq!(normalize("^genesis:method$"), "method");
        assert_eq!(normalize("method"), "method");
        assert_eq!(normalize("  ^sensei$  "), "sensei");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn payload_reads_both_cases() {
        assert_eq!(
            agent_from_payload(&json!({"agent_type": "genesis:sensei"})),
            "sensei"
        );
        assert_eq!(
            agent_from_payload(&json!({"agentType": "method"})),
            "method"
        );
        assert_eq!(agent_from_payload(&json!({"foo": 1})), "");
        assert_eq!(agent_from_payload(&json!([1, 2])), "");
    }

    #[test]
    fn resolve_precedence() {
        let ev = json!({"agent_type": "method"});
        assert_eq!(resolve_agent(&ev, "sensei", "main"), "sensei"); // argv wins
        assert_eq!(resolve_agent(&ev, "", "main"), "method"); // payload
        assert_eq!(resolve_agent(&json!({}), "", "main"), "main"); // fallback
        assert_eq!(resolve_agent(&json!({}), "", ""), "");
    }

    #[test]
    fn split_args_mixed() {
        let a: Vec<String> = ["/root", "method", "--main-agent", "sensei"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let (root, agent, main) = split_args(&a);
        assert_eq!(root.as_deref(), Some("/root"));
        assert_eq!(agent, "method");
        assert_eq!(main, "sensei");

        let (root2, agent2, main2) = split_args(&[]);
        assert_eq!(root2, None);
        assert_eq!(agent2, "");
        assert_eq!(main2, "");
    }

    #[test]
    fn runtime_dir_guards_nesting() {
        assert_eq!(
            runtime_dir(Path::new("/proj")),
            PathBuf::from("/proj/.genesis")
        );
        assert_eq!(
            runtime_dir(Path::new("/proj/.genesis")),
            PathBuf::from("/proj/.genesis")
        );
        assert_eq!(
            runtime_dir(Path::new("/proj/.genesis/")),
            PathBuf::from("/proj/.genesis/")
        );
    }
}
