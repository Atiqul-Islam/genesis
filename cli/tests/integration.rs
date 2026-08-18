//! End-to-end integration tests for genesis-cli — the coverage that used to live in the deleted Node
//! installer tests (test_bootstrap.js / test_main_install.js / test_portability.js / test_promote.js) plus
//! the committed-agents drift check (formerly in hooks/test_plugin.js).
//!
//! Strategy: the real repo is the read-only genesis-home *source*; every test writes only into an OS temp
//! dir. Where a test needs a writable genesis-home (register_required mutates `<gh>/expertise/required.json`)
//! it copies the needed subtree into the temp dir first, so nothing ever touches the repo tree.

// Integration tests legitimately abort on failure; the strict restriction lints don't apply to test code.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use genesis_cli::{assemble, bootstrap, build_plugin_agents, fsx, promote};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    // cli/ -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli has a parent")
        .to_path_buf()
}

/// Copy the writable genesis-home subset (expertise + team + skills) into `dst` so tests can register
/// required expertise without mutating the repo.
fn seed_gh(dst: &Path) {
    let src = repo_root();
    for sub in ["expertise", "team", "skills"] {
        fsx::copy_tree(&src.join(sub), &dst.join(sub))
            .unwrap_or_else(|e| panic!("copy {sub}: {e}"));
    }
}

fn read(p: &Path) -> String {
    fsx::read_text(p).unwrap_or_else(|| panic!("missing file: {}", p.display()))
}

// ── assemble: subagent mode ──────────────────────────────────────────────────────────────
#[test]
fn assemble_subagent_writes_agent_md_and_registers_expertise() {
    let gh_dir = tempdir().unwrap();
    let gh = gh_dir.path();
    seed_gh(gh);
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    let r = assemble::assemble_one(&gh.join("team").join("method"), "method", target, gh, false)
        .expect("assemble subagent");
    assert_eq!(r.get("agent").and_then(|v| v.as_str()), Some("method"));

    let md = target.join(".claude").join("agents").join("method.md");
    let text = read(&md);
    assert!(text.starts_with("---\nname: method\n"), "frontmatter name");
    assert!(
        text.contains("genesis-hook"),
        "wires the native hook binary"
    );
    // required expertise auto-registered in the (temp) genesis-home
    let required = read(&gh.join("expertise").join("required.json"));
    assert!(
        required.contains("method"),
        "method registered in required.json"
    );
}

// ── assemble: main mode (persona -> CLAUDE.md managed block + main-thread hooks; idempotent) ──
#[test]
fn assemble_main_writes_managed_block_and_main_thread_hooks_idempotently() {
    let gh_dir = tempdir().unwrap();
    let gh = gh_dir.path();
    seed_gh(gh);
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    assemble::assemble_one(&gh.join("team").join("method"), "method", target, gh, true)
        .expect("assemble --main");

    let claude_md = target.join("CLAUDE.md");
    let md1 = read(&claude_md);
    assert!(
        md1.contains(">>> genesis agent: method"),
        "managed block open sentinel"
    );
    assert!(
        md1.contains("<<< genesis agent: method"),
        "managed block close sentinel"
    );

    let settings_path = target.join(".claude").join("settings.json");
    let s1 = read(&settings_path);
    for hook in ["SessionStart", "PreToolUse", "Stop"] {
        assert!(s1.contains(hook), "settings.json wires {hook}");
    }
    assert!(
        s1.contains("--main-agent"),
        "main-thread hooks carry --main-agent"
    );
    assert!(
        s1.contains("method"),
        "main-thread hooks name the promoted agent"
    );

    // idempotent: a second run must not change either file.
    assemble::assemble_one(&gh.join("team").join("method"), "method", target, gh, true)
        .expect("assemble --main (2nd)");
    assert_eq!(md1, read(&claude_md), "CLAUDE.md idempotent");
    assert_eq!(s1, read(&settings_path), "settings.json idempotent");
}

// ── portability: ${CLAUDE_PROJECT_DIR} when gh is under target, absolute otherwise ──────────
#[test]
fn assemble_frontmatter_is_portable_when_gh_is_inside_target() {
    // gh INSIDE target -> the hook path must be repo-relative via ${CLAUDE_PROJECT_DIR}.
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();
    let gh = target.join(".genesis");
    seed_gh(&gh);

    assemble::assemble_one(
        &gh.join("team").join("method"),
        "method",
        target,
        &gh,
        false,
    )
    .expect("assemble (gh inside target)");
    let text = read(&target.join(".claude").join("agents").join("method.md"));
    assert!(
        text.contains("${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-hook"),
        "portable ${{CLAUDE_PROJECT_DIR}} hook path, got:\n{text}"
    );
}

#[test]
fn assemble_frontmatter_is_absolute_when_gh_is_outside_target() {
    let gh_dir = tempdir().unwrap();
    let gh = gh_dir.path();
    seed_gh(gh);
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    assemble::assemble_one(&gh.join("team").join("method"), "method", target, gh, false)
        .expect("assemble (gh outside target)");
    let text = read(&target.join(".claude").join("agents").join("method.md"));
    let abs = gh.join("bin").join("genesis-hook");
    assert!(
        text.contains(&abs.to_string_lossy().to_string()),
        "absolute hook path when gh is outside target"
    );
    assert!(
        !text.contains("${CLAUDE_PROJECT_DIR}"),
        "no portable token when gh is outside target"
    );
}

// ── promote: an existing built subagent -> the folder's main Claude ─────────────────────────
#[test]
fn promote_turns_existing_agent_into_main() {
    let gh_dir = tempdir().unwrap();
    let gh = gh_dir.path();
    seed_gh(gh);
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    // First build "method" as a subagent, then promote that same agent.
    assemble::assemble_one(&gh.join("team").join("method"), "method", target, gh, false)
        .expect("assemble subagent for promote");
    let code = promote::run(&[
        "method".to_string(),
        target.to_string_lossy().into_owned(),
        gh.to_string_lossy().into_owned(),
    ]);
    assert_eq!(code, 0, "promote exits 0");

    let md = read(&target.join("CLAUDE.md"));
    assert!(
        md.contains(">>> genesis agent: method"),
        "promoted persona managed block"
    );
    let settings = read(&target.join(".claude").join("settings.json"));
    assert!(
        settings.contains("--main-agent"),
        "promote wires main-thread hooks"
    );
}

// ── bootstrap: a full repo-level .genesis workspace ─────────────────────────────────────────
#[test]
fn bootstrap_builds_self_contained_workspace() {
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    // staging the native binaries needs `node` + a release/override; it is NON-fatal, so bootstrap still
    // completes and everything else must be present.
    let code = bootstrap::run(&[
        target.to_string_lossy().into_owned(),
        repo_root().to_string_lossy().into_owned(),
    ]);
    assert_eq!(code, 0, "bootstrap exits 0");

    let dest = target.join(".genesis");
    for sub in ["expertise", "hooks", "team", "bin"] {
        assert!(dest.join(sub).is_dir(), ".genesis/{sub} copied");
    }
    let mcp = read(&target.join(".mcp.json"));
    assert!(
        mcp.contains("genesis-memory"),
        ".mcp.json registers the memory server"
    );
    assert!(
        mcp.contains("genesis-memory.js"),
        ".mcp.json launches via the Node launcher"
    );

    let gitignore = read(&target.join(".gitignore"));
    assert!(
        gitignore.contains("genesis runtime"),
        ".gitignore managed block present"
    );

    for n in ["sensei", "method"] {
        assert!(
            target
                .join(".claude")
                .join("agents")
                .join(format!("{n}.md"))
                .is_file(),
            "{n} installed"
        );
    }

    // Repo-local SessionStart promote-offer hook (workspace-only — the plugin stays dormant-by-default).
    let settings = read(&target.join(".claude").join("settings.json"));
    assert!(
        settings.contains("SessionStart") && settings.contains("--run-hook promote-offer"),
        "bootstrap wires the repo-local promote-offer SessionStart hook"
    );

    // Idempotent: a second bootstrap must not duplicate the promote-offer hook.
    assert_eq!(
        bootstrap::run(&[
            target.to_string_lossy().into_owned(),
            repo_root().to_string_lossy().into_owned(),
        ]),
        0
    );
    let settings2 = read(&target.join(".claude").join("settings.json"));
    assert_eq!(
        settings2.matches("--run-hook promote-offer").count(),
        1,
        "promote-offer hook is not duplicated on re-bootstrap"
    );
}

// ── portability: bootstrap must emit ${CLAUDE_PROJECT_DIR}-relative paths, never absolute ────
// Regression for the hardcoded-path bug: .mcp.json + the promote-offer hook were written with the
// building machine's canonicalized absolute path, so they broke on any clone/move/commit.
#[test]
fn bootstrap_emits_portable_project_dir_paths_not_absolute() {
    let tgt_dir = tempdir().unwrap();
    let target = tgt_dir.path();

    let code = bootstrap::run(&[
        target.to_string_lossy().into_owned(),
        repo_root().to_string_lossy().into_owned(),
    ]);
    assert_eq!(code, 0, "bootstrap exits 0");

    // The target's canonical absolute prefix (what bootstrap canonicalizes internally) must NOT
    // appear in either generated config file — they travel with the repo.
    let abs_prefix = std::fs::canonicalize(target)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    let mcp = read(&target.join(".mcp.json")).replace('\\', "/");
    assert!(
        mcp.contains("${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js"),
        ".mcp.json launcher must be ${{CLAUDE_PROJECT_DIR}}-relative, got:\n{mcp}"
    );
    assert!(
        mcp.contains("${CLAUDE_PROJECT_DIR}/.genesis/memory.db")
            && mcp.contains("${CLAUDE_PROJECT_DIR}/.genesis/memory/memory.jsonl"),
        ".mcp.json memory paths must be ${{CLAUDE_PROJECT_DIR}}-relative, got:\n{mcp}"
    );
    assert!(
        !mcp.contains(&abs_prefix),
        ".mcp.json must not contain the absolute target path {abs_prefix}, got:\n{mcp}"
    );

    // Assert on the portable substring (not the full quoted command) so the check is agnostic to JSON
    // quote-escaping; the promote-offer marker confirms it is that hook's command.
    let settings = read(&target.join(".claude").join("settings.json"));
    assert!(
        settings.contains("${CLAUDE_PROJECT_DIR}/.genesis/bin/genesis-memory.js")
            && settings.contains("--run-hook promote-offer"),
        "promote-offer hook must be ${{CLAUDE_PROJECT_DIR}}-relative, got:\n{settings}"
    );
    assert!(
        !settings.replace('\\', "/").contains(&abs_prefix),
        "settings.json must not contain the absolute target path {abs_prefix}, got:\n{settings}"
    );
}

// ── drift: committed agents/*.md == what genesis-cli build-plugin-agents regenerates ────────
#[test]
fn build_plugin_agents_matches_committed_no_drift() {
    let src = repo_root();
    let work_dir = tempdir().unwrap();
    let work = work_dir.path();
    // copy just the inputs build-plugin-agents reads (+ an agents/ output dir).
    fsx::copy_tree(&src.join(".claude-plugin"), &work.join(".claude-plugin")).unwrap();
    fsx::copy_tree(&src.join("team"), &work.join("team")).unwrap();
    fsx::copy_tree(&src.join("skills"), &work.join("skills")).unwrap();

    let code = build_plugin_agents::run(&[work.to_string_lossy().into_owned()]);
    assert_eq!(code, 0, "build-plugin-agents exits 0");

    for n in ["sensei", "method", "mneme"] {
        let regenerated = read(&work.join("agents").join(format!("{n}.md")));
        let committed = read(&src.join("agents").join(format!("{n}.md")));
        assert_eq!(
            regenerated, committed,
            "committed agents/{n}.md drifted from team sources — run `genesis-cli build-plugin-agents`"
        );
    }
}
