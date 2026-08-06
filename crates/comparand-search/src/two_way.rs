//! Two-way substring search over `&[u8]` (Crochemore & Perrin 1991).
//!
//! # Algorithm
//!
//! The Crochemore-Perrin *two-way* algorithm is one of the few
//! substring-search algorithms with both `O(1)` extra space at
//! preprocessing time and a guaranteed `O(n)` worst-case scan. Unlike
//! Boyer-Moore or KMP it requires no auxiliary tables — only a pair of
//! integers describing a *critical factorization* of the pattern.
//!
//! Preprocessing computes the critical factorization `pattern = u · v`
//! where `|u|` is the length of the longer of the two maximal suffixes
//! of the pattern (under `<=` and `>=` lexicographic orders) and the
//! local period of that factorization equals the global period of the
//! pattern. Search then scans left-to-right within each window, first
//! comparing `v` (the right half) forward and then `u` (the left half)
//! backward. On any mismatch inside `v` the pattern can shift by the
//! matched length; on a mismatch inside `u` the pattern shifts by the
//! critical period.
//!
//! The **periodic** vs **non-periodic** cases are handled separately: in
//! the periodic case, a memory variable records the number of pattern
//! characters known to match at the start of the current alignment so
//! the forward scan can skip them.
//!
//! # References
//!
//! * M. Crochemore and D. Perrin, "Two-way string-matching",
//!   *Journal of the ACM*, 38(3):651-675, 1991.
//! * M. Crochemore, C. Hancart, T. Lecroq, *Algorithms on Strings*,
//!   Cambridge University Press, 2007 — chapter on two-way matching.
//! * glibc's `str-two-way.h` — reference implementation whose structure
//!   this implementation mirrors, adapted to Rust with signed indices in
//!   place of the C code's `SIZE_MAX` sentinel.
//!
//! # Overlapping matches
//!
//! The published algorithm targets non-overlapping matches. This crate's
//! contract is overlapping matches (per the `SinglePatternSearch` trait
//! documentation), so [`TwoWay::find_all`] advances the alignment cursor
//! by exactly `1` after each successful match rather than by the
//! algorithm's larger period-based shift. That preserves the
//! per-comparison bound within each window but reduces the total scan to
//! `O(n · m)` in the worst case for `find_all` — [`TwoWay::find`] retains
//! the `O(n)` guarantee because it stops at the first match.
//!
//! # Descriptor
//!
//! The variant slug is `"crochemore-perrin-1991"`, matching the
//! algorithm's paper.

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};

/// Two-way substring search (Crochemore & Perrin, 1991).
///
/// A zero-sized unit struct that carries the algorithm's descriptor and
/// the `prepare` / `find` / `find_all` methods. See the module documentation
/// for the algorithm and its guarantees.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TwoWay;

/// Preprocessed state produced by [`TwoWay::prepare`].
///
/// Holds the pattern bytes (cloned so the state can outlive the caller's
/// slice), the critical position `l` (the length of the `u` prefix in
/// the factorization `pattern = u · v`), the local period `p` at that
/// factorization, and whether the pattern is periodic in the sense
/// required by the two-way memory optimization.
#[derive(Clone, Debug)]
pub struct TwoWayPrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// Critical position `l = |u|` in the factorization `pattern = u · v`.
    critical: usize,
    /// Local period `p` at the critical factorization.
    period: usize,
    /// Whether `pattern[..l]` matches `pattern[p..p + l]`.
    ///
    /// When `true`, the algorithm's periodic branch runs; the search
    /// uses a memory variable to record prior partial matches. When
    /// `false`, the non-periodic branch runs with a period widened to
    /// `max(l, m - l) + 1` — the classical two-way "adjust period"
    /// trick — and no memory is needed.
    periodic: bool,
}

impl TwoWayPrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the critical position `l` in the factorization
    /// `pattern = u · v`. `|u| = l`, `|v| = pattern.len() - l`.
    #[inline]
    #[must_use]
    pub fn critical(&self) -> usize {
        self.critical
    }

    /// Returns the local period of the critical factorization.
    #[inline]
    #[must_use]
    pub fn period(&self) -> usize {
        self.period
    }

    /// Returns whether the periodic branch of the algorithm applies.
    ///
    /// See the type-level documentation for the exact meaning.
    #[inline]
    #[must_use]
    pub fn is_periodic(&self) -> bool {
        self.periodic
    }
}

impl TwoWay {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::TwoWaySearch,
        variant: VariantId("crochemore-perrin-1991"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Two-way string-matching",
            authors: "M. Crochemore, D. Perrin",
            year: 1991,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// Computes the maximal suffix of `pattern` under a lexicographic order
/// determined by `less_than`.
///
/// Returns `(l, p)` where the maximal suffix starts at position `l`
/// (so `|u| = l`) and has local period `p`.
///
/// The two calls with `less_than = true` and `less_than = false` produce
/// the two maximal suffixes needed for critical factorization. The
/// critical position is the max of the two returned `l` values; the
/// period is that call's `p`.
///
/// # Notes on signed arithmetic
///
/// The classical formulation uses `-1` as the initial "empty maximal
/// suffix" sentinel. Rust doesn't have SIZE_MAX-style wrap for `usize`,
/// so `ms_pos` is kept as `isize` throughout to make the code faithful
/// to the reference. Slice indices are still bounded by `isize::MAX`,
/// so every subsequent `usize` cast fits without loss.
#[allow(
    clippy::many_single_char_names,
    reason = "m, l, a, b are the letters used across every published two-way / critical-factorization presentation"
)]
fn maximal_suffix(pattern: &[u8], less_than: bool) -> (usize, usize) {
    let m = pattern.len();
    if m == 0 {
        return (0, 0);
    }

    // `ms_pos` = index just before the start of the current best maximal
    // suffix. The "starting position" is `ms_pos + 1`; initially -1
    // encodes "the empty suffix", so the starting position is 0.
    let mut ms_pos: isize = -1;
    let mut probe: isize = 0;
    let mut offset: isize = 1;
    let mut period_isize: isize = 1;

    #[allow(
        clippy::cast_possible_wrap,
        reason = "pattern length is bounded by isize::MAX for any real slice; the cast cannot wrap"
    )]
    let m_signed = m as isize;

    while probe + offset < m_signed {
        // ms_pos + offset, where ms_pos >= -1 and offset >= 1, so
        // (ms_pos + offset) >= 0.
        #[allow(
            clippy::cast_sign_loss,
            reason = "probe + offset is non-negative by invariant"
        )]
        let a = pattern[(probe + offset) as usize];
        #[allow(
            clippy::cast_sign_loss,
            reason = "ms_pos + offset is non-negative by invariant (ms_pos >= -1, offset >= 1)"
        )]
        let b = pattern[(ms_pos + offset) as usize];

        let a_before_b = if less_than { a < b } else { a > b };

        if a_before_b {
            // The candidate suffix at probe is smaller (or larger, if
            // `less_than == false`). Slide the window forward and
            // record the current period.
            probe += offset;
            offset = 1;
            period_isize = probe - ms_pos;
        } else if a == b {
            if offset == period_isize {
                probe += period_isize;
                offset = 1;
            } else {
                offset += 1;
            }
        } else {
            // Found a strictly-better candidate — restart tracking from
            // position `probe`.
            ms_pos = probe;
            probe = ms_pos + 1;
            offset = 1;
            period_isize = 1;
        }
    }

    // `ms_pos + 1` is the actual starting position of the maximal
    // suffix; that is the `l` we return (i.e., `|u|`).
    #[allow(
        clippy::cast_sign_loss,
        reason = "ms_pos >= -1, so ms_pos + 1 >= 0 and fits in usize"
    )]
    let l = (ms_pos + 1) as usize;
    #[allow(
        clippy::cast_sign_loss,
        reason = "period_isize >= 1 throughout by construction"
    )]
    let period = period_isize as usize;
    (l, period)
}

impl SearchAlgorithm for TwoWay {
    type Prepared = TwoWayPrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        let m = pattern.len();
        if m == 0 {
            return TwoWayPrepared {
                pattern: Vec::new(),
                critical: 0,
                period: 0,
                periodic: false,
            };
        }

        // Compute both maximal suffixes; the critical position is the
        // longer prefix (i.e. the max `|u|`).
        let (lex_lo_start, lex_lo_period) = maximal_suffix(pattern, true);
        let (lex_hi_start, lex_hi_period) = maximal_suffix(pattern, false);
        let (critical, period) = if lex_lo_start > lex_hi_start {
            (lex_lo_start, lex_lo_period)
        } else {
            (lex_hi_start, lex_hi_period)
        };

        // Periodic iff the u-prefix matches the period-shifted region.
        // For that check to be meaningful the period must fit inside the
        // pattern and the prefix must extend past its shift.
        let periodic =
            critical <= m / 2 && pattern[..critical] == pattern[period..period + critical];

        TwoWayPrepared {
            pattern: pattern.to_vec(),
            critical,
            period,
            periodic,
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

/// Scans forward from position `pos`, looking for the first match.
///
/// Returns `Some(match_start)` on success, `None` if no match exists at
/// `pos` or later. Advances `pos` past unsuccessful alignments.
///
/// This is the two-way inner loop; the periodic and non-periodic
/// branches are handled explicitly.
#[allow(
    clippy::many_single_char_names,
    reason = "l, m, n, p, j, i are standard letters in the two-way presentation"
)]
fn scan_from(prepared: &TwoWayPrepared, haystack: &[u8], mut pos: usize) -> Option<usize> {
    let m = prepared.pattern.len();
    let n = haystack.len();
    let l = prepared.critical;

    if prepared.periodic {
        let p = prepared.period;
        // `memory` = number of leading pattern characters already known
        // to match at the current alignment; the forward scan skips them.
        let mut memory: usize = 0;
        while pos + m <= n {
            // Forward scan over v (indices >= l), skipping the memory prefix.
            let start = l.max(memory);
            let mut i = start;
            while i < m && prepared.pattern[i] == haystack[pos + i] {
                i += 1;
            }
            if i < m {
                // Mismatch inside v: shift by (i - l + 1) — the matched
                // length plus one — and drop memory.
                pos += i - l + 1;
                memory = 0;
                continue;
            }
            // v matched. Now check u backward.
            let mut j = l;
            // The prefix pattern[..memory] is known to match; only
            // pattern[memory..l] needs to be checked.
            while j > memory && prepared.pattern[j - 1] == haystack[pos + j - 1] {
                j -= 1;
            }
            if j <= memory {
                return Some(pos);
            }
            // Mismatch inside u: shift by the pattern's period, retain
            // memory of the m - p already-matched suffix.
            pos += p;
            memory = m - p;
        }
    } else {
        // Non-periodic case: widen the period so shifting cannot skip a
        // match, and never use memory.
        let period_effective = l.max(m - l) + 1;
        while pos + m <= n {
            let mut i = l;
            while i < m && prepared.pattern[i] == haystack[pos + i] {
                i += 1;
            }
            if i < m {
                pos += i - l + 1;
                continue;
            }
            // v matched; check u.
            let mut j = l;
            while j > 0 && prepared.pattern[j - 1] == haystack[pos + j - 1] {
                j -= 1;
            }
            if j == 0 {
                return Some(pos);
            }
            pos += period_effective;
        }
    }
    None
}

impl SinglePatternSearch for TwoWay {
    fn find(prepared: &Self::Prepared, haystack: &[u8]) -> Option<Match> {
        let m = prepared.pattern.len();
        if m == 0 {
            return Some(Match::new(0));
        }
        if haystack.len() < m {
            return None;
        }
        scan_from(prepared, haystack, 0).map(Match::new)
    }

    fn find_all(prepared: &Self::Prepared, haystack: &[u8]) -> Vec<Match> {
        let m = prepared.pattern.len();
        if m == 0 {
            return alloc::vec![Match::new(0)];
        }
        let mut out = Vec::new();
        if haystack.len() < m {
            return out;
        }
        let mut pos = 0;
        // Overlap: after each match, advance by 1 rather than by the
        // algorithm's period-based shift. See the module documentation
        // for the correctness / complexity trade-off.
        while let Some(m_start) = scan_from(prepared, haystack, pos) {
            out.push(Match::new(m_start));
            pos = m_start + 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_variant_and_source() {
        let d = TwoWay::descriptor();
        assert_eq!(d.family, AlgorithmFamily::TwoWaySearch);
        assert_eq!(d.variant, VariantId("crochemore-perrin-1991"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1991, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = TwoWay::DESCRIPTOR;
        assert_eq!(D.variant.0, "crochemore-perrin-1991");
    }

    #[test]
    fn find_returns_first_match() {
        let p = TwoWay::prepare(b"abc");
        assert_eq!(TwoWay::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let p = TwoWay::prepare(b"xyz");
        assert_eq!(TwoWay::find(&p, b"abcabcabc"), None);
    }

    #[test]
    fn find_all_overlapping() {
        let p = TwoWay::prepare(b"aa");
        let matches = TwoWay::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn find_all_periodic_pattern() {
        let p = TwoWay::prepare(b"abab");
        let matches = TwoWay::find_all(&p, b"ababab");
        assert_eq!(matches, alloc::vec![Match::new(0), Match::new(2)]);
    }

    #[test]
    fn empty_pattern_matches_at_zero() {
        let p = TwoWay::prepare(b"");
        assert_eq!(TwoWay::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(TwoWay::find_all(&p, b"abc"), alloc::vec![Match::new(0)]);
    }

    #[test]
    fn empty_haystack_finds_nothing() {
        let p = TwoWay::prepare(b"abc");
        assert_eq!(TwoWay::find(&p, b""), None);
        assert!(TwoWay::find_all(&p, b"").is_empty());
    }

    #[test]
    fn pattern_equal_to_haystack_matches_at_zero() {
        let p = TwoWay::prepare(b"abc");
        assert_eq!(TwoWay::find(&p, b"abc"), Some(Match::new(0)));
    }

    #[test]
    fn pattern_longer_than_haystack_finds_nothing() {
        let p = TwoWay::prepare(b"abcdef");
        assert_eq!(TwoWay::find(&p, b"abc"), None);
    }

    #[test]
    fn critical_factorization_periodic_example() {
        // For "abcabc" the critical position is 3 and the period is 3;
        // pattern is periodic.
        let p = TwoWay::prepare(b"abcabc");
        assert!(p.is_periodic());
        assert_eq!(p.period(), 3);
    }

    #[test]
    fn critical_factorization_non_periodic_example() {
        // "abcdef" has period equal to its length — non-periodic.
        let p = TwoWay::prepare(b"abcdef");
        assert!(!p.is_periodic());
    }

    #[test]
    fn single_byte_pattern() {
        let p = TwoWay::prepare(b"a");
        assert_eq!(
            TwoWay::find_all(&p, b"aXaXaa"),
            alloc::vec![Match::new(0), Match::new(2), Match::new(4), Match::new(5),]
        );
    }

    #[test]
    fn textbook_periodic_pattern() {
        // A well-known worst-case pattern for naive algorithms: two-way
        // handles it linearly.
        let p = TwoWay::prepare(b"aaaaab");
        assert_eq!(TwoWay::find(&p, b"aaaaaaaaaab"), Some(Match::new(5)));
    }

    #[test]
    fn utf8_multibyte_pattern() {
        // "café" — the é is two bytes; byte-level search returns exactly
        // one match because UTF-8 is prefix-free.
        let pattern: &[u8] = &[0x63, 0x61, 0x66, 0xC3, 0xA9];
        let haystack: &[u8] = &[
            0x63, 0x61, 0x66, 0xC3, 0xA9, 0x20, 0x6C, 0x61, 0x74, 0x74, 0x65,
        ];
        let p = TwoWay::prepare(pattern);
        assert_eq!(TwoWay::find(&p, haystack), Some(Match::new(0)));
    }
}
