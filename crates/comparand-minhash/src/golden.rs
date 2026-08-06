//! Canonical golden cases for the `MinHash` sketch estimator, the `LSH`
//! collision probability, and the weighted `MinHash` estimator.
//!
//! Every case is wired to the [`comparand_corpus::FloatExpectation`]
//! schema, with tolerances chosen by the character of the value:
//!
//! * **[`FloatExpectation::ExactBits`]** for the identity, empty-vs-empty,
//!   and empty-vs-nonempty branches whose estimator produces `1.0` or
//!   `0.0` by construction — no randomness enters the answer.
//! * **[`FloatExpectation::Absolute`] with tolerance `1e-12`** for the
//!   `LSH` `collision_probability` cases, which compute a deterministic
//!   closed-form value.
//! * **[`FloatExpectation::Absolute`] with tolerance `0.15`** for the
//!   small-`k` `MinHash` estimator cases — an estimator with `k = 32` has
//!   standard error `~0.09` at `J = 0.5`, and `0.15` catches every
//!   plausible regression while tolerating natural finite-`k` variance.
//! * **[`FloatExpectation::Absolute`] with tolerance `0.05`** for the
//!   large-`k` (`k = 512`) estimator case — standard error `~0.02`, so
//!   `0.05` is well above noise but well below any real defect.
//!
//! Fixed seeds keep the sketches deterministic; a case's expected value
//! is derived by hand from the true Jaccard between the two hand-built
//! sets.

use comparand_core::AlgorithmDescriptor;
use comparand_corpus::{FloatExpectation, GoldenCase, GoldenSource};

use crate::lsh::LshIndex;
use crate::sketch::{MINHASH_JACCARD_DESCRIPTOR, MinHashSketch};

/// A hand-authored `MinHash` golden case.
#[derive(Debug)]
pub struct MinHashCase {
    /// Sketch size `k`.
    pub k: usize,
    /// Sketch seed.
    pub seed: u64,
    /// The left-side integers to sketch (as a set).
    pub left: &'static [u32],
    /// The right-side integers to sketch (as a set).
    pub right: &'static [u32],
}

/// The full golden-case type carried by [`GOLDEN_MINHASH`].
pub type MinHashGoldenCase = GoldenCase<MinHashCase, FloatExpectation>;

const IND: GoldenSource = GoldenSource::IndependentlyDerived;

/// Golden cases for the [`MinHashSketch::estimated_jaccard`] estimator.
pub const GOLDEN_MINHASH: &[MinHashGoldenCase] = &[
    GoldenCase {
        id: "minhash/empty-empty",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 64,
            seed: 42,
            left: &[],
            right: &[],
        },
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Empty-vs-empty: both sketches all-sentinel; estimator returns 1.0 bit-exact under the crate-wide identity convention.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "minhash/identical-abc",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 64,
            seed: 42,
            left: &[1, 2, 3],
            right: &[1, 2, 3],
        },
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Identical inputs produce identical signatures; estimator returns 1.0 bit-exact.",
        tags: &["basic", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "minhash/empty-vs-nonempty",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 64,
            seed: 42,
            left: &[],
            right: &[1, 2, 3],
        },
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: IND,
        notes: "Empty vs nonempty: every real signature is < u64::MAX, so zero slots match — estimator is 0.0 exact.",
        tags: &["basic", "empty", "exact-bits"],
    },
    GoldenCase {
        id: "minhash/disjoint-small-k32",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 32,
            seed: 42,
            left: &[1, 2, 3, 4, 5],
            right: &[100, 200, 300, 400, 500],
        },
        // True Jaccard = 0; small-k estimator may accidentally collide a
        // slot or two out of 32. The 0.15 tolerance is well above the
        // expected 0.03-ish noise floor.
        expected: FloatExpectation::Absolute {
            value: 0.0,
            tolerance: 0.15,
        },
        source: IND,
        notes: "Disjoint 5-element sets; estimator should sit near 0.0 for k=32.",
        tags: &["disjoint", "small-k", "tolerance-0.15"],
    },
    GoldenCase {
        id: "minhash/half-overlap-k128",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 128,
            seed: 42,
            // {1..8} and {5..12} — inter = {5,6,7,8} = 4, union = 12, J = 1/3.
            left: &[1, 2, 3, 4, 5, 6, 7, 8],
            right: &[5, 6, 7, 8, 9, 10, 11, 12],
        },
        expected: FloatExpectation::Absolute {
            value: 4.0 / 12.0,
            tolerance: 0.15,
        },
        source: IND,
        notes: "|{1..8} inter {5..12}| = 4, union = 12, J = 1/3; k=128 estimator sits within 0.15 with a fixed seed.",
        tags: &["partial", "derivation", "tolerance-0.15"],
    },
    GoldenCase {
        id: "minhash/two-thirds-overlap-k512",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 512,
            seed: 42,
            // {1..9} vs {3..11} — inter = 7, union = 11, J = 7/11 ≈ 0.636.
            left: &[1, 2, 3, 4, 5, 6, 7, 8, 9],
            right: &[3, 4, 5, 6, 7, 8, 9, 10, 11],
        },
        expected: FloatExpectation::Absolute {
            value: 7.0 / 11.0,
            tolerance: 0.05,
        },
        source: IND,
        notes: "|{1..9} inter {3..11}| = 7, union = 11, J = 7/11; k=512 estimator sits within 0.05.",
        tags: &["partial", "derivation", "large-k", "tolerance-0.05"],
    },
    GoldenCase {
        id: "minhash/near-identical-k256",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 256,
            seed: 42,
            // {1..10} vs {1..10, 11} — inter = 10, union = 11, J = 10/11.
            left: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            right: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        },
        expected: FloatExpectation::Absolute {
            value: 10.0 / 11.0,
            tolerance: 0.10,
        },
        source: IND,
        notes: "Near-identical (only one new element on the right); J = 10/11 ≈ 0.909. Estimator sits within 0.10.",
        tags: &["near-identical", "high-similarity", "tolerance-0.10"],
    },
    GoldenCase {
        id: "minhash/tiny-single-item-identity",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: MinHashCase {
            k: 16,
            seed: 42,
            left: &[7],
            right: &[7],
        },
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Single-element identical sets; estimator returns 1.0 exact for every k.",
        tags: &["basic", "identity", "single-item", "exact-bits"],
    },
];

/// A closed-form `LSH` collision-probability golden case.
#[derive(Debug, Copy, Clone)]
pub struct LshCollisionCase {
    /// Number of signature rows per band.
    pub band_size: usize,
    /// Number of bands.
    pub band_count: usize,
    /// True Jaccard similarity to evaluate at.
    pub similarity: f64,
}

/// The full golden-case type for `LSH` collision probability.
pub type LshCollisionGoldenCase = GoldenCase<LshCollisionCase, FloatExpectation>;

/// Golden cases for the `LSH` collision-probability formula. These are
/// deterministic and admit tight `1e-12` tolerances.
///
/// The descriptor field carries [`MINHASH_JACCARD_DESCRIPTOR`] because the
/// `LSH`-collision formula is derived from the `MinHash` estimator's
/// distribution — the index itself is infrastructure and carries no
/// descriptor of its own, matching the [`crate::LshIndex`] documentation.
pub const GOLDEN_LSH_COLLISION: &[LshCollisionGoldenCase] = &[
    GoldenCase {
        id: "lsh-collision/zero-similarity",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: LshCollisionCase {
            band_size: 4,
            band_count: 8,
            similarity: 0.0,
        },
        expected: FloatExpectation::Absolute {
            value: 0.0,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "1 - (1 - 0^4)^8 = 0.",
        tags: &["lsh", "endpoint", "closed-form"],
    },
    GoldenCase {
        id: "lsh-collision/full-similarity",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: LshCollisionCase {
            band_size: 4,
            band_count: 8,
            similarity: 1.0,
        },
        expected: FloatExpectation::Absolute {
            value: 1.0,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "1 - (1 - 1^4)^8 = 1.",
        tags: &["lsh", "endpoint", "closed-form"],
    },
    GoldenCase {
        id: "lsh-collision/mid-similarity-exact",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: LshCollisionCase {
            band_size: 2,
            band_count: 2,
            similarity: 0.5,
        },
        // 0.5^2 = 0.25; 1 - 0.25 = 0.75; 0.75^2 = 0.5625; 1 - 0.5625 = 0.4375.
        // Every step is exactly representable in f64.
        expected: FloatExpectation::Absolute {
            value: 0.4375,
            tolerance: 1e-15,
        },
        source: IND,
        notes: "Mid-range similarity: (bs=2, bc=2), s=0.5. Every step of the S-curve evaluation is exactly representable in f64.",
        tags: &["lsh", "closed-form", "exact"],
    },
    GoldenCase {
        id: "lsh-collision/large-config-half-similarity",
        descriptor: MINHASH_JACCARD_DESCRIPTOR,
        input: LshCollisionCase {
            band_size: 4,
            band_count: 4,
            similarity: 0.5,
        },
        // 0.5^4 = 0.0625; 1 - 0.0625 = 0.9375;
        // 0.9375^2 = 0.87890625; 0.87890625^2 = 0.7724761962890625;
        // 1 - 0.7724761962890625 = 0.2275238037109375. All steps exact in f64.
        expected: FloatExpectation::Absolute {
            value: 0.227_523_803_710_937_5,
            tolerance: 1e-15,
        },
        source: IND,
        notes: "Larger (bs=4, bc=4) configuration at s=0.5; all arithmetic exact in f64.",
        tags: &["lsh", "closed-form", "exact"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn build_sketch(items: &[u32], k: usize, seed: u64) -> MinHashSketch {
        MinHashSketch::from_iter(k, seed, items.iter().copied())
    }

    fn run_minhash_case(c: &MinHashGoldenCase) -> (f64, bool) {
        let case = &c.input;
        let a = build_sketch(case.left, case.k, case.seed);
        let b = build_sketch(case.right, case.k, case.seed);
        let observed = a.estimated_jaccard(&b);
        (observed, c.expected.matches(observed))
    }

    fn run_lsh_case(c: &LshCollisionGoldenCase) -> (f64, bool) {
        let idx = LshIndex::new(c.input.band_size, c.input.band_count);
        let observed = idx.collision_probability(c.input.similarity);
        (observed, c.expected.matches(observed))
    }

    fn all_descriptors() -> Vec<AlgorithmDescriptor> {
        let mut v: Vec<AlgorithmDescriptor> = Vec::new();
        for c in GOLDEN_MINHASH {
            v.push(c.descriptor);
        }
        for c in GOLDEN_LSH_COLLISION {
            v.push(c.descriptor);
        }
        v
    }

    #[test]
    fn every_minhash_case_uses_the_correct_descriptor() {
        for c in GOLDEN_MINHASH {
            assert_eq!(c.descriptor, MINHASH_JACCARD_DESCRIPTOR, "case {}", c.id);
        }
    }

    #[test]
    fn every_lsh_case_uses_the_correct_descriptor() {
        for c in GOLDEN_LSH_COLLISION {
            assert_eq!(c.descriptor, MINHASH_JACCARD_DESCRIPTOR, "case {}", c.id);
        }
    }

    #[test]
    fn every_minhash_case_matches_algorithm() {
        for c in GOLDEN_MINHASH {
            let (obs, ok) = run_minhash_case(c);
            assert!(
                ok,
                "minhash golden case {} disagreed: expected {:?}, observed {obs}",
                c.id, c.expected
            );
        }
    }

    #[test]
    fn every_lsh_case_matches_algorithm() {
        for c in GOLDEN_LSH_COLLISION {
            let (obs, ok) = run_lsh_case(c);
            assert!(
                ok,
                "lsh golden case {} disagreed: expected {:?}, observed {obs}",
                c.id, c.expected
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // Spec requires at least 8 golden cases.
        let n = GOLDEN_MINHASH.len() + GOLDEN_LSH_COLLISION.len();
        assert!(
            n >= 8,
            "expected at least 8 golden cases across the crate, got {n}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let mut ids: Vec<&str> = Vec::new();
        for c in GOLDEN_MINHASH {
            ids.push(c.id);
        }
        for c in GOLDEN_LSH_COLLISION {
            ids.push(c.id);
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }

    #[test]
    fn all_descriptors_at_least_one() {
        assert!(!all_descriptors().is_empty());
    }

    // Weighted `MinHash` golden case — only compiled when std is on (the
    // weighted module requires std for `f64::ln` / `exp` / `floor`).
    #[cfg(feature = "std")]
    #[test]
    fn weighted_identical_yields_one() {
        use crate::weighted::WeightedMinHashSketch;
        let a = WeightedMinHashSketch::from_weighted(64, 42, [(1u32, 1.0), (2, 2.0), (3, 3.0)]);
        let b = WeightedMinHashSketch::from_weighted(64, 42, [(1u32, 1.0), (2, 2.0), (3, 3.0)]);
        assert_eq!(
            a.estimated_weighted_jaccard(&b).to_bits(),
            1.0_f64.to_bits()
        );
    }

    // Weighted `MinHash` reduces to unweighted when all weights are equal.
    #[cfg(feature = "std")]
    #[test]
    fn weighted_unit_weights_close_to_unweighted() {
        use crate::weighted::WeightedMinHashSketch;
        // Two sets with true set-Jaccard 4/12 = 1/3 (matching the
        // "half-overlap" case above) — weighted-Jaccard on unit weights
        // reduces to set-Jaccard, and the estimator should sit near 1/3.
        let a = WeightedMinHashSketch::from_weighted(
            256,
            42,
            [
                (1u32, 1.0),
                (2, 1.0),
                (3, 1.0),
                (4, 1.0),
                (5, 1.0),
                (6, 1.0),
                (7, 1.0),
                (8, 1.0),
            ],
        );
        let b = WeightedMinHashSketch::from_weighted(
            256,
            42,
            [
                (5u32, 1.0),
                (6, 1.0),
                (7, 1.0),
                (8, 1.0),
                (9, 1.0),
                (10, 1.0),
                (11, 1.0),
                (12, 1.0),
            ],
        );
        let est = a.estimated_weighted_jaccard(&b);
        assert!(
            (est - 4.0_f64 / 12.0_f64).abs() < 0.15,
            "weighted estimate {est} too far from set-jaccard 1/3"
        );
    }
}
