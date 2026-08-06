//! The [`NGramGenerator`] trait and the [`count_grams`] preallocation helper.
//!
//! # Design: owned gram output
//!
//! The trait's associated `Gram` type is the gram itself, not a `&[T]`
//! window into the input. That choice is deliberate:
//!
//! * A padded generator prepends symbols that do not exist inside the
//!   input. The window that spans a boundary marker and an input symbol
//!   cannot be a borrowed slice of the input alone — it must own the
//!   padded prefix, or borrow from a materialized padded buffer. Owned
//!   gram types make padded and unpadded generators uniform.
//! * A token generator yields `Vec<&'a str>` (a slice of string handles),
//!   which is a fundamentally different type from a character generator's
//!   `Vec<char>`. Making `Gram` a per-generator associated type lets each
//!   generator commit to the natural shape for its symbol type.
//! * Downstream representations ([`GramSet`], [`GramMultiSet`],
//!   [`GramVector`]) all key on owned grams anyway — a borrowed window
//!   would have to be cloned into an owned form before it could enter a
//!   `BTreeSet`. Doing the clone in the generator centralizes the cost.
//!
//! The zero-allocation fast path — iterating windows without padding —
//! lives on the sibling [`CharacterGramSlices`] type. That type does *not*
//! implement [`NGramGenerator`] because the trait's owned-gram associated
//! type is not a fit for borrowed windows; a separate inherent-method API
//! keeps the fast path allocation-free.
//!
//! # Preallocation
//!
//! [`count_grams`] returns the exact number of grams the corresponding
//! generator will yield. Consumers that materialize gram sets, multisets,
//! or vectors from a generator's output can use this to size the backing
//! store up front — a small but consistent win for large inputs.
//!
//! [`CharacterGramSlices`]: crate::CharacterGramSlices
//! [`GramSet`]: crate::GramSet
//! [`GramMultiSet`]: crate::GramMultiSet
//! [`GramVector`]: crate::GramVector

use crate::padding::PaddingPolicy;

/// A source of grams over an input sequence.
///
/// Implementations pin the input and gram types to what makes sense for
/// their symbol kind: [`CharacterGrams`] with symbols of type `T` yields
/// `Vec<T>` from an input of `[T]`; [`TokenGrams`] yields `Vec<&'a str>`
/// from an input of `[&'a str]`.
///
/// # Iteration order
///
/// The trait requires ordered iteration — grams are produced in the order
/// they appear in the padded input. Downstream types that dedupe (e.g.
/// [`GramSet`]) impose their own iteration order.
///
/// [`CharacterGrams`]: crate::CharacterGrams
/// [`TokenGrams`]: crate::TokenGrams
/// [`GramSet`]: crate::GramSet
pub trait NGramGenerator {
    /// The input the generator windows over. Typically `[T]` or `[&str]`.
    type Input: ?Sized;
    /// The gram type this generator produces. Typically an owned `Vec` of
    /// the generator's symbol type; see the module-level docs for the
    /// rationale behind owned output.
    type Gram;
    /// The iterator returned by [`grams`](Self::grams). Borrows from both
    /// `self` (for configuration) and the input (for the input's symbols).
    type Iter<'a>: Iterator<Item = Self::Gram>
    where
        Self: 'a,
        Self::Input: 'a;

    /// Returns the arity of the generator — the length of each gram it
    /// produces. Guaranteed to be at least `1`; constructors reject zero.
    fn n(&self) -> usize;

    /// Iterates the grams the generator produces from `input`.
    ///
    /// The returned iterator yields grams in the order they appear in the
    /// (padded, if applicable) input. For an input of length `L`, arity
    /// `n`, and padding that adds `p_l` on the left and `p_r` on the
    /// right, the iterator yields exactly
    /// `max(0, L + p_l + p_r - n + 1)` grams — the value returned by
    /// [`count_grams`].
    fn grams<'a>(&'a self, input: &'a Self::Input) -> Self::Iter<'a>;
}

/// Returns the exact number of grams a generator of arity `n` with
/// `padding` will produce from an input of length `input_len`.
///
/// This is the closed form of the trait's iteration contract; it lets
/// consumers preallocate a backing store before iterating. The formula
/// mirrors the one described in the [`generator`][self] module docs.
///
/// # Notes
///
/// Returns `0` when `n == 0` — this crate's constructors reject that
/// case, so a valid call site never observes it, but the helper stays
/// total for cases where the caller assembles arguments dynamically.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "alloc")] {
/// use stringcheese_ngram::{PaddingPolicy, count_grams};
///
/// // `"cat"` with `n = 2`, no padding: two bigrams.
/// let p: PaddingPolicy<char> = PaddingPolicy::None;
/// assert_eq!(count_grams(3, 2, &p), 2);
///
/// // `"cat"` with `n = 3`, boundary padding: five trigrams
/// // (over `^^cat$$`).
/// let p = PaddingPolicy::Boundary { start: '^', end: '$' };
/// assert_eq!(count_grams(3, 3, &p), 5);
/// # }
/// ```
#[must_use]
pub fn count_grams<T: Clone>(input_len: usize, n: usize, padding: &PaddingPolicy<T>) -> usize {
    if n == 0 {
        return 0;
    }
    let total = input_len
        .saturating_add(padding.left_len(n))
        .saturating_add(padding.right_len(n));
    // Number of length-`n` windows in a sequence of length `total`.
    total.saturating_sub(n - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn count_zero_arity_is_zero() {
        let p: PaddingPolicy<u8> = PaddingPolicy::None;
        assert_eq!(count_grams(5, 0, &p), 0);
    }

    #[test]
    fn count_unpadded_matches_windows_len() {
        let p: PaddingPolicy<char> = PaddingPolicy::None;
        // Same as slice::windows(n).count() for a length-L slice.
        assert_eq!(count_grams(0, 1, &p), 0);
        assert_eq!(count_grams(1, 1, &p), 1);
        assert_eq!(count_grams(3, 1, &p), 3);
        assert_eq!(count_grams(3, 2, &p), 2);
        assert_eq!(count_grams(3, 3, &p), 1);
        assert_eq!(count_grams(3, 4, &p), 0);
    }

    #[test]
    fn count_boundary_matches_formula() {
        let p = PaddingPolicy::Boundary {
            start: '^',
            end: '$',
        };
        // For boundary padding, count = input_len + n - 1 for n >= 1.
        assert_eq!(count_grams(0, 3, &p), 2); // padded seq is ^^$$
        assert_eq!(count_grams(3, 2, &p), 4); // ^cat$
        assert_eq!(count_grams(3, 3, &p), 5); // ^^cat$$
        assert_eq!(count_grams(3, 4, &p), 6); // ^^^cat$$$
    }

    #[test]
    fn count_custom_uses_prefix_and_suffix_lengths() {
        let p = PaddingPolicy::Custom {
            prefix: vec!['<', '<'],
            suffix: vec!['>'],
        };
        // Effective length = input_len + 2 + 1 = input_len + 3.
        assert_eq!(count_grams(0, 3, &p), 1); // "<<>" is one trigram
        assert_eq!(count_grams(3, 3, &p), 4); // "<<cat>" has 4 trigrams
        assert_eq!(count_grams(3, 10, &p), 0); // still too short
    }
}
