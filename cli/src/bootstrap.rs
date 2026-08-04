//! `genesis-cli bootstrap <target_repo> [genesis_home]` — set up a self-contained repo-level Genesis
//! workspace at `<target>/.genesis/`. Port of bootstrap.js, updated for the all-Rust + GitHub-Releases model.
//!
//! Copies the functional tree into `.genesis/`, stages the native binaries into `.genesis/bin` via the Node
//! fetch-launcher, registers a repo-local `genesis-memory` MCP server in `.mcp.json` (launched via the Node
//! launcher), manages the `.gitignore` block, and installs sensei+method wired to the repo-level `.genesis`.

use crate::{assemble, fsx};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const TEAM: [&str; 2] = ["sensei", "method"];

const GITIGNORE_START: &str = "# >>> genesis runtime (managed by bootstrap) >>>";
const GITIGNORE_END: &str = "# <<< genesis runtime <<<";

fn gitignore_block() -> String {
    [
        GITIGNORE_START,
        "# Commit the agent brain (expertise + hooks) and portable memory (JSONL) so agents and their",
        "# learned memory travel with the repo across systems. Ignore machine-local / regenerable junk.",
        ".genesis/*",
        "!.genesis/expertise/",
        "!.genesis/hooks/",
        "!.genesis/memory/",
        ".genesis/**/__pycache__/",
        ".genesis/expertise/.genesis/",
        ".genesis/memory/*.tmp",
        "*.db",
        ".mcp.json",
        GITIGNORE_END,
        "",
    ]
    .join("\n")
}

/// Idempotently write the managed genesis block into `<target>/.gitignore` (replace between sentinels,
/// else append), never touching the user's own lines.
fn merge_gitignore(target: &Path) {
    let p = target.join(".gitignore");
    let existing = fsx::read_text(&p).unwrap_or_default();
    let block = gitignore_block();
    let merged = match (existing.find(GITIGNORE_START), existing.find(GITIGNORE_END)) {
        (Some(si), Some(ei)) if ei > si => {
            let pre = &existing[..si];
            let post = &existing[ei + GITIGNORE_END.len()..];
            let pre_t = pre.trim_end_matches('\n');
            let sep = if pre.trim().is_empty() { "" } else { "\n\n" };
            format!("{pre_t}{sep}{block}{}", post.trim_start_matches('\n'))
        }
        _ => {
            let sep = if existing.is_empty() || existing.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let gap = if existing.trim().is_empty() { "" } else { "\n" };
            format!("{existing}{sep}{gap}{block}")
        }
    };
    let _ = fsx::write_text(&p, &merged);
}

/// Stage the native binaries (genesis-hook + genesis-cli) into `<dest>/bin` via the Node fetch-launcher
/// (which downloads from the release, or uses the GENESIS_*_BIN dev overrides). Non-fatal.
fn stage_binaries(dest: &Path) -> bool {
    let launcher = dest.join("bin").join("genesis-memory.js");
    let bin_dir = dest.join("bin");
    let run = |mode: &str| -> bool {
        std::process::Command::new("node")
            .arg(&launcher)
            .arg(mode)
            .arg(&bin_dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    let hook_exe = if cfg!(windows) {
        "genesis-hook.exe"
    } else {
        "genesis-hook"
    };
    run("--stage-hook") && run("--stage-cli") && bin_dir.join(hook_exe).is_file()
}

/// Register the repo-local `genesis-memory` MCP server in `<target>/.mcp.json` (merge, preserve others).
fn write_mcp(target: &Path, launcher: &Path, mem_db: &Path, mem_export: &Path) -> PathBuf {
    let p = target.join(".mcp.json");
    let mut mcp = fsx::read_json(&p)
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if !mcp.get("mcpServers").is_some_and(Value::is_object) {
        if let Some(o) = mcp.as_object_mut() {
            o.insert("mcpServers".to_string(), json!({}));
        }
    }
    if let Some(servers) = mcp.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.insert(
            "genesis-memory".to_string(),
            json!({
                "command": "node",
                "args": [launcher.to_string_lossy()],
                "env": {
                    "GENESIS_MEMORY_DB": mem_db.to_string_lossy(),
                    "GENESIS_MEMORY_EXPORT": mem_export.to_string_lossy(),
                },
            }),
        );
    }
    let _ = fsx::write_text(&p, &fsx::json_pretty(&mcp));
    p
}

/// Entry point. Returns the process exit code.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let Some(target_arg) = args.first() else {
        fsx::fail(
            "usage: genesis-cli bootstrap <target_repo> [genesis_home]  (or set $GENESIS_HOME)",
        );
    };
    let target = std::fs::canonicalize(target_arg).unwrap_or_else(|_| PathBuf::from(target_arg));
    let Some(gh) = args
        .get(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("GENESIS_HOME").ok().map(PathBuf::from))
    else {
        fsx::fail("genesis_home required — pass it as the 2nd arg or set $GENESIS_HOME");
    };
    if !target.is_dir() {
        fsx::fail(&format!("target repo not found: {}", target.display()));
    }
    let dest = target.join(".genesis");
    if gh == dest {
        fsx::fail("refusing to bootstrap a genesis into itself.");
    }

    // 1. Copy the functional Genesis tree (self-contained). `bin` carries the Node launcher.
    for sub in ["expertise", "hooks", "skills", "team", "bin"] {
        let s = gh.join(sub);
        if !s.is_dir() {
            fsx::fail(&format!(
                "genesis home is missing {sub}/ at {} — is it a Genesis install?",
                gh.display()
            ));
        }
        if let Err(e) = fsx::copy_tree(&s, &dest.join(sub)) {
            fsx::fail(&format!("failed to copy {sub}/: {e}"));
        }
    }

    // 1b. Stage the native binaries into .genesis/bin (via the copied launcher). Non-fatal.
    let hook_bin_staged = stage_binaries(&dest);

    // 2. Memory paths (the launched server creates the DB on first use; JSONL is the portable mirror).
    let mem_db = dest.join("memory.db");
    let mem_export = dest.join("memory").join("memory.jsonl");

    // 3. Register the repo-local memory MCP server (launched via the Node launcher in .genesis/bin).
    let launcher = dest.join("bin").join("genesis-memory.js");
    let mcp_path = write_mcp(&target, &launcher, &mem_db, &mem_export);

    // 4. Manage the .gitignore block (commit the brain + portable memory; ignore machine-local junk).
    merge_gitignore(&target);

    // 5. Install sensei + method, wired to the repo-level .genesis home.
    let mut installed = Vec::new();
    for name in TEAM {
        match assemble::assemble_one(&dest.join("team").join(name), name, &target, &dest, false) {
            Ok(r) => installed.push(r.get("agent").cloned().unwrap_or(json!(name))),
            Err(e) => fsx::fail(&format!("assemble {name} failed: {e}")),
        }
    }

    println!(
        "{}",
        fsx::json_pretty(&json!({
            "bootstrapped": dest.to_string_lossy(),
            "target_repo": target.to_string_lossy(),
            "agents_installed": installed,
            "agents_dir": target.join(".claude").join("agents").to_string_lossy(),
            "hook_bin_staged": hook_bin_staged,
            "memory_db": mem_db.to_string_lossy(),
            "memory_export": mem_export.to_string_lossy(),
            "mcp_json": mcp_path.to_string_lossy(),
            "mcp_server": format!("node {}", launcher.to_string_lossy()),
            "next": "Open Claude Code in the repo; talk to Sensei to build agents.",
        }))
    );
    0
}
