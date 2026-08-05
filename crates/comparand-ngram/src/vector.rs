//! The [`GramVector`] weighted-vector representation.
//!
//! # What lives here, what doesn't
//!
//! [`GramVector`] is the representation TF–IDF weighting and cosine
//! similarity land on top of. This module contains the type, the
//! normalization helpers (`normalize_l1`, `normalize_l2`), and a dot
//! product between two vectors of matching gram type.
//!
//! It intentionally *does not* provide cosine similarity itself: cosine
//! is a similarity kernel, and similarity kernels belong in the
//! set-similarity crate that lands on top of this representation layer.
//! Dot product is the minimum a downstream cosine implementation needs
//! from us, and it lives here because the storage layout is opaque to
//! consumers.
//!
//! # Backing store
//!
//! [`BTreeMap<G, f64>`] for the same reasons [`GramSet`](crate::GramSet)
//! and [`GramMultiSet`](crate::GramMultiSet) use ordered maps:
//! `no_std`+`alloc` compatibility and deterministic iteration order.
//!
//! Floating-point weights are stored as `f64` for the reasons the
//! result-type discussion of `NormalizedSimilarity` gives — `f64` is the
//! natural type for most continuous similarity work and matches
//! `Similarity<f64>`'s default parameter.
//!
//! # Weighting schemes we defer
//!
//! Corpus-level TF–IDF weighting requires a document-frequency table
//! computed over the corpus, which is a preprocessing-pipeline concern
//! and is out of scope for this representation-layer crate. Sublinear
//! TF (`1 + log(count)`) and BM25-shaped weightings are similarly
//! deferred. Callers can populate a [`GramVector`] with any weights they
//! choose; this module supplies the storage and the normalization.
//!
//! [`BTreeMap<G, f64>`]: alloc::collections::BTreeMap

use alloc::collections::BTreeMap;

use crate::generator::NGramGenerator;

/// A sparse gram vector — grams paired with weights.
///
/// Iteration order follows the underlying [`BTreeMap`], i.e. ascending
/// gram order.
///
/// [`BTreeMap`]: alloc::collections::BTreeMap
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GramVector<G: Ord> {
    /// Gram-to-weight map. Absent entries have implicit weight `0.0`.
    counts: BTreeMap<G, f64>,
}

impl<G: Ord> GramVector<G> {
    /// Constructs an empty gram vector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: BTreeMap::new(),
        }
    }

    /// Builds a vector from a generator whose gram counts become the
    /// per-gram weights (`f64`-converted).
    ///
    /// This is the natural starting point for cosine or weighted-Jaccard
    /// consumers: the vector is initialized with raw counts and the
    /// caller can then reweight (TF–IDF, BM25, log-scaling) in place
    /// before normalizing.
    #[must_use]
    pub fn from_generator_counts<Gen>(generator: &Gen, input: &Gen::Input) -> Self
    where
        Gen: NGramGenerator<Gram = G>,
    {
        let mut counts = BTreeMap::new();
        for g in generator.grams(input) {
            *counts.entry(g).or_insert(0.0f64) += 1.0;
        }
        Self { counts }
    }

    /// Returns the number of distinct grams (non-zero-weight support
    /// entries in the vector; zero-weight entries left by the caller are
    /// counted too, since the storage is sparse and cannot tell them
    /// apart from a genuine zero-with-storage).
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// Returns `true` if the vector has no stored entries.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Returns the weight of `gram`, or `0.0` if it is absent.
    #[must_use]
    pub fn weight(&self, gram: &G) -> f64 {
        self.counts.get(gram).copied().unwrap_or(0.0)
    }

    /// Iterates the vector's `(gram, weight)` pairs in ascending gram
    /// order.
    pub fn iter(&self) -> alloc::collections::btree_map::Iter<'_, G, f64> {
        self.counts.iter()
    }

    /// Sets the weight of `gram` to `weight`, replacing any previous
    /// entry.
    pub fn set(&mut self, gram: G, weight: f64) {
        self.counts.insert(gram, weight);
    }

    /// Adds `delta` to the weight of `gram`, inserting a new entry if
    /// necessary.
    pub fn add(&mut self, gram: G, delta: f64)
    where
        G: Clone,
    {
        *self.counts.entry(gram).or_insert(0.0) += delta;
    }

    /// The L1 norm of the vector — the sum of the absolute values of
    /// the weights.
    #[must_use]
    pub fn l1_norm(&self) -> f64 {
        self.counts.values().map(|w| w.abs()).sum()
    }

    /// The sum of the squared weights — the L2 norm before the square
    /// root. Available in every build; [`l2_norm`](Self::l2_norm) itself
    /// requires `std` because `f64::sqrt` is not part of `core`.
    #[must_use]
    pub fn l2_norm_squared(&self) -> f64 {
        self.counts.values().map(|w| w * w).sum()
    }

    /// The L2 norm of the vector — the Euclidean length.
    ///
    /// This method requires the `std` feature. `f64::sqrt` is a
    /// libm-backed floating-point intrinsic and is not part of Rust's
    /// `core`; a pure `no_std`+`alloc` build has access to
    /// [`l2_norm_squared`](Self::l2_norm_squared) but not to the square
    /// root itself. Downstream cosine-similarity kernels that live under
    /// their own `std` gate can bridge the two.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn l2_norm(&self) -> f64 {
        self.l2_norm_squared().sqrt()
    }

    /// Normalizes the vector to unit L1 norm in place. No-op when the
    /// vector is empty or the L1 norm is zero.
    pub fn normalize_l1(&mut self) {
        let n = self.l1_norm();
        if n == 0.0 || !is_finite_no_std(n) {
            return;
        }
        for w in self.counts.values_mut() {
            *w /= n;
        }
    }

    /// Normalizes the vector to unit L2 norm in place. No-op when the
    /// vector is empty or the L2 norm is zero.
    ///
    /// Requires `std` for the same reason [`l2_norm`](Self::l2_norm)
    /// does.
    #[cfg(feature = "std")]
    pub fn normalize_l2(&mut self) {
        let n = self.l2_norm();
        if n == 0.0 || !n.is_finite() {
            return;
        }
        for w in self.counts.values_mut() {
            *w /= n;
        }
    }
}

/// A `no_std`-friendly `is_finite`. `f64::is_finite` is stable in core
/// since Rust 1.85 (see the `float_is_finite` tracking issue), but the
/// classify-via-bit-pattern form here works on every version this crate
/// targets without a doc-comment on the guarantee.
#[inline]
fn is_finite_no_std(x: f64) -> bool {
    // Exponent bits all set means Inf or NaN; anything else is finite.
    (x.to_bits() & 0x7ff0_0000_0000_0000) != 0x7ff0_0000_0000_0000
}

impl<'a, G: Ord> IntoIterator for &'a GramVector<G> {
    type Item = (&'a G, &'a f64);
    type IntoIter = alloc::collections::btree_map::Iter<'a, G, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<G: Ord + Clone> GramVector<G> {
    /// The dot product of `self` and `other`.
    ///
    /// Iterates the smaller of the two vectors and does a lookup in the
    /// larger — the natural sparse dot product for sorted maps.
    #[must_use]
    pub fn dot(&self, other: &Self) -> f64 {
        // Iterate the smaller side to keep the loop count small; look up
        // in the larger.
        let (small, large) = if self.counts.len() <= other.counts.len() {
            (&self.counts, &other.counts)
        } else {
            (&other.counts, &self.counts)
        };
        small
            .iter()
            .filter_map(|(g, &w)| large.get(g).map(|&v| w * v))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::CharacterGrams;
    use crate::padding::PaddingPolicy;

    #[test]
    fn from_generator_counts_matches_multiset_totals() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let v = GramVector::from_generator_counts(&generator, &['a', 'a', 'a', 'b']);
        // Bigrams: ['a','a'], ['a','a'], ['a','b']
        assert!((v.weight(&alloc::vec!['a', 'a']) - 2.0).abs() < 1e-12);
        assert!((v.weight(&alloc::vec!['a', 'b']) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn l1_norm_of_normalized_is_one() {
        let mut v: GramVector<u8> = GramVector::new();
        v.set(1u8, 1.0);
        v.set(2u8, 3.0);
        v.set(3u8, -2.0);
        v.normalize_l1();
        assert!((v.l1_norm() - 1.0).abs() < 1e-12);
    }

    #[cfg(feature = "std")]
    #[test]
    fn l2_norm_of_normalized_is_one() {
        let mut v: GramVector<u8> = GramVector::new();
        v.set(1u8, 3.0);
        v.set(2u8, 4.0);
        v.normalize_l2();
        assert!((v.l2_norm() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_l1_on_empty_is_noop() {
        let mut v: GramVector<u8> = GramVector::new();
        v.normalize_l1();
        // An empty vector has L1 norm exactly zero — bitwise equality is
        // what we want.
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(v.l1_norm(), 0.0);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn normalize_l2_on_zero_norm_is_noop() {
        // A single zero-weight entry has L2 norm 0; normalization must
        // not produce NaN.
        let mut v: GramVector<u8> = GramVector::new();
        v.set(1u8, 0.0);
        v.normalize_l2();
        assert!(v.weight(&1u8).is_finite());
    }

    #[test]
    fn dot_product_matches_hand_computation() {
        let mut a: GramVector<u8> = GramVector::new();
        a.set(1, 1.0);
        a.set(2, 2.0);
        a.set(3, 3.0);
        let mut b: GramVector<u8> = GramVector::new();
        b.set(2, 4.0);
        b.set(3, 5.0);
        b.set(4, 6.0);
        // Overlap on {2, 3}: 2*4 + 3*5 = 8 + 15 = 23
        assert!((a.dot(&b) - 23.0).abs() < 1e-12);
    }

    #[test]
    fn dot_product_is_symmetric() {
        let mut a: GramVector<u8> = GramVector::new();
        a.set(1, 1.5);
        a.set(2, -0.5);
        let mut b: GramVector<u8> = GramVector::new();
        b.set(1, 2.0);
        b.set(2, 3.0);
        b.set(9, 100.0);
        assert!((a.dot(&b) - b.dot(&a)).abs() < 1e-12);
    }

    #[test]
    fn add_accumulates_weight() {
        let mut v: GramVector<u8> = GramVector::new();
        v.add(1, 0.5);
        v.add(1, 0.25);
        assert!((v.weight(&1) - 0.75).abs() < 1e-12);
    }
}
