//! `genesis-cli build-session-agent` — session-copy Phase 3 orchestrator (port of build_session_agent.js).
//!
//! One call turns a live session into a session-copy agent's MEMORY: capture the current session's stores
//! (scrubbed) → store a portable history + start-time summary → embed the records into the repo's shared
//! Genesis memory under `agent_id=<name>` (recallable). Does NOT author the persona or run the assembler —
//! that is Sensei's normal single-agent build; this owns only the copy-the-session-into-memory half.

use crate::{capture, embed, fsx, store};
use serde_json::{json, Value};
use std::path::Path;

/// Resolve `--session current` from the session-pointer hook's `<repo>/.genesis/current-session.json`.
///
/// # Errors
/// Returns a message if `current` is requested but no pointer file / session id is found.
pub fn resolve_session(session_id: &str, repo: &Path) -> Result<String, String> {
    if !session_id.is_empty() && session_id != "current" {
        return Ok(session_id.to_string());
    }
    let ptr = repo.join(".genesis").join("current-session.json");
    let sid = fsx::read_json(&ptr)
        .and_then(|v| {
            v.get("session_id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_default();
    if sid.is_empty() {
        return Err(
            "session '--current' requested but no current-session.json pointer found — is the \
             session_pointer hook wired into this repo? (or pass an explicit --session <id>)"
                .to_string(),
        );
    }
    Ok(sid)
}

/// Chain capture + store + embed for the session-copy half of a build. Returns the manifest.
///
/// # Errors
/// Returns a message if any stage fails (capture write, store write, or the embed handshake).
#[allow(clippy::too_many_arguments)]
pub fn build(
    session_id: &str,
    name: &str,
    repo: &Path,
    genesis_home: &Path,
    server_bin: &Path,
    model_dir: &Path,
    memory_db: &Path,
    known: &[String],
    include_user_config: bool,
) -> Result<Value, String> {
    let session_id = resolve_session(session_id, repo)?;
    let bundle = genesis_home.join("agents").join(name);
    std::fs::create_dir_all(&bundle).map_err(|e| format!("mkdir {}: {e}", bundle.display()))?;

    // 1. capture (scrubbed) — a main-session copy has no prior Genesis memory to merge.
    let cap = capture::capture(
        &session_id,
        Some(repo),
        &bundle,
        None,
        None,
        include_user_config,
        known,
    )?;

    // 2. store: portable history + start-time summary digest.
    store::build_bundle(&bundle.join("records.jsonl"), &bundle, Some(name))?;

    // 3. embed the records into the repo's SHARED memory DB under agent_id=<name>.
    let recs = embed::load_from_db(&bundle.join("history.sqlite"))?;
    let em = embed::embed_records(
        &recs,
        name,
        server_bin,
        model_dir,
        memory_db,
        embed::DEFAULT_MAX_CHARS,
    )?;

    let repo_abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let manifest = json!({
        "agent": name,
        "session_id": session_id,
        "repo": repo_abs.to_string_lossy(),
        "bundle": bundle.to_string_lossy(),
        "memory_db": memory_db.to_string_lossy(),
        "captured": cap.get("total_records").cloned().unwrap_or(json!(0)),
        "redactions": cap.get("total_redactions").cloned().unwrap_or(json!(0)),
        "embedded": em.get("stored").cloned().unwrap_or(json!(0)),
        "embed_failed": em.get("failed").cloned().unwrap_or(json!(0)),
        "summary": bundle.join("summary.md").to_string_lossy(),
        "by_source": cap.get("by_source").cloned().unwrap_or_else(|| json!({})),
        "next": "Sensei: author the agent's specialized persona (Method), then genesis-cli assemble — the agent will recall this history via its memory tools and load summary.md at start.",
    });
    let mp = bundle.join("session_copy_manifest.json");
    fsx::write_text(&mp, &fsx::json_pretty(&manifest))
        .map_err(|e| format!("write {}: {e}", mp.display()))?;
    Ok(manifest)
}

/// Entry point for `genesis-cli build-session-agent`. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let mut opt: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    let mut known: Vec<String> = Vec::new();
    let mut no_user_config = false;
    let mut i = 0;
    let keys = [
        "--session",
        "--name",
        "--repo",
        "--genesis-home",
        "--server-bin",
        "--model-dir",
        "--memory-db",
    ];
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--no-user-config" {
            no_user_config = true;
            i += 1;
        } else if a == "--known-secret" {
            if let Some(v) = args.get(i + 1).cloned() {
                known.push(v);
            }
            i += 2;
        } else if let Some(&k) = keys.iter().find(|&&k| k == a) {
            if let Some(v) = args.get(i + 1).cloned() {
                opt.insert(k, v);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let usage = "usage: genesis-cli build-session-agent --session <id> --name <agent> --repo <target_repo> --genesis-home <dir> --server-bin <bin> --model-dir <dir> --memory-db <path> [--known-secret V ...] [--no-user-config]";
    let get = |k: &str| opt.get(k).cloned();
    let (
        Some(session),
        Some(name),
        Some(repo),
        Some(gh),
        Some(server_bin),
        Some(model_dir),
        Some(memory_db),
    ) = (
        get("--session"),
        get("--name"),
        get("--repo"),
        get("--genesis-home"),
        get("--server-bin"),
        get("--model-dir"),
        get("--memory-db"),
    )
    else {
        fsx::fail(usage);
    };
    match build(
        &session,
        &name,
        Path::new(&repo),
        Path::new(&gh),
        Path::new(&server_bin),
        Path::new(&model_dir),
        Path::new(&memory_db),
        &known,
        !no_user_config,
    ) {
        Ok(m) => {
            println!("{}", fsx::json_pretty(&m));
            0
        }
        Err(e) => fsx::fail(&e),
    }
}
