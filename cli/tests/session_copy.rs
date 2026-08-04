//! Integration tests for the session-copy pipeline (port of session_copy/test_*.js).
//!
//! `capture` is exercised through the REAL `genesis-cli capture` binary with a controlled `CLAUDE_CONFIG_DIR`
//! (a subprocess, so no in-process env mutation / test races). `store` + the pure `embed` loaders are tested
//! in-process. The full capture→store→embed round-trip runs only when the real memory server + ONNX model
//! are built (mirrors the Node integration tests, which need the binary + model).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines
)]

use genesis_cli::{embed, store};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

const CLI: &str = env!("CARGO_BIN_EXE_genesis-cli");

// ── capture: through the real binary, with a fully-populated fake ~/.claude home ────────────
#[test]
fn capture_binary_extracts_every_store_and_scrubs_secrets() {
    let home = tempdir().unwrap();
    let h = home.path();
    let session = "sess-abc";

    // A. transcript (user string with an inline secret; assistant block; system turn)
    let proj = h.join("projects").join("enc-repo");
    fs::create_dir_all(&proj).unwrap();
    let transcript = format!(
        "{}\n{}\n{}\n",
        json!({"type":"user","timestamp":"t1","message":{"content":"deploy with password = supersecretval please"}}),
        json!({"type":"assistant","timestamp":"t2","message":{"content":[{"type":"text","text":"acknowledged"}]}}),
        json!({"type":"system","content":"system note here"}),
    );
    fs::write(proj.join(format!("{session}.jsonl")), transcript).unwrap();

    // B3. auto-memory
    fs::create_dir_all(proj.join("memory")).unwrap();
    fs::write(
        proj.join("memory").join("MEMORY.md"),
        "# index\nstanding context",
    )
    .unwrap();

    // B5a. context-mode DB (chunks for this session)
    let cm = h.join("context-mode").join("content");
    fs::create_dir_all(&cm).unwrap();
    {
        let c = Connection::open(cm.join("a.db")).unwrap();
        c.execute_batch(
            "CREATE TABLE chunks(title TEXT, content TEXT, session_id TEXT, timestamp TEXT);",
        )
        .unwrap();
        c.execute(
            "INSERT INTO chunks VALUES(?,?,?,?)",
            params!["chunk title", "context chunk body", session, "ts"],
        )
        .unwrap();
        // a different session's chunk must NOT be captured
        c.execute(
            "INSERT INTO chunks VALUES(?,?,?,?)",
            params!["x", "other session body", "OTHER", "ts"],
        )
        .unwrap();
    }

    // B5b. claude-mem observer jsonl (session-matched)
    let obs = h.join("projects").join("z-claude-mem-observer-1");
    fs::create_dir_all(&obs).unwrap();
    fs::write(
        obs.join("o.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"content":"observed thing","sessionId":session,"operation":"add","timestamp":"ts"}),
            json!({"content":"other-session obs","sessionId":"OTHER","operation":"add"}),
        ),
    )
    .unwrap();

    // C7. user-config (CLAUDE.md, settings.json with a token, a SKILL.md)
    fs::write(h.join("CLAUDE.md"), "global claude md").unwrap();
    fs::write(
        h.join("settings.json"),
        r#"{"token":"ghp_ABCDEFGHIJKLMNOPQRSTUVWX"}"#,
    )
    .unwrap();
    let sk = h.join("skills").join("foo");
    fs::create_dir_all(&sk).unwrap();
    fs::write(sk.join("SKILL.md"), "skill body").unwrap();

    // run: genesis-cli capture (real binary), home overridden in the CHILD env only
    let out = tempdir().unwrap();
    let res = Command::new(CLI)
        .args([
            "capture",
            "--session",
            session,
            "--out",
            out.path().to_str().unwrap(),
            "--cwd",
            out.path().to_str().unwrap(),
        ])
        .env("CLAUDE_CONFIG_DIR", h)
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "capture failed: {}",
        String::from_utf8_lossy(&res.stderr)
    );

    let records = fs::read_to_string(out.path().join("records.jsonl")).unwrap();
    for src in [
        "transcript",
        "auto-memory",
        "context-mode",
        "claude-mem",
        "user-config",
    ] {
        assert!(
            records.contains(&format!("\"source\":\"{src}\"")),
            "missing source: {src}"
        );
    }
    // scrubbing (values gone, marker present)
    assert!(!records.contains("supersecretval"), "password value leaked");
    assert!(
        !records.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWX"),
        "github token leaked"
    );
    assert!(records.contains("[REDACTED credential]"));
    // session scoping: other-session content must not appear
    assert!(
        !records.contains("other session body"),
        "cross-session context-mode leaked"
    );
    assert!(
        !records.contains("other-session obs"),
        "cross-session claude-mem leaked"
    );

    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(out.path().join("capture_manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(manifest["total_records"].as_u64().unwrap() >= 5);
    assert!(manifest["total_redactions"].as_u64().unwrap() >= 2);
    assert_eq!(manifest["session_id"], session);
}

// ── store: build a portable bundle (history.sqlite + deterministic summary) ─────────────────
#[test]
fn store_build_bundle_writes_history_and_deterministic_summary() {
    let dir = tempdir().unwrap();
    let records = dir.path().join("records.jsonl");
    let body = format!(
        "{}\n{}\n{}\n",
        json!({"source":"auto-memory","kind":"memory-file","ts":"","title":"MEMORY.md","text":"# idx\nstanding memory here"}),
        json!({"source":"transcript","kind":"user","ts":"t1","title":"","text":"hello    world"}),
        json!({"source":"transcript","kind":"assistant","ts":"t2","title":"","text":"a reply"}),
    );
    fs::write(&records, body).unwrap();

    let out = dir.path().join("agent");
    let m = store::build_bundle(&records, &out, Some("myagent")).unwrap();
    assert_eq!(m["agent"], "myagent");
    assert_eq!(m["records"], 3);

    // history.sqlite round-trips through the embed loader
    let loaded = embed::load_from_db(&out.join("history.sqlite")).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0]["source"], "auto-memory");

    let summary = fs::read_to_string(out.join("summary.md")).unwrap();
    assert!(summary.contains("# Session-copy memory — myagent"));
    assert!(summary.contains("## What was carried over"));
    assert!(summary.contains("Standing memory (from the prior session's MEMORY.md)"));
    assert!(summary.contains("Most recent 2 turns"));
    assert!(summary.contains("hello world")); // whitespace collapsed
}

#[test]
fn store_defaults_agent_name_to_out_dir_basename() {
    let dir = tempdir().unwrap();
    let records = dir.path().join("records.jsonl");
    fs::write(&records, "").unwrap();
    let out = dir.path().join("derived-name");
    let m = store::build_bundle(&records, &out, None).unwrap();
    assert_eq!(m["agent"], "derived-name");
}

// ── embed: pure helpers (the server round-trip is gated below) ───────────────────────────────
#[test]
fn embed_record_text_adds_provenance() {
    assert_eq!(
        embed::record_text(&json!({"source":"transcript","kind":"user","title":"","text":"hi"})),
        "[transcript] hi"
    );
    assert_eq!(
        embed::record_text(&json!({"source":"auto-memory","title":"MEMORY.md","text":"body"})),
        "[auto-memory] MEMORY.md: body"
    );
}

#[test]
fn embed_loaders_skip_bad_lines_and_read_db() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("r.jsonl");
    fs::write(
        &p,
        "{\"source\":\"x\",\"text\":\"t\"}\n\nnot-json\n{\"source\":\"y\"}\n",
    )
    .unwrap();
    assert_eq!(embed::load_from_jsonl(&p).len(), 2);
}

// ── full pipeline: real memory server + model (skips cleanly when they aren't built) ─────────
#[test]
fn build_session_agent_embeds_into_real_server_when_available() {
    let root = repo_root();
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let server = root
        .join("server")
        .join("target")
        .join("release")
        .join(format!("genesis-memory-server{ext}"));
    let model = root.join("server").join("models");
    if !server.is_file() || !model.join("onnx").join("model.onnx").is_file() {
        eprintln!("skip: real server binary / model not built (server/target/release + server/models) — pipeline round-trip not exercised");
        return;
    }

    // minimal fake home with a small transcript for the session
    let home = tempdir().unwrap();
    let session = "pipe-1";
    let proj = home.path().join("projects").join("enc");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join(format!("{session}.jsonl")),
        format!("{}\n", json!({"type":"user","timestamp":"t","message":{"content":"remember the blue widget spec"}})),
    )
    .unwrap();

    let repo = tempdir().unwrap();
    let gh = repo.path().join(".genesis");
    let memory_db = gh.join("memory.db");

    let res = Command::new(CLI)
        .args([
            "build-session-agent",
            "--session",
            session,
            "--name",
            "copied",
            "--repo",
            repo.path().to_str().unwrap(),
            "--genesis-home",
            gh.to_str().unwrap(),
            "--server-bin",
            server.to_str().unwrap(),
            "--model-dir",
            model.to_str().unwrap(),
            "--memory-db",
            memory_db.to_str().unwrap(),
            "--no-user-config",
        ])
        .env("CLAUDE_CONFIG_DIR", home.path())
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "build-session-agent failed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let manifest: Value = serde_json::from_slice(&res.stdout).unwrap();
    assert_eq!(manifest["agent"], "copied");
    assert!(manifest["captured"].as_u64().unwrap() >= 1);
    assert!(
        manifest["embedded"].as_u64().unwrap() >= 1,
        "nothing embedded: {manifest}"
    );
    assert!(gh
        .join("agents")
        .join("copied")
        .join("summary.md")
        .is_file());
    assert!(memory_db.is_file(), "shared memory DB created");
}
