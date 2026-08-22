//! CAPTURE-SESSION hook (Stop / SubagentStop) — copy the live transcript into the repo so it travels.
//!
//! Part of cross-system resume (issue #9). Reads the event's `transcript_path` and copies that `.jsonl`
//! into `<repo>/.genesis/sessions/`, committed later by `/genesis:sync` so the conversation travels with
//! the repo. Self-guards (dormant unless a genesis agent is active) and fail-open (a copy error never
//! breaks a session). Emits no decision.
//!
//! argv: `[--main-agent <name>]`.

use crate::{agent, cli, io, session_transfer};
use serde_json::Value;

/// Entry point for `genesis-hook capture-session [--main-agent <name>]`.
pub fn run(args: &[String]) {
    let (main_agent, _rest) = cli::take_option(args, "--main-agent");
    let ev = io::parse_event(&io::read_stdin());

    // DORMANCY GUARD: no-op unless a genesis agent is active.
    if agent::resolve_agent(&ev, "", main_agent.as_deref().unwrap_or("")).is_empty() {
        std::process::exit(0);
    }

    let transcript = ev
        .get("transcript_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    let cwd = std::env::current_dir().unwrap_or_default();
    let _ = session_transfer::capture(&cwd, transcript); // fail-open: ignore any error
    std::process::exit(0);
}
