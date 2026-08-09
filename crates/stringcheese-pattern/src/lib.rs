//! # StringCheese pattern-matching subsystem
//!
//! One [`Pattern`] trait; four (soon) implementations behind it:
//!
//! - [`Literal`] — exact substring match, accelerated with `memchr`.
//! - [`Wildcard`] — `?` for one code point, `*` for zero-or-more.
//! - [`Glob`] — POSIX-style with character classes (`[abc]`, `[a-z]`,
//!   `[!0-9]`) on top of wildcard semantics.
//! - `Regex` — finite-automata engine landing in a follow-up crate
//!   (`stringcheese-pattern-regex`) that plugs into the same trait.
//!
//! ## Explicit Unicode semantic units
//!
//! Every pattern is anchored to a semantic unit at construction
//! time — [`MatchUnit`] names whether `?` / `.` matches a byte, a
//! code point, or a grapheme cluster. Callers pick; the pattern
//! engine doesn't silently choose a default. Same discipline as
//! everywhere else in StringCheese.
//!
//! ## Uniform result shape
//!
//! Every pattern returns [`Match`]es — byte offset + matched slice
//! + optional capture groups (for engines that support them).
//! Iterators are lazy where the underlying engine allows.
//!
//! ## Example
//!
//! ```
//! use stringcheese_pattern::{Pattern, Literal, MatchUnit};
//!
//! let pat = Literal::new("foo", MatchUnit::Bytes);
//! let hits: Vec<_> = pat.find_iter("foobar-foo-baz").collect();
//! assert_eq!(hits.len(), 2);
//! assert_eq!(hits[0].start, 0);
//! assert_eq!(hits[1].start, 7);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// The wildcard / glob matchers use short single-character bindings
// (`pi`, `hi`, `sp`) that mirror the algorithm's paper notation,
// and cast between byte offsets and code-point values in ways
// clippy's pedantic lints flag as "similar names" and "prefer
// From". Both are noise on algorithmic code that's correct by
// construction; silence them at the crate level rather than
// sprinkle allows through every function.
#![allow(
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::cast_lossless,
    clippy::missing_panics_doc,
    clippy::needless_lifetimes,
    clippy::question_mark,
    clippy::doc_lazy_continuation,
    clippy::doc_markdown,
    clippy::manual_contains
)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub mod glob;
pub mod literal;
pub mod wildcard;

mod glob_engine;

pub use glob::Glob;
pub use literal::Literal;
pub use wildcard::Wildcard;

// ---------------------------------------------------------------------
// Core types — MatchUnit, Match, Pattern
// ---------------------------------------------------------------------

/// The semantic unit `.` / `?` / character-class atoms operate on.
///
/// Chosen at pattern construction time. Byte-level patterns treat
/// each byte as an atom (fastest, ASCII-oriented); code-point-level
/// patterns treat each Unicode scalar as an atom (the sane default
/// for text). Grapheme-level requires a segmenter; today only
/// [`Bytes`](Self::Bytes) and [`CodePoints`](Self::CodePoints) are
/// implemented — [`Graphemes`](Self::Graphemes) is reserved for the
/// follow-up that wires this crate to `stringcheese-segment`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum MatchUnit {
    /// Each byte is an atom. `?` matches one byte; `*` matches any
    /// number of bytes including zero. Slice indexing uses byte
    /// offsets throughout.
    #[default]
    Bytes,

    /// Each Unicode scalar is an atom. `?` matches one scalar; `*`
    /// matches any number of scalars. All returned offsets remain
    /// valid `str::is_char_boundary` positions.
    CodePoints,

    /// Each grapheme cluster is an atom. Requires the segmenter
    /// integration — not implemented yet; a build attempting to use
    /// this variant panics at pattern construction time.
    Graphemes,
}

/// One match of a pattern against a haystack.
///
/// [`start`](Self::start) and [`end`](Self::end) are byte offsets
/// into the original haystack, regardless of the pattern's
/// [`MatchUnit`]. [`matched`](Self::matched) is the substring the
/// pattern actually matched; when the pattern is anchored (e.g.
/// [`Literal`]), this is exactly the pattern text.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Match<'a> {
    /// Start byte offset in the haystack (inclusive).
    pub start: usize,
    /// End byte offset in the haystack (exclusive).
    pub end: usize,
    /// The substring of the haystack this match covers.
    pub matched: &'a str,
}

impl Match<'_> {
    /// Length of the match in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// True when the match is zero-length (some pattern variants
    /// legitimately match an empty span).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// The unified pattern-matching contract.
///
/// Every implementation ([`Literal`], [`Wildcard`], [`Glob`], and
/// the future `Regex`) satisfies this shape. Callers hold
/// `&dyn Pattern` when they want runtime polymorphism, or a
/// concrete type when the pattern kind is known statically.
///
/// Trait is object-safe by construction.
#[cfg(feature = "alloc")]
pub trait Pattern {
    /// True when the pattern matches somewhere in `haystack`. Some
    /// engines can answer this without materialising the full match
    /// list; callers that only need yes/no should prefer this over
    /// `find_iter().next().is_some()`.
    fn is_match(&self, haystack: &str) -> bool {
        self.find(haystack).is_some()
    }

    /// The first match, if any.
    fn find<'h>(&self, haystack: &'h str) -> Option<Match<'h>> {
        self.find_iter(haystack).next()
    }

    /// All matches, left-to-right, non-overlapping.
    fn find_iter<'h>(&self, haystack: &'h str) -> Box<dyn Iterator<Item = Match<'h>> + 'h>;

    /// Replace every match with `replacement`, returning the new
    /// string. Default implementation walks [`find_iter`] and
    /// concatenates.
    ///
    /// [`find_iter`]: Self::find_iter
    fn replace_all(&self, haystack: &str, replacement: &str) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(haystack.len());
        let mut cursor = 0usize;
        for m in self.find_iter(haystack) {
            out.push_str(&haystack[cursor..m.start]);
            out.push_str(replacement);
            cursor = m.end;
        }
        out.push_str(&haystack[cursor..]);
        out
    }

    /// Split `haystack` at every match, returning the between-match
    /// spans. Consecutive matches yield an empty span; a match at
    /// the very start or very end yields empty spans there too
    /// (same semantics as `str::split(pattern)`).
    fn split<'h>(&self, haystack: &'h str) -> Vec<&'h str> {
        let mut out: Vec<&'h str> = Vec::new();
        let mut cursor = 0usize;
        for m in self.find_iter(haystack) {
            out.push(&haystack[cursor..m.start]);
            cursor = m.end;
        }
        out.push(&haystack[cursor..]);
        out
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn match_len_and_empty() {
        let m = Match {
            start: 3,
            end: 8,
            matched: "hello",
        };
        assert_eq!(m.len(), 5);
        assert!(!m.is_empty());

        let empty = Match {
            start: 7,
            end: 7,
            matched: "",
        };
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn match_unit_default_is_bytes() {
        let u: MatchUnit = MatchUnit::default();
        assert_eq!(u, MatchUnit::Bytes);
    }
}
