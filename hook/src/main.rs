//! Genesis hook binary — busybox-style multi-call dispatch.
//!
//! `genesis-hook <subcommand> [args...]` reads the Claude Code event JSON on stdin and emits a
//! decision on stdout. An unknown subcommand is a fail-open no-op (exit 0), matching the Node
//! hooks' "not our event -> allow" posture.

fn main() {
    let sub = std::env::args().nth(1).unwrap_or_default();
    let rest: Vec<String> = std::env::args().skip(2).collect();
    match sub.as_str() {
        "gate" => genesis_hook::gate::run(&rest),
        "enforce-research" => genesis_hook::enforce_research::run(&rest),
        "inject" => genesis_hook::inject::run(&rest),
        "precompact" => genesis_hook::precompact::run(&rest),
        "capture-session" => genesis_hook::capture_session::run(&rest),
        "validate" => genesis_hook::validate::run(&rest),
        "session-pointer" => genesis_hook::session_pointer::run(&rest),
        "promote-offer" => genesis_hook::promote_offer::run(&rest),
        // Unknown subcommand -> fail-open no-op (matches the Node hooks' "not our event -> allow").
        _ => std::process::exit(0),
    }
}
