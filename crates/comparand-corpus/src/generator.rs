//! Exhaustive enumeration of test inputs over a small alphabet.
//!
//! The core building block of the exhaustive small-domain oracle strategy:
//! given an alphabet Σ and a bound `max_length`, enumerate every sequence
//! over Σ up to and including `max_length`, then feed each sequence (or
//! pair of sequences) to the algorithm under test and check its output
//! against the [`crate::oracle::Oracle`].
//!
//! For a two-argument algorithm (e.g. edit distance), [`exhaustive_pairs`]
//! yields the Cartesian product of the single-sequence enumeration with
//! itself. The counts grow quickly — see [`count_sequences`] — but for
//! `|Σ| = 2, max_length ≈ 8` or `|Σ| = 3, max_length ≈ 6` the coverage is
//! genuinely exhaustive.
//!
//! Iteration order is *length-lexicographic*: the empty sequence first,
//! then all length-1 sequences in alphabet order, then all length-2, and so
//! on. Within a length, sequences are enumerated in the alphabet's index
//! order.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

/// Returns an iterator over every sequence over `alphabet` of length 0
/// through `max_length` (inclusive), in length-lexicographic order.
///
/// The empty sequence is yielded first. For each length `L` from `1`
/// through `max_length`, all `|alphabet|^L` sequences are yielded in
/// alphabet-index order — for `alphabet = ['a', 'b']` this means
/// `['a']`, `['b']`, `['a', 'a']`, `['a', 'b']`, `['b', 'a']`,
/// `['b', 'b']`, and so on.
///
/// The iterator is `Clone` only if `T: Clone` (it holds a `Vec<usize>` of
/// its current position, which is always `Clone`); it is `Send + Sync`
/// whenever `T: Sync`.
///
/// # Special cases
///
/// * An empty alphabet yields exactly the empty sequence: no non-empty
///   sequence can be constructed over Σ = ∅.
/// * `max_length = 0` yields exactly the empty sequence, regardless of
///   the alphabet.
#[must_use]
pub fn exhaustive_over_alphabet<T: Clone>(
    alphabet: &[T],
    max_length: usize,
) -> ExhaustiveIter<'_, T> {
    ExhaustiveIter::new(alphabet, max_length)
}

/// Concrete iterator yielded by [`exhaustive_over_alphabet`].
///
/// Named explicitly (rather than returned as `impl Iterator`) so that
/// callers can store it in a struct field or return it from their own
/// functions without opaque-type gymnastics.
#[derive(Debug, Clone)]
pub struct ExhaustiveIter<'a, T> {
    alphabet: &'a [T],
    max_length: usize,
    /// Indices into `alphabet` describing the *next* sequence to yield.
    /// Its length equals the current sequence length.
    indices: Vec<usize>,
    /// Length of the sequence currently in `indices`.
    current_length: usize,
    /// Set once the last sequence has been yielded.
    done: bool,
    /// Total sequences this iterator will ever yield (over its lifetime).
    total: u64,
    /// Sequences already yielded.
    emitted: u64,
}

impl<'a, T> ExhaustiveIter<'a, T> {
    fn new(alphabet: &'a [T], max_length: usize) -> Self {
        let total = count_sequences(alphabet.len(), max_length);
        Self {
            alphabet,
            max_length,
            indices: Vec::new(),
            current_length: 0,
            done: false,
            total,
            emitted: 0,
        }
    }

    /// Advance `indices` / `current_length` to point at the sequence that
    /// should be yielded *after* the current one.
    fn advance(&mut self) {
        // Try to increment the current-length sequence in place.
        if !self.indices.is_empty() && self.increment_indices() {
            return;
        }
        // Otherwise move to the next longer length, if any.
        if self.current_length >= self.max_length || self.alphabet.is_empty() {
            self.done = true;
            return;
        }
        self.current_length += 1;
        self.indices = vec![0; self.current_length];
    }

    /// Mixed-radix increment on `indices` with radix `|alphabet|`. Returns
    /// `true` if the increment stayed within `|alphabet|^current_length`
    /// values, `false` on overflow.
    fn increment_indices(&mut self) -> bool {
        let radix = self.alphabet.len();
        if radix == 0 || self.indices.is_empty() {
            return false;
        }
        for i in (0..self.indices.len()).rev() {
            self.indices[i] += 1;
            if self.indices[i] < radix {
                return true;
            }
            self.indices[i] = 0;
        }
        false
    }
}

impl<T: Clone> Iterator for ExhaustiveIter<'_, T> {
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let current: Vec<T> = self
            .indices
            .iter()
            .map(|&i| self.alphabet[i].clone())
            .collect();
        self.emitted = self.emitted.saturating_add(1);
        self.advance();
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining_u64 = self.total.saturating_sub(self.emitted);
        // Clamp to usize. On 32-bit targets an enumeration with a >4G
        // remainder is *possible* in principle but not realistically
        // achievable in memory; the clamp is a portability convenience.
        let remaining_usize = usize::try_from(remaining_u64).unwrap_or(usize::MAX);
        (remaining_usize, Some(remaining_usize))
    }
}

/// Returns the Cartesian product of [`exhaustive_over_alphabet`] with
/// itself — every ordered pair `(left, right)` of sequences over
/// `alphabet` with both `left` and `right` no longer than `max_length`.
///
/// The count of yielded pairs is exactly
/// [`count_sequences`]`(|alphabet|, max_length).pow(2)`. For
/// `|Σ| = 2, max_length = 4`, that is `31.pow(2) = 961` pairs; for
/// `|Σ| = 3, max_length = 5`, it is `364.pow(2) = 132_496`.
///
/// Iteration order: the outer iterator advances `right` fastest — for a
/// fixed `left`, every `right` is yielded before `left` advances.
pub fn exhaustive_pairs<'a, T: Clone + 'a>(
    alphabet: &'a [T],
    max_length: usize,
) -> impl Iterator<Item = (Vec<T>, Vec<T>)> + 'a {
    exhaustive_over_alphabet(alphabet, max_length).flat_map(move |left| {
        exhaustive_over_alphabet(alphabet, max_length)
            .map(move |right| (left.clone(), right))
    })
}

/// Counts the total number of sequences over an alphabet of the given size
/// with length between `0` and `max_length` inclusive — closed form,
/// `sum_{n=0..=max_length} alphabet_size^n`.
///
/// # Overflow
///
/// The result is `u64` so that realistic exhaustive spaces
/// (`|Σ| = 4, max_length = 15` yields ~1.4 billion sequences, which fits)
/// return an exact count. When the true count exceeds `u64::MAX`, the
/// function *saturates* to `u64::MAX` rather than panicking or wrapping.
/// Callers who care about the distinction should compare to `u64::MAX` and
/// treat that value as "overflowed".
///
/// # Special cases
///
/// * `alphabet_size == 0` returns `1` for any `max_length`: the only
///   sequence over the empty alphabet is the empty sequence.
/// * `max_length == 0` returns `1` for any `alphabet_size`: the only
///   sequence of length `0` is the empty sequence.
#[must_use]
pub fn count_sequences(alphabet_size: usize, max_length: usize) -> u64 {
    let k = alphabet_size as u64;
    let mut total: u64 = 0;
    // K^0 == 1, always.
    let mut power: u64 = 1;
    for _ in 0..=max_length {
        total = total.saturating_add(power);
        power = power.saturating_mul(k);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_sequences_binary_alphabet() {
        // 1 + 2 + 4 + 8 + 16 = 31
        assert_eq!(count_sequences(2, 4), 31);
        // 1 + 2 = 3
        assert_eq!(count_sequences(2, 1), 3);
        // 1 (only empty)
        assert_eq!(count_sequences(2, 0), 1);
    }

    #[test]
    fn count_sequences_ternary_alphabet() {
        // 1 + 3 + 9 + 27 + 81 + 243 = 364
        assert_eq!(count_sequences(3, 5), 364);
    }

    #[test]
    fn count_sequences_empty_alphabet() {
        // Only the empty sequence.
        assert_eq!(count_sequences(0, 0), 1);
        assert_eq!(count_sequences(0, 5), 1);
    }

    #[test]
    fn count_sequences_saturates_on_overflow() {
        // 2^63 is representable; 2^64 saturates.
        assert_eq!(count_sequences(2, 100), u64::MAX);
    }

    #[test]
    fn exhaustive_iter_binary_length_two_expected_order() {
        let alphabet = ['a', 'b'];
        let seqs: Vec<Vec<char>> = exhaustive_over_alphabet(&alphabet, 2).collect();
        let expected: Vec<Vec<char>> = vec![
            vec![],
            vec!['a'],
            vec!['b'],
            vec!['a', 'a'],
            vec!['a', 'b'],
            vec!['b', 'a'],
            vec!['b', 'b'],
        ];
        assert_eq!(seqs, expected);
    }

    #[test]
    fn exhaustive_iter_count_agrees_with_closed_form() {
        for &(k, max) in &[(2_u8, 4_usize), (3, 3), (1, 5), (4, 2)] {
            let alphabet: Vec<u8> = (0..k).collect();
            let iterated = exhaustive_over_alphabet(&alphabet, max).count();
            let closed_form = count_sequences(usize::from(k), max);
            assert_eq!(iterated as u64, closed_form, "k={k}, max={max}");
        }
    }

    #[test]
    fn exhaustive_iter_size_hint_is_exact() {
        let alphabet = ['a', 'b'];
        let iter = exhaustive_over_alphabet(&alphabet, 4);
        let (lo, hi) = iter.size_hint();
        assert_eq!(lo, 31);
        assert_eq!(hi, Some(31));

        // After yielding a few items the size_hint should shrink.
        let mut iter = exhaustive_over_alphabet(&alphabet, 4);
        let _ = iter.next();
        let _ = iter.next();
        let _ = iter.next();
        assert_eq!(iter.size_hint(), (28, Some(28)));
    }

    #[test]
    fn exhaustive_iter_empty_alphabet_yields_only_empty() {
        let alphabet: [u8; 0] = [];
        let seqs: Vec<Vec<u8>> = exhaustive_over_alphabet(&alphabet, 5).collect();
        assert_eq!(seqs, vec![Vec::<u8>::new()]);
    }

    #[test]
    fn exhaustive_iter_zero_max_length_yields_only_empty() {
        let alphabet = ['a', 'b', 'c'];
        let seqs: Vec<Vec<char>> = exhaustive_over_alphabet(&alphabet, 0).collect();
        assert_eq!(seqs, vec![Vec::<char>::new()]);
    }

    #[test]
    fn exhaustive_iter_unary_alphabet() {
        let alphabet = ['x'];
        let seqs: Vec<Vec<char>> = exhaustive_over_alphabet(&alphabet, 3).collect();
        let expected: Vec<Vec<char>> = vec![
            vec![],
            vec!['x'],
            vec!['x', 'x'],
            vec!['x', 'x', 'x'],
        ];
        assert_eq!(seqs, expected);
    }

    #[test]
    fn exhaustive_pairs_count_is_squared() {
        for &(k, max) in &[(2_u8, 3_usize), (3, 2), (1, 4)] {
            let alphabet: Vec<u8> = (0..k).collect();
            let iterated = exhaustive_pairs(&alphabet, max).count();
            let expected = count_sequences(usize::from(k), max);
            assert_eq!(iterated as u64, expected * expected, "k={k}, max={max}");
        }
    }

    #[test]
    fn exhaustive_pairs_binary_max_length_one() {
        // count_sequences(2, 1) = 3. So 3^2 = 9 pairs.
        let alphabet = ['a', 'b'];
        let pairs: Vec<(Vec<char>, Vec<char>)> =
            exhaustive_pairs(&alphabet, 1).collect();
        assert_eq!(pairs.len(), 9);
        // The first pair should be (empty, empty).
        assert_eq!(pairs[0], (vec![], vec![]));
        // The last pair should be (['b'], ['b']).
        assert_eq!(pairs[8], (vec!['b'], vec!['b']));
    }

    #[test]
    fn exhaustive_iter_is_send_and_sync() {
        // Compile-time assertion via generic bound; runtime is trivial.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ExhaustiveIter<'_, u8>>();
    }
}
