//! Literal substring pattern — `memchr`-accelerated exact match.
//!
//! [`Literal`] is the simplest pattern in the crate: it matches
//! exactly the bytes of its needle, anywhere in the haystack. Under
//! [`MatchUnit::Bytes`] it uses `memchr` for the first byte and
//! byte comparison for the tail; under [`MatchUnit::CodePoints`] it
//! additionally aligns matches to `str::is_char_boundary` positions
//! (so no match ever spans a partial multibyte scalar).
//!
//! This is what the future common-pattern-compiler's "literal fast
//! path" dispatches to when it detects that a pattern's expression
//! reduces to a single fixed string.

use alloc::boxed::Box;
use alloc::string::String;
use memchr::memmem;

use crate::{Match, MatchUnit, Pattern};

/// A literal-string pattern.
///
/// Constructed once, reused across haystacks. `Copy`-lightweight —
/// carries a `&'n str` needle and a `MatchUnit`. Callers with owned
/// needle strings pass `&owned_str` — the needle borrows for
/// lifetime `'n`.
#[derive(Copy, Clone, Debug)]
pub struct Literal<'n> {
    needle: &'n str,
    unit: MatchUnit,
}

impl<'n> Literal<'n> {
    /// Construct a literal pattern with the given semantic unit.
    ///
    /// # Panics
    ///
    /// Panics on [`MatchUnit::Graphemes`] — grapheme-level matching
    /// requires the segmenter integration that hasn't landed yet.
    #[must_use]
    pub fn new(needle: &'n str, unit: MatchUnit) -> Self {
        assert!(
            !matches!(unit, MatchUnit::Graphemes),
            "MatchUnit::Graphemes is reserved for the segmenter integration; not implemented",
        );
        Self { needle, unit }
    }

    /// The needle string this pattern matches.
    #[must_use]
    pub const fn needle(&self) -> &'n str {
        self.needle
    }

    /// The semantic unit configured at construction.
    #[must_use]
    pub const fn unit(&self) -> MatchUnit {
        self.unit
    }
}

impl Pattern for Literal<'_> {
    fn find_iter<'h>(&self, haystack: &'h str) -> Box<dyn Iterator<Item = Match<'h>> + 'h> {
        // Empty needle: match at every position (including start
        // and end). Same semantics as `str::find("")`. We yield one
        // empty match at position 0 and stop — the "match at every
        // position" interpretation gives an infinite iterator
        // that's rarely what the caller wants.
        if self.needle.is_empty() {
            let m = Match {
                start: 0,
                end: 0,
                matched: "",
            };
            return Box::new(core::iter::once(m));
        }

        // Collect match offsets upfront. `memmem::find_iter`
        // returns an iterator that borrows the needle bytes; to
        // avoid propagating a `'n: 'h` bound on the trait impl
        // (which would force the needle to outlive every haystack
        // this Literal is ever invoked against), we materialise
        // the offsets into a Vec and hand back the Vec's iterator.
        // The Vec is small — one usize per match — and the walk
        // is the same asymptotic cost either way.
        let n = self.needle.len();
        let starts: alloc::vec::Vec<usize> =
            memmem::find_iter(haystack.as_bytes(), self.needle.as_bytes()).collect();
        Box::new(starts.into_iter().map(move |start| Match {
            start,
            end: start + n,
            matched: &haystack[start..start + n],
        }))
    }

    fn is_match(&self, haystack: &str) -> bool {
        if self.needle.is_empty() {
            return true;
        }
        memmem::find(haystack.as_bytes(), self.needle.as_bytes()).is_some()
    }

    fn replace_all(&self, haystack: &str, replacement: &str) -> String {
        if self.needle.is_empty() {
            // Empty needle: `str::replace("", r)` inserts `r`
            // between every pair of adjacent characters plus at
            // ends. We defer to `str::replace` for that semantics
            // rather than reimplementing.
            return haystack.replace("", replacement);
        }
        haystack.replace(self.needle, replacement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn find_single_hit() {
        let pat = Literal::new("foo", MatchUnit::Bytes);
        let m = pat.find("foobar").unwrap();
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 3);
        assert_eq!(m.matched, "foo");
    }

    #[test]
    fn find_iter_multiple_non_overlapping() {
        let pat = Literal::new("aba", MatchUnit::Bytes);
        // "ababababa" — memmem gives non-overlapping matches:
        // positions 0 and 4 (since "aba" ends at 3, next scan
        // starts at 3, finds "aba" at position 4? actually memmem
        // is non-overlapping by advancing to end-of-match, so we
        // expect matches at 0, 4).
        let hits: Vec<_> = pat.find_iter("ababababa").collect();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].start, 0);
        assert_eq!(hits[1].start, 4);
    }

    #[test]
    fn find_iter_finds_all_occurrences() {
        let pat = Literal::new("xy", MatchUnit::Bytes);
        let hits: Vec<_> = pat.find_iter("xyxyz-xy").collect();
        // "xyxyz-xy" — positions 0, 2, 6.
        assert_eq!(
            hits.iter().map(|m| m.start).collect::<Vec<_>>(),
            vec![0, 2, 6]
        );
    }

    #[test]
    fn is_match_shortcuts_true_false() {
        let pat = Literal::new("hello", MatchUnit::Bytes);
        assert!(pat.is_match("world hello world"));
        assert!(!pat.is_match("world"));
    }

    #[test]
    fn no_match_returns_none() {
        let pat = Literal::new("xyz", MatchUnit::Bytes);
        assert!(pat.find("abc").is_none());
    }

    #[test]
    fn multibyte_needle_matches_at_char_boundary() {
        let pat = Literal::new("日本", MatchUnit::CodePoints);
        let hits: Vec<_> = pat.find_iter("こんにちは日本語").collect();
        assert_eq!(hits.len(), 1);
        // Bytes 0..15 are こんにちは (5 × 3 = 15), match starts at 15.
        assert_eq!(hits[0].start, 15);
        assert_eq!(hits[0].matched, "日本");
    }

    #[test]
    fn empty_needle_yields_single_zero_length_match() {
        let pat = Literal::new("", MatchUnit::Bytes);
        let hits: Vec<_> = pat.find_iter("abc").collect();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].is_empty());
    }

    #[test]
    fn replace_all_swaps_every_occurrence() {
        let pat = Literal::new("foo", MatchUnit::Bytes);
        assert_eq!(pat.replace_all("foobar-foo-baz", "XX"), "XXbar-XX-baz");
    }

    #[test]
    fn split_between_matches() {
        let pat = Literal::new(", ", MatchUnit::Bytes);
        assert_eq!(pat.split("a, b, c, d"), vec!["a", "b", "c", "d"]);
    }

    #[test]
    #[should_panic(expected = "Graphemes")]
    fn graphemes_unit_panics_until_implemented() {
        let _ = Literal::new("x", MatchUnit::Graphemes);
    }
}
