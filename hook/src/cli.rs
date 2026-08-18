//! Tiny argv helpers shared by the hook subcommands.

/// Extract `--<name> <value>` from `args`, returning the value (last wins) and the remaining args
/// with that option removed. Used for `--expertise <dir>` (the manifest/required.json root, which
/// the Node hooks derived from `__dirname` but the Rust binary receives explicitly from hooks.json).
#[must_use]
pub fn take_option(args: &[String], name: &str) -> (Option<String>, Vec<String>) {
    let mut val = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            val = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        rest.push(args[i].clone());
        i += 1;
    }
    (val, rest)
}

/// Take the first `n` characters of `s` (char-wise, so multibyte-safe).
#[must_use]
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_option_extracts_and_removes() {
        let a: Vec<String> = ["validate", ".", "--expertise", "/x/expertise", "method"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let (v, rest) = take_option(&a, "--expertise");
        assert_eq!(v.as_deref(), Some("/x/expertise"));
        assert_eq!(rest, vec!["validate", ".", "method"]);
    }

    #[test]
    fn take_chars_is_char_wise() {
        assert_eq!(take_chars("abcdef", 3), "abc");
        assert_eq!(take_chars("é—ok", 2), "é—");
    }
}
