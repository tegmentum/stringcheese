//! Shannon entropy over Unicode code points.
//!
//! Answers "how random-looking is this text?" in a scale-free way.
//! Useful for detecting compressed / encrypted content
//! (approaches `log2(alphabet_size)`), spotting low-signal input
//! (a run of one character has entropy 0), and gating whether it's
//! worth invoking a heavier downstream operation.
//!
//! ## Unit
//!
//! Code points (Unicode scalars), not bytes. Byte-level entropy is
//! trivial from `text.bytes()`; ambiguity between the two isn't
//! worth silent defaults.

// Bench 2026-08-09 showed BTreeMap<char, u64> degraded per-byte
// throughput 3× from 1 KB to 8 KB inputs — `.entry(c).or_insert(0)`
// per-char cost climbs with map depth as the observed alphabet
// fills. Swapping to hashbrown::HashMap (same crate the histogram
// module already uses) restores O(1) per-char and flattens the
// per-byte scaling curve. Deterministic output for a fixed seed
// still holds — the entropy value is a pure function of frequency
// counts, not of iteration order.
#[cfg(feature = "alloc")]
use hashbrown::HashMap;

/// Shannon entropy in **bits per code point**.
///
/// Formally, `H = -Σ p(c) · log₂ p(c)` for every distinct code
/// point `c` in `text`, where `p(c)` is the code point's empirical
/// frequency. Ranges from 0.0 (single-character input) to
/// `log₂(N)` where `N` is the number of distinct code points.
///
/// Returns 0.0 for the empty string — an empty distribution has no
/// uncertainty.
///
/// # Example
///
/// ```
/// use stringcheese_stats::entropy;
///
/// // Uniform 2-symbol alphabet — entropy is log₂(2) = 1.
/// let e = entropy("abababab");
/// assert!((e - 1.0).abs() < 1e-9);
///
/// // Single-symbol input — no uncertainty.
/// assert_eq!(entropy("aaaa"), 0.0);
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut counts: HashMap<char, u64> = HashMap::new();
    let mut total: u64 = 0;
    for c in text.chars() {
        *counts.entry(c).or_insert(0) += 1;
        total += 1;
    }
    let total_f = total as f64;
    let mut h = 0.0f64;
    for &count in counts.values() {
        let p = count as f64 / total_f;
        // p > 0 by construction — every entry has count >= 1.
        h -= p * p.log2();
    }
    h
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_zero_entropy() {
        assert_eq!(entropy(""), 0.0);
    }

    #[test]
    fn single_character_has_zero_entropy() {
        assert_eq!(entropy("aaaaaa"), 0.0);
        assert_eq!(entropy("a"), 0.0);
    }

    #[test]
    fn uniform_alphabet_matches_log2() {
        // Two symbols, equal frequency → log2(2) = 1.
        let h = entropy("abababab");
        assert!((h - 1.0).abs() < 1e-9, "expected 1.0, got {h}");

        // Four symbols, equal frequency → log2(4) = 2.
        let h4 = entropy("abcdabcd");
        assert!((h4 - 2.0).abs() < 1e-9, "expected 2.0, got {h4}");
    }

    #[test]
    fn skewed_distribution_below_uniform() {
        // Same alphabet size (2), heavily skewed → less than 1 bit.
        let h = entropy("aaaaaaaab");
        assert!(h < 1.0);
        assert!(h > 0.0);
    }

    #[test]
    fn multibyte_scalars_counted_as_one_symbol() {
        // Three distinct Unicode scalars, equal frequency → log2(3).
        let h = entropy("aü日aü日aü日");
        let expected = 3.0_f64.log2();
        assert!((h - expected).abs() < 1e-9, "expected {expected}, got {h}");
    }
}
