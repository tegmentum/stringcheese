//! `SimHash` (Charikar 2002) — signed random projections for cosine LSH.
//!
//! # What it estimates
//!
//! Given a multiset `A` over a hashable domain (typically bag-of-words or
//! bag-of-grams), the sketch is a single `u64` whose bit `i` is the sign
//! of `Σ_{g ∈ A} r_i(g)`, where `r_i(g) ∈ {−1, +1}` is a deterministic
//! per-bit-per-gram pseudo-random label. Two sketches `H_A` and `H_B` have
//! expected Hamming distance
//!
//! ```text
//!     E[hamming(H_A, H_B)] = 64 · θ(A, B) / π
//! ```
//!
//! where `θ(A, B) = arccos(cos_similarity(A, B))` is the angle between the
//! aggregate vectors implied by `A` and `B`. Inverting gives the cosine
//! estimator
//!
//! ```text
//!     cos_similarity(A, B) ≈ cos(π · hamming(H_A, H_B) / 64)
//! ```
//!
//! implemented by [`SimHashSketch::estimated_cosine_similarity`].
//!
//! # Multiset semantics
//!
//! Unlike the ordinary [`crate::minhash::MinHashSketch`], `SimHash`'s
//! sign-of-sum construction is *not* set-invariant: repeated grams
//! contribute repeatedly to the accumulator. This matches Charikar's
//! original formulation, which is defined on frequency-weighted vectors
//! (bag-of-words), and is what makes the sketch a cosine estimator rather
//! than a Jaccard estimator.
//!
//! A caller who wants set semantics (each distinct gram contributes once)
//! should coalesce their input to distinct items upstream — e.g. by
//! collecting into a `BTreeSet` before calling
//! [`SimHashSketch::from_iter`].
//!
//! # Signature width
//!
//! The signature is a fixed `u64` — 64 signed random projections. Wider
//! signatures (128 or 256 bits) buy proportionally lower Hamming-distance
//! standard error at the cost of a wider storage word; the 64-bit choice
//! matches every common `SimHash` deployment and keeps the sketch a
//! single-word value that fits in a register.
//!
//! # `std` gate on the cosine estimator
//!
//! [`SimHashSketch::estimated_cosine_similarity`] uses `f64::cos`, which
//! lives in `std`; it is compiled only under `--features std`.
//! [`SimHashSketch::hamming_distance`] is `alloc`-only — the sketch itself
//! is bit-integer arithmetic and does not need floats.
//!
//! # References
//!
//! * Charikar, M. S. (2002). "Similarity estimation techniques from
//!   rounding algorithms." *Proceedings of the 34th Annual ACM Symposium
//!   on Theory of Computing (STOC '02)*, 380-388.
//!   <https://doi.org/10.1145/509907.509965> — introduces `SimHash` as an
//!   LSH scheme for cosine similarity via signed random projections.

use core::hash::Hash;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::minhash::hash::{permutation_seed, permuted_hash, portable_hash};

/// Bit-width of the `SimHash` signature. Fixed at 64 — see the
/// [module-level documentation](self) for the choice's rationale.
pub const SIMHASH_BITS: u32 = 64;

/// Charikar 2002 `SimHash` cosine-similarity estimator descriptor.
pub const SIMHASH_COSINE_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::Cosine,
    VariantId("simhash-signed-random-projections-64bit-portable-splitmix"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "Similarity estimation techniques from rounding algorithms",
        authors: "M. S. Charikar",
        year: 2002,
    },
);

/// A `SimHash` sketch — a single 64-bit signature whose bit `i` records
/// the sign of the `i`-th signed-random-projection accumulator.
///
/// See the [module-level documentation](self) for the construction and
/// the cosine estimator that consumes two of these.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SimHashSketch {
    /// The 64-bit signature: bit `i` is `1` iff the `i`-th accumulator's
    /// final sign is non-negative.
    signature: u64,
    /// The caller-supplied seed. Two sketches must share seeds to be
    /// comparable — different seeds mean different signed-projection
    /// families, and Hamming distance between them is meaningless.
    seed: u64,
}

impl SimHashSketch {
    /// Constructs a `SimHash` sketch from an iterator of items.
    ///
    /// Each item contributes `±1` to each of the 64 accumulators; the
    /// sign is drawn per (item, bit) from the deterministic hash stream
    /// `splitmix64(portable_hash(item) XOR permutation_seed(seed, bit))`.
    /// After all items are consumed, bit `i` of the output signature is
    /// `1` iff accumulator `i` finished non-negative.
    ///
    /// # Multiset semantics
    ///
    /// Repeated items contribute repeatedly — the sketch is a sketch of a
    /// multiset (bag-of-grams), not a set. See the [module docs](self)
    /// for the reasoning.
    ///
    /// # Empty input
    ///
    /// An empty input leaves every accumulator at `0`. The tie-break at
    /// `0` treats it as the non-negative sign, so the empty-input
    /// signature is [`u64::MAX`] (all bits set). Two empty-input
    /// sketches compare identical (Hamming distance `0`), which yields
    /// [`SimHashSketch::estimated_cosine_similarity`] `= 1.0` bit-exact
    /// under the crate-wide empty-vs-empty identity convention.
    #[must_use]
    pub fn from_iter<G, I>(seed: u64, items: I) -> Self
    where
        G: Hash,
        I: IntoIterator<Item = G>,
    {
        // Pre-materialize per-bit seeds so the inner loop is a
        // straight-line reduction — same pattern as `MinHashSketch`.
        let mut bit_seeds: [u64; SIMHASH_BITS as usize] = [0u64; SIMHASH_BITS as usize];
        for (i, s) in bit_seeds.iter_mut().enumerate() {
            *s = permutation_seed(seed, i);
        }

        // Signed-integer accumulator per bit. `i32` fits every input up
        // to ~2^31 items; a caller with more than that has bigger
        // problems than accumulator overflow.
        let mut accum: [i32; SIMHASH_BITS as usize] = [0i32; SIMHASH_BITS as usize];

        for gram in items {
            let base = portable_hash(0, &gram);
            for (i, a) in accum.iter_mut().enumerate() {
                // A single well-mixed u64 supplies the +1/-1 label for
                // bit `i` of this gram: the top bit of the mix is
                // effectively independent of the top bit for a different
                // seed_i.
                let h = permuted_hash(base, bit_seeds[i]);
                // Top-bit test: `h >> 63` is 0 or 1. Convert to +1 / -1
                // and add. Using the top bit rather than the low bit
                // matches the "sign is the high bit of a mixed hash"
                // idiom and is what `splitmix64`'s excellent high-bit
                // avalanche is meant for.
                let bit = (h >> 63) as i32;
                // bit ∈ {0, 1} → contribution ∈ {-1, +1}.
                let contribution = 2 * bit - 1;
                // `saturating_add` protects against pathologically large
                // inputs; realistic corpora never approach `i32::MAX`
                // items per bin.
                *a = a.saturating_add(contribution);
            }
        }

        // Fold accumulator signs into a u64. Bit `i` is set iff
        // accumulator `i` finished non-negative.
        let mut signature: u64 = 0;
        for (i, a) in accum.iter().enumerate() {
            if *a >= 0 {
                signature |= 1u64 << i;
            }
        }

        Self { signature, seed }
    }

    /// Returns the sketch's 64-bit signature.
    #[inline]
    #[must_use]
    pub fn signature(&self) -> u64 {
        self.signature
    }

    /// Returns the sketch's stored seed. Two sketches with different
    /// seeds are *not* comparable — their signed-projection families
    /// differ.
    #[inline]
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the bit-width of the sketch signature. Constant `64` for
    /// this variant.
    #[inline]
    #[must_use]
    pub const fn bits() -> u32 {
        SIMHASH_BITS
    }

    /// The algorithm descriptor for the cosine estimator this sketch
    /// participates in.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        SIMHASH_COSINE_DESCRIPTOR
    }

    /// Returns the Hamming distance between two signatures — the number
    /// of bit positions in which they differ.
    ///
    /// This is the raw signal on which
    /// [`SimHashSketch::estimated_cosine_similarity`] is built. Small
    /// Hamming distance implies high cosine similarity.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches have different seeds.
    #[must_use]
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        assert_eq!(
            self.seed, other.seed,
            "SimHashSketch::hamming_distance requires equal seeds"
        );
        (self.signature ^ other.signature).count_ones()
    }

    /// Estimates the cosine similarity between the two multisets whose
    /// `SimHash` sketches these are.
    ///
    /// Inverts the Charikar estimator `E[hamming / b] = θ / π` to get
    /// `cos_similarity ≈ cos(π · hamming / bits)`. The estimator is
    /// asymptotically unbiased in the sketch bit-width; at 64 bits its
    /// standard error is approximately `π · sin(θ) / (2·√64)` — a few
    /// percent at any nontrivial similarity.
    ///
    /// # Empty-vs-empty
    ///
    /// Two empty sketches have Hamming distance `0` (both signatures are
    /// `0`), giving `cos(0) = 1.0` bit-exact under the crate-wide
    /// empty-vs-empty identity convention.
    ///
    /// # `std` gate
    ///
    /// Uses `f64::cos`, so this method is only present with the `std`
    /// feature. `no-std` users can read the Hamming distance from
    /// [`SimHashSketch::hamming_distance`] and apply their own inversion.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches have different seeds.
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "Hamming distance is bounded by 64; fits in an f64 mantissa without loss"
    )]
    pub fn estimated_cosine_similarity(&self, other: &Self) -> f64 {
        let h = self.hamming_distance(other);
        let fraction = f64::from(h) / f64::from(SIMHASH_BITS);
        (core::f64::consts::PI * fraction).cos()
    }
}

/// Convenience iterator collector that computes a `SimHash` from any
/// [`IntoIterator`], matching [`crate::minhash::MinHashSketch::from_iter`]'s
/// shape for consumers building both sketches from the same input.
///
/// Distinct from [`SimHashSketch::from_iter`] only in taking the seed
/// last, so a `Vec::into_iter().collect()` chain can be written as
/// `simhash_from(items, seed)`.
#[must_use]
pub fn simhash_from<G, I>(items: I, seed: u64) -> SimHashSketch
where
    G: Hash,
    I: IntoIterator<Item = G>,
{
    SimHashSketch::from_iter(seed, items)
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn empty_sketch_has_all_ones_signature() {
        // Every accumulator finishes at 0; the tie-break `>= 0`
        // classifies each as non-negative, so every bit is set.
        let s: SimHashSketch = SimHashSketch::from_iter::<u8, _>(42, core::iter::empty());
        assert_eq!(s.signature(), u64::MAX);
    }

    #[test]
    fn empty_vs_empty_hamming_is_zero() {
        let a: SimHashSketch = SimHashSketch::from_iter::<u8, _>(42, core::iter::empty());
        let b: SimHashSketch = SimHashSketch::from_iter::<u8, _>(42, core::iter::empty());
        assert_eq!(a.hamming_distance(&b), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn empty_vs_empty_cosine_is_one_bit_exact() {
        let a: SimHashSketch = SimHashSketch::from_iter::<u8, _>(42, core::iter::empty());
        let b: SimHashSketch = SimHashSketch::from_iter::<u8, _>(42, core::iter::empty());
        assert_eq!(
            a.estimated_cosine_similarity(&b).to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn identical_inputs_produce_identical_signatures() {
        let a = SimHashSketch::from_iter(7, [1u32, 2, 3, 4, 5]);
        let b = SimHashSketch::from_iter(7, [1u32, 2, 3, 4, 5]);
        assert_eq!(a.signature(), b.signature());
        assert_eq!(a.hamming_distance(&b), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn identical_inputs_estimate_cosine_one() {
        let a = SimHashSketch::from_iter(7, [1u32, 2, 3, 4, 5]);
        let b = SimHashSketch::from_iter(7, [1u32, 2, 3, 4, 5]);
        assert_eq!(
            a.estimated_cosine_similarity(&b).to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn small_perturbation_yields_small_hamming_distance() {
        // Large shared prefix, one added item — the perturbation is
        // small, so Hamming distance should be much less than 32
        // (which would be the expected distance for random inputs).
        let base: Vec<u32> = (0..200u32).collect();
        let mut perturbed = base.clone();
        perturbed.push(9999);

        let a = SimHashSketch::from_iter(42, base.iter().copied());
        let b = SimHashSketch::from_iter(42, perturbed.iter().copied());
        let h = a.hamming_distance(&b);
        // 200 shared "votes" versus 1 differing vote is a tiny
        // perturbation; the bit-flip count is typically 0-3 here, and
        // the test asserts a very loose bound to stay non-flaky.
        assert!(
            h <= 8,
            "hamming distance {h} too large for small perturbation"
        );
    }

    #[test]
    fn different_seeds_yield_different_signatures() {
        // Same input, different seed → almost certainly different
        // signatures. This documents the "seed carries all the randomness"
        // invariant.
        let a = SimHashSketch::from_iter(7, [1u32, 2, 3, 4, 5, 6, 7, 8]);
        let b = SimHashSketch::from_iter(8, [1u32, 2, 3, 4, 5, 6, 7, 8]);
        assert_ne!(a.signature(), b.signature());
    }

    #[test]
    #[should_panic(expected = "requires equal seeds")]
    fn hamming_across_seeds_panics() {
        let a = SimHashSketch::from_iter(7, [1u32]);
        let b = SimHashSketch::from_iter(8, [1u32]);
        let _ = a.hamming_distance(&b);
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = SimHashSketch::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Cosine);
        assert_eq!(
            d.variant,
            VariantId("simhash-signed-random-projections-64bit-portable-splitmix")
        );
    }

    #[test]
    fn convenience_constructor_agrees_with_method() {
        let a = SimHashSketch::from_iter(7, [1u32, 2, 3]);
        let b = simhash_from([1u32, 2, 3], 7);
        assert_eq!(a.signature(), b.signature());
    }

    #[test]
    fn hamming_is_bounded_by_signature_width() {
        // Even for two totally unrelated inputs the distance never
        // exceeds 64 — the signature only has 64 bits to disagree on.
        let a = SimHashSketch::from_iter(7, [1u32, 2, 3]);
        let b = SimHashSketch::from_iter(7, [100u32, 200, 300]);
        assert!(a.hamming_distance(&b) <= 64);
    }
}
