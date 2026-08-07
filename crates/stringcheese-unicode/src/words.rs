//! UAX #29 word segmentation.
//!
//! A **word** in the sense of Unicode Standard Annex #29 is a run of
//! text separated from its neighbours by *word boundaries*. Word
//! boundaries are computed from the Unicode `Word_Break` property of
//! adjacent scalars; the underlying rules span writing systems, so
//! they carry text-shape assumptions that `str::split_whitespace`
//! cannot:
//!
//! - `"don't"` is *one* word, not three: the apostrophe joins its
//!   neighbours rather than acting as a boundary.
//! - `"人々"` (Japanese "people, people") is *two* words even without
//!   any whitespace between them: the iteration mark and the
//!   preceding ideograph belong to distinct word classes.
//! - `"3.14"` is one word: numeric-decimal joins the two runs into a
//!   single word.
//! - Punctuation such as `","` or `"!"` produces a boundary on both
//!   sides — it is *between* words. Whether the punctuation itself
//!   surfaces to the caller is controlled by
//!   [`SplitWordBoundsBehavior`].
//!
//! # Two APIs, one boundary rule
//!
//! `unicode-segmentation` exposes two related iterators, both grounded
//! in the same UAX #29 rules but with different filtering:
//!
//! - [`UnicodeWords`] — yields only the pieces that look like
//!   *actual words* (letters, digits, and joiners). Punctuation and
//!   whitespace are dropped. This is what most callers want.
//! - [`UWordBounds`] — yields *every* boundary-delimited piece,
//!   including whitespace and punctuation. This is what a caller
//!   that needs a reversible view of the input reaches for.
//!
//! Both variants are exposed here: [`words`] wraps the first,
//! [`word_bounds`] wraps the second, and
//! [`SplitWordBoundsBehavior`] lets a caller pick between them
//! through a single common entry point.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
//! [`UnicodeWords`]: unicode_segmentation::UnicodeWords
//! [`UWordBounds`]: unicode_segmentation::UWordBounds
//!
//! # References
//!
//! * Unicode Standard Annex #29. *Unicode Text Segmentation*. URL:
//!   <https://www.unicode.org/reports/tr29/> — the specification the
//!   underlying `unicode-segmentation` crate implements (word-boundary
//!   rules WB1 … WB999).
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.

use alloc::vec::Vec;
use stringcheese_core::IndexableSequence;
use unicode_segmentation::UnicodeSegmentation;

/// Chooses whether a boundary iterator yields *only words* or *every
/// boundary-delimited piece*.
///
/// Both variants segment the input at the same UAX #29 word boundaries;
/// they only differ in which pieces reach the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplitWordBoundsBehavior {
    /// Yield only the pieces that look like real words — letters,
    /// digits, and joiners. Whitespace and punctuation are dropped.
    /// Equivalent to [`str::unicode_words`](unicode_segmentation::UnicodeSegmentation::unicode_words).
    #[default]
    WordsOnly,
    /// Yield *every* piece between adjacent word boundaries, including
    /// runs of whitespace and punctuation. Equivalent to
    /// [`str::split_word_bounds`](unicode_segmentation::UnicodeSegmentation::split_word_bounds).
    /// The concatenation of every yielded piece reconstructs the input.
    AllBoundaries,
}

/// Iterates the UAX #29 words of `input`, dropping whitespace and
/// punctuation.
///
/// The returned iterator yields subslices of `input`; no allocation is
/// performed. Where you need indexed access see [`WordSequence`].
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::words;
/// // Apostrophe joins its neighbours — one word.
/// let ws: Vec<&str> = words("don't stop!").collect();
/// assert_eq!(ws, ["don't", "stop"]);
///
/// // Numeric decimal is one word.
/// let ws: Vec<&str> = words("3.14 is pi").collect();
/// assert_eq!(ws, ["3.14", "is", "pi"]);
///
/// // Punctuation is dropped.
/// let ws: Vec<&str> = words("hello, world!").collect();
/// assert_eq!(ws, ["hello", "world"]);
/// ```
pub fn words(input: &str) -> impl Iterator<Item = &str> {
    UnicodeSegmentation::unicode_words(input)
}

/// Iterates *every* UAX #29 boundary-delimited piece of `input`,
/// including runs of whitespace and punctuation.
///
/// The concatenation of the yielded pieces exactly reconstructs `input`
/// — no scalar is dropped. Where [`words`] hides whitespace and
/// punctuation, `word_bounds` surfaces them as their own pieces.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::word_bounds;
/// let ps: Vec<&str> = word_bounds("hello, world!").collect();
/// assert_eq!(ps, ["hello", ",", " ", "world", "!"]);
///
/// // Round-trip: joining the pieces recovers the input.
/// let s = "3.14 - pi";
/// let joined: String = word_bounds(s).collect();
/// assert_eq!(joined, s);
/// ```
pub fn word_bounds(input: &str) -> impl Iterator<Item = &str> {
    UnicodeSegmentation::split_word_bounds(input)
}

/// Iterates `(byte_offset, word)` pairs for the UAX #29 words of
/// `input`, dropping whitespace and punctuation.
///
/// Same word filtering as [`words`]; each yielded item additionally
/// carries the byte offset at which the word begins in `input`, so
/// callers that need span information do not have to perform pointer
/// arithmetic. Zero allocation.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::word_indices;
/// let vs: Vec<(usize, &str)> = word_indices("hello, world!").collect();
/// assert_eq!(vs, vec![(0, "hello"), (7, "world")]);
/// ```
pub fn word_indices(input: &str) -> impl Iterator<Item = (usize, &str)> {
    UnicodeSegmentation::unicode_word_indices(input)
}

/// Iterates `(byte_offset, piece)` pairs for *every* UAX #29
/// boundary-delimited piece of `input`.
///
/// Same boundary rule as [`word_bounds`]; each yielded item
/// additionally carries the byte offset at which the piece begins.
/// The concatenation of the yielded pieces exactly reconstructs
/// `input`. Zero allocation.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::word_bound_indices;
/// let vs: Vec<(usize, &str)> = word_bound_indices("hi, u").collect();
/// assert_eq!(vs, vec![(0, "hi"), (2, ","), (3, " "), (4, "u")]);
/// ```
pub fn word_bound_indices(input: &str) -> impl Iterator<Item = (usize, &str)> {
    UnicodeSegmentation::split_word_bound_indices(input)
}

/// A materialized sequence of UAX #29 words (or word-bound pieces),
/// indexable in `O(1)`.
///
/// [`words`] and [`word_bounds`] yield subslices lazily and cannot
/// answer `get(i)` without walking the string from the start. This
/// type does that walk once, buffering the results, so downstream
/// algorithms that need indexed access (edit-distance kernels over
/// words, alignment kernels) can treat words as a first-class
/// sequence.
///
/// The behaviour is selected at construction time by a
/// [`SplitWordBoundsBehavior`] — the default constructor uses
/// [`SplitWordBoundsBehavior::WordsOnly`], matching the semantics of
/// [`words`].
///
/// ```
/// # use stringcheese_unicode::{SplitWordBoundsBehavior, WordSequence};
/// // Default: words-only.
/// let s = WordSequence::new("hello, world!");
/// assert_eq!(s.len(), 2);
/// assert_eq!(s.get(0).copied(), Some("hello"));
///
/// // All boundaries — includes the comma, the space, and the bang.
/// let s = WordSequence::with_behavior(
///     "hello, world!",
///     SplitWordBoundsBehavior::AllBoundaries,
/// );
/// assert_eq!(s.len(), 5);
/// ```
///
/// # Lifetimes
///
/// `WordSequence<'a>` borrows from the input string `'a`. The buffered
/// `Vec<&'a str>` holds subslice pointers into the source string, not
/// owned copies — segmentation therefore allocates once (the `Vec`
/// spine) and does not copy any text. As long as the input string
/// outlives the sequence, the sequence is valid.
///
/// If you need an owned form that can outlive the input, `collect`
/// into a `Vec<String>` explicitly.
#[derive(Debug, Clone)]
pub struct WordSequence<'a> {
    words: Vec<&'a str>,
}

impl<'a> WordSequence<'a> {
    /// Segments `input` into its UAX #29 words (dropping whitespace and
    /// punctuation) and buffers the results for `O(1)` indexed access.
    ///
    /// Equivalent to `WordSequence::with_behavior(input,
    /// SplitWordBoundsBehavior::WordsOnly)`. Performs one allocation
    /// (the spine `Vec`).
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self::with_behavior(input, SplitWordBoundsBehavior::WordsOnly)
    }

    /// Segments `input` under the requested boundary behavior and
    /// buffers the pieces for `O(1)` indexed access.
    ///
    /// - [`SplitWordBoundsBehavior::WordsOnly`] uses [`words`].
    /// - [`SplitWordBoundsBehavior::AllBoundaries`] uses [`word_bounds`].
    #[must_use]
    pub fn with_behavior(input: &'a str, behavior: SplitWordBoundsBehavior) -> Self {
        let words = match behavior {
            SplitWordBoundsBehavior::WordsOnly => words(input).collect(),
            SplitWordBoundsBehavior::AllBoundaries => word_bounds(input).collect(),
        };
        Self { words }
    }

    /// The number of pieces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// `true` if the sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Returns the piece at position `i`, or `None` if `i >= len()`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&&'a str> {
        self.words.get(i)
    }

    /// The underlying buffered slice, suitable for handing to any
    /// algorithm that consumes `&[&str]` (which most StringCheese
    /// distance kernels do via [`IndexableSequence`]).
    #[must_use]
    pub fn as_slice(&self) -> &[&'a str] {
        &self.words
    }
}

impl<'a> IndexableSequence for WordSequence<'a> {
    type Item = &'a str;

    #[inline]
    fn len(&self) -> usize {
        self.words.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        self.words.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[test]
    fn ascii_words_split_on_whitespace() {
        let ws: Vec<&str> = words("hello world").collect();
        assert_eq!(ws, ["hello", "world"]);
    }

    #[test]
    fn apostrophe_stays_inside_word() {
        // UAX #29 treats the apostrophe as an internal word joiner.
        let ws: Vec<&str> = words("don't stop").collect();
        assert_eq!(ws, ["don't", "stop"]);
    }

    #[test]
    fn numeric_decimal_is_one_word() {
        let ws: Vec<&str> = words("3.14 pi").collect();
        assert_eq!(ws, ["3.14", "pi"]);
    }

    #[test]
    fn punctuation_dropped_by_words_only() {
        let ws: Vec<&str> = words("hello, world!").collect();
        assert_eq!(ws, ["hello", "world"]);
    }

    #[test]
    fn word_bounds_includes_punctuation_and_whitespace() {
        let ps: Vec<&str> = word_bounds("hello, world!").collect();
        assert_eq!(ps, ["hello", ",", " ", "world", "!"]);
    }

    #[test]
    fn word_bounds_round_trip() {
        let s = "The quick, brown fox — 42!";
        let joined: String = word_bounds(s).collect();
        assert_eq!(joined, s);
    }

    #[test]
    fn empty_input_yields_no_words() {
        assert!(words("").next().is_none());
        assert!(word_bounds("").next().is_none());
    }

    #[test]
    fn sequence_default_is_words_only() {
        let s = WordSequence::new("hello, world!");
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(0).copied(), Some("hello"));
        assert_eq!(s.get(1).copied(), Some("world"));
    }

    #[test]
    fn sequence_all_boundaries_includes_punctuation() {
        let s = WordSequence::with_behavior("a, b", SplitWordBoundsBehavior::AllBoundaries);
        // "a", ",", " ", "b"
        assert_eq!(s.len(), 4);
        assert_eq!(s.get(1).copied(), Some(","));
    }

    #[test]
    fn is_empty_matches_len_zero() {
        assert!(WordSequence::new("").is_empty());
        assert!(WordSequence::with_behavior("", SplitWordBoundsBehavior::AllBoundaries).is_empty());
        assert!(!WordSequence::new("x").is_empty());
    }

    #[test]
    fn indexable_sequence_impl_works() {
        let seq = WordSequence::new("one two three");
        assert_eq!(<WordSequence<'_> as IndexableSequence>::len(&seq), 3);
        assert_eq!(
            <WordSequence<'_> as IndexableSequence>::get(&seq, 1).copied(),
            Some("two")
        );
        assert_eq!(<WordSequence<'_> as IndexableSequence>::get(&seq, 3), None);
    }

    #[test]
    fn as_slice_round_trip_for_all_boundaries() {
        let s = "a b";
        let seq = WordSequence::with_behavior(s, SplitWordBoundsBehavior::AllBoundaries);
        let joined: String = seq.as_slice().concat();
        assert_eq!(joined, s);
    }

    #[test]
    fn default_behavior_matches_words_only() {
        assert_eq!(
            SplitWordBoundsBehavior::default(),
            SplitWordBoundsBehavior::WordsOnly
        );
    }
}
