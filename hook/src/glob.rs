//! Artifact discovery: find the files an agent plausibly authored under a root.
//!
//! Ports the Node `produced_files`/glob machinery, with ONE intentional, semantics-preserving change:
//! the traversal PRUNES heavy build directories (`node_modules`, `target`, `dist`, `build`,
//! `vendor`, `coverage`, `out`) in addition to skipping dot-dirs. Those dirs never hold genesis
//! persona/behavior/CLAUDE artifacts, so pruning them changes the result set for genesis artifacts
//! not at all — it only stops the walker from descending a huge dependency tree (the measured
//! ~59 s → sub-second win, especially on WSL `/mnt/c`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Glob patterns for agent-authored artifacts (matches the Node `ARTIFACT_GLOBS`).
pub const ARTIFACT_GLOBS: &[&str] = &[
    "*persona.md",
    "*behavior.md",
    // A bare `CLAUDE.md` is intentionally NOT here. A normal agent build never authors CLAUDE.md — its
    // persona lives in `*persona.md` / `.claude/agents/*.md`. Matching a bare `CLAUDE.md` made validate/
    // review judge the USER's own project CLAUDE.md (and nested ones) against the expertise rules, producing
    // spurious complaints (agents even edited the user's CLAUDE.md to "fix" them). A promoted main agent's
    // persona is a managed-block COPY of an already-validated persona, so nothing is lost by excluding it.
    ".claude/agents/*.md",
    "*persona.spec.json",
    "*.tests.json",
];

/// Heavy directories that never contain genesis artifacts — pruned from the walk.
const PRUNE: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    "coverage",
    "out",
];

/// Match a single glob segment (`*`, `?`) against a directory-entry name, honoring the glob rule
/// that `*` does not match a leading dot (unless the segment itself begins with `.`).
fn seg_matches(seg: &str, name: &str) -> bool {
    if !seg.starts_with('.') && name.starts_with('.') {
        return false;
    }
    let p: Vec<char> = seg.chars().collect();
    let t: Vec<char> = name.chars().collect();
    wildcard(&p, &t)
}

/// Classic iterative `*`/`?` wildcard match with backtracking.
fn wildcard(p: &[char], t: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Every dir reachable from `root` by descending through non-hidden, non-pruned subdirs
/// (including `root` itself).
fn reachable_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    walk(root, &mut dirs);
    dirs
}

fn walk(d: &Path, dirs: &mut Vec<PathBuf>) {
    dirs.push(d.to_path_buf());
    let Ok(entries) = std::fs::read_dir(d) else {
        return;
    };
    for e in entries.flatten() {
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || PRUNE.contains(&name.as_str()) {
            continue;
        }
        walk(&e.path(), dirs);
    }
}

/// Match a `/`-separated glob (`segs`) starting from `dir`, collecting matching file paths.
fn match_segs(dir: &Path, segs: &[&str], idx: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let last = idx == segs.len() - 1;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !seg_matches(segs[idx], &name) {
            continue;
        }
        let ft = e.file_type();
        if last {
            if ft.map(|t| t.is_file()).unwrap_or(false) {
                out.push(e.path());
            }
        } else if ft.map(|t| t.is_dir()).unwrap_or(false) {
            match_segs(&e.path(), segs, idx + 1, out);
        }
    }
}

/// One glob's matches under `root` (across every reachable base dir).
fn glob_one(root: &Path, name: &str) -> Vec<PathBuf> {
    let segs: Vec<&str> = name.split('/').collect();
    let mut out = Vec::new();
    for base in reachable_dirs(root) {
        match_segs(&base, &segs, 0, &mut out);
    }
    out
}

/// `(path, contents)` for every artifact an agent plausibly authored under `root` (deduped, in
/// glob/traversal order). Unreadable files are skipped.
#[must_use]
pub fn produced_files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for name in ARTIFACT_GLOBS {
        for f in glob_one(root, name) {
            let key = f.to_string_lossy().into_owned();
            if seen.contains(&key) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&f) {
                seen.insert(key.clone());
                out.push((key, text));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn wildcard_and_dot_guard() {
        assert!(seg_matches("*persona.md", "release-manager.persona.md"));
        assert!(seg_matches("CLAUDE.md", "CLAUDE.md"));
        assert!(seg_matches("*.tests.json", "x.tests.json"));
        assert!(!seg_matches("*persona.md", ".hidden.persona.md")); // * doesn't match leading dot
        assert!(seg_matches(".claude", ".claude")); // literal dot segment matches
        assert!(!seg_matches("CLAUDE.md", "notCLAUDE.md"));
    }

    #[test]
    fn finds_artifacts_and_prunes_heavy_dirs() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path();
        fs::write(root.join("worker.persona.md"), "p").unwrap();
        // The user's OWN project CLAUDE.md (root + nested) must NOT be treated as an agent artifact.
        fs::write(root.join("CLAUDE.md"), "user project file").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/CLAUDE.md"), "c").unwrap();
        fs::create_dir_all(root.join(".claude/agents")).unwrap();
        fs::write(root.join(".claude/agents/a.md"), "a").unwrap();
        // heavy dir that must be pruned (contains a same-named artifact that must NOT be found)
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/x.persona.md"), "nope").unwrap();

        let found = produced_files(root);
        let names: Vec<String> = found.iter().map(|(p, _)| p.clone()).collect();
        assert!(names.iter().any(|p| p.ends_with("worker.persona.md")));
        assert!(
            !names.iter().any(|p| p.ends_with("CLAUDE.md")),
            "a project CLAUDE.md must NOT be collected as an agent artifact: {names:?}"
        );
        assert!(names
            .iter()
            .any(|p| p.replace('\\', "/").ends_with(".claude/agents/a.md")));
        assert!(
            !names.iter().any(|p| p.contains("node_modules")),
            "node_modules must be pruned: {names:?}"
        );
    }
}
