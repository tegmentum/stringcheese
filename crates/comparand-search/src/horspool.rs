//! Horspool substring search over `&[u8]`.
//!
//! # Algorithm
//!
//! Horspool is a bad-character-only variant of Boyer-Moore, published by
//! Nigel Horspool in 1980 ("Practical fast searching in strings"). It
//! preserves Boyer-Moore's right-to-left comparison within a window but
//! **always** looks up the bad-character shift on the byte aligned with
//! the *rightmost* pattern position — regardless of where the mismatch
//! actually occurred. That single simplification is the distinguishing
//! feature relative to the classical Boyer-Moore bad-character rule,
//! which consults the mismatched byte's own position.
//!
//! Because the shift is always based on the byte at the current window's
//! last position, Horspool's preprocessing table has a slightly different
//! shape: for each byte value `c ∈ [0, 256)`, `shift[c]` is the amount
//! to slide the pattern forward when `c` appears at the last window
//! position. The value is:
//!
//! * `pattern.len() - 1 - i` for the *last* occurrence of `c` at index
//!   `i` strictly less than `pattern.len() - 1`, or
//! * `pattern.len()` if `c` does not occur in `pattern[..pattern.len() -
//!   1]`.
//!
//! Note that the pattern's **last** byte is deliberately excluded from
//! the table: a shift of `0` would be pathological (the algorithm would
//! not advance), so Horspool uses the *second-to-last* occurrence, or
//! `pattern.len()` if none. This is the classical trick that keeps every
//! shift at least `1`.
//!
//! # Compared to Boyer-Moore
//!
//! Horspool is often preferred in textbooks and reference material for
//! its simpler code: one lookup per window rather than one lookup per
//! mismatch position, and no per-position arithmetic on the mismatched
//! index. The two algorithms are **not** equivalent — Horspool's shifts
//! are sometimes shorter than Boyer-Moore's — but both produce the same
//! match set. The crate-internal differential property suite pins this
//! equivalence.
//!
//! Complexity is `O(n · m)` worst-case (the same pathological patterns
//! that defeat Boyer-Moore's bad-character heuristic defeat Horspool
//! too), but sublinear in practice on natural text.
//!
//! # Descriptor
//!
//! The variant slug is `"classic-1980"`, matching Horspool's paper.

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};

/// Horspool substring search (bad-character-only Boyer-Moore variant).
///
/// A zero-sized unit struct that carries the algorithm's descriptor and
/// the `prepare` / `find` / `find_all` methods. See the module documentation
/// for the algorithm and its guarantees.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Horspool;

/// Preprocessed state produced by [`Horspool::prepare`].
///
/// Holds the pattern bytes (cloned so the state can outlive the caller's
/// slice) and the Horspool bad-character shift table.
#[derive(Clone, Debug)]
pub struct HorspoolPrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// For each byte value, the number of positions to slide the pattern
    /// when that byte sits at the window's rightmost position and the
    /// current window does not fully match.
    ///
    /// The table is dense — 256 entries — so a byte-value lookup is
    /// `O(1)` with no hashing or bounds check.
    shift: [usize; 256],
}

impl HorspoolPrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the bad-character shift table.
    ///
    /// Exposed for cross-crate verification and inspection.
    #[inline]
    #[must_use]
    pub fn shift(&self) -> &[usize; 256] {
        &self.shift
    }
}

impl Horspool {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Horspool,
        variant: VariantId("classic-1980"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Practical fast searching in strings",
            authors: "R. N. Horspool",
            year: 1980,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// Builds the Horspool bad-character shift table.
///
/// Only positions `0..pattern.len() - 1` (i.e. **excluding** the last
/// byte) contribute to the table; the last-byte exclusion is exactly what
/// keeps every produced shift at least `1`.
fn build_shift(pattern: &[u8]) -> [usize; 256] {
    let m = pattern.len();
    // The default shift for a byte that does not occur in `pattern[..m-1]`
    // is the full pattern length: skip the window past that byte entirely.
    let mut table = [m; 256];
    if m == 0 {
        return table;
    }
    for i in 0..m - 1 {
        table[pattern[i] as usize] = m - 1 - i;
    }
    table
}

impl SearchAlgorithm for Horspool {
    type Prepared = HorspoolPrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        HorspoolPrepared {
            pattern: pattern.to_vec(),
            shift: build_shift(pattern),
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

impl SinglePatternSearch for Horspool {
    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j are the standard letters used across every published Boyer-Moore/Horspool presentation"
    )]
    fn find(prepared: &Self::Prepared, haystack: &[u8]) -> Option<Match> {
        let m = prepared.pattern.len();
        if m == 0 {
            return Some(Match::new(0));
        }
        let n = haystack.len();
        if n < m {
            return None;
        }

        let mut s: usize = 0;
        while s <= n - m {
            // Right-to-left compare within the current window.
            let mut j = m;
            while j > 0 && prepared.pattern[j - 1] == haystack[s + j - 1] {
                j -= 1;
            }
            if j == 0 {
                return Some(Match::new(s));
            }
            // Horspool: always shift by the table entry for the byte at
            // the window's rightmost position, regardless of where the
            // mismatch actually was.
            let right_byte = haystack[s + m - 1];
            s += prepared.shift[right_byte as usize];
        }
        None
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j are the standard letters used across every published Boyer-Moore/Horspool presentation"
    )]
    fn find_all(prepared: &Self::Prepared, haystack: &[u8]) -> Vec<Match> {
        let m = prepared.pattern.len();
        if m == 0 {
            return alloc::vec![Match::new(0)];
        }
        let mut out = Vec::new();
        let n = haystack.len();
        if n < m {
            return out;
        }

        let mut s: usize = 0;
        while s <= n - m {
            let mut j = m;
            while j > 0 && prepared.pattern[j - 1] == haystack[s + j - 1] {
                j -= 1;
            }
            if j == 0 {
                out.push(Match::new(s));
                // Advance by 1 to catch overlapping matches. The
                // Horspool shift after a full match would in general
                // skip overlapping occurrences.
                s += 1;
            } else {
                let right_byte = haystack[s + m - 1];
                s += prepared.shift[right_byte as usize];
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
        let d = Horspool::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Horspool);
        assert_eq!(d.variant, VariantId("classic-1980"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1980, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = Horspool::DESCRIPTOR;
        assert_eq!(D.variant.0, "classic-1980");
    }

    #[test]
    fn shift_table_excludes_last_byte() {
        // For "abcab" the table should have:
        //   'a' -> 1 (last occurrence excluding the tail is at index 3, shift = 5-1-3 = 1)
        //   'b' -> 3 (last occurrence excluding the tail is at index 1, shift = 5-1-1 = 3)
        //   'c' -> 2 (only occurrence at index 2, shift = 5-1-2 = 2)
        //   others -> 5 (pattern length)
        let p = Horspool::prepare(b"abcab");
        let t = p.shift();
        assert_eq!(t[b'a' as usize], 1);
        assert_eq!(t[b'b' as usize], 3);
        assert_eq!(t[b'c' as usize], 2);
        assert_eq!(t[b'z' as usize], 5);
    }

    #[test]
    fn every_shift_is_positive() {
        // The last-byte exclusion is what keeps Horspool progressing.
        let p = Horspool::prepare(b"abcab");
        for &s in p.shift() {
            assert!(s >= 1, "Horspool shift must be positive, got {s}");
        }
    }

    #[test]
    fn find_returns_first_match() {
        let p = Horspool::prepare(b"abc");
        assert_eq!(Horspool::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let p = Horspool::prepare(b"xyz");
        assert_eq!(Horspool::find(&p, b"abcabcabc"), None);
    }

    #[test]
    fn find_all_overlapping() {
        let p = Horspool::prepare(b"aa");
        let matches = Horspool::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn large_shift_when_bad_character_absent() {
        let p = Horspool::prepare(b"BCDFGH");
        assert_eq!(Horspool::find(&p, b"aaaaaaBCDFGH"), Some(Match::new(6)));
    }

    #[test]
    fn empty_pattern_matches_at_zero() {
        let p = Horspool::prepare(b"");
        assert_eq!(Horspool::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(Horspool::find_all(&p, b"abc"), alloc::vec![Match::new(0)]);
    }

    #[test]
    fn empty_haystack_finds_nothing() {
        let p = Horspool::prepare(b"abc");
        assert_eq!(Horspool::find(&p, b""), None);
        assert!(Horspool::find_all(&p, b"").is_empty());
    }

    #[test]
    fn pattern_equal_to_haystack_matches_at_zero() {
        let p = Horspool::prepare(b"abc");
        assert_eq!(Horspool::find(&p, b"abc"), Some(Match::new(0)));
    }

    #[test]
    fn pattern_longer_than_haystack_finds_nothing() {
        let p = Horspool::prepare(b"abcdef");
        assert_eq!(Horspool::find(&p, b"abc"), None);
    }

    #[test]
    fn single_byte_pattern() {
        // A one-byte pattern is the degenerate case: the shift table has
        // every entry equal to 1 (pattern.len()), and the algorithm
        // reduces to linear scan.
        let p = Horspool::prepare(b"x");
        assert_eq!(
            Horspool::find_all(&p, b"xxaxxxax"),
            alloc::vec![
                Match::new(0),
                Match::new(1),
                Match::new(3),
                Match::new(4),
                Match::new(5),
                Match::new(7),
            ]
        );
    }
}
