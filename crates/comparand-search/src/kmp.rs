//! Knuth-Morris-Pratt (KMP) substring search over `&[u8]`.
//!
//! # Algorithm
//!
//! KMP preprocesses the pattern into a *failure function* — a table where
//! `failure[i]` is the length of the longest proper prefix of
//! `pattern[..=i]` that is also a suffix. On a mismatch at pattern index
//! `j > 0` the algorithm shifts the pattern by `j − failure[j - 1]` and
//! continues the comparison; this guarantees that no haystack byte is ever
//! re-examined, giving `O(n + m)` worst-case time regardless of the
//! alphabet or pattern shape.
//!
//! # Failure function
//!
//! The failure function is computed in `O(m)` time by a self-referential
//! sweep: for each new pattern character, follow failure links until either
//! the character matches the next-to-extend prefix or the link chain
//! bottoms out. Every increment of `i` costs `O(1)` amortized; every fall
//! along a failure link decreases `k`, and `k` is only ever incremented
//! `O(m)` times overall.
//!
//! # Descriptor
//!
//! The variant slug is `"classic-1977"`, matching the algorithm as
//! published in the original Knuth-Morris-Pratt paper. Golden test cases
//! reference this variant so that a future implementation with a
//! restructured failure function or a different empty-pattern policy
//! cannot be silently validated against these cases.
//!
//! # References
//!
//! * Knuth, D. E., Morris, J. H., & Pratt, V. R. (1977). "Fast pattern
//!   matching in strings." *SIAM Journal on Computing*, 6(2), 323-350.
//!   <https://doi.org/10.1137/0206024>

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};

/// Knuth-Morris-Pratt substring search.
///
/// A zero-sized unit struct that carries the algorithm's descriptor and
/// the `prepare` / `find` / `find_all` methods. See the module documentation
/// for the algorithm and its guarantees.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Kmp;

/// Preprocessed state produced by [`Kmp::prepare`].
///
/// Holds the pattern bytes (cloned so the state can outlive the caller's
/// slice) and the failure function.
#[derive(Clone, Debug)]
pub struct KmpPrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// The failure function: `failure[i]` is the length of the longest
    /// proper prefix of `pattern[..=i]` that is also a suffix.
    failure: Vec<usize>,
}

impl KmpPrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the failure function.
    ///
    /// Exposed for cross-crate verification and inspection; not required
    /// by ordinary callers.
    #[inline]
    #[must_use]
    pub fn failure(&self) -> &[usize] {
        &self.failure
    }
}

impl Kmp {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::KnuthMorrisPratt,
        variant: VariantId("classic-1977"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Fast pattern matching in strings",
            authors: "D. E. Knuth, J. H. Morris, V. R. Pratt",
            year: 1977,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// Builds the classical KMP failure function.
///
/// `failure[i]` is the length of the longest proper prefix of
/// `pattern[..=i]` that is also a suffix. `failure[0]` is always `0`.
fn build_failure(pattern: &[u8]) -> Vec<usize> {
    let m = pattern.len();
    let mut failure = alloc::vec![0usize; m];
    if m <= 1 {
        return failure;
    }
    let mut k: usize = 0;
    for i in 1..m {
        while k > 0 && pattern[k] != pattern[i] {
            k = failure[k - 1];
        }
        if pattern[k] == pattern[i] {
            k += 1;
        }
        failure[i] = k;
    }
    failure
}

impl SearchAlgorithm for Kmp {
    type Prepared = KmpPrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        let failure = build_failure(pattern);
        KmpPrepared {
            pattern: pattern.to_vec(),
            failure,
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

impl SinglePatternSearch for Kmp {
    fn find(prepared: &Self::Prepared, haystack: &[u8]) -> Option<Match> {
        let pattern = &prepared.pattern;
        let m = pattern.len();
        if m == 0 {
            return Some(Match::new(0));
        }
        let failure = &prepared.failure;

        let mut j: usize = 0; // number of pattern bytes matched so far
        for (i, &b) in haystack.iter().enumerate() {
            while j > 0 && pattern[j] != b {
                j = failure[j - 1];
            }
            if pattern[j] == b {
                j += 1;
            }
            if j == m {
                return Some(Match::new(i + 1 - m));
            }
        }
        None
    }

    fn find_all(prepared: &Self::Prepared, haystack: &[u8]) -> Vec<Match> {
        let pattern = &prepared.pattern;
        let m = pattern.len();
        if m == 0 {
            return alloc::vec![Match::new(0)];
        }
        let failure = &prepared.failure;
        let mut out = Vec::new();

        let mut j: usize = 0;
        for (i, &b) in haystack.iter().enumerate() {
            while j > 0 && pattern[j] != b {
                j = failure[j - 1];
            }
            if pattern[j] == b {
                j += 1;
            }
            if j == m {
                out.push(Match::new(i + 1 - m));
                // Continue searching for overlapping matches by falling
                // along the failure link — this is what makes the
                // "abaab in abaabaab" case report two matches at 0 and 3.
                j = failure[j - 1];
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_variant_and_source() {
        let d = Kmp::descriptor();
        assert_eq!(d.family, AlgorithmFamily::KnuthMorrisPratt);
        assert_eq!(d.variant, VariantId("classic-1977"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1977, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = Kmp::DESCRIPTOR;
        assert_eq!(D.variant.0, "classic-1977");
    }

    #[test]
    fn failure_function_abcabd() {
        // Textbook example: for "abcabd" the failure function is
        // [0, 0, 0, 1, 2, 0].
        let p = Kmp::prepare(b"abcabd");
        assert_eq!(p.failure(), &[0, 0, 0, 1, 2, 0]);
    }

    #[test]
    fn failure_function_aabaaab() {
        // For "aabaaab": [0, 1, 0, 1, 2, 2, 3].
        let p = Kmp::prepare(b"aabaaab");
        assert_eq!(p.failure(), &[0, 1, 0, 1, 2, 2, 3]);
    }

    #[test]
    fn find_returns_first_match() {
        let p = Kmp::prepare(b"abc");
        assert_eq!(Kmp::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let p = Kmp::prepare(b"xyz");
        assert_eq!(Kmp::find(&p, b"abcabcabc"), None);
    }

    #[test]
    fn find_all_overlapping() {
        let p = Kmp::prepare(b"aa");
        let matches = Kmp::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn find_all_periodic_pattern() {
        // "abab" in "ababab" matches at positions 0 and 2 — a classical
        // check on the failure-link continuation branch.
        let p = Kmp::prepare(b"abab");
        let matches = Kmp::find_all(&p, b"ababab");
        assert_eq!(matches, alloc::vec![Match::new(0), Match::new(2)]);
    }

    #[test]
    fn empty_pattern_matches_at_zero() {
        let p = Kmp::prepare(b"");
        assert_eq!(Kmp::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(Kmp::find_all(&p, b"abc"), alloc::vec![Match::new(0)]);
    }

    #[test]
    fn empty_haystack_finds_nothing() {
        let p = Kmp::prepare(b"abc");
        assert_eq!(Kmp::find(&p, b""), None);
        assert!(Kmp::find_all(&p, b"").is_empty());
    }

    #[test]
    fn pattern_equal_to_haystack_matches_at_zero() {
        let p = Kmp::prepare(b"abc");
        assert_eq!(Kmp::find(&p, b"abc"), Some(Match::new(0)));
    }

    #[test]
    fn pattern_longer_than_haystack_finds_nothing() {
        let p = Kmp::prepare(b"abcdef");
        assert_eq!(Kmp::find(&p, b"abc"), None);
    }
}
