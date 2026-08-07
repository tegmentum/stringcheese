//! UAX #29 sentence segmentation.
//!
//! A **sentence** in the sense of Unicode Standard Annex #29 is the
//! run of text between two sentence boundaries. Sentence boundaries
//! are inferred from the `Sentence_Break` property of adjacent
//! scalars — mostly `.`, `!`, and `?` followed by whitespace, with
//! carve-outs for abbreviations (`Dr. Smith`), numeric decimals
//! (`3.14`), and terminal punctuation followed by closing brackets or
//! quotes.
//!
//! The rules are deliberately conservative — they aim to reproduce
//! what a rules-based tokenizer would call a "sentence" without any
//! locale-specific knowledge. For genuinely locale-tailored
//! segmentation (Japanese `。`, Arabic `؟`, and so on beyond what UAX
//! #29 baseline covers), a downstream caller can compose this
//! iterator with locale-aware post-processing.
//!
//! # A note on retained whitespace
//!
//! The [`UnicodeSentences`] iterator this module wraps yields
//! *sentence content* — the trailing whitespace between one sentence
//! and the next is included as part of the *earlier* sentence's
//! slice, so the concatenation of the yielded sentences reconstructs
//! the input.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
//! [`UnicodeSentences`]: unicode_segmentation::UnicodeSentences
//!
//! # References
//!
//! * Unicode Standard Annex #29. *Unicode Text Segmentation*. URL:
//!   <https://www.unicode.org/reports/tr29/> — the specification the
//!   underlying `unicode-segmentation` crate implements
//!   (sentence-boundary rules SB1 … SB999).
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.

use alloc::vec::Vec;
use stringcheese_core::IndexableSequence;
use unicode_segmentation::UnicodeSegmentation;

/// Iterates the UAX #29 sentences of `input`.
///
/// The returned iterator yields subslices of `input`; no allocation is
/// performed. Where you need indexed access see [`SentenceSequence`].
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::sentences;
/// let ss: Vec<&str> = sentences("Hello, world. How are you?").collect();
/// assert_eq!(ss.len(), 2);
/// assert!(ss[0].starts_with("Hello"));
/// assert!(ss[1].starts_with("How"));
///
/// // Trailing whitespace between sentences belongs to the earlier
/// // sentence, so joining the pieces reconstructs the input.
/// let s = "One. Two. Three.";
/// let joined: String = sentences(s).collect();
/// assert_eq!(joined, s);
/// ```
pub fn sentences(input: &str) -> impl Iterator<Item = &str> {
    UnicodeSegmentation::unicode_sentences(input)
}

/// Iterates `(byte_offset, sentence)` pairs for the UAX #29 sentences
/// of `input`.
///
/// Same filtering as [`sentences`] — pieces with no alphanumeric
/// content are dropped — but each yielded item additionally carries
/// the byte offset at which the sentence begins in `input`. Zero
/// allocation.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::sentence_indices;
/// let vs: Vec<(usize, &str)> = sentence_indices("Hi. Bye.").collect();
/// assert_eq!(vs.len(), 2);
/// assert_eq!(vs[0].0, 0);
/// assert!(vs[0].1.starts_with("Hi"));
/// assert!(vs[1].1.starts_with("Bye"));
/// ```
pub fn sentence_indices(input: &str) -> impl Iterator<Item = (usize, &str)> {
    // `sentences()` yields subslices of the input; compute the byte
    // offset by subtracting the input's base pointer from each slice's
    // base pointer. Both pointers derive from the same allocation, so
    // the difference is a valid byte offset. No `unsafe` needed — the
    // `pointer as usize` cast and integer subtraction are safe Rust.
    let base = input.as_ptr() as usize;
    sentences(input).map(move |s| ((s.as_ptr() as usize) - base, s))
}

/// A materialized sequence of UAX #29 sentences, indexable in `O(1)`.
///
/// [`sentences`] yields sentence subslices lazily and cannot answer
/// `get(i)` without walking the string from the start. This type does
/// that walk once, buffering the results, so downstream algorithms
/// that need indexed access (edit-distance kernels over sentences,
/// alignment kernels for paragraph-level diffs) can treat sentences
/// as a first-class sequence.
///
/// ```
/// # use stringcheese_unicode::SentenceSequence;
/// let s = SentenceSequence::new("Hello. World.");
/// assert_eq!(s.len(), 2);
/// ```
///
/// # Lifetimes
///
/// `SentenceSequence<'a>` borrows from the input string `'a`. The
/// buffered `Vec<&'a str>` holds subslice pointers into the source
/// string, not owned copies — segmentation therefore allocates once
/// (the `Vec` spine) and does not copy any text. As long as the input
/// string outlives the sequence, the sequence is valid.
///
/// If you need an owned form that can outlive the input, `collect`
/// into a `Vec<String>` explicitly.
#[derive(Debug, Clone)]
pub struct SentenceSequence<'a> {
    sentences: Vec<&'a str>,
}

impl<'a> SentenceSequence<'a> {
    /// Segments `input` into its UAX #29 sentences and buffers the
    /// results for `O(1)` indexed access.
    ///
    /// Performs one allocation (the spine `Vec`).
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            sentences: sentences(input).collect(),
        }
    }

    /// The number of sentences.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sentences.len()
    }

    /// `true` if the sequence is empty (which happens exactly when the
    /// input string was empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sentences.is_empty()
    }

    /// Returns the sentence at position `i`, or `None` if `i >= len()`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&&'a str> {
        self.sentences.get(i)
    }

    /// The underlying buffered slice, suitable for handing to any
    /// algorithm that consumes `&[&str]` (which most StringCheese
    /// distance kernels do via [`IndexableSequence`]).
    #[must_use]
    pub fn as_slice(&self) -> &[&'a str] {
        &self.sentences
    }
}

impl<'a> IndexableSequence for SentenceSequence<'a> {
    type Item = &'a str;

    #[inline]
    fn len(&self) -> usize {
        self.sentences.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.sentences.is_empty()
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        self.sentences.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[test]
    fn two_sentences_separated_by_period() {
        let ss: Vec<&str> = sentences("Hello. World.").collect();
        assert_eq!(ss.len(), 2);
    }

    #[test]
    fn question_and_exclamation_break_too() {
        let ss: Vec<&str> = sentences("Really? Yes! Great.").collect();
        assert_eq!(ss.len(), 3);
    }

    #[test]
    fn single_sentence_no_terminator() {
        let ss: Vec<&str> = sentences("no terminator here").collect();
        assert_eq!(ss.len(), 1);
    }

    #[test]
    fn empty_input_yields_no_sentences() {
        assert!(sentences("").next().is_none());
        assert!(SentenceSequence::new("").is_empty());
    }

    #[test]
    fn concatenation_reconstructs_input() {
        let s = "One. Two. Three.";
        let joined: String = sentences(s).collect();
        assert_eq!(joined, s);
    }

    #[test]
    fn numeric_decimal_is_not_a_sentence_break() {
        // "3.14" is not two sentences.
        let ss: Vec<&str> = sentences("Pi is 3.14 today.").collect();
        assert_eq!(ss.len(), 1);
    }

    #[test]
    fn sequence_get_returns_borrowed_slice() {
        let s = SentenceSequence::new("Hi. Bye.");
        assert_eq!(s.len(), 2);
        assert!(s.get(0).copied().unwrap().starts_with("Hi"));
        assert!(s.get(1).copied().unwrap().starts_with("Bye"));
        assert_eq!(s.get(2), None);
    }

    #[test]
    fn indexable_sequence_impl_works() {
        let seq = SentenceSequence::new("A. B. C.");
        assert_eq!(<SentenceSequence<'_> as IndexableSequence>::len(&seq), 3);
        assert!(
            <SentenceSequence<'_> as IndexableSequence>::get(&seq, 0)
                .copied()
                .unwrap()
                .starts_with('A')
        );
        assert_eq!(
            <SentenceSequence<'_> as IndexableSequence>::get(&seq, 3),
            None
        );
    }

    #[test]
    fn as_slice_round_trip() {
        let s = "Alpha. Beta.";
        let seq = SentenceSequence::new(s);
        let joined: String = seq.as_slice().concat();
        assert_eq!(joined, s);
    }
}
