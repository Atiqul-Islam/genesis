//! End-to-end tests for the `genesis-hook` binary: spawn it with a real event JSON on stdin and
//! assert the decision JSON on stdout. Exercises every subcommand's decision branches against the
//! repo's real expertise manifests (`../expertise`). This is the committed replacement for the
//! Rust<->Node parity harness (the Node hooks it compared against were removed).
#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only: a panic IS the failure signal here

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

const EXP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../expertise");

/// Run the binary with `args` and `stdin`, returning trimmed stdout.
fn run(args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_genesis-hook"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn genesis-hook");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn parse(out: &str) -> Value {
    serde_json::from_str(out).expect("stdout is JSON")
}

// ---- gate ----

#[test]
fn gate_denies_banned_phrase() {
    let ev = json!({"agent_type":"method","tool_input":{"file_path":"a/persona.md","content":"uses chain-of-thought here"}});
    let v = parse(&run(&["gate", "--expertise", EXP], &ev.to_string()));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
}

#[test]
fn gate_denies_credential() {
    let ev = json!({"agent_type":"method","tool_input":{"file_path":"a/notes.md","content":"password = supersecretvalue"}});
    let v = parse(&run(&["gate", "--expertise", EXP], &ev.to_string()));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn gate_denies_over_budget() {
    let content = format!("CLAUDE.md\n{}", "line\n".repeat(210));
    let ev =
        json!({"agent_type":"method","tool_input":{"file_path":"CLAUDE.md","content":content}});
    let v = parse(&run(&["gate", "--expertise", EXP], &ev.to_string()));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn gate_surfaces_rules_for_authoring_write() {
    let long = format!(
        "You are a release manager. {}",
        "Be terse and precise. ".repeat(15)
    );
    let ev = json!({"agent_type":"method","tool_input":{"file_path":"worker/persona.md","content":long}});
    let v = parse(&run(&["gate", "--expertise", EXP], &ev.to_string()));
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("");
    assert!(
        ctx.contains("persona-creation"),
        "expected surfaced rules, got: {ctx}"
    );
    assert!(v["hookSpecificOutput"].get("permissionDecision").is_none()); // advisory, not a block
}

#[test]
fn gate_is_dormant_without_a_genesis_agent() {
    let ev =
        json!({"tool_input":{"file_path":"a/persona.md","content":"uses chain-of-thought here"}});
    assert_eq!(run(&["gate", "--expertise", EXP], &ev.to_string()), ""); // no agent_type -> silent no-op
}

#[test]
fn gate_fires_for_promoted_main_via_main_agent() {
    // A promoted main thread carries NO payload agent_type; --main-agent makes the gate treat it as that agent.
    let ev =
        json!({"tool_input":{"file_path":"a/persona.md","content":"uses chain-of-thought here"}});
    let v = parse(&run(
        &["gate", "--expertise", EXP, "--main-agent", "method"],
        &ev.to_string(),
    ));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

// ---- enforce-research ----

#[test]
fn enforce_allows_non_assembler_bash() {
    let ev = json!({"agent_type":"sensei","tool_input":{"command":"ls -la"}});
    assert_eq!(run(&["enforce-research"], &ev.to_string()), ""); // allow, silent
}

#[test]
fn enforce_allows_builtin_assemble() {
    let ev = json!({"agent_type":"sensei","tool_input":{"command":"node install/assemble.js src method /r /g"}});
    assert_eq!(run(&["enforce-research"], &ev.to_string()), ""); // builtin-exempt
}

#[test]
fn enforce_denies_nonbuiltin_without_research_skill() {
    let ev = json!({"agent_type":"sensei","tool_input":{"command":"node install/assemble.js src mybot /r /g"},"transcript_path":""});
    let v = parse(&run(&["enforce-research"], &ev.to_string()));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn enforce_fires_for_promoted_main_via_main_agent() {
    // no payload agent_type, but --main-agent set -> enforce still evaluates the assembler command
    let ev = json!({"tool_input":{"command":"node install/assemble.js src mybot /r /g"},"transcript_path":""});
    let v = parse(&run(
        &["enforce-research", "--main-agent", "sensei"],
        &ev.to_string(),
    ));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

// ---- validate ----

#[test]
fn validate_blocks_on_offender_artifact() {
    let td = tempfile::tempdir().unwrap();
    std::fs::write(
        td.path().join("worker.persona.md"),
        "this has chain-of-thought inside",
    )
    .unwrap();
    // agent 'qa-bot' has no required expertise -> only the content-offender layer runs
    let ev = json!({"agent_type":"qa-bot","transcript_path":"","session_id":"s"});
    let root = td.path().to_str().unwrap();
    let v = parse(&run(
        &["validate", root, "qa-bot", "--expertise", EXP],
        &ev.to_string(),
    ));
    assert_eq!(v["decision"], "block");
    assert!(v["reason"]
        .as_str()
        .unwrap_or("")
        .contains("chain-of-thought"));
}

#[test]
fn validate_allows_clean_unrequired_agent() {
    let td = tempfile::tempdir().unwrap();
    std::fs::write(
        td.path().join("worker.persona.md"),
        "a perfectly clean persona",
    )
    .unwrap();
    let ev = json!({"agent_type":"qa-bot","transcript_path":"","session_id":"s"});
    let root = td.path().to_str().unwrap();
    assert_eq!(
        run(
            &["validate", root, "qa-bot", "--expertise", EXP],
            &ev.to_string()
        ),
        ""
    );
}

#[test]
fn validate_blocks_when_required_expertise_undeclared() {
    // 'method' HAS required expertise in ../expertise/required.json; with no declarations it must block.
    let td = tempfile::tempdir().unwrap();
    std::fs::write(td.path().join("worker.persona.md"), "clean persona").unwrap();
    let ev = json!({"agent_type":"method","transcript_path":"","session_id":"s"});
    let root = td.path().to_str().unwrap();
    let v = parse(&run(
        &["validate", root, "method", "--expertise", EXP],
        &ev.to_string(),
    ));
    assert_eq!(v["decision"], "block");
    assert!(v["reason"]
        .as_str()
        .unwrap_or("")
        .contains("APPLIED-EXPERTISE"));
}

// ---- inject ----

#[test]
fn inject_delivers_rules_and_required_expertise() {
    let v = parse(&run(&["inject", EXP, "method"], "{}"));
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("");
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(ctx.contains("Genesis house rules"));
    assert!(ctx.contains("persona-creation")); // method's required expertise
}

// ---- unknown subcommand ----

#[test]
fn unknown_subcommand_is_a_noop() {
    assert_eq!(run(&["frobnicate"], "{}"), "");
}
