//! Small filesystem + JSON helpers shared by the installer subcommands.

use serde_json::Value;
use std::path::Path;

/// Read a file, trimming trailing whitespace (parity with the Node `read()` = Python `str.rstrip()`).
#[must_use]
pub fn read_rstrip(p: &Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim_end().to_string())
}

/// Read a file as text.
#[must_use]
pub fn read_text(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// Write text, creating parent dirs.
///
/// # Errors
/// Returns any I/O error from creating the parent directory or writing the file.
pub fn write_text(p: &Path, s: &str) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, s)
}

/// Read + parse a JSON file, or `None` on any error.
#[must_use]
pub fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Pretty JSON (2-space indent) + trailing newline, matching `JSON.stringify(obj, null, 2) + "\n"`.
#[must_use]
pub fn json_pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

/// Recursively copy `src` into `dst` (like `shutil.copytree(dirs_exist_ok=True)`), skipping
/// `__pycache__` dirs and `*.pyc` files.
///
/// # Errors
/// Returns the first I/O error encountered while reading a dir entry or copying a file.
pub fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let ns = name.to_string_lossy();
        if ns == "__pycache__" || ns.ends_with(".pyc") {
            continue;
        }
        let sp = entry.path();
        let dp = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&sp, &dp)?;
        } else {
            std::fs::copy(&sp, &dp)?;
        }
    }
    Ok(())
}

/// Print stderr + exit 1 (installer fatal error, matching the Node `fail()`).
pub fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copytree_copies_and_skips_pycache() {
        let td = tempfile::tempdir().unwrap();
        let src = td.path().join("src");
        let dst = td.path().join("dst");
        std::fs::create_dir_all(src.join("sub/__pycache__")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        std::fs::write(src.join("sub/b.txt"), "b").unwrap();
        std::fs::write(src.join("sub/__pycache__/c.pyc"), "c").unwrap();
        std::fs::write(src.join("d.pyc"), "d").unwrap();
        copy_tree(&src, &dst).unwrap();
        assert!(dst.join("a.txt").is_file());
        assert!(dst.join("sub/b.txt").is_file());
        assert!(!dst.join("sub/__pycache__").exists());
        assert!(!dst.join("d.pyc").exists());
    }

    #[test]
    fn read_rstrip_trims_trailing() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("x");
        std::fs::write(&p, "hello\n\n  \n").unwrap();
        assert_eq!(read_rstrip(&p).unwrap(), "hello");
    }
}
