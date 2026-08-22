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

/// Like `run`, but with the child's working directory set to `dir` (so per-repo files like a guard at
/// `<dir>/.genesis/team/<agent>/guard.json` resolve).
fn run_in(dir: &std::path::Path, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_genesis-hook"))
        .args(args)
        .current_dir(dir)
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

/// Deny only when the gate returned a `deny` decision.
fn is_deny(out: &str) -> bool {
    !out.is_empty() && parse(out)["hookSpecificOutput"]["permissionDecision"] == "deny"
}

// ---- gate: per-agent guard (Feature 1 — agent-scoped-guards) ----

fn write_atlas_guard(dir: &std::path::Path, guard: &Value) {
    let gdir = dir.join(".genesis/team/atlas");
    std::fs::create_dir_all(&gdir).unwrap();
    std::fs::write(gdir.join("guard.json"), guard.to_string()).unwrap();
}

#[test]
fn gate_guard_denies_dropping_invariant_and_self_protect() {
    let td = tempfile::tempdir().unwrap();
    write_atlas_guard(
        td.path(),
        &json!({
            "self_protect": [".genesis/team/atlas/guard.json"],
            "invariants": [{"id":"c1","files":["persona.md"],"must_match":"per-action approval","why":"keep the approval gate"}]
        }),
    );
    // AC1: a write that DROPS the invariant is denied.
    let ev = json!({"agent_type":"atlas","tool_input":{"file_path":"persona.md","content":"no phrase here"}});
    assert!(
        is_deny(&run_in(
            td.path(),
            &["gate", "--expertise", EXP],
            &ev.to_string()
        )),
        "dropping a must_match invariant must deny"
    );
    // AC2: a write that KEEPS the invariant is allowed (short content => silent).
    let ev = json!({"agent_type":"atlas","tool_input":{"file_path":"persona.md","content":"requires per-action approval"}});
    assert!(
        run_in(td.path(), &["gate", "--expertise", EXP], &ev.to_string()).is_empty(),
        "keeping the invariant must allow"
    );
    // AC3: writing the guard file itself is denied (self-protect).
    let ev = json!({"agent_type":"atlas","tool_input":{"file_path":".genesis/team/atlas/guard.json","content":"{}"}});
    assert!(
        is_deny(&run_in(
            td.path(),
            &["gate", "--expertise", EXP],
            &ev.to_string()
        )),
        "an agent must not edit its own guard"
    );
}

#[test]
fn gate_guard_is_scoped_to_the_active_agent() {
    // AC4: atlas's guard must not constrain a DIFFERENT active agent.
    let td = tempfile::tempdir().unwrap();
    write_atlas_guard(
        td.path(),
        &json!({"self_protect": [], "invariants": [{"id":"c1","files":["persona.md"],"must_match":"per-action approval","why":"x"}]}),
    );
    let ev = json!({"agent_type":"method","tool_input":{"file_path":"persona.md","content":"a short clean persona with no such phrase"}});
    assert!(
        run_in(td.path(), &["gate", "--expertise", EXP], &ev.to_string()).is_empty(),
        "atlas's guard must not fire for method"
    );
}

#[test]
fn gate_no_guard_behaves_like_before() {
    // AC5: with no guard file, a clean short persona write is allowed/silent, exactly as pre-feature.
    let td = tempfile::tempdir().unwrap();
    let ev = json!({"agent_type":"atlas","tool_input":{"file_path":"persona.md","content":"a short clean persona"}});
    assert!(
        run_in(td.path(), &["gate", "--expertise", EXP], &ev.to_string()).is_empty(),
        "absent guard => no new behavior"
    );
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

// ---- enforce-research: no-grep guard (no-grep-guard) ----

#[test]
fn grep_guard_blocks_genesis_engineer_file_grep() {
    let ev = json!({"agent_type":"genesis-engineer","tool_input":{"command":"grep foo src/x.rs"}});
    let v = parse(&run(&["enforce-research"], &ev.to_string()));
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn grep_guard_allows_piped_grep() {
    let ev = json!({"agent_type":"genesis-engineer","tool_input":{"command":"cargo test | grep result"}});
    assert_eq!(run(&["enforce-research"], &ev.to_string()), ""); // piped -> stdin -> allow (silent)
}

#[test]
fn grep_guard_does_not_touch_agents_with_the_grep_tool() {
    // method holds the Grep tool -> the no-grep guard must NOT block its file grep.
    let ev = json!({"agent_type":"method","tool_input":{"command":"grep foo src/x.rs"}});
    assert_eq!(run(&["enforce-research"], &ev.to_string()), ""); // allow (silent)
}

#[test]
fn grep_guard_fires_for_promoted_main_via_main_agent() {
    // genesis-engineer as a promoted main carries no payload agent_type; --main-agent makes it fire.
    let ev = json!({"tool_input":{"command":"rg foo"}});
    let v = parse(&run(
        &["enforce-research", "--main-agent", "genesis-engineer"],
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

#[test]
fn validate_allows_when_declared_via_quiet_record_channel() {
    // Feature 2 (verbose-declarations) AC1/AC6: an agent that RECORDS its declarations to
    // `applied-expertise.jsonl` (a Write tool_use) — with NO declarations in visible prose — finishes
    // exactly as if it had printed them. mneme requires memory-management + expertise-application.
    let td = tempfile::tempdir().unwrap();
    let recorded = "APPLIED-EXPERTISE: memory-management#mm-1 — applied\n\
         APPLIED-EXPERTISE: memory-management#mm-2 — applied\n\
         APPLIED-EXPERTISE: memory-management#mm-3 — applied\n\
         APPLIED-EXPERTISE: expertise-application#ea-1 — applied\n\
         APPLIED-EXPERTISE: expertise-application#ea-2 — applied\n\
         APPLIED-EXPERTISE: expertise-application#ea-3 — applied";
    let human = json!({"type":"user","message":{"role":"user","content":"do it"}});
    let rec = json!({"type":"assistant","message":{"content":[
        {"type":"text","text":"Done — declarations recorded quietly."},
        {"type":"tool_use","name":"Write","input":{
            "file_path":"/proj/.genesis/applied-expertise.jsonl","content":recorded}}
    ]}});
    let tpath = td.path().join("transcript.jsonl");
    std::fs::write(&tpath, format!("{human}\n{rec}\n")).unwrap();
    let ev =
        json!({"agent_type":"mneme","transcript_path":tpath.to_str().unwrap(),"session_id":"s"});
    let root = td.path().to_str().unwrap();
    // Empty stdout == allow (no block emitted).
    assert_eq!(
        run(
            &["validate", root, "mneme", "--expertise", EXP],
            &ev.to_string()
        ),
        "",
        "quiet record-channel declarations must satisfy validate the same as printed ones"
    );
}

// ---- precompact + resume restore (issue #1) ----

#[test]
fn precompact_writes_snapshot_and_inject_restores_it() {
    let td = tempfile::tempdir().unwrap();
    // a transcript with recent conversation
    let human = json!({"type":"user","message":{"role":"user","content":"resume test marker"}});
    let asst = json!({"type":"assistant","message":{"content":[{"type":"text","text":"acknowledged marker"}]}});
    let tp = td.path().join("t.jsonl");
    std::fs::write(&tp, format!("{human}\n{asst}\n")).unwrap();

    // precompact (run with cwd = td so runtime_dir -> td/.genesis) writes the snapshot
    let ev = json!({"agent_type":"genesis-engineer","transcript_path":tp.to_str().unwrap(),"trigger":"manual","session_id":"s"});
    assert_eq!(
        run_in(td.path(), &["precompact"], &ev.to_string()),
        "",
        "precompact emits no decision (side-effect only)"
    );
    let snap = td.path().join(".genesis/resume-state.md");
    assert!(snap.is_file(), "precompact wrote the resume snapshot");
    let snap_text = std::fs::read_to_string(&snap).unwrap();
    assert!(snap_text.contains("resume test marker") && snap_text.contains("acknowledged marker"));

    // inject on source=compact restores it
    let iev = json!({"agent_type":"genesis-engineer","source":"compact"});
    let v = parse(&run_in(
        td.path(),
        &["inject", EXP, "genesis-engineer"],
        &iev.to_string(),
    ));
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("");
    assert!(
        ctx.contains("Resume — recent session state"),
        "inject restores the snapshot on compact"
    );
    assert!(ctx.contains("resume test marker"));

    // inject on source=startup does NOT restore it
    let sev = json!({"agent_type":"genesis-engineer","source":"startup"});
    let v2 = parse(&run_in(
        td.path(),
        &["inject", EXP, "genesis-engineer"],
        &sev.to_string(),
    ));
    let ctx2 = v2["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("");
    assert!(
        !ctx2.contains("Resume — recent session state"),
        "no resume on a fresh startup"
    );
}

#[test]
fn precompact_is_dormant_without_a_genesis_agent() {
    let td = tempfile::tempdir().unwrap();
    let tp = td.path().join("t.jsonl");
    std::fs::write(&tp, "{\"type\":\"user\",\"message\":{\"content\":\"x\"}}\n").unwrap();
    let ev = json!({"transcript_path":tp.to_str().unwrap(),"trigger":"manual"}); // no agent_type
    assert_eq!(run_in(td.path(), &["precompact"], &ev.to_string()), "");
    assert!(
        !td.path().join(".genesis/resume-state.md").exists(),
        "dormant: no snapshot written"
    );
}

// ---- capture-session (issue #9) ----

#[test]
fn capture_session_copies_transcript_into_repo() {
    let td = tempfile::tempdir().unwrap();
    let tp = td.path().join("sess-42.jsonl");
    std::fs::write(
        &tp,
        "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\n",
    )
    .unwrap();
    let ev = json!({"agent_type":"genesis-engineer","transcript_path":tp.to_str().unwrap()});
    assert_eq!(
        run_in(td.path(), &["capture-session"], &ev.to_string()),
        "",
        "capture-session emits no decision"
    );
    assert!(
        td.path().join(".genesis/sessions/sess-42.jsonl").is_file(),
        "transcript captured into the repo"
    );
}

#[test]
fn capture_session_is_dormant_without_a_genesis_agent() {
    let td = tempfile::tempdir().unwrap();
    let tp = td.path().join("s.jsonl");
    std::fs::write(&tp, "x").unwrap();
    let ev = json!({"transcript_path":tp.to_str().unwrap()}); // no agent_type
    assert_eq!(run_in(td.path(), &["capture-session"], &ev.to_string()), "");
    assert!(
        !td.path().join(".genesis/sessions").exists(),
        "dormant: nothing captured"
    );
}

// ---- validate: reply-format guard (reply-format-guard) ----

#[test]
fn validate_blocks_genesis_engineer_on_overlong_bullet() {
    let td = tempfile::tempdir().unwrap();
    let long = "word ".repeat(25);
    let human = json!({"type":"user","message":{"role":"user","content":"go"}});
    let assistant = json!({"type":"assistant","message":{"content":[{"type":"text","text":format!("- {long}")}]}});
    let tp = td.path().join("t.jsonl");
    std::fs::write(&tp, format!("{human}\n{assistant}\n")).unwrap();
    let ev = json!({"agent_type":"genesis-engineer","transcript_path":tp.to_str().unwrap(),"session_id":"s"});
    let root = td.path().to_str().unwrap();
    let v = parse(&run(
        &["validate", root, "genesis-engineer", "--expertise", EXP],
        &ev.to_string(),
    ));
    assert_eq!(v["decision"], "block");
    assert!(
        v["reason"].as_str().unwrap_or("").contains("Reply-format"),
        "expected a reply-format block, got: {}",
        v["reason"]
    );
}

#[test]
fn validate_allows_genesis_engineer_with_short_bullets() {
    let td = tempfile::tempdir().unwrap();
    let human = json!({"type":"user","message":{"role":"user","content":"go"}});
    let assistant = json!({"type":"assistant","message":{"content":[
        {"type":"text","text":"- short bullet, well under twenty words\n- another fine point"}]}});
    let tp = td.path().join("t.jsonl");
    std::fs::write(&tp, format!("{human}\n{assistant}\n")).unwrap();
    let ev = json!({"agent_type":"genesis-engineer","transcript_path":tp.to_str().unwrap(),"session_id":"s"});
    let root = td.path().to_str().unwrap();
    assert_eq!(
        run(
            &["validate", root, "genesis-engineer", "--expertise", EXP],
            &ev.to_string()
        ),
        "",
        "short bullets pass"
    );
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
