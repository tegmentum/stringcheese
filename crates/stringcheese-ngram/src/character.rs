//! Character (or byte) n-gram generators.
//!
//! Two shapes ship in this module:
//!
//! * [`CharacterGrams`] implements [`NGramGenerator`] with an owned
//!   `Vec<T>` gram type. It supports every [`PaddingPolicy`] and is the
//!   general-purpose choice — the one to reach for unless a caller has
//!   already committed to a specific layout.
//! * [`CharacterGramSlices`] is the zero-allocation fast path for
//!   pre-padded inputs. It skips padding entirely and yields borrowed
//!   `&'a [T]` windows via `slice::windows`. It does not implement
//!   [`NGramGenerator`] because the trait's owned-gram associated type is
//!   incompatible with a borrowed-slice output; borrowed windows carry a
//!   lifetime the trait's `Gram` cannot express.
//!
//! # Choosing between them
//!
//! * If you know the input is already padded to your satisfaction — for
//!   example, because you built the sequence yourself with sentinel
//!   symbols already spliced in — [`CharacterGramSlices`] avoids the
//!   per-gram `Vec` allocation.
//! * Otherwise use [`CharacterGrams`]. The extra allocation is unavoidable
//!   when padding markers may appear inside a gram: no borrow can point at
//!   a symbol that does not physically exist in the input.
//!
//! Both types are generic over `T`. Byte grams instantiate them with `u8`;
//! Unicode-scalar grams instantiate them with `char`.

use alloc::vec::Vec;

use crate::generator::NGramGenerator;
use crate::padding::{InvalidN, PaddingPolicy};

/// Generic character (or byte) n-gram generator with owned-window output.
///
/// See the [module docs][self] for how this type relates to
/// [`CharacterGramSlices`], and see [`PaddingPolicy`] for the padding
/// choices this type supports.
#[derive(Clone, Debug)]
pub struct CharacterGrams<T: Clone> {
    /// The generator's arity. Constructors reject zero, so callers never
    /// need to defend against it here.
    n: usize,
    /// The padding policy applied before window enumeration. See
    /// [`PaddingPolicy`] for the semantics.
    padding: PaddingPolicy<T>,
}

impl<T: Clone> CharacterGrams<T> {
    /// Constructs a new generator.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`. Zero-length grams have no defined meaning; use
    /// [`try_new`](Self::try_new) for a fallible variant that returns
    /// [`InvalidN`] instead.
    #[must_use]
    pub fn new(n: usize, padding: PaddingPolicy<T>) -> Self {
        assert!(n >= 1, "n-gram arity `n` must be at least 1");
        Self { n, padding }
    }

    /// Constructs a new generator, returning [`InvalidN`] if `n == 0`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidN`] if `n == 0`.
    pub fn try_new(n: usize, padding: PaddingPolicy<T>) -> Result<Self, InvalidN> {
        if n == 0 {
            Err(InvalidN)
        } else {
            Ok(Self { n, padding })
        }
    }

    /// Returns the padding policy this generator applies.
    #[must_use]
    pub fn padding(&self) -> &PaddingPolicy<T> {
        &self.padding
    }
}

impl<T: Clone> NGramGenerator for CharacterGrams<T> {
    type Input = [T];
    type Gram = Vec<T>;
    type Iter<'a>
        = CharacterGramsIter<T>
    where
        Self: 'a,
        T: 'a;

    #[inline]
    fn n(&self) -> usize {
        self.n
    }

    fn grams<'a>(&'a self, input: &'a [T]) -> Self::Iter<'a> {
        // Materialize the padded sequence once, then iterate windows into
        // it. The iterator owns the padded buffer; on the `None`-padding
        // path this is a single clone of the input, which matches the cost
        // of the padded path (a single clone of the input plus a handful
        // of marker copies).
        let padded = self.padding.apply(input, self.n);
        CharacterGramsIter {
            padded,
            n: self.n,
            pos: 0,
        }
    }
}

/// Iterator returned by [`CharacterGrams::grams`].
///
/// Owns the padded input rather than borrowing it, so it does not tie its
/// lifetime to any single call site.
#[derive(Clone, Debug)]
pub struct CharacterGramsIter<T: Clone> {
    /// The padded sequence, materialized once by
    /// [`CharacterGrams::grams`].
    padded: Vec<T>,
    /// The generator's arity, copied here to avoid re-borrowing the
    /// generator.
    n: usize,
    /// The next window's start index within [`padded`](Self::padded).
    pos: usize,
}

impl<T: Clone> Iterator for CharacterGramsIter<T> {
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Vec<T>> {
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

impl<T: Clone> ExactSizeIterator for CharacterGramsIter<T> {}

/// Zero-allocation character-gram windows over a pre-padded input.
///
/// This is the fast path when the caller has already committed to a
/// specific padding layout (or does not want padding at all). Because it
/// yields borrowed slices, it does not implement [`NGramGenerator`]; see
/// the [module docs][self] for why.
///
/// The type is a plain wrapper around the arity — it holds no allocation
/// and can be constructed on demand.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharacterGramSlices {
    /// The generator's arity. Constructors reject zero.
    n: usize,
}

impl CharacterGramSlices {
    /// Constructs a new fast-path generator.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`.
    #[must_use]
    pub const fn new(n: usize) -> Self {
        assert!(n >= 1, "n-gram arity `n` must be at least 1");
        Self { n }
    }

    /// Constructs a new fast-path generator, returning [`InvalidN`] if
    /// `n == 0`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidN`] if `n == 0`.
    pub const fn try_new(n: usize) -> Result<Self, InvalidN> {
        if n == 0 {
            Err(InvalidN)
        } else {
            Ok(Self { n })
        }
    }

    /// Returns the generator's arity.
    #[inline]
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Iterates borrowed windows into `input`. Yields the empty iterator
    /// when `input.len() < self.n`.
    #[inline]
    pub fn grams<'a, T>(&self, input: &'a [T]) -> core::slice::Windows<'a, T> {
        input.windows(self.n)
    }

    /// Returns the exact number of grams this generator will yield from
    /// an input of length `input_len`.
    ///
    /// Equivalent to
    /// `count_grams(input_len, self.n, &PaddingPolicy::None)`.
    #[must_use]
    pub fn count(&self, input_len: usize) -> usize {
        input_len.saturating_sub(self.n - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn unpadded_bigrams_over_cat() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let grams: Vec<Vec<char>> = generator.grams(&['c', 'a', 't']).collect();
        assert_eq!(grams, vec![vec!['c', 'a'], vec!['a', 't']]);
    }

    #[test]
    fn unpadded_unigrams_over_cat() {
        let generator = CharacterGrams::new(1, PaddingPolicy::<char>::None);
        let grams: Vec<Vec<char>> = generator.grams(&['c', 'a', 't']).collect();
        assert_eq!(grams, vec![vec!['c'], vec!['a'], vec!['t']]);
    }

    #[test]
    fn boundary_padded_bigrams_over_cat() {
        let generator = CharacterGrams::new(
            2,
            PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
        );
        let grams: Vec<Vec<char>> = generator.grams(&['c', 'a', 't']).collect();
        assert_eq!(
            grams,
            vec![
                vec!['^', 'c'],
                vec!['c', 'a'],
                vec!['a', 't'],
                vec!['t', '$'],
            ]
        );
    }

    #[test]
    fn unpadded_too_short_yields_nothing() {
        let generator = CharacterGrams::new(4, PaddingPolicy::<char>::None);
        let grams: Vec<Vec<char>> = generator.grams(&['c', 'a', 't']).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn boundary_padded_too_short_still_yields_grams() {
        let generator = CharacterGrams::new(
            4,
            PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
        );
        let grams: Vec<Vec<char>> = generator.grams(&['c', 'a', 't']).collect();
        // Padded sequence is ['^','^','^','c','a','t','$','$','$'] (length 9),
        // which contains 9 - 4 + 1 = 6 four-grams.
        assert_eq!(grams.len(), 6);
        assert_eq!(grams.first(), Some(&vec!['^', '^', '^', 'c']));
        assert_eq!(grams.last(), Some(&vec!['t', '$', '$', '$']));
    }

    #[test]
    fn empty_input_unpadded_yields_nothing() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let grams: Vec<Vec<char>> = generator.grams(&[]).collect();
        assert!(grams.is_empty());
    }

    #[test]
    fn empty_input_boundary_padded_yields_boundary_grams() {
        let generator = CharacterGrams::new(
            3,
            PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
        );
        let grams: Vec<Vec<char>> = generator.grams(&[]).collect();
        // Padded sequence is ['^','^','$','$'] — two trigrams.
        assert_eq!(grams, vec![vec!['^', '^', '$'], vec!['^', '$', '$'],]);
    }

    #[test]
    fn size_hint_is_exact() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let iter = generator.grams(&['a', 'b', 'c', 'd', 'e']);
        assert_eq!(iter.size_hint(), (4, Some(4)));
    }

    #[test]
    fn slices_yields_borrowed_windows() {
        let generator = CharacterGramSlices::new(3);
        let input = [1u8, 2, 3, 4, 5];
        let grams: Vec<&[u8]> = generator.grams(&input).collect();
        assert_eq!(
            grams,
            vec![&[1u8, 2, 3][..], &[2, 3, 4][..], &[3, 4, 5][..]]
        );
    }

    #[test]
    fn slices_count_agrees_with_iterator() {
        let generator = CharacterGramSlices::new(3);
        let input = [1u8, 2, 3, 4, 5];
        assert_eq!(
            generator.count(input.len()),
            generator.grams(&input).count()
        );
    }

    #[test]
    #[should_panic(expected = "at least 1")]
    fn character_grams_new_panics_on_zero() {
        let _ = CharacterGrams::<char>::new(0, PaddingPolicy::None);
    }

    #[test]
    fn character_grams_try_new_rejects_zero() {
        let e = CharacterGrams::<char>::try_new(0, PaddingPolicy::None).unwrap_err();
        assert_eq!(e, InvalidN);
    }

    #[test]
    #[should_panic(expected = "at least 1")]
    fn slices_new_panics_on_zero() {
        let _ = CharacterGramSlices::new(0);
    }

    #[test]
    fn slices_try_new_rejects_zero() {
        assert_eq!(CharacterGramSlices::try_new(0), Err(InvalidN));
    }

    #[test]
    fn byte_generator_is_a_full_citizen() {
        // Same shape as the char case but over `u8`. Guards against
        // over-specialization for character-shaped symbols in the
        // implementation.
        let generator = CharacterGrams::new(
            2,
            PaddingPolicy::Boundary {
                start: 0u8,
                end: 255u8,
            },
        );
        let grams: Vec<Vec<u8>> = generator.grams(&[1u8, 2, 3]).collect();
        assert_eq!(
            grams,
            vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 255]]
        );
    }
}
