//! Cross-system conversation resume (issue #9).
//!
//! Claude Code stores each conversation transcript at `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`
//! — machine-local, not in the repo, so `claude -c` / `--resume` finds nothing after a clone on another
//! system. This module carries the transcript IN the repo (`.genesis/sessions/`) and, on the target,
//! restores it into the machine's project dir (where Claude Code already looks) so native resume works.
//!
//! Verified: encoding replaces each path separator with `-`; the `.jsonl` alone is sufficient to resume.

use std::path::{Path, PathBuf};

/// Claude Code's project-dir name for a working directory: EVERY non-alphanumeric character becomes `-`
/// (not just path separators — also `:`, spaces, `()`, `.`, …), with no run-collapsing. Verified against a
/// real `~/.claude/projects` listing: `/mnt/c/Users/x/proj` -> `-mnt-c-Users-x-proj`,
/// `C:\Users\me\proj` -> `C--Users-me-proj`, `…repo - Copy` -> `…repo---Copy`. Matching this exactly is
/// what lets cross-system resume (issue #9) land the transcript where `claude -c` looks — the old
/// separators-only encoder broke on Windows drive-letters and spaces.
#[must_use]
pub fn encode_project_dir(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The repo's committed transcript store: `<repo>/.genesis/sessions/`.
fn sessions_dir(repo: &Path) -> PathBuf {
    repo.join(".genesis").join("sessions")
}

/// The base name of a path (after the last `/` or `\`).
fn basename(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// Capture: copy the live transcript at `transcript_path` into `<repo>/.genesis/sessions/<basename>` so it
/// travels with the repo. Returns the destination path, or `None` if the source is missing/unreadable
/// (fail-open — capture must never break a session).
#[must_use]
pub fn capture(repo: &Path, transcript_path: &str) -> Option<PathBuf> {
    if transcript_path.is_empty() || !Path::new(transcript_path).is_file() {
        return None;
    }
    let dir = sessions_dir(repo);
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let dest = dir.join(basename(transcript_path));
    std::fs::copy(transcript_path, &dest).ok().map(|_| dest)
}

/// Restore: copy every `<repo>/.genesis/sessions/*.jsonl` into `<home>/.claude/projects/<encode(cwd)>/`
/// (the dir Claude Code reads for `claude -c`), skipping any already present. Returns the restored session
/// ids (the file stems). Fail-open: any per-file error is skipped, never propagated.
#[must_use]
pub fn restore(repo: &Path, home: &Path, cwd: &str) -> Vec<String> {
    let src = sessions_dir(repo);
    let Ok(entries) = std::fs::read_dir(&src) else {
        return Vec::new();
    };
    let target = home
        .join(".claude")
        .join("projects")
        .join(encode_project_dir(cwd));
    if std::fs::create_dir_all(&target).is_err() {
        return Vec::new();
    }
    let mut restored = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(name) = p.file_name() else { continue };
        let dest = target.join(name);
        if dest.exists() {
            continue; // already present — never clobber the machine's own transcript
        }
        if std::fs::copy(&p, &dest).is_ok() {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                restored.push(stem.to_string());
            }
        }
    }
    restored.sort();
    restored
}

/// The user-facing resume notice for restored session ids — injected into the session so the agent relays
/// it. Empty when nothing was restored.
#[must_use]
pub fn resume_notice(ids: &[String]) -> String {
    if ids.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = ids
        .iter()
        .map(|id| format!("  claude --resume {id}"))
        .collect();
    format!(
        "\n\n## Portable session(s) restored (cross-system resume)\nGenesis copied {} conversation \
         transcript(s) from this repo into Claude Code's store on this machine. To continue where you \
         left off on another system, run:\n{}\n(Or `claude -c` for the most recent. Tell the user this.)",
        ids.len(),
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_matches_claude_code_scheme() {
        // Claude Code names its project dir by replacing EVERY non-alphanumeric char with '-' (verified
        // against ~/.claude/projects on a real Windows install): `:`, `\`, `/`, spaces, and `()` each
        // become '-', with NO run-collapsing. The old code only handled `/`+`\`, so Windows paths (drive
        // colon, spaces like " - Copy") landed in the WRONG project dir and cross-system resume failed.
        assert_eq!(
            encode_project_dir("/mnt/c/Users/x/proj"),
            "-mnt-c-Users-x-proj"
        );
        assert_eq!(encode_project_dir("/home/user/p"), "-home-user-p");
        // Windows drive + backslashes: `C:\` -> `C--` (colon AND backslash each become '-').
        assert_eq!(encode_project_dir(r"C:\Users\me\proj"), "C--Users-me-proj");
        // spaces and parens also become '-', not preserved (the actual bug that broke a copied folder).
        assert_eq!(
            encode_project_dir(r"C:\Users\me\repo - Copy"),
            "C--Users-me-repo---Copy"
        );
        assert_eq!(encode_project_dir("New Folder (5)"), "New-Folder--5-");
    }

    #[test]
    fn capture_copies_transcript_into_repo() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let tp = td.path().join("abc-123.jsonl");
        std::fs::write(&tp, "{\"type\":\"user\"}\n").unwrap();
        let dest = capture(&repo, tp.to_str().unwrap()).unwrap();
        assert_eq!(dest, repo.join(".genesis/sessions/abc-123.jsonl"));
        assert!(dest.is_file());
        // missing source -> None (fail-open)
        assert!(capture(&repo, "/no/such.jsonl").is_none());
        assert!(capture(&repo, "").is_none());
    }

    #[test]
    fn restore_places_committed_transcripts_and_skips_existing() {
        let td = tempfile::tempdir().unwrap();
        let repo = td.path().join("repo");
        let home = td.path().join("home");
        let cwd = "/work/proj";
        std::fs::create_dir_all(repo.join(".genesis/sessions")).unwrap();
        std::fs::write(repo.join(".genesis/sessions/s1.jsonl"), "a").unwrap();
        std::fs::write(repo.join(".genesis/sessions/s2.jsonl"), "b").unwrap();
        std::fs::write(repo.join(".genesis/sessions/notes.txt"), "x").unwrap(); // ignored
                                                                                // pre-existing s2 on the target must not be clobbered / re-reported
        let target = home.join(".claude/projects/-work-proj");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("s2.jsonl"), "ORIGINAL").unwrap();

        let ids = restore(&repo, &home, cwd);
        assert_eq!(ids, vec!["s1".to_string()], "only the new one is restored");
        assert!(target.join("s1.jsonl").is_file());
        assert_eq!(
            std::fs::read_to_string(target.join("s2.jsonl")).unwrap(),
            "ORIGINAL",
            "existing transcript is never clobbered"
        );
    }

    #[test]
    fn restore_lands_in_claude_code_dir_for_windows_path() {
        // END-TO-END regression for the cross-system-resume Windows bug: restore must place the transcript
        // in the SAME project dir Claude Code uses (every non-alnum -> '-', incl. the drive colon and the
        // spaces in " - Copy"), or `claude -c` / `--resume` finds nothing. The old encoder produced
        // `C:-...-repo - Copy` and the transcript never landed where Claude Code looks.
        let td = tempfile::tempdir().unwrap();
        let repo = td.path().join("repo");
        let home = td.path().join("home");
        std::fs::create_dir_all(repo.join(".genesis/sessions")).unwrap();
        std::fs::write(repo.join(".genesis/sessions/7d2ff59b.jsonl"), "x").unwrap();
        let cwd = r"C:\Users\me\ifs-repo - Copy";
        let ids = restore(&repo, &home, cwd);
        assert_eq!(ids, vec!["7d2ff59b".to_string()]);
        assert!(
            home.join(".claude/projects/C--Users-me-ifs-repo---Copy/7d2ff59b.jsonl")
                .is_file(),
            "transcript must land in Claude Code's real project dir for a Windows path"
        );
    }

    #[test]
    fn restore_empty_when_no_sessions() {
        let td = tempfile::tempdir().unwrap();
        assert!(restore(&td.path().join("repo"), &td.path().join("home"), "/x").is_empty());
    }

    #[test]
    fn resume_notice_lists_commands_or_empty() {
        assert!(resume_notice(&[]).is_empty());
        let n = resume_notice(&["abc-123".to_string()]);
        assert!(n.contains("claude --resume abc-123"));
        assert!(n.contains("Portable session"));
    }
}
