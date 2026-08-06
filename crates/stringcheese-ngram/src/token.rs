//! Token n-gram generator — the classical "shingle" representation.
//!
//! # This crate does not tokenize
//!
//! [`TokenGrams`] consumes a pre-tokenized slice of `&str` handles. The
//! decision of what a token *is* — words, subwords, code identifiers,
//! whitespace-separated pieces — lives in StringCheese's preprocessing
//! pipeline (see `docs/design/preprocessing-pipeline.md`); this crate
//! stays out of it. Consumers that need language-aware tokenization pull
//! it from the preprocessing crate and hand the resulting slice here.
//!
//! # Shape
//!
//! * `Input = [&'a str]` — a slice of borrowed string handles produced by
//!   a tokenizer.
//! * `Gram = Vec<&'a str>` — an owned window of borrowed handles. The
//!   handles themselves still point into the caller's storage; only the
//!   window's `Vec` allocation is owned by the generator. This makes
//!   token grams cheap even for long inputs — the per-gram allocation is
//!   `n * size_of::<&str>()`, not the sum of the token lengths.

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::generator::NGramGenerator;
use crate::padding::{InvalidN, PaddingPolicy};

/// Pre-tokenized token n-gram generator.
///
/// Lifetime `'a` is the lifetime of the token handles the caller supplies;
/// it appears in the generator so that any padding markers the caller
/// injects (via [`PaddingPolicy::Boundary`] or [`PaddingPolicy::Custom`])
/// share the same lifetime as the input tokens.
#[derive(Clone, Debug)]
pub struct TokenGrams<'a> {
    /// The generator's arity. Constructors reject zero.
    n: usize,
    /// The padding policy applied before window enumeration.
    padding: PaddingPolicy<&'a str>,
}

impl<'a> TokenGrams<'a> {
    /// Constructs a new token generator.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`. Use [`try_new`](Self::try_new) for the
    /// fallible variant.
    #[must_use]
    pub fn new(n: usize, padding: PaddingPolicy<&'a str>) -> Self {
        assert!(n >= 1, "n-gram arity `n` must be at least 1");
        Self { n, padding }
    }

    /// Constructs a new token generator, returning [`InvalidN`] if
    /// `n == 0`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidN`] if `n == 0`.
    pub fn try_new(n: usize, padding: PaddingPolicy<&'a str>) -> Result<Self, InvalidN> {
        if n == 0 {
            Err(InvalidN)
        } else {
            Ok(Self { n, padding })
        }
    }

    /// Returns the padding policy this generator applies.
    #[must_use]
    pub fn padding(&self) -> &PaddingPolicy<&'a str> {
        &self.padding
    }
}

impl<'a> NGramGenerator for TokenGrams<'a> {
    type Input = [&'a str];
    type Gram = Vec<&'a str>;
    type Iter<'b>
        = TokenGramsIter<'a>
    where
        Self: 'b,
        Self::Input: 'b;

    #[inline]
    fn n(&self) -> usize {
        self.n
    }

    fn grams<'b>(&'b self, input: &'b [&'a str]) -> Self::Iter<'b> {
        let padded = self.padding.apply(input, self.n);
        TokenGramsIter {
            padded,
            n: self.n,
            pos: 0,
            _phantom: PhantomData,
        }
    }
}

/// Iterator returned by [`TokenGrams::grams`].
#[derive(Clone, Debug)]
pub struct TokenGramsIter<'a> {
    /// The padded sequence of token handles, materialized by
    /// [`TokenGrams::grams`].
    padded: Vec<&'a str>,
    /// The generator's arity.
    n: usize,
    /// The next window's start index within [`padded`](Self::padded).
    pos: usize,
    /// Anchors the `'a` lifetime for the iterator's yielded windows.
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Iterator for TokenGramsIter<'a> {
    type Item = Vec<&'a str>;

    fn next(&mut self) -> Option<Vec<&'a str>> {
        if self.pos + self.n > self.padded.len() {
            return None;
        }
        let start = self.pos;
        self.pos += 1;
        Some(self.padded[start..start + self.n].to_vec())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.padded.len().saturating_sub(self.pos + self.n - 1);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TokenGramsIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn unpadded_bigrams_over_three_tokens() {
        let generator = TokenGrams::new(2, PaddingPolicy::None);
        let tokens: [&str; 3] = ["the", "quick", "fox"];
        let grams: Vec<Vec<&str>> = generator.grams(&tokens).collect();
        assert_eq!(grams, vec![vec!["the", "quick"], vec!["quick", "fox"]]);
    }

    #[test]
    fn boundary_padded_bigrams_over_three_tokens() {
        let generator = TokenGrams::new(
            2,
            PaddingPolicy::Boundary {
                start: "<BOS>",
                end: "<EOS>",
            },
        );
        let tokens: [&str; 3] = ["the", "quick", "fox"];
        let grams: Vec<Vec<&str>> = generator.grams(&tokens).collect();
        assert_eq!(
            grams,
            vec![
                vec!["<BOS>", "the"],
                vec!["the", "quick"],
                vec!["quick", "fox"],
                vec!["fox", "<EOS>"],
            ]
        );
    }

    #[test]
    fn unpadded_too_short_yields_nothing() {
        let generator = TokenGrams::new(4, PaddingPolicy::None);
        let tokens: [&str; 3] = ["the", "quick", "fox"];
        let grams: Vec<Vec<&str>> = generator.grams(&tokens).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn size_hint_is_exact() {
        let generator = TokenGrams::new(2, PaddingPolicy::None);
        let tokens: [&str; 4] = ["a", "b", "c", "d"];
        let iter = generator.grams(&tokens);
        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    #[should_panic(expected = "at least 1")]
    fn new_panics_on_zero() {
        let _ = TokenGrams::new(0, PaddingPolicy::<&str>::None);
    }

    #[test]
    fn try_new_rejects_zero() {
        assert_eq!(
            TokenGrams::try_new(0, PaddingPolicy::<&str>::None).unwrap_err(),
            InvalidN
        );
    }
}
