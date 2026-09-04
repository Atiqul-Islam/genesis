//! Vector skip-detection scorer (feature: cosine warn). The DETERMINISTIC core, no I/O.
//!
//! Given each required rule's distance to the response embedding + the set of rules the agent DECLARED,
//! return the undeclared rules that are among the nearest AND within `margin` of the closest — the
//! "you may have applied these without declaring them" WARNINGS. Warn-only; never blocks. The spike
//! (test/specs/vector-completeness-warn.md) confirmed used-topic rules are the response's nearest
//! neighbours, so a top-k + margin cut over the distances separates skips from noise. Thresholds are
//! calibration items (memory-management mm-26) surfaced as parameters, never hidden constants.

use std::collections::HashSet;
use std::hash::BuildHasher;

/// Undeclared rules among the top-`k` nearest whose distance is within `margin` of the nearest rule's
/// distance — the skip warnings. Deterministic given fixed inputs. Empty inputs / k==0 -> no warnings.
#[must_use]
pub fn skip_warnings<S: BuildHasher>(
    distances: &[(String, f32)],
    declared: &HashSet<String, S>,
    k: usize,
    margin: f32,
) -> Vec<String> {
    if distances.is_empty() || k == 0 {
        return Vec::new();
    }
    let mut sorted: Vec<&(String, f32)> = distances.iter().collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let nearest = sorted[0].1;
    sorted
        .iter()
        .take(k)
        .filter(|(key, dist)| !declared.contains(key) && *dist <= nearest + margin)
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn flags_close_undeclared_within_topk_and_margin() {
        let dists = vec![
            ("tdd-1".to_string(), 0.90f32),
            ("tdd-2".to_string(), 0.95),
            ("mm-1".to_string(), 1.30),
            ("som-3".to_string(), 1.40),
        ];
        // nearest=0.90, margin 0.10 -> within 1.00: tdd-1 (declared, skip), tdd-2 (0.95, undeclared -> WARN);
        // mm-1/som-3 outside the margin -> no warn.
        let w = skip_warnings(&dists, &set(&["tdd-1"]), 3, 0.10);
        assert_eq!(w, vec!["tdd-2".to_string()]);
    }

    #[test]
    fn empty_when_all_close_rules_declared() {
        let dists = vec![("a".to_string(), 0.5f32), ("b".to_string(), 0.55)];
        assert!(skip_warnings(&dists, &set(&["a", "b"]), 5, 0.2).is_empty());
    }

    #[test]
    fn respects_topk_and_margin_bounds() {
        let dists = vec![
            ("a".to_string(), 0.5f32),
            ("b".to_string(), 0.6),
            ("c".to_string(), 0.65),
        ];
        // margin 0 -> only the nearest (a) qualifies, even with a large k.
        assert_eq!(
            skip_warnings(&dists, &set(&[]), 9, 0.0),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn empty_inputs_no_warnings() {
        assert!(skip_warnings(&[], &set(&[]), 3, 0.1).is_empty());
        let dists = vec![("a".to_string(), 0.5f32)];
        assert!(skip_warnings(&dists, &set(&[]), 0, 0.1).is_empty());
    }
}
