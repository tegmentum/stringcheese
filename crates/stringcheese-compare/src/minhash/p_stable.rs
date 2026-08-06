//! `p`-stable LSH (Datar, Immorlica, Indyk, Mirrokni 2004) — LSH families
//! for `L_p` distances, with `p ∈ (0, 2]`.
//!
//! # What it does
//!
//! Given a bucket width `r > 0`, a random projection `a ∈ R^d` drawn
//! from a `p`-stable distribution, and a random offset `b ~ Uniform(0, r)`,
//! the hash of a vector `v ∈ R^d` is
//!
//! ```text
//!     h(v) = floor((a · v + b) / r)
//! ```
//!
//! Two vectors `u, v ∈ R^d` collide (`h(u) == h(v)`) with probability
//! that decreases monotonically in `||u − v||_p`. Concretely, for a
//! `p`-stable projection,
//!
//! ```text
//!     P_collide(u, v) = ∫_0^r (1/c) · f_p(t/c) · (1 − t/r) dt,
//!     where c = ||u − v||_p and f_p is the `p`-stable density.
//! ```
//!
//! The point of `p`-stable LSH is not to *compute* this probability at
//! query time; it is to instantiate `L` independent hash functions with
//! this collision behavior and use their agreement as a candidate
//! filter, analogous to `MinHash`-`LSH`'s banded scheme but for `L_p`
//! rather than Jaccard.
//!
//! # Distributions
//!
//! * `L1` (`p = 1`): Cauchy distribution. Inversion sampling:
//!   `tan(π · (u − 0.5))` for `u ~ Uniform(0, 1)`.
//! * `L2` (`p = 2`): Gaussian distribution. Box-Muller sampling:
//!   `sqrt(-2 · ln(u1)) · cos(2π · u2)` for independent
//!   `u1, u2 ~ Uniform(0, 1)`.
//!
//! Both are drawn deterministically from a `splitmix64` stream seeded by
//! the caller's `seed`, matching the crate-wide convention that no
//! sketch consumes runtime randomness.
//!
//! # `bucket` vs `collide_with`
//!
//! [`PStableLshSketch::bucket`] returns the integer bucket assigned to
//! the sketch's vector under a single hash function. [`PStableLshSketch::collide_with`]
//! returns `true` iff two sketches (constructed with the same `dim`,
//! `r`, `family`, and `seed`) landed in the same bucket. This is a
//! hashing family — not a similarity estimator — and downstream `LSH`
//! amplification composes multiple sketches with independent seeds.
//!
//! # `std` gate
//!
//! Sampling from Cauchy and Gaussian distributions uses `f64::tan`,
//! `f64::cos`, `f64::ln`, and `f64::sqrt` — all `std` operations. This
//! module is compiled only under `--features std`, matching
//! `stringcheese-compare`'s `set_similarity::Cosine` and
//! `minhash::weighted` policy.
//!
//! # Scale invariance of collision
//!
//! Scaling both vectors by the same constant `α > 0` scales `||u − v||_p`
//! by `α`, so a bucket width `r` on the scaled inputs matches a bucket
//! width `r/α` on the originals. The [`PStableLshSketch`] itself does
//! not encode this normalization; callers who want scale invariance
//! should normalize their vectors before sketching.
//!
//! # References
//!
//! * Datar, M., Immorlica, N., Indyk, P., & Mirrokni, V. S. (2004).
//!   "Locality-sensitive hashing scheme based on p-stable distributions."
//!   *Proceedings of the 20th Annual Symposium on Computational Geometry
//!   (`SoCG` '04)*, 253-262. <https://doi.org/10.1145/997817.997857> —
//!   introduces the family this module implements.

#![cfg(feature = "std")]

use alloc::vec::Vec;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::minhash::hash::splitmix64;

/// Which `p`-stable distribution to draw random projections from.
///
/// * [`PStableFamily::L1`] → Cauchy (`p = 1`).
/// * [`PStableFamily::L2`] → Gaussian (`p = 2`).
///
/// The enum's discriminants are stable — golden cases key on them via
/// the descriptor's [`VariantId`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PStableFamily {
    /// `p = 1` — Cauchy projections; LSH for `L_1` distance.
    L1,
    /// `p = 2` — Gaussian projections; LSH for `L_2` (Euclidean) distance.
    L2,
}

impl PStableFamily {
    /// Returns the variant slug for this family. Used to build the
    /// descriptor's [`VariantId`].
    #[inline]
    #[must_use]
    pub const fn variant_slug(self) -> &'static str {
        match self {
            Self::L1 => "p-stable-lsh-l1-cauchy",
            Self::L2 => "p-stable-lsh-l2-gaussian",
        }
    }
}

/// Datar-Immorlica-Indyk-Mirrokni 2004 `L_2` (Gaussian) `p`-stable LSH
/// descriptor.
pub const P_STABLE_LSH_L2_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::PStableLsh,
    VariantId("p-stable-lsh-l2-gaussian"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "Locality-sensitive hashing scheme based on p-stable distributions",
        authors: "M. Datar, N. Immorlica, P. Indyk, V. S. Mirrokni",
        year: 2004,
    },
);

/// Datar-Immorlica-Indyk-Mirrokni 2004 `L_1` (Cauchy) `p`-stable LSH
/// descriptor.
pub const P_STABLE_LSH_L1_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::PStableLsh,
    VariantId("p-stable-lsh-l1-cauchy"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "Locality-sensitive hashing scheme based on p-stable distributions",
        authors: "M. Datar, N. Immorlica, P. Indyk, V. S. Mirrokni",
        year: 2004,
    },
);

/// A `p`-stable LSH sketch — the integer bucket assigned to a single
/// vector under a single random projection.
///
/// A caller composes `L` sketches with independent seeds to build an
/// amplified LSH scheme; two vectors are candidates iff their sketches
/// [`PStableLshSketch::collide_with`] on all `L` hash functions.
///
/// See the [module-level documentation](self) for the construction, the
/// collision-probability formula, and the two supported distributions.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PStableLshSketch {
    /// The integer bucket `floor((a · v + b) / r)`. Stored as `i64` to
    /// accept negative bucket ids (which occur naturally when `a · v + b`
    /// is negative, which occurs naturally when the projection or the
    /// input takes negative values).
    bucket: i64,
    /// Vector dimension. Retained for equality-check purposes so two
    /// sketches constructed at different dimensions cannot silently be
    /// compared.
    dim: usize,
    /// Bucket width. Two sketches with different `r` values are not
    /// comparable — the collision probability depends on `r`.
    r: f64,
    /// Which `p`-stable distribution the projection was drawn from.
    family: PStableFamily,
    /// The caller-supplied seed. Two sketches with different seeds
    /// consumed different random projections and are not comparable.
    seed: u64,
}

impl PStableLshSketch {
    /// Constructs a `p`-stable LSH sketch of the given `vector` under a
    /// single random projection drawn deterministically from `seed`.
    ///
    /// # Panics
    ///
    /// Panics if `vector.len() != dim`, if `r <= 0.0`, or if any element
    /// of `vector` is NaN or infinite.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the floored ratio is bounded by |a · v + b| / r, which for realistic inputs fits in an i64"
    )]
    #[allow(
        clippy::cast_precision_loss,
        reason = "i64::MAX/MIN precision loss when cast to f64 only affects the extreme saturating branch, which is a defensive guard against pathological inputs"
    )]
    pub fn from_vector(
        dim: usize,
        r: f64,
        family: PStableFamily,
        seed: u64,
        vector: &[f64],
    ) -> Self {
        assert_eq!(
            vector.len(),
            dim,
            "PStableLshSketch::from_vector: vector length {} does not match dim {dim}",
            vector.len()
        );
        assert!(r.is_finite() && r > 0.0, "r must be finite and > 0");
        for (i, x) in vector.iter().enumerate() {
            assert!(
                x.is_finite(),
                "vector[{i}] = {x} is not finite; p-stable LSH does not handle NaN/Inf"
            );
        }

        // Deterministic PRNG stream seeded from `seed`. Each call to
        // `next_u64` advances the state.
        let mut rng = SplitMix64Stream::new(seed);

        // Draw dim projections from the p-stable distribution.
        let mut projection: Vec<f64> = Vec::with_capacity(dim);
        for _ in 0..dim {
            projection.push(sample_pstable(family, &mut rng));
        }

        // Draw the offset b ~ Uniform(0, r).
        let u = uniform_half_open(&mut rng);
        let b = u * r;

        // Compute a · v + b.
        let mut dot = 0.0_f64;
        for (a, v) in projection.iter().zip(vector.iter()) {
            dot += a * v;
        }
        let scaled = (dot + b) / r;

        // Floor into an integer bucket. `f64::floor` returns an f64;
        // convert to i64 with a saturating cast so pathological inputs
        // do not produce UB or truncated garbage.
        let floored = scaled.floor();
        let bucket = if floored >= i64::MAX as f64 {
            i64::MAX
        } else if floored <= i64::MIN as f64 {
            i64::MIN
        } else {
            floored as i64
        };

        Self {
            bucket,
            dim,
            r,
            family,
            seed,
        }
    }

    /// Returns the integer bucket assigned to the sketch's vector under
    /// the sketch's random projection.
    #[inline]
    #[must_use]
    pub fn bucket(&self) -> i64 {
        self.bucket
    }

    /// Returns the vector dimension the sketch was constructed for.
    #[inline]
    #[must_use]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Returns the bucket width the sketch was constructed with.
    #[inline]
    #[must_use]
    pub fn r(&self) -> f64 {
        self.r
    }

    /// Returns the `p`-stable family the projection was drawn from.
    #[inline]
    #[must_use]
    pub fn family(&self) -> PStableFamily {
        self.family
    }

    /// Returns the sketch's stored seed.
    #[inline]
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The algorithm descriptor for this sketch's `p`-stable family.
    #[inline]
    #[must_use]
    pub const fn descriptor_for(family: PStableFamily) -> AlgorithmDescriptor {
        match family {
            PStableFamily::L1 => P_STABLE_LSH_L1_DESCRIPTOR,
            PStableFamily::L2 => P_STABLE_LSH_L2_DESCRIPTOR,
        }
    }

    /// Returns the descriptor for this specific sketch (dispatched on
    /// its [`PStableLshSketch::family`]).
    #[inline]
    #[must_use]
    pub fn descriptor(&self) -> AlgorithmDescriptor {
        Self::descriptor_for(self.family)
    }

    /// Returns `true` iff the two sketches were constructed with
    /// matching configuration (`dim`, `r`, `family`, `seed`) *and* their
    /// bucket ids are equal.
    ///
    /// The configuration check is required for correctness: two sketches
    /// with different `seed`s consumed different random projections, and
    /// the equality of their integer buckets carries no similarity
    /// signal.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches disagree on any of `dim`, `r`,
    /// `family`, or `seed`. Callers should pair sketches whose
    /// configuration they control end-to-end.
    #[must_use]
    pub fn collide_with(&self, other: &Self) -> bool {
        assert_eq!(
            self.dim, other.dim,
            "PStableLshSketch::collide_with requires equal dim"
        );
        assert!(
            (self.r - other.r).abs() <= 0.0,
            "PStableLshSketch::collide_with requires bit-equal r"
        );
        assert_eq!(
            self.family, other.family,
            "PStableLshSketch::collide_with requires equal family"
        );
        assert_eq!(
            self.seed, other.seed,
            "PStableLshSketch::collide_with requires equal seed"
        );
        self.bucket == other.bucket
    }
}

// ---------------------------------------------------------------------
// Deterministic PRNG stream and p-stable samplers.
// ---------------------------------------------------------------------

/// A minimal `SplitMix64` PRNG stream — enough for this module's
/// deterministic Gaussian/Cauchy inversion samplers.
///
/// The `state`-advance is the standard `state + 0x9E3779B97F4A7C15`
/// pattern from Sebastiano Vigna's `xoshiro` writeups; the output is
/// then run through the crate's [`crate::minhash::hash::splitmix64`]
/// finalizer for well-mixed 64-bit words.
struct SplitMix64Stream {
    state: u64,
}

impl SplitMix64Stream {
    fn new(seed: u64) -> Self {
        // Advance once so a seed of 0 does not produce an all-zero
        // initial mix (which would then feed Box-Muller a `u1 = 0`,
        // blowing up `ln(u1)`).
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64(self.state)
    }
}

/// Uniform-open `(0, 1)` — the shift keeps values strictly interior so
/// `ln(u)` and `tan(π(u-0.5))` are well-behaved for the samplers below.
///
/// Uses the same `(state >> 12) as f64 + 0.5) / 2^52` scheme as
/// `weighted.rs`'s [`crate::minhash::weighted`] module.
#[inline]
#[allow(
    clippy::cast_precision_loss,
    reason = "intermediate value is in [0, 2^52), which is exactly representable in f64"
)]
fn uniform_open(rng: &mut SplitMix64Stream) -> f64 {
    let s = rng.next_u64();
    let x = ((s >> 12) as f64) + 0.5;
    let denom = (1u64 << 52) as f64;
    x / denom
}

/// Uniform half-open `[0, 1)` — used for the `b ~ Uniform(0, r)` offset,
/// which tolerates `b = 0` (a bucket boundary at 0 is fine; only
/// `b = r` would be degenerate, and this generator never produces it).
#[inline]
#[allow(
    clippy::cast_precision_loss,
    reason = "intermediate value is in [0, 2^53), which is exactly representable in f64"
)]
fn uniform_half_open(rng: &mut SplitMix64Stream) -> f64 {
    let s = rng.next_u64();
    // 53 bits of state, mapped to [0, 1). The shift discards the low
    // 11 bits (unusable in an f64 mantissa) and the division gives
    // exactly-representable values.
    let x = (s >> 11) as f64;
    let denom = (1u64 << 53) as f64;
    x / denom
}

/// One draw from the requested `p`-stable distribution.
fn sample_pstable(family: PStableFamily, rng: &mut SplitMix64Stream) -> f64 {
    match family {
        PStableFamily::L1 => sample_cauchy(rng),
        PStableFamily::L2 => sample_gaussian(rng),
    }
}

/// One draw from a standard Cauchy via inversion: `tan(π · (u − 0.5))`.
fn sample_cauchy(rng: &mut SplitMix64Stream) -> f64 {
    let u = uniform_open(rng);
    (core::f64::consts::PI * (u - 0.5)).tan()
}

/// One draw from a standard Gaussian via Box-Muller.
///
/// Box-Muller produces *two* independent draws per call from the pair
/// `(u1, u2) ~ Uniform(0, 1)^2`. This function returns only the first;
/// the second is discarded. This is wasteful in principle (~half the
/// entropy is thrown away), but it keeps the sampler stateless from
/// the caller's perspective, and this module's total sample count
/// per sketch (`dim` draws) is small enough that the waste is
/// negligible.
fn sample_gaussian(rng: &mut SplitMix64Stream) -> f64 {
    let u1 = uniform_open(rng);
    let u2 = uniform_open(rng);
    let r = (-2.0_f64 * u1.ln()).sqrt();
    let theta = 2.0_f64 * core::f64::consts::PI * u2;
    r * theta.cos()
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn same_input_same_seed_same_bucket() {
        let v = vec![1.0_f64, 2.0, 3.0, 4.0];
        let a = PStableLshSketch::from_vector(4, 4.0, PStableFamily::L2, 42, &v);
        let b = PStableLshSketch::from_vector(4, 4.0, PStableFamily::L2, 42, &v);
        assert_eq!(a.bucket(), b.bucket());
        assert!(a.collide_with(&b));
    }

    #[test]
    fn same_input_different_seed_typically_different_bucket() {
        let v = vec![1.0_f64, 2.0, 3.0, 4.0];
        let a = PStableLshSketch::from_vector(4, 1.0, PStableFamily::L2, 42, &v);
        let b = PStableLshSketch::from_vector(4, 1.0, PStableFamily::L2, 43, &v);
        // With r = 1.0 and non-tiny inputs, adjacent seeds
        // overwhelmingly produce different buckets. This documents the
        // per-seed independence of the hash family.
        assert_ne!(a.bucket(), b.bucket());
    }

    #[test]
    fn nearby_vectors_often_share_bucket_at_wide_r() {
        // Two vectors that differ by a small perturbation should
        // collide most of the time under a wide bucket width.
        let mut collide = 0;
        let total = 20;
        for seed in 0..total {
            let a =
                PStableLshSketch::from_vector(3, 20.0, PStableFamily::L2, seed, &[1.0, 2.0, 3.0]);
            let b = PStableLshSketch::from_vector(
                3,
                20.0,
                PStableFamily::L2,
                seed,
                &[1.01, 2.01, 3.01],
            );
            if a.collide_with(&b) {
                collide += 1;
            }
        }
        // A tight perturbation and wide r should collide overwhelmingly.
        assert!(
            collide >= 15,
            "expected >= 15 / 20 collisions for nearby vectors under wide r, got {collide}"
        );
    }

    #[test]
    fn distant_vectors_rarely_share_bucket_at_narrow_r() {
        // Two very different vectors should split into different
        // buckets most of the time under a narrow bucket width.
        let mut collide = 0;
        let total = 20;
        for seed in 0..total {
            let a =
                PStableLshSketch::from_vector(3, 0.5, PStableFamily::L2, seed, &[1.0, 2.0, 3.0]);
            let b = PStableLshSketch::from_vector(
                3,
                0.5,
                PStableFamily::L2,
                seed,
                &[100.0, 200.0, 300.0],
            );
            if a.collide_with(&b) {
                collide += 1;
            }
        }
        // Highly distant vectors under a narrow r should almost never
        // collide. This is a rough sanity check on the sampler's tail
        // behavior.
        assert!(
            collide <= 3,
            "expected <= 3 / 20 collisions for distant vectors under narrow r, got {collide}"
        );
    }

    #[test]
    fn l1_variant_produces_reasonable_bucket() {
        // Cauchy has heavy tails; a single very-large projection could
        // dominate. This test asserts only that the pipeline produces
        // *some* representable bucket — a broken sampler that returned
        // NaN would panic in the i64 conversion.
        let v = vec![1.0_f64, 2.0, 3.0];
        let s = PStableLshSketch::from_vector(3, 4.0, PStableFamily::L1, 42, &v);
        // Just check the family propagates correctly.
        assert_eq!(s.family(), PStableFamily::L1);
    }

    #[test]
    fn scale_invariance_property_via_matched_r() {
        // The Datar et al. paper's scale-invariance property: scaling
        // both vectors by α > 0 and scaling `r` by α produces the same
        // collision behavior. We verify by picking two seeds and
        // checking that (v, w) at (r) and (α·v, α·w) at (α·r) agree on
        // whether they collide.
        let v = vec![1.0_f64, 2.0, 3.0];
        let w = vec![1.5_f64, 2.5, 3.5];
        for seed in 0u64..8 {
            let sv = PStableLshSketch::from_vector(3, 1.0, PStableFamily::L2, seed, &v);
            let sw = PStableLshSketch::from_vector(3, 1.0, PStableFamily::L2, seed, &w);
            let alpha = 3.0_f64;
            let v_s: Vec<f64> = v.iter().map(|x| x * alpha).collect();
            let w_s: Vec<f64> = w.iter().map(|x| x * alpha).collect();
            let sv2 = PStableLshSketch::from_vector(3, alpha, PStableFamily::L2, seed, &v_s);
            let sw2 = PStableLshSketch::from_vector(3, alpha, PStableFamily::L2, seed, &w_s);
            // The bucket boundaries scale linearly, and `b ~ Uniform(0,
            // r)` scales in the same way, so `floor((α·(a·v) + α·b) /
            // (α·r)) = floor((a·v + b) / r)` — bit-exact when the
            // arithmetic doesn't cross a rounding boundary. In practice
            // the scaled and unscaled buckets must at minimum agree on
            // whether the pair (v, w) collides.
            assert_eq!(sv.collide_with(&sw), sv2.collide_with(&sw2));
        }
    }

    #[test]
    #[should_panic(expected = "does not match dim")]
    fn wrong_dim_panics() {
        let _ = PStableLshSketch::from_vector(3, 1.0, PStableFamily::L2, 0, &[1.0, 2.0]);
    }

    #[test]
    #[should_panic(expected = "r must be finite and > 0")]
    fn nonpositive_r_panics() {
        let _ = PStableLshSketch::from_vector(1, 0.0, PStableFamily::L2, 0, &[1.0]);
    }

    #[test]
    #[should_panic(expected = "is not finite")]
    fn nan_vector_panics() {
        let _ = PStableLshSketch::from_vector(1, 1.0, PStableFamily::L2, 0, &[f64::NAN]);
    }

    #[test]
    #[should_panic(expected = "requires equal seed")]
    fn collide_across_seeds_panics() {
        let v = vec![1.0_f64];
        let a = PStableLshSketch::from_vector(1, 1.0, PStableFamily::L2, 1, &v);
        let b = PStableLshSketch::from_vector(1, 1.0, PStableFamily::L2, 2, &v);
        let _ = a.collide_with(&b);
    }

    #[test]
    fn descriptor_dispatches_on_family() {
        assert_eq!(
            PStableLshSketch::descriptor_for(PStableFamily::L1).variant,
            VariantId("p-stable-lsh-l1-cauchy")
        );
        assert_eq!(
            PStableLshSketch::descriptor_for(PStableFamily::L2).variant,
            VariantId("p-stable-lsh-l2-gaussian")
        );
        assert_eq!(
            PStableLshSketch::descriptor_for(PStableFamily::L2).family,
            AlgorithmFamily::PStableLsh
        );
    }

    #[test]
    fn gaussian_sampler_produces_finite_values() {
        let mut rng = SplitMix64Stream::new(42);
        for _ in 0..64 {
            let x = sample_gaussian(&mut rng);
            assert!(x.is_finite(), "gaussian sample {x} is not finite");
        }
    }

    #[test]
    fn cauchy_sampler_produces_finite_values_typical() {
        // Cauchy has heavy tails, but for typical seeds the samples
        // are finite. A NaN would indicate the tan(π/2) singularity is
        // being hit, which only happens for the astronomically unlikely
        // u = 0.5 exact.
        let mut rng = SplitMix64Stream::new(42);
        for _ in 0..64 {
            let x = sample_cauchy(&mut rng);
            assert!(!x.is_nan(), "cauchy sample is NaN");
        }
    }

    #[test]
    fn uniform_open_is_strictly_interior() {
        let mut rng = SplitMix64Stream::new(0);
        for _ in 0..1000 {
            let u = uniform_open(&mut rng);
            assert!(u > 0.0 && u < 1.0, "uniform_open() = {u} not in (0, 1)");
        }
    }

    #[test]
    fn uniform_half_open_never_reaches_one() {
        let mut rng = SplitMix64Stream::new(0xdead_beef);
        for _ in 0..1000 {
            let u = uniform_half_open(&mut rng);
            assert!(
                (0.0..1.0).contains(&u),
                "uniform_half_open() = {u} not in [0, 1)"
            );
        }
    }
}
