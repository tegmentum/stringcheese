//! The shared search-algorithm interface.
//!
//! Every single-pattern search algorithm in this crate is a zero-sized unit
//! struct that implements [`SearchAlgorithm`] and [`SinglePatternSearch`].
//! The multi-pattern [`AhoCorasick`] algorithm has its own extended API for
//! reasons documented on that type — it is stateful across many patterns and
//! does not fit the single-pattern trait cleanly.
//!
//! # Trait shape
//!
//! Two traits are split intentionally:
//!
//! * [`SearchAlgorithm`] carries the associated `Prepared` type, the
//!   `prepare` constructor, and the [`descriptor`](SearchAlgorithm::descriptor)
//!   accessor. Any implementer — including a hypothetical multi-pattern
//!   algorithm that does not fit the single-pattern shape — can implement
//!   this trait without committing to the `find` / `find_all` methods.
//! * [`SinglePatternSearch`] adds the `find` and `find_all` methods on top,
//!   and is implemented only by the single-pattern algorithms.
//!
//! This lets [`AhoCorasick`] declare a descriptor via [`SearchAlgorithm`]
//! and expose its multi-pattern-shaped `find_all` as an inherent method,
//! while [`RabinKarp`], [`Kmp`], and [`BoyerMoore`] share the full
//! single-pattern interface.
//!
//! # Match semantics
//!
//! [`Match`] carries a byte offset and a `pattern_index`. For all
//! single-pattern algorithms `pattern_index` is always `0`. For
//! [`AhoCorasick`] the index refers to the pattern's position in the input
//! slice passed to [`AhoCorasick::build`].
//!
//! `find_all` returns matches in ascending `position` order. When two
//! patterns can match at the same position (Aho-Corasick), the order among
//! them is unspecified except that the sequence overall is nondecreasing
//! in `position`. Overlapping matches are included; the algorithms never
//! silently deduplicate.
//!
//! # Empty pattern
//!
//! Every algorithm in this crate defines the empty pattern to match at
//! position `0` exactly once. This mirrors the `memmem`/`strstr` family
//! and avoids the alternative ("matches at every position, including one
//! past the end") that would inflate `find_all` linearly with haystack
//! length for a pathologically uninformative input. Callers who need the
//! alternate semantics should filter or generate positions themselves.
//!
//! [`AhoCorasick`]: crate::AhoCorasick
//! [`AhoCorasick::build`]: crate::AhoCorasick::build
//! [`RabinKarp`]: crate::RabinKarp
//! [`Kmp`]: crate::Kmp
//! [`BoyerMoore`]: crate::BoyerMoore

use comparand_core::AlgorithmDescriptor;

/// A single reported match — a byte offset and the index of the pattern
/// that matched at that offset.
///
/// For the single-pattern algorithms ([`RabinKarp`], [`Kmp`],
/// [`BoyerMoore`]) `pattern_index` is always `0`. For [`AhoCorasick`] the
/// index refers to the pattern's position in the slice passed to
/// [`AhoCorasick::build`].
///
/// [`RabinKarp`]: crate::RabinKarp
/// [`Kmp`]: crate::Kmp
/// [`BoyerMoore`]: crate::BoyerMoore
/// [`AhoCorasick`]: crate::AhoCorasick
/// [`AhoCorasick::build`]: crate::AhoCorasick::build
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Match {
    /// The byte offset in the haystack where the matched pattern begins.
    ///
    /// `position + pattern_length <= haystack.len()` always holds, so a
    /// caller may safely slice `&haystack[m.position..m.position +
    /// pattern_len]` to recover the matched bytes.
    pub position: usize,
    /// For multi-pattern algorithms, the index of the pattern that matched
    /// at this position. Always `0` for the single-pattern algorithms.
    pub pattern_index: usize,
}

impl Match {
    /// Constructs a single-pattern match at the given byte offset.
    #[inline]
    #[must_use]
    pub const fn new(position: usize) -> Self {
        Self {
            position,
            pattern_index: 0,
        }
    }

    /// Constructs a multi-pattern match with an explicit pattern index.
    #[inline]
    #[must_use]
    pub const fn with_pattern(position: usize, pattern_index: usize) -> Self {
        Self {
            position,
            pattern_index,
        }
    }
}

/// The common surface for any search algorithm in this crate.
///
/// Implementers own their preprocessing shape via the associated
/// [`Prepared`](SearchAlgorithm::Prepared) type. A [`prepare`] call
/// consumes a pattern and returns preprocessed state that can be searched
/// against any number of haystacks — the classical
/// prepare-once / query-many contract.
///
/// See [`SinglePatternSearch`] for the single-pattern `find` / `find_all`
/// methods layered on top of this trait.
///
/// [`prepare`]: SearchAlgorithm::prepare
pub trait SearchAlgorithm {
    /// The algorithm's preprocessed state — Rabin-Karp's pattern hash,
    /// KMP's failure function, Boyer-Moore's bad-character table.
    ///
    /// The type is intentionally left to each algorithm to define; the
    /// only contract this trait imposes is that it can be constructed from
    /// a `&[u8]` via [`prepare`](SearchAlgorithm::prepare) and consumed by
    /// this algorithm's own methods.
    type Prepared;

    /// Preprocesses `pattern` and returns the algorithm's preparation
    /// state.
    ///
    /// The returned value is opaque; downstream callers hand it back into
    /// [`SinglePatternSearch::find`] or
    /// [`SinglePatternSearch::find_all`] rather than inspecting it. Reuse
    /// the prepared value across haystacks — that is the whole point of
    /// separating `prepare` from the search itself.
    fn prepare(pattern: &[u8]) -> Self::Prepared;

    /// The algorithm's descriptor, pinning its family, variant, and source.
    ///
    /// Golden test cases reference algorithms by this descriptor rather
    /// than by common name, so a future descriptor bump cannot silently
    /// invalidate an older golden case.
    fn descriptor() -> AlgorithmDescriptor;
}

/// The single-pattern search surface, layered on top of [`SearchAlgorithm`].
///
/// Implemented by [`RabinKarp`], [`Kmp`], and [`BoyerMoore`]. The
/// multi-pattern [`AhoCorasick`] does not implement this trait — it has
/// its own inherent [`AhoCorasick::find_all`] because its `Prepared` state
/// carries many patterns rather than one.
///
/// # Overlap
///
/// [`find_all`](SinglePatternSearch::find_all) reports overlapping matches.
/// For example, searching for `b"aa"` in `b"aaaa"` returns three matches
/// at positions 0, 1, and 2.
///
/// # Order
///
/// `find_all` returns matches in ascending `position` order.
///
/// [`RabinKarp`]: crate::RabinKarp
/// [`Kmp`]: crate::Kmp
/// [`BoyerMoore`]: crate::BoyerMoore
/// [`AhoCorasick`]: crate::AhoCorasick
/// [`AhoCorasick::find_all`]: crate::AhoCorasick::find_all
#[cfg(feature = "alloc")]
pub trait SinglePatternSearch: SearchAlgorithm {
    /// Finds the first match of the prepared pattern in `haystack`, if any.
    ///
    /// Returns the leftmost match — for [`Kmp`] and [`BoyerMoore`] this
    /// is trivially the first match encountered by the scan; for
    /// [`RabinKarp`] the same guarantee holds because the rolling-hash
    /// scan progresses left-to-right and verifies matches immediately.
    ///
    /// [`Kmp`]: crate::Kmp
    /// [`BoyerMoore`]: crate::BoyerMoore
    /// [`RabinKarp`]: crate::RabinKarp
    fn find(prepared: &Self::Prepared, haystack: &[u8]) -> Option<Match>;

    /// Finds every match of the prepared pattern in `haystack`, in
    /// ascending `position` order, including overlapping matches.
    ///
    /// Overlapping matches are the default. Callers who want only
    /// non-overlapping matches should either use [`find`] repeatedly
    /// (advancing past `pattern.len()` bytes after each match) or filter
    /// the returned vector.
    ///
    /// [`find`]: SinglePatternSearch::find
    fn find_all(prepared: &Self::Prepared, haystack: &[u8]) -> alloc::vec::Vec<Match>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_constructors_are_const() {
        const M: Match = Match::new(7);
        const N: Match = Match::with_pattern(3, 2);
        assert_eq!(M.position, 7);
        assert_eq!(M.pattern_index, 0);
        assert_eq!(N.position, 3);
        assert_eq!(N.pattern_index, 2);
    }
}
