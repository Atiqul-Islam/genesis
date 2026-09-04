//! `genesis-cli bootstrap <target_repo> [genesis_home]` — set up a self-contained repo-level Genesis
//! workspace at `<target>/.genesis/`. Port of bootstrap.js, updated for the all-Rust + GitHub-Releases model.
//!
//! Copies the functional tree into `.genesis/`, stages the native binaries into `.genesis/bin` via the Node
//! fetch-launcher, registers a repo-local `genesis-memory` MCP server in `.mcp.json` (launched via the Node
//! launcher), manages the `.gitignore` block, and installs sensei+method wired to the repo-level `.genesis`.

use crate::{assemble, fsx, render};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const TEAM: [&str; 2] = ["sensei", "method"];

const GITIGNORE_START: &str = "# >>> genesis runtime (managed by bootstrap) >>>";
const GITIGNORE_END: &str = "# <<< genesis runtime <<<";

fn gitignore_block() -> String {
    [
        GITIGNORE_START,
        "# Commit the agent brain (expertise + hooks) AND its learned memory — both the vector DB",
        "# (memory.db, ready-to-use with embeddings) and the portable JSONL mirror (the diff-friendly",
        "# merge substrate) — so memory travels with the repo across systems. On `update remote`, git",
        "# takes the latest .db and `genesis-cli reconcile` unions the JSONL so nothing is ever lost.",
        "# Ignore only machine-local / regenerable junk (stray DBs, archived strays, temp exports).",
        ".genesis/*",
        "!.genesis/expertise/",
        "!.genesis/hooks/",
        "!.genesis/memory/",
        "!.genesis/sessions/",
        ".genesis/**/__pycache__/",
        ".genesis/expertise/.genesis/",
        ".genesis/memory/*.tmp",
        ".genesis/memory/archived-strays/",
        "*.db",
        "!.genesis/memory.db",
        ".mcp.json",
        GITIGNORE_END,
        "",
    ]
    .join("\n")
}

/// Idempotently write the managed genesis block into `<target>/.gitignore` (replace between sentinels,
/// else append), never touching the user's own lines.
pub fn merge_gitignore(target: &Path) {
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
fn write_mcp(target: &Path, launcher: &str, mem_db: &str, mem_export: &str) -> PathBuf {
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
                "args": [launcher],
                "env": {
                    "GENESIS_MEMORY_DB": mem_db,
                    "GENESIS_MEMORY_EXPORT": mem_export,
                },
            }),
        );
    }
    let _ = fsx::write_text(&p, &fsx::json_pretty(&mcp));
    p
}

/// Install a REPO-LOCAL `SessionStart` promote-offer hook into `<target>/.claude/settings.json` (merge,
/// preserving the user's own settings/hooks). This is how the "offer to promote on startup" works WITHOUT
/// breaking the plugin's dormant-by-default posture: the offer lives in THIS workspace's project settings,
/// so it fires only here; the plugin's own `hooks/hooks.json` still wires no main-thread SessionStart. The
/// hook self-silences once a Genesis agent is promoted to main (see `genesis-hook promote-offer`), and it is
/// fail-open. Idempotent: re-bootstrapping never duplicates it.
fn write_promote_offer_hook(target: &Path, launcher: &str) -> PathBuf {
    let p = target.join(".claude").join("settings.json");
    let command = format!("node \"{launcher}\" --run-hook promote-offer");
    let mut settings = fsx::read_json(&p)
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    // `settings` is always an object here (the filter/unwrap above guarantees it), but the crate denies
    // `expect_used`, so guard instead of asserting. The else branch is unreachable in practice.
    let Some(obj) = settings.as_object_mut() else {
        let _ = fsx::write_text(&p, &fsx::json_pretty(&settings));
        return p;
    };
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    if let Some(hooks) = hooks {
        let sessionstart = hooks
            .entry("SessionStart")
            .or_insert_with(|| json!([]))
            .as_array_mut();
        if let Some(arr) = sessionstart {
            // Idempotent: skip if a promote-offer command is already wired here.
            let already = arr.iter().any(|blk| {
                blk.get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hs| {
                        hs.iter().any(|h| {
                            h.get("command")
                                .and_then(Value::as_str)
                                .is_some_and(|c| c.contains("--run-hook promote-offer"))
                        })
                    })
            });
            if !already {
                arr.push(json!({
                    "matcher": "startup|resume|compact",
                    "hooks": [{ "type": "command", "command": command }],
                }));
            }
        }
    }
    let _ = fsx::write_text(&p, &fsx::json_pretty(&settings));
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

    // 1a. Feature 2: build the derived expertise.db from the just-copied substrate so the hooks read it
    // immediately. Fail-open — a bad migrate must never abort bootstrap (the readers fall back to files).
    let _ = crate::expertise_migrate::build(&dest.join("expertise"));

    // 1b. Stage the native binaries into .genesis/bin (via the copied launcher). Non-fatal.
    let hook_bin_staged = stage_binaries(&dest);

    // 2. Memory + launcher paths — written into .mcp.json / settings.json as ${CLAUDE_PROJECT_DIR}-relative
    //    strings. `dest` is always `<repo>/.genesis` (inside `target`), so `portable_home` yields
    //    `${CLAUDE_PROJECT_DIR}/.genesis`; the generated config then survives a clone/move/commit instead of
    //    baking in the building machine's absolute path. Claude Code expands the variable in both the MCP
    //    server config and the hook command at runtime.
    let home = render::portable_home(&target, &dest);
    let launcher = format!("{home}/bin/genesis-memory.js");
    let mem_db = format!("{home}/memory.db");
    let mem_export = format!("{home}/memory/memory.jsonl");

    // 3. Register the repo-local memory MCP server (launched via the Node launcher in .genesis/bin).
    let mcp_path = write_mcp(&target, &launcher, &mem_db, &mem_export);

    // 3b. Install the repo-local SessionStart promote-offer hook (workspace-only; plugin stays dormant).
    let settings_path = write_promote_offer_hook(&target, &launcher);

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
            "memory_db": mem_db,
            "memory_export": mem_export,
            "mcp_json": mcp_path.to_string_lossy(),
            "mcp_server": format!("node {launcher}"),
            "promote_offer_settings": settings_path.to_string_lossy(),
            "next": "Open Claude Code in the repo; talk to Sensei to build agents.",
        }))
    );
    0
}

/// `genesis-cli sync-gitignore <target>` — regenerate ONLY the managed `.gitignore` block of an existing
/// repo so a workspace bootstrapped with an OLDER template adopts the current block (e.g. the
/// `!.genesis/memory.db` re-include that lets the ready-to-use vector DB travel). Idempotent; changes
/// nothing else (never re-copies committed expertise/hooks). The launcher's `--sync` runs this on every
/// plugin update, so stale repos self-heal. Returns the process exit code.
#[must_use]
pub fn run_sync_gitignore(args: &[String]) -> i32 {
    let Some(target_arg) = args.first() else {
        fsx::fail("usage: genesis-cli sync-gitignore <target_repo>");
    };
    let target = std::fs::canonicalize(target_arg).unwrap_or_else(|_| PathBuf::from(target_arg));
    if !target.is_dir() {
        fsx::fail(&format!("target repo not found: {}", target.display()));
    }
    // Regenerate the managed block to the current template (single-sourced in `gitignore_block`).
    // Idempotent: a no-op when the block already matches; preserves everything outside the sentinels.
    merge_gitignore(&target);
    println!(
        "{}",
        fsx::json_pretty(&json!({
            "synced_gitignore": target.join(".gitignore").to_string_lossy(),
        }))
    );
    0
}
