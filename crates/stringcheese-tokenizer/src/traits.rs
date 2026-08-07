//! The [`Segmenter`], [`Tokenizer`], and [`Encoding`] traits.
//!
//! See the crate-level documentation for the two-trait taxonomy and the
//! full design commentary in `docs/design/tokenizers.md` § 2.

use core::ops::Range;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use crate::error::TokenizerError;

/// A byte-offset-preserving slice of an input string.
///
/// Segmenters yield [`Segment`]s so that a caller who wants to relate a
/// piece back to the input — for highlighting, chunking at natural
/// boundaries, or aligning two tokenizations — has the byte range at
/// hand. The `offset` is measured in bytes into the original input, not
/// in characters or scalars; this matches every other span-carrying
/// StringCheese API.
///
/// The lifetime `'a` ties the segment's slice back to the input the
/// segmenter was invoked on: dropping the input invalidates every
/// outstanding segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Segment<'a> {
    /// Byte offset of the first byte of `text` within the input.
    pub offset: usize,
    /// Borrowed text of the segment. Always a well-formed UTF-8 slice.
    pub text: &'a str,
}

impl<'a> Segment<'a> {
    /// Constructs a segment from its two components.
    #[inline]
    #[must_use]
    pub const fn new(offset: usize, text: &'a str) -> Self {
        Self { offset, text }
    }

    /// The half-open byte range this segment covers in the input.
    #[inline]
    #[must_use]
    pub const fn range(&self) -> Range<usize> {
        self.offset..self.offset + self.text.len()
    }
}

/// A non-round-trippable walker over an input string.
///
/// Concatenating a segmenter's output does not have to recover the input:
/// whitespace, discarded punctuation, casing, or entire boundary regions
/// may be lost. This is what distinguishes a segmenter from a
/// [`Tokenizer`]: a segmenter is a *view*, not a *representation you can
/// decode back*.
///
/// # Associated types
///
/// The trait uses a generic-associated `Unit<'a>` so that a segmenter can
/// yield either a borrowed [`Segment`] (the zero-alloc common case —
/// [`WhitespaceTokenizer`][crate::WhitespaceTokenizer],
/// [`DelimiterTokenizer`][crate::DelimiterTokenizer], and friends all
/// pick this) or an owned unit (a hypothetical lowercasing segmenter
/// would yield `String`). The design decision is documented in
/// `docs/design/tokenizers.md` § 2.1.
pub trait Segmenter {
    /// The yielded unit type. `Segment<'a>` is the span-preserving default;
    /// alternative implementations may return owned strings if they need
    /// to transform their input.
    type Unit<'a>
    where
        Self: 'a;
    /// The iterator returned by [`segment`](Self::segment).
    type Iter<'a>: Iterator<Item = Self::Unit<'a>>
    where
        Self: 'a;

    /// Walks `text` and yields one [`Unit`](Self::Unit) per boundary.
    ///
    /// Implementations must be *pure*: repeated calls with the same input
    /// yield the same sequence of units. This is what lets downstream
    /// code (index builders, cachers, deduplicators) memoise the output.
    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a>;
}

/// The output of a [`Tokenizer::encode`] call.
///
/// Bundles three parallel arrays:
///
/// * `ids` — the token IDs in emission order.
/// * `offsets` — the half-open byte range in the *input* that produced
///   each token. Empty when a tokenizer chooses not to track offsets
///   (subword tokenizers over normalized input often can't produce a
///   meaningful range).
/// * `special_mask` — one `bool` per token; `true` iff the token is one
///   of the tokenizer's registered special tokens. Empty when the
///   tokenizer has no special tokens.
///
/// The three arrays, when non-empty, are always the same length as
/// `ids`. Callers who only need one axis can index the array they care
/// about; a tokenizer that tracks all three exposes the same information
/// downstream algorithms use for highlighting, diffing, and cost
/// accounting.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoding<Token> {
    /// The tokens in emission order.
    pub ids: Vec<Token>,
    /// One half-open byte range per token, or empty if not tracked.
    pub offsets: Vec<Range<usize>>,
    /// One flag per token indicating a special-token identity, or empty
    /// if not tracked.
    pub special_mask: Vec<bool>,
}

#[cfg(feature = "alloc")]
impl<Token> Encoding<Token> {
    /// Constructs a new, empty encoding.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ids: Vec::new(),
            offsets: Vec::new(),
            special_mask: Vec::new(),
        }
    }

    /// Number of tokens in the encoding.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Returns `true` if the encoding has zero tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<Token> Default for Encoding<Token> {
    fn default() -> Self {
        Self::new()
    }
}

/// A round-trippable text ↔ token bijection.
///
/// The load-bearing invariant is `decode(encode(text)) == text` for a
/// well-defined class of inputs. Documented lossy exceptions —
/// normalization, unknown-byte replacement, truncation — are listed in
/// `docs/design/tokenizers.md` § 2.2 and each implementation names the
/// ones it applies.
///
/// The trait requires [`Token: PartialEq`](PartialEq) as the minimum
/// bound so that a `&[Token]` returned from
/// [`Encoding::ids`](Encoding::ids) is a valid substrate for the generic
/// comparison kernels in `stringcheese-compare`. Individual algorithm
/// crates add stricter bounds (`Ord + Hash` for `MinHash` bucketing, for
/// instance) at their own call sites.
#[cfg(feature = "alloc")]
pub trait Tokenizer {
    /// The token type. `u32` (aliased as `TokenId`) for every subword
    /// tokenizer; `&'a str` or `String` for word-level tokenizers.
    type Token: PartialEq;

    /// Encodes `text` into a sequence of tokens plus optional metadata.
    ///
    /// Returns the full [`Encoding`] value; if only the count is needed,
    /// [`count`](Tokenizer::count) may avoid materialising it.
    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError>;

    /// Decodes a sequence of tokens back into a string.
    ///
    /// Given `encode(text)?.ids` as input, must return the original text
    /// modulo the documented exceptions in this trait's contract.
    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError>;

    /// Returns the number of tokens `encode(text)` would produce, without
    /// necessarily materialising the offset or special-mask arrays.
    ///
    /// The default implementation calls [`encode`](Tokenizer::encode) and
    /// discards everything but the length. Implementations with a
    /// cheaper count fast path override this — subword tokenizers, in
    /// particular, save allocation by skipping the offset tracking.
    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        Ok(self.encode(text)?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_range_matches_offset_plus_length() {
        let s = Segment::new(10, "hello");
        assert_eq!(s.range(), 10..15);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn encoding_new_is_empty() {
        let e: Encoding<u32> = Encoding::new();
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
    }
}
