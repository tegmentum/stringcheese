//! Weighted `MinHash` via Consistent Weighted Sampling (Ioffe 2010).
//!
//! # What it estimates
//!
//! Given two weighted multisets `A = { (k, w_k^A) }` and `B = { (k, w_k^B) }`
//! with `w_k^{A,B} >= 0`, the *generalized* ("weighted") Jaccard
//! similarity is
//!
//! ```text
//!     J_w(A, B) = Σ_k min(w_k^A, w_k^B) / Σ_k max(w_k^A, w_k^B)
//! ```
//!
//! which reduces to the ordinary set Jaccard when every weight is `0`
//! or `1`. The estimator here is unbiased:
//! `Pr[sketch_A[i] == sketch_B[i]] = J_w(A, B)` for every `i`, so the
//! fraction of matching sketch positions is an unbiased estimate of
//! `J_w(A, B)`.
//!
//! # Ioffe's Consistent Weighted Sampling
//!
//! Per sketch position `i`, for every element `k` with weight `w > 0`:
//!
//! ```text
//!     draw r_k, c_k ~ Gamma(2, 1)
//!     draw β_k       ~ Uniform(0, 1)
//!     t_k = floor(ln(w) / r_k + β_k)
//!     y_k = exp(r_k · (t_k - β_k))
//!     a_k = c_k / (y_k · exp(r_k))
//! ```
//!
//! and the sketch stores `(k*, y_k*)` where `k*` is the argmin of `a_k`.
//! The Gamma(2, 1) draws are made via the sum of two Exponential(1)
//! draws (each `-ln(U)` for `U ~ Uniform(0, 1)`) — the classical
//! textbook construction, exact modulo the finite-precision `ln`.
//!
//! The randomness `(r_k, c_k, β_k)` is generated deterministically from
//! `(element_hash, sketch_seed, position)` via [`crate::hash::splitmix64`]
//! — every derivation shared by two sketches must produce identical
//! draws, and reproducibility across a producer / consumer boundary is
//! a hard requirement.
//!
//! # Std gate
//!
//! `f64::ln`, `f64::exp`, and `f64::floor` are `std` operations. The
//! module compiles only when both `alloc` and `std` are enabled;
//! `--no-default-features --features alloc` builds see an empty module,
//! which is the same rule `stringcheese-set-similarity` applies to its
//! `Cosine` implementation.
//!
//! # References
//!
//! * Ioffe, S. (2010). "Improved consistent sampling, weighted minhash
//!   and L1 sketching." *2010 IEEE International Conference on Data
//!   Mining*, 246-255. <https://doi.org/10.1109/ICDM.2010.80> —
//!   introduces the Consistent Weighted Sampling scheme implemented here.

#![cfg(feature = "std")]

use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hash;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::hash::{permutation_seed, portable_hash, splitmix64};

/// Ioffe 2010 Consistent Weighted Sampling Jaccard-estimator descriptor.
pub const WEIGHTED_MINHASH_JACCARD_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::WeightedJaccard,
    VariantId("consistent-weighted-sampling-ioffe-2010"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "Improved consistent sampling, weighted minhash and L1 sketching",
        authors: "S. Ioffe",
        year: 2010,
    },
);

/// A weighted `MinHash` sketch — sequence of `(element_key_hash, y_value)`
/// pairs, one per sketch position.
///
/// Two sketches with the same size and seed can be compared with
/// [`WeightedMinHashSketch::estimated_weighted_jaccard`].
///
/// The stored `y_value` is the [`f64`] `y_k` from the Ioffe construction
/// — retaining it in the sketch (rather than storing only the element
/// key) matches the standard formulation and makes the sketch a
/// self-contained "sample" of the input multiset.
#[derive(Clone, Debug, PartialEq)]
pub struct WeightedMinHashSketch {
    /// Per-position `(element_hash, y_value)` pairs. Empty inputs
    /// receive `(u64::MAX, 0.0)` in every slot.
    signatures: Vec<(u64, f64)>,
    /// The caller-supplied seed. Compared for equality before the
    /// estimator runs.
    seed: u64,
}

impl WeightedMinHashSketch {
    /// Constructs a weighted sketch of size `k` seeded with `seed`, from
    /// an iterator of `(element, weight)` pairs.
    ///
    /// Elements with `weight <= 0.0` are silently skipped; a caller who
    /// wants a specific per-element weight of exactly `0.0` to
    /// participate should raise it to a tiny positive epsilon or supply
    /// a different item type.
    ///
    /// Elements repeated in the iterator have their weights summed —
    /// the sketch treats the input as a *multiset*, and repeat entries
    /// are equivalent to a single entry whose weight is the total.
    ///
    /// # Panics
    ///
    /// Panics if `k == 0`, or if any supplied weight is NaN.
    #[must_use]
    pub fn from_weighted<G, I>(k: usize, seed: u64, weighted_items: I) -> Self
    where
        G: Hash,
        I: IntoIterator<Item = (G, f64)>,
    {
        assert!(k > 0, "sketch size k must be > 0");

        // Collect (element_hash, weight_total) via a small BTreeMap-like
        // accumulator over the element hash. We could also do this online
        // by re-computing per element, but coalescing lets a caller pass a
        // possibly-repeated iterator without penalizing them.
        let mut coalesced: alloc::collections::BTreeMap<u64, f64> =
            alloc::collections::BTreeMap::new();
        for (g, w) in weighted_items {
            assert!(!w.is_nan(), "weight must not be NaN");
            if w <= 0.0 {
                continue;
            }
            let key = portable_hash(0, &g);
            *coalesced.entry(key).or_insert(0.0) += w;
        }

        // Empty-input sketch: sentinel (u64::MAX, 0.0) everywhere.
        // The comparator handles the empty-vs-empty case specially.
        if coalesced.is_empty() {
            return Self {
                signatures: vec![(u64::MAX, 0.0); k],
                seed,
            };
        }

        let mut signatures: Vec<(u64, f64)> = Vec::with_capacity(k);
        for pos in 0..k {
            let pos_seed = permutation_seed(seed, pos);
            let mut best_a = f64::INFINITY;
            let mut best_key: u64 = u64::MAX;
            let mut best_y: f64 = 0.0;
            for (key, weight) in &coalesced {
                let (a, y) = cws_sample(*key, *weight, pos_seed);
                if a < best_a {
                    best_a = a;
                    best_key = *key;
                    best_y = y;
                }
            }
            signatures.push((best_key, best_y));
        }

        Self { signatures, seed }
    }

    /// Returns the sketch size `k`.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.signatures.len()
    }

    /// Returns the sketch's stored seed.
    #[inline]
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the sketch's per-position `(element_hash, y_value)` pairs.
    #[inline]
    #[must_use]
    pub fn signatures(&self) -> &[(u64, f64)] {
        &self.signatures
    }

    /// Returns `true` if the sketch was constructed from an empty (or
    /// all-nonpositive-weight) input.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        // The sentinel is bit-exact `(u64::MAX, 0.0)`; a real sample
        // never produces `u64::MAX` as an element hash with astronomical
        // probability (~2^-64).
        self.signatures
            .iter()
            .all(|(k, y)| *k == u64::MAX && *y == 0.0)
    }

    /// The algorithm descriptor for the weighted-Jaccard estimator this
    /// sketch participates in.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        WEIGHTED_MINHASH_JACCARD_DESCRIPTOR
    }

    /// Estimates the weighted (generalized) Jaccard similarity between
    /// this sketch and `other`.
    ///
    /// The estimate is the fraction of positions where the two sketches
    /// carry the same `(element_hash, y_value)` pair. Two empty sketches
    /// return `1.0` bit-exactly under the crate-wide empty-vs-empty
    /// identity convention.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches have different sizes or seeds.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "sketch size k fits in u32 for every practical configuration"
    )]
    pub fn estimated_weighted_jaccard(&self, other: &Self) -> f64 {
        assert_eq!(
            self.signatures.len(),
            other.signatures.len(),
            "estimated_weighted_jaccard requires equal sketch sizes"
        );
        assert_eq!(
            self.seed, other.seed,
            "estimated_weighted_jaccard requires equal seeds"
        );

        if self.is_empty() && other.is_empty() {
            return 1.0;
        }

        let mut matches: usize = 0;
        for ((ka, ya), (kb, yb)) in self.signatures.iter().zip(other.signatures.iter()) {
            // Bit-exact equality on both the u64 key and the f64 y-value.
            // `y_k` is deterministic given (k, position, seed) — matching
            // keys implies matching y-values in exact arithmetic; the
            // extra check guards against a hypothetical y-only regression
            // that would otherwise silently violate the estimator.
            if ka == kb && ya.to_bits() == yb.to_bits() {
                matches += 1;
            }
        }
        matches as f64 / self.signatures.len() as f64
    }
}

/// One consistent-weighted-sampling draw for element `key` with weight
/// `weight` at the given position seed.
///
/// Returns `(a_k, y_k)` — the argmin-key and the y-value stored in the
/// sketch alongside the argmin element.
fn cws_sample(key: u64, weight: f64, pos_seed: u64) -> (f64, f64) {
    // Deterministic stream of five u64s from (key, pos_seed).
    let s0 = splitmix64(key ^ pos_seed);
    let s1 = splitmix64(s0);
    let s2 = splitmix64(s1);
    let s3 = splitmix64(s2);
    let s4 = splitmix64(s3);

    // Convert each state to a value in (0, 1). Using the (n + 0.5) / 2^53
    // encoding avoids both 0 (which would blow up ln) and 1.
    let u1 = uniform_open(s0);
    let u2 = uniform_open(s1);
    let u3 = uniform_open(s2);
    let u4 = uniform_open(s3);
    let u5 = uniform_open(s4);

    // r_k ~ Gamma(2, 1) via sum of two Exponential(1) draws.
    let r_k = -u1.ln() - u2.ln();
    // c_k ~ Gamma(2, 1) via the same construction, independent stream.
    let c_k = -u3.ln() - u4.ln();
    // β_k ~ Uniform(0, 1).
    let beta_k = u5;

    // Ioffe's Consistent Weighted Sampling core.
    let ln_w = weight.ln();
    let t_k = (ln_w / r_k + beta_k).floor();
    let y_k = (r_k * (t_k - beta_k)).exp();
    let z_k = y_k * r_k.exp();
    let a_k = c_k / z_k;

    (a_k, y_k)
}

/// Maps a `u64` state to a value in the open interval `(0, 1)`.
///
/// The `+ 0.5` shift keeps the extremes strictly interior, so the
/// caller can safely take `ln` of the result. We use `state >> 12` (52
/// bits) rather than `state >> 11` (53 bits) because at magnitude
/// `2^53` the f64 spacing is `1.0`, and rounding `(2^53 - 1) + 0.5`
/// to nearest-even produces `2^53` exactly — which after division by
/// `2^53` gives `1.0`, breaking the "strictly less than 1" invariant.
/// At magnitude `2^52` the spacing is `0.5`, so `(2^52 - 1) + 0.5`
/// stays exactly representable.
#[inline]
#[allow(
    clippy::cast_precision_loss,
    reason = "the intermediate value is in [0, 2^52), which is exactly representable in f64"
)]
fn uniform_open(state: u64) -> f64 {
    let x = (state >> 12) as f64 + 0.5;
    let denom = (1u64 << 52) as f64;
    x / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_open_is_strictly_interior() {
        for s in [0u64, 1, u64::MAX, 0xdead_beef_dead_beef] {
            let u = uniform_open(s);
            assert!(u > 0.0 && u < 1.0, "uniform_open({s}) = {u} not in (0, 1)");
        }
    }

    #[test]
    fn empty_input_is_all_sentinel() {
        let s: WeightedMinHashSketch =
            WeightedMinHashSketch::from_weighted::<u8, _>(8, 42, core::iter::empty());
        assert!(s.is_empty());
    }

    #[test]
    fn empty_vs_empty_is_one_bit_exact() {
        let a: WeightedMinHashSketch =
            WeightedMinHashSketch::from_weighted::<u8, _>(8, 42, core::iter::empty());
        let b = a.clone();
        assert_eq!(
            a.estimated_weighted_jaccard(&b).to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn identical_inputs_produce_identical_sketches() {
        let a = WeightedMinHashSketch::from_weighted(32, 7, [(1u32, 1.0), (2, 2.0), (3, 3.0)]);
        let b = WeightedMinHashSketch::from_weighted(32, 7, [(1u32, 1.0), (2, 2.0), (3, 3.0)]);
        assert_eq!(a.signatures(), b.signatures());
        assert_eq!(
            a.estimated_weighted_jaccard(&b).to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn unit_weights_reduce_to_unweighted_jaccard_shape() {
        // Two sets with 50%-overlap unit weights should estimate close
        // to the true 0.5 for sufficiently large k.
        let a = WeightedMinHashSketch::from_weighted(
            256,
            7,
            [(1u32, 1.0), (2, 1.0), (3, 1.0), (4, 1.0)],
        );
        let b = WeightedMinHashSketch::from_weighted(
            256,
            7,
            [(3u32, 1.0), (4, 1.0), (5, 1.0), (6, 1.0)],
        );
        // True Jaccard: 2/6 = 0.3333... ; allow a broad tolerance for
        // finite-k variance.
        let est = a.estimated_weighted_jaccard(&b);
        assert!(
            (est - (2.0_f64 / 6.0_f64)).abs() < 0.25,
            "estimate {est} too far from 1/3"
        );
    }

    #[test]
    fn coalesces_repeated_entries() {
        // Passing (k, 1.0) twice must be equivalent to passing (k, 2.0)
        // once — the sketch treats the input as a multiset over weights.
        let a = WeightedMinHashSketch::from_weighted(64, 7, [(1u32, 1.0), (1, 1.0), (2, 1.0)]);
        let b = WeightedMinHashSketch::from_weighted(64, 7, [(1u32, 2.0), (2, 1.0)]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    fn skips_nonpositive_weights() {
        let a = WeightedMinHashSketch::from_weighted(64, 7, [(1u32, 1.0), (2, 0.0)]);
        let b = WeightedMinHashSketch::from_weighted(64, 7, [(1u32, 1.0)]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    #[should_panic(expected = "weight must not be NaN")]
    fn nan_weight_panics() {
        let _: WeightedMinHashSketch =
            WeightedMinHashSketch::from_weighted(8, 42, [(1u32, f64::NAN)]);
    }

    #[test]
    #[should_panic(expected = "k must be > 0")]
    fn zero_k_panics() {
        let _: WeightedMinHashSketch =
            WeightedMinHashSketch::from_weighted::<u8, _>(0, 42, core::iter::empty());
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = WeightedMinHashSketch::descriptor();
        assert_eq!(d.family, AlgorithmFamily::WeightedJaccard);
        assert_eq!(
            d.variant,
            VariantId("consistent-weighted-sampling-ioffe-2010")
        );
    }
}
