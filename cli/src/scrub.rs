//! Credential scrubbing for the session-copy capture step (port of capture.py / capture.js's scrubber).
//!
//! A matched secret becomes `[REDACTED credential]`; the value itself is NEVER returned or persisted
//! (hard workspace rule). Redacts, in order: caller-supplied exact `known` values (guaranteed), then the
//! same credential shapes the enforcement hooks use, then labelled `key=value` / `key: value` pairs
//! (keeping the label, redacting the value). ASCII-oriented shapes, matching the hooks' semantics.

use regex::{Captures, Regex};
use std::sync::LazyLock;

const REDACTED: &str = "[REDACTED credential]";

/// The credential shapes. `keep_label` marks the labelled `key=value` pattern (2 capture groups): its
/// group 1 (label + delimiter) is kept and only the value is redacted. All others redact the whole match.
struct Pat {
    re: Regex,
    keep_label: bool,
}

static PATTERNS: LazyLock<Vec<Pat>> = LazyLock::new(|| {
    [
        (r"AKIA[0-9A-Z]{16}", false),
        (
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            false,
        ),
        // key=value | key: value | "key": "value" — keep group 1 (label+delim), redact group 2 (value).
        (
            r#"(?i)(\b(?:password|passwd|secret|api[_-]?key|token|authorization|bearer)\b['"]?\s*[:=]\s*['"]?)([^\s'"]{6,})"#,
            true,
        ),
        (r"gh[pousr]_[A-Za-z0-9]{20,}", false), // GitHub tokens
        (r"sk-[A-Za-z0-9]{20,}", false),        // OpenAI-style keys
        (r"xox[baprs]-[A-Za-z0-9-]{10,}", false), // Slack tokens
    ]
    .into_iter()
    // Fail-safe compile (matches the hooks' idiom); these shapes are hand-verified and always compile.
    .filter_map(|(p, keep_label)| Regex::new(p).ok().map(|re| Pat { re, keep_label }))
    .collect()
});

/// Scrub `text`, returning `(scrubbed, n_redacted)`. `known` holds exact secret values to guarantee-redact
/// (each counted once if present, all occurrences replaced), applied before the shape patterns.
#[must_use]
pub fn scrub_text(text: &str, known: &[String]) -> (String, usize) {
    if text.is_empty() {
        return (String::new(), 0);
    }
    let mut out = text.to_string();
    let mut n = 0usize;
    for val in known {
        if !val.is_empty() && out.contains(val.as_str()) {
            out = out.replace(val.as_str(), REDACTED);
            n += 1;
        }
    }
    for pat in PATTERNS.iter() {
        n += pat.re.find_iter(&out).count();
        if pat.keep_label {
            out = pat
                .re
                .replace_all(&out, |caps: &Captures| format!("{}{REDACTED}", &caps[1]))
                .into_owned();
        } else {
            out = pat.re.replace_all(&out, REDACTED).into_owned();
        }
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_shapes_and_counts() {
        let (out, n) = scrub_text("key AKIAABCDEFGHIJKLMNOP here", &[]);
        assert!(out.contains(REDACTED) && !out.contains("AKIAABCDEFGHIJKLMNOP"));
        assert_eq!(n, 1);
    }

    #[test]
    fn keeps_label_redacts_value() {
        let (out, n) = scrub_text("password = hunter2secret", &[]);
        assert!(out.starts_with("password = ") || out.contains("password ="));
        assert!(out.contains(REDACTED) && !out.contains("hunter2secret"));
        assert_eq!(n, 1);
    }

    #[test]
    fn redacts_github_and_slack_and_openai() {
        let sample = "ghp_ABCDEFGHIJKLMNOPQRSTUVWX sk-ABCDEFGHIJKLMNOPQRSTUV xoxb-1234567890-abc";
        let (out, n) = scrub_text(sample, &[]);
        assert!(!out.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWX"));
        assert!(!out.contains("sk-ABCDEFGHIJKLMNOPQRSTUV"));
        assert!(!out.contains("xoxb-1234567890-abc"));
        assert_eq!(n, 3);
    }

    #[test]
    fn redacts_caller_known_secret_once_all_occurrences() {
        let (out, n) = scrub_text(
            "val=topsecretpass and again topsecretpass",
            &["topsecretpass".to_string()],
        );
        assert!(!out.contains("topsecretpass"));
        // known-secret pass counts the value once (both occurrences replaced by that one pass)…
        assert!(n >= 1);
    }

    #[test]
    fn private_key_block_is_scrubbed() {
        let pem =
            "before -----BEGIN RSA PRIVATE KEY-----\nMIIabc\n-----END RSA PRIVATE KEY----- after";
        let (out, n) = scrub_text(pem, &[]);
        assert!(!out.contains("MIIabc") && out.contains("before") && out.contains("after"));
        assert_eq!(n, 1);
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(scrub_text("", &[]), (String::new(), 0));
    }
}
