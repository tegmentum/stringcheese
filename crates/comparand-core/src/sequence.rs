//! Sequence abstractions.
//!
//! Comparand algorithms operate over sequences of comparable symbols rather
//! than always over UTF-8 bytes. The traits in this module are intentionally
//! narrow — they expose only what generic distance kernels need. Specialized
//! kernels for `&[u8]`, `&str` over Unicode scalar values, grapheme
//! clusters, and token slices are provided by higher-level crates and
//! usually outperform the fully generic path.
//!
//! # No `impl` for `&str`
//!
//! There is intentionally no [`IndexableSequence`] impl for `&str`. A
//! string's "length" and "get by index" are meaningless without a
//! representation choice: UTF-8 bytes, Unicode scalar values, or grapheme
//! clusters all give different answers. Comparand refuses to make that
//! choice silently — callers convert to `&[u8]`, `&[char]`, or a grapheme
//! slice explicitly.

/// A sequence of items indexable by position in `O(1)` time.
///
/// Implementations must be `O(1)` for both [`len`] and [`get`]. Sequences
/// whose indexing is not `O(1)` (`&str` under any character-oriented
/// interpretation, iterators, linked lists) should not implement this trait
/// directly; instead they should be materialized into an `O(1)` form before
/// comparison.
///
/// [`len`]: IndexableSequence::len
/// [`get`]: IndexableSequence::get
pub trait IndexableSequence {
    /// The symbol type.
    type Item;

    /// The length of the sequence in items.
    fn len(&self) -> usize;

    /// Returns `true` if the sequence has zero items.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the item at `index`, or `None` if `index >= len()`.
    fn get(&self, index: usize) -> Option<&Self::Item>;
}

impl<T> IndexableSequence for [T] {
    type Item = T;

    #[inline]
    fn len(&self) -> usize {
        <[T]>::len(self)
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        <[T]>::get(self, index)
    }
}

impl<T, const N: usize> IndexableSequence for [T; N] {
    type Item = T;

    #[inline]
    fn len(&self) -> usize {
        N
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        <[T]>::get(self, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_impl() {
        let bytes: &[u8] = b"hello";
        assert_eq!(IndexableSequence::len(bytes), 5);
        assert!(!IndexableSequence::is_empty(bytes));
        assert_eq!(IndexableSequence::get(bytes, 0), Some(&b'h'));
        assert_eq!(IndexableSequence::get(bytes, 4), Some(&b'o'));
        assert_eq!(IndexableSequence::get(bytes, 5), None);
    }

    #[test]
    fn array_impl() {
        let arr: [char; 3] = ['a', 'b', 'c'];
        assert_eq!(IndexableSequence::len(&arr), 3);
        assert_eq!(IndexableSequence::get(&arr, 1), Some(&'b'));
        assert_eq!(IndexableSequence::get(&arr, 3), None);
    }

    #[test]
    fn empty_slice() {
        let empty: &[i32] = &[];
        assert_eq!(IndexableSequence::len(empty), 0);
        assert!(IndexableSequence::is_empty(empty));
        assert_eq!(IndexableSequence::get(empty, 0), None);
    }
}
