//! Boyer-Moore substring search over `&[u8]`.
//!
//! This module ships two related handles that share the family
//! [`AlgorithmFamily::BoyerMoore`] but expose distinct descriptor
//! variants:
//!
//! * [`BoyerMoore`] — the classical algorithm with only the bad-character
//!   heuristic. Variant slug `"bad-character-only"`.
//! * [`BoyerMooreFull`] — the full algorithm with both the bad-character
//!   and the good-suffix heuristics. Variant slug `"full-with-good-suffix"`.
//!
//! Both types produce **identical match sets** on every input — the
//! good-suffix heuristic only changes performance, never correctness. A
//! proptest in the crate-internal property suite pins this equivalence.
//!
//! # Shape choice
//!
//! Two distinct zero-sized handles (rather than one type parameterized by
//! a `BoyerMooreVariant` enum) are used deliberately:
//!
//! * The existing `BoyerMoore` descriptor and behavior stay exactly as
//!   published in the previous release — this crate's addition of
//!   `BoyerMooreFull` is a strictly additive semver-compatible change.
//! * Each handle implements [`SearchAlgorithm`] and
//!   [`SinglePatternSearch`] directly, with no per-call variant match.
//! * Descriptor accessors stay `const` and pin the variant slug at compile
//!   time.
//!
//! # Algorithm
//!
//! Boyer-Moore aligns the pattern with the haystack and scans from the
//! *right* end of the pattern. On a mismatch at pattern index `j` against
//! haystack byte `b`, the bad-character rule shifts the pattern so that the
//! rightmost occurrence of `b` in `pattern[..j]` lines up with the
//! haystack position. If `b` does not occur to the left of `j`, the
//! pattern is shifted entirely past that byte.
//!
//! Concretely: for each byte value `c ∈ [0, 256)` the preprocessing table
//! `last_occurrence[c]` records the largest index `i` such that
//! `pattern[i] == c`, or `-1` if `c` does not occur. On a mismatch at
//! pattern index `j` against haystack byte `b`, the bad-character shift is
//! `max(1, j - last_occurrence[b])`.
//!
//! The **good-suffix** heuristic contributes an independent shift derived
//! from the matched *suffix* rather than the mismatched byte. Two cases
//! feed the shift table:
//!
//! 1. If a suffix of the pattern that already matched occurs again earlier
//!    in the pattern as a *non-suffix substring*, shift so that occurrence
//!    aligns with the matched suffix.
//! 2. Otherwise, shift by the length of the longest prefix of the pattern
//!    that is also a suffix of the matched suffix.
//!
//! The full Boyer-Moore shift is `max(bad_char_shift, good_suffix_shift)`.
//! Preprocessing follows Cormen §32.4 / the "Handbook of Exact
//! String-Matching Algorithms" presentation of Boyer-Moore.
//!
//! # Complexity
//!
//! The bad-character variant is `O(n · m)` worst-case (contrived patterns
//! like `b"aaaa"` in `b"aaaa...aab"` defeat the heuristic), but sublinear
//! in practice on natural text with a large alphabet. The full variant
//! (with good-suffix) is `O(n)` worst-case when applied to non-overlapping
//! matches per the Galil optimization; the classical formulation used
//! here is `O(n · m)` on the pathological cases but strictly no worse
//! than the bad-character-only variant on any input, and often much
//! better.
//!
//! # Descriptor
//!
//! * [`BoyerMoore::DESCRIPTOR`] pins variant `"bad-character-only"`.
//! * [`BoyerMooreFull::DESCRIPTOR`] pins variant `"full-with-good-suffix"`.
//!
//! Golden test cases reference the descriptor of the specific variant so
//! that a bad-character-only case cannot be silently validated against
//! the full variant, and vice versa.
//!
//! # References
//!
//! * Boyer, R. S., & Moore, J. S. (1977). "A fast string searching
//!   algorithm." *Communications of the ACM*, 20(10), 762-772.
//!   <https://doi.org/10.1145/359842.359859>
//! * Galil, Z. (1979). "On improving the worst case running time of the
//!   Boyer-Moore string matching algorithm." *Communications of the ACM*,
//!   22(9), 505-508. <https://doi.org/10.1145/359146.359148> — worst-case
//!   analysis and the linear-time bound for the full good-suffix variant
//!   under the non-overlapping-match convention.
//!
//! [`AlgorithmFamily::BoyerMoore`]: stringcheese_core::AlgorithmFamily::BoyerMoore

use alloc::vec::Vec;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};

/// Boyer-Moore substring search (bad-character heuristic only).
///
/// A zero-sized unit struct that carries the algorithm's descriptor and
/// the `prepare` / `find` / `find_all` methods. See the module documentation
/// for the algorithm and its guarantees.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BoyerMoore;

/// Preprocessed state produced by [`BoyerMoore::prepare`].
///
/// Holds the pattern bytes (cloned so the state can outlive the caller's
/// slice) and the bad-character `last_occurrence` table indexed by byte
/// value.
#[derive(Clone, Debug)]
pub struct BoyerMoorePrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// For each byte value, the largest index at which that byte appears
    /// in the pattern, or `-1` (encoded as `isize`) if it does not appear.
    ///
    /// The table is dense — 256 entries — so a byte-value lookup is
    /// `O(1)` with no hashing or bounds check.
    last_occurrence: [isize; 256],
}

impl BoyerMoorePrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the last-occurrence table.
    ///
    /// Exposed for cross-crate verification and inspection; not required
    /// by ordinary callers. A `-1` entry means the byte does not occur in
    /// the pattern.
    #[inline]
    #[must_use]
    pub fn last_occurrence(&self) -> &[isize; 256] {
        &self.last_occurrence
    }
}

impl BoyerMoore {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::BoyerMoore,
        variant: VariantId("bad-character-only"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "A fast string searching algorithm",
            authors: "R. S. Boyer, J. S. Moore",
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

/// Builds the bad-character last-occurrence table.
///
/// Pattern lengths are bounded by `isize::MAX` at the type level (Rust
/// slices cannot exceed that size), so the `usize -> isize` cast used to
/// record positions never wraps.
fn build_last_occurrence(pattern: &[u8]) -> [isize; 256] {
    let mut table = [-1isize; 256];
    for (i, &b) in pattern.iter().enumerate() {
        // Later indices overwrite earlier ones, which is what
        // "rightmost occurrence" means.
        #[allow(
            clippy::cast_possible_wrap,
            reason = "slice indices are bounded by isize::MAX; no wraparound is reachable"
        )]
        let index = i as isize;
        table[b as usize] = index;
    }
    table
}

impl SearchAlgorithm for BoyerMoore {
    type Prepared = BoyerMoorePrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        BoyerMoorePrepared {
            pattern: pattern.to_vec(),
            last_occurrence: build_last_occurrence(pattern),
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// Attempts to match the prepared pattern against `haystack` starting at
/// byte offset `s`, returning `true` on a full match.
///
/// Compares right-to-left so that on a mismatch we know which pattern
/// index we stopped at and can consult the bad-character table.
#[inline]
fn matches_at(prepared: &BoyerMoorePrepared, haystack: &[u8], s: usize) -> Option<usize> {
    let m = prepared.pattern.len();
    let mut j = m;
    while j > 0 {
        j -= 1;
        if prepared.pattern[j] != haystack[s + j] {
            return Some(j);
        }
    }
    None
}

/// Given a mismatch at pattern index `j` against haystack byte `b`,
/// returns the number of positions to shift the pattern (at least 1).
#[inline]
fn bad_character_shift(prepared: &BoyerMoorePrepared, j: usize, b: u8) -> usize {
    let last = prepared.last_occurrence[b as usize];
    // shift = max(1, j - last). `last` may be -1 (byte not in pattern),
    // in which case `j - last = j + 1 > 0`. Slice indices are bounded
    // by isize::MAX so the `usize -> isize` cast never wraps; the
    // subsequent `isize -> usize` cast is guarded by `raw > 0`.
    #[allow(
        clippy::cast_possible_wrap,
        reason = "slice indices are bounded by isize::MAX; no wraparound is reachable"
    )]
    let j_signed = j as isize;
    let raw = j_signed - last;
    if raw > 0 {
        #[allow(
            clippy::cast_sign_loss,
            reason = "guarded by `raw > 0` immediately above"
        )]
        {
            raw as usize
        }
    } else {
        1
    }
}

impl SinglePatternSearch for BoyerMoore {
    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j, b are the standard letters used across every published Boyer-Moore presentation"
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
            match matches_at(prepared, haystack, s) {
                None => return Some(Match::new(s)),
                Some(j) => {
                    let b = haystack[s + j];
                    s += bad_character_shift(prepared, j, b);
                }
            }
        }
        None
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j, b are the standard letters used across every published Boyer-Moore presentation"
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
            match matches_at(prepared, haystack, s) {
                None => {
                    out.push(Match::new(s));
                    // Advance by 1 to catch overlapping matches. The
                    // bad-character rule cannot in general skip further
                    // after a full match without additional preprocessing
                    // (which is exactly what the good-suffix heuristic
                    // provides).
                    s += 1;
                }
                Some(j) => {
                    let b = haystack[s + j];
                    s += bad_character_shift(prepared, j, b);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Full Boyer-Moore with the good-suffix heuristic.
// ---------------------------------------------------------------------------

/// Boyer-Moore substring search with **both** the bad-character and the
/// good-suffix heuristics — the full classical Boyer-Moore algorithm.
///
/// Produces the exact same match set as [`BoyerMoore`] on every input; the
/// good-suffix heuristic only changes performance, never correctness. A
/// crate-internal proptest pins this equivalence.
///
/// See the module documentation for the algorithm; see [`BoyerMoore`] for
/// the bad-character-only variant that shares this module.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BoyerMooreFull;

/// Preprocessed state produced by [`BoyerMooreFull::prepare`].
///
/// Carries the pattern bytes, the bad-character `last_occurrence` table,
/// and the good-suffix shift table. All three feed the per-mismatch shift
/// decision at scan time.
#[derive(Clone, Debug)]
pub struct BoyerMooreFullPrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// The bad-character `last_occurrence` table indexed by byte value —
    /// same shape as the bad-character-only variant.
    last_occurrence: [isize; 256],
    /// The good-suffix shift table. `good_suffix[j]` is the shift to apply
    /// when a mismatch occurs at pattern index `j` and the suffix
    /// `pattern[j+1..]` has already been matched — i.e., how far the
    /// pattern must be shifted so the matched suffix re-aligns with an
    /// earlier occurrence in the pattern (or, failing that, with the
    /// longest border of the pattern that is a proper prefix of that
    /// suffix). Length `pattern.len() + 1`; `good_suffix[pattern.len()]`
    /// is consulted after a full match to advance past it.
    good_suffix: Vec<usize>,
}

impl BoyerMooreFullPrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the bad-character `last_occurrence` table.
    ///
    /// Exposed for cross-crate verification and inspection.
    #[inline]
    #[must_use]
    pub fn last_occurrence(&self) -> &[isize; 256] {
        &self.last_occurrence
    }

    /// Returns the good-suffix shift table.
    ///
    /// Length is `pattern.len() + 1`. Exposed for cross-crate verification
    /// and inspection.
    #[inline]
    #[must_use]
    pub fn good_suffix(&self) -> &[usize] {
        &self.good_suffix
    }
}

impl BoyerMooreFull {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::BoyerMoore,
        variant: VariantId("full-with-good-suffix"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "A fast string searching algorithm",
            authors: "R. S. Boyer, J. S. Moore",
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

/// Builds the good-suffix shift table for a pattern.
///
/// The implementation follows the classical two-phase construction
/// (Cormen §32.4 / Crochemore & Rytter "Text Algorithms"):
///
/// 1. Compute a temporary `border` array by a right-to-left sweep. `border[i]`
///    is the starting position of the widest border of `pattern[i..]` that
///    starts at position `i` or later. This is the analogue of KMP's
///    failure function taken from the right.
/// 2. First pass over `good_suffix`: for each `i`, fill in the shift for
///    the *case-1* condition where the matched suffix has a non-suffix
///    re-occurrence in the pattern.
/// 3. Second pass: fill in any positions left at the default with the
///    *case-2* shift derived from the border array's tail — the longest
///    prefix of `pattern` that is also a suffix of `pattern[i..]`.
fn build_good_suffix(pattern: &[u8]) -> Vec<usize> {
    let m = pattern.len();
    if m == 0 {
        return alloc::vec::Vec::new();
    }
    let mut good_suffix = alloc::vec![m; m + 1];
    let mut border = alloc::vec![0usize; m + 1];

    // Phase 1 — compute `border` by scanning right-to-left. This is the
    // classical "suffix" preprocessing from Cormen §32.4.
    let mut i = m;
    let mut j = m + 1;
    border[i] = j;
    while i > 0 {
        // Extend the previous border while the boundary chars mismatch.
        while j <= m && pattern[i - 1] != pattern[j - 1] {
            if good_suffix[j] == m {
                good_suffix[j] = j - i;
            }
            j = border[j];
        }
        i -= 1;
        j -= 1;
        border[i] = j;
    }

    // Phase 2 — fill any position still at the default. `border[0]` is the
    // starting position of the longest border of the whole pattern; we
    // propagate that shift down to every uninitialized entry.
    let mut j = border[0];
    for (i, slot) in good_suffix.iter_mut().enumerate() {
        if *slot == m {
            *slot = j;
        }
        if i == j {
            j = border[j];
        }
    }

    good_suffix
}

impl SearchAlgorithm for BoyerMooreFull {
    type Prepared = BoyerMooreFullPrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        BoyerMooreFullPrepared {
            pattern: pattern.to_vec(),
            last_occurrence: build_last_occurrence(pattern),
            good_suffix: build_good_suffix(pattern),
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// The full BM shift at a mismatch: `max(bad_char_shift, good_suffix_shift)`.
#[inline]
fn full_shift(prepared: &BoyerMooreFullPrepared, j: usize, b: u8) -> usize {
    // Bad-character shift. Reuses the same rule as the bad-character-only
    // variant; open-coded here so this function does not depend on the
    // other variant's helper.
    #[allow(
        clippy::cast_possible_wrap,
        reason = "slice indices are bounded by isize::MAX; no wraparound is reachable"
    )]
    let j_signed = j as isize;
    let last = prepared.last_occurrence[b as usize];
    let bad = j_signed - last;
    let bad_shift = if bad > 0 {
        #[allow(
            clippy::cast_sign_loss,
            reason = "guarded by `bad > 0` immediately above"
        )]
        {
            bad as usize
        }
    } else {
        1
    };
    // Good-suffix shift, always at least 1 by construction of the table.
    let gs_shift = prepared.good_suffix[j + 1];
    bad_shift.max(gs_shift)
}

impl SinglePatternSearch for BoyerMooreFull {
    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j, b are the standard letters used across every published Boyer-Moore presentation"
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
            let mut j = m;
            while j > 0 && prepared.pattern[j - 1] == haystack[s + j - 1] {
                j -= 1;
            }
            if j == 0 {
                return Some(Match::new(s));
            }
            let b = haystack[s + j - 1];
            s += full_shift(prepared, j - 1, b);
        }
        None
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "m, n, s, j, b are the standard letters used across every published Boyer-Moore presentation"
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
                // Advance by 1 so overlapping matches are reported. The
                // good-suffix shift after a full match (good_suffix[0]) is
                // the correct step for *non-overlapping* matches only;
                // this crate's contract is overlapping matches.
                s += 1;
            } else {
                let b = haystack[s + j - 1];
                s += full_shift(prepared, j - 1, b);
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
        let d = BoyerMoore::descriptor();
        assert_eq!(d.family, AlgorithmFamily::BoyerMoore);
        assert_eq!(d.variant, VariantId("bad-character-only"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1977, .. }
        ));
    }

    #[test]
    fn full_descriptor_pins_variant_and_source() {
        let d = BoyerMooreFull::descriptor();
        assert_eq!(d.family, AlgorithmFamily::BoyerMoore);
        assert_eq!(d.variant, VariantId("full-with-good-suffix"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1977, .. }
        ));
    }

    #[test]
    fn full_descriptor_is_const() {
        const D: AlgorithmDescriptor = BoyerMooreFull::DESCRIPTOR;
        assert_eq!(D.variant.0, "full-with-good-suffix");
    }

    #[test]
    fn full_find_returns_first_match() {
        let p = BoyerMooreFull::prepare(b"abc");
        assert_eq!(BoyerMooreFull::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn full_find_all_overlapping() {
        let p = BoyerMooreFull::prepare(b"aa");
        let matches = BoyerMooreFull::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn full_empty_pattern_matches_at_zero() {
        let p = BoyerMooreFull::prepare(b"");
        assert_eq!(BoyerMooreFull::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(
            BoyerMooreFull::find_all(&p, b"abc"),
            alloc::vec![Match::new(0)]
        );
    }

    #[test]
    fn full_empty_haystack_finds_nothing() {
        let p = BoyerMooreFull::prepare(b"abc");
        assert_eq!(BoyerMooreFull::find(&p, b""), None);
        assert!(BoyerMooreFull::find_all(&p, b"").is_empty());
    }

    #[test]
    fn full_good_suffix_agrees_with_bad_character_on_textbook() {
        // Classic textbook example that exercises the good-suffix path
        // (Boyer-Moore's own paper).
        let full = BoyerMooreFull::prepare(b"ANPANMAN");
        let bad = BoyerMoore::prepare(b"ANPANMAN");
        let haystack: &[u8] = b"WOWANPANMANMANPANMANANPANMAN";
        assert_eq!(
            BoyerMooreFull::find_all(&full, haystack),
            BoyerMoore::find_all(&bad, haystack),
        );
    }

    #[test]
    fn full_good_suffix_table_shape_is_pattern_len_plus_one() {
        let p = BoyerMooreFull::prepare(b"abcab");
        assert_eq!(p.good_suffix().len(), p.pattern().len() + 1);
        // Every entry is a valid non-zero shift.
        for &s in p.good_suffix() {
            assert!(s >= 1, "good-suffix shift must be >= 1, got {s}");
        }
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = BoyerMoore::DESCRIPTOR;
        assert_eq!(D.variant.0, "bad-character-only");
    }

    #[test]
    fn last_occurrence_records_rightmost_index() {
        // For "abcab" the last occurrences are: a -> 3, b -> 4, c -> 2.
        let p = BoyerMoore::prepare(b"abcab");
        let table = p.last_occurrence();
        assert_eq!(table[b'a' as usize], 3);
        assert_eq!(table[b'b' as usize], 4);
        assert_eq!(table[b'c' as usize], 2);
        assert_eq!(table[b'z' as usize], -1);
    }

    #[test]
    fn find_returns_first_match() {
        let p = BoyerMoore::prepare(b"abc");
        assert_eq!(BoyerMoore::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let p = BoyerMoore::prepare(b"xyz");
        assert_eq!(BoyerMoore::find(&p, b"abcabcabc"), None);
    }

    #[test]
    fn find_all_overlapping() {
        let p = BoyerMoore::prepare(b"aa");
        let matches = BoyerMoore::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn large_shift_when_bad_character_absent() {
        // Pattern has no vowels; haystack windows always fail on the
        // last position, and the bad-character shift skips the whole
        // pattern each time.
        let p = BoyerMoore::prepare(b"BCDFGH");
        assert_eq!(BoyerMoore::find(&p, b"aaaaaaBCDFGH"), Some(Match::new(6)));
    }

    #[test]
    fn empty_pattern_matches_at_zero() {
        let p = BoyerMoore::prepare(b"");
        assert_eq!(BoyerMoore::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(BoyerMoore::find_all(&p, b"abc"), alloc::vec![Match::new(0)]);
    }

    #[test]
    fn empty_haystack_finds_nothing() {
        let p = BoyerMoore::prepare(b"abc");
        assert_eq!(BoyerMoore::find(&p, b""), None);
        assert!(BoyerMoore::find_all(&p, b"").is_empty());
    }

    #[test]
    fn pattern_equal_to_haystack_matches_at_zero() {
        let p = BoyerMoore::prepare(b"abc");
        assert_eq!(BoyerMoore::find(&p, b"abc"), Some(Match::new(0)));
    }

    #[test]
    fn pattern_longer_than_haystack_finds_nothing() {
        let p = BoyerMoore::prepare(b"abcdef");
        assert_eq!(BoyerMoore::find(&p, b"abc"), None);
    }
}
