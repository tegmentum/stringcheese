//! Property-based tests for the `MinHash` sketch, the `LSH` index, and the
//! weighted `MinHash` estimator.
//!
//! # What is asserted
//!
//! * **Determinism.** Same items, same seed => same signatures.
//! * **Set invariance.** Duplicates and iteration-order changes never
//!   affect the sketch.
//! * **Bounded estimator.** `estimated_jaccard(a, b) ∈ [0, 1]`.
//! * **Symmetry.** `estimated_jaccard(a, b) == estimated_jaccard(b, a)`.
//! * **Identity.** `estimated_jaccard(a, a) == 1.0` bit-exactly.
//! * **Statistical unbiasedness.** Across many random pairs, the *average*
//!   estimator value tracks the true Jaccard within a few standard errors.
//!   This is the load-bearing property test — the estimator's whole point
//!   is unbiasedness, and a hash-mixing regression would show up here
//!   long before it corrupted a real workload.
//! * **`LSH` collision-probability formula.** For a range of `s`,
//!   [`LshIndex::collision_probability(s)`] matches the closed-form
//!   `1 - (1 - s^band_size)^band_count` to within `1e-12`.
//! * **`LSH` candidate superset property.** Every item whose true Jaccard
//!   with the query exceeds the `LSH` configuration's threshold is
//!   returned as a candidate with high probability. Framed as an average
//!   over many random pairs.
//!
//! # Tolerance / iteration counts
//!
//! Statistical tests use a fixed iteration count (200) and modest sketch
//! sizes (k = 128) — enough to keep the average close to the mean without
//! blowing up test-suite runtime. The tolerances (`0.05` on the
//! statistical-unbiasedness assertion) are picked to be many standard
//! errors above zero but far below any real regression.

use proptest::prelude::*;

use crate::hash::{permutation_seed, permuted_hash, portable_hash};
use crate::lsh::LshIndex;
use crate::sketch::MinHashSketch;

use comparand_core::SimilarityMetric;
use comparand_ngram::GramSet;
use comparand_set_similarity::jaccard::JaccardOverSet;

/// Small integer alphabet — small enough that random sets frequently
/// overlap, giving the estimator work to do.
fn arb_items() -> impl Strategy<Value = alloc::vec::Vec<u32>> {
    proptest::collection::vec(0u32..40, 0..20)
}

fn true_jaccard(a: &[u32], b: &[u32]) -> f64 {
    let sa: GramSet<u32> = a.iter().copied().collect();
    let sb: GramSet<u32> = b.iter().copied().collect();
    JaccardOverSet.similarity(&sa, &sb).into_inner()
}

proptest! {
    // -------- Determinism, set invariance --------

    #[test]
    fn sketch_is_deterministic(a in arb_items()) {
        let s1 = MinHashSketch::from_iter(64, 42, a.iter().copied());
        let s2 = MinHashSketch::from_iter(64, 42, a.iter().copied());
        prop_assert_eq!(s1.signatures(), s2.signatures());
    }

    #[test]
    fn sketch_is_set_invariant(a in arb_items()) {
        let doubled: alloc::vec::Vec<u32> = a.iter().chain(a.iter()).copied().collect();
        let s1 = MinHashSketch::from_iter(64, 42, a.iter().copied());
        let s2 = MinHashSketch::from_iter(64, 42, doubled.iter().copied());
        prop_assert_eq!(s1.signatures(), s2.signatures());
    }

    #[test]
    fn sketch_is_permutation_invariant(a in arb_items()) {
        let mut rev = a.clone();
        rev.reverse();
        let s1 = MinHashSketch::from_iter(64, 42, a.iter().copied());
        let s2 = MinHashSketch::from_iter(64, 42, rev.iter().copied());
        prop_assert_eq!(s1.signatures(), s2.signatures());
    }

    // -------- Estimator properties --------

    #[test]
    fn estimator_bounded(a in arb_items(), b in arb_items()) {
        let sa = MinHashSketch::from_iter(64, 42, a.iter().copied());
        let sb = MinHashSketch::from_iter(64, 42, b.iter().copied());
        let j = sa.estimated_jaccard(&sb);
        prop_assert!((0.0..=1.0).contains(&j), "out of range: {j}");
    }

    #[test]
    fn estimator_symmetric(a in arb_items(), b in arb_items()) {
        let sa = MinHashSketch::from_iter(64, 42, a.iter().copied());
        let sb = MinHashSketch::from_iter(64, 42, b.iter().copied());
        prop_assert_eq!(
            sa.estimated_jaccard(&sb).to_bits(),
            sb.estimated_jaccard(&sa).to_bits(),
        );
    }

    #[test]
    fn estimator_identity_bit_exact(a in arb_items()) {
        let s = MinHashSketch::from_iter(64, 42, a.iter().copied());
        prop_assert_eq!(s.estimated_jaccard(&s).to_bits(), 1.0_f64.to_bits());
    }

    // -------- `LSH` collision-probability formula --------

    #[test]
    fn lsh_collision_probability_matches_formula(
        s in 0.0_f64..=1.0_f64,
        band_size in 1usize..=8,
        band_count in 1usize..=16,
    ) {
        let idx = LshIndex::new(band_size, band_count);
        let bs = i32::try_from(band_size).unwrap();
        let bc = i32::try_from(band_count).unwrap();
        let expected = 1.0 - (1.0 - s.powi(bs)).powi(bc);
        let observed = idx.collision_probability(s);
        prop_assert!(
            (observed - expected).abs() < 1e-12,
            "collision_probability({s}) with (bs={band_size}, bc={band_count}): expected {expected}, observed {observed}"
        );
    }

    // -------- Hash primitives --------

    #[test]
    fn portable_hash_stable_across_calls(s in any::<u64>(), v in any::<u64>()) {
        prop_assert_eq!(portable_hash(s, &v), portable_hash(s, &v));
    }

    #[test]
    fn permuted_hash_depends_on_seed(base in any::<u64>()) {
        // Different permutation seeds map the same base_hash to
        // (with astronomically high probability) different u64s.
        let s0 = permutation_seed(0, 0);
        let s1 = permutation_seed(0, 1);
        prop_assume!(s0 != s1);
        prop_assert_ne!(permuted_hash(base, s0), permuted_hash(base, s1));
    }
}

// -------- Non-generative statistical unbiasedness --------

/// The **load-bearing** statistical test.
///
/// For each of `N_PAIRS` randomly-sampled pairs of small integer sets,
/// sketch each side with `k = 128` and record `(estimated, true)`.
/// Assert that the average absolute deviation is small (< 0.05) — a
/// broken hash-mixing scheme would show up as a large systematic bias
/// here.
///
/// The test uses a deterministic PRNG seed (`SplitMix64` chain) so the
/// suite is stable run-to-run. A false failure at the current tolerance
/// (0.05) would be astronomically unlikely if the estimator is unbiased
/// with the expected standard error of ~0.04 at k=128, J=0.5.
#[test]
fn estimator_is_statistically_unbiased_across_many_pairs() {
    // A tiny deterministic PRNG: seed a stream with `SplitMix64` and use
    // it as a source of "random" small integers.
    let mut rng_state: u64 = 0xdead_beef_1234_5678;
    let mut next = |m: u32| -> u32 {
        rng_state = crate::hash::splitmix64(rng_state);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "truncation is the intended behavior of this test PRNG"
        )]
        let low = rng_state as u32;
        low % m
    };

    let n_pairs: usize = 200;
    let k: usize = 128;
    let seed: u64 = 42;

    let mut total_abs_error: f64 = 0.0;

    for _ in 0..n_pairs {
        let alpha: u32 = 40; // universe size
        let len_a = 1 + (next(15) as usize);
        let len_b = 1 + (next(15) as usize);
        let a: alloc::vec::Vec<u32> = (0..len_a).map(|_| next(alpha)).collect();
        let b: alloc::vec::Vec<u32> = (0..len_b).map(|_| next(alpha)).collect();

        let true_j = true_jaccard(&a, &b);

        let sa = MinHashSketch::from_iter(k, seed, a.iter().copied());
        let sb = MinHashSketch::from_iter(k, seed, b.iter().copied());
        let est = sa.estimated_jaccard(&sb);

        total_abs_error += (est - true_j).abs();
    }

    #[allow(clippy::cast_precision_loss, reason = "n_pairs is small")]
    let avg_abs_error = total_abs_error / n_pairs as f64;

    // Standard error at k=128, worst-case J=0.5 is 0.044. Average
    // absolute deviation of |Est - J| for such an estimator is well
    // under 0.05; anything above signals a bias.
    assert!(
        avg_abs_error < 0.05,
        "average absolute estimator error {avg_abs_error} exceeds tolerance; \
         a broken hash mixing scheme would spike this metric."
    );
}

/// `LSH` candidate superset property: for random pairs, if the true
/// Jaccard exceeds the `LSH` configuration's threshold, the pair is a
/// candidate with high probability.
///
/// Formalized as an *average recall* check across many random pairs
/// above the threshold — the fraction retrieved as candidates should
/// exceed a conservative floor (0.6 for a `(4, 8)` configuration whose
/// threshold sits near `J ≈ 0.6`).
#[test]
fn lsh_candidates_contain_high_similarity_pairs() {
    let mut rng_state: u64 = 0xfeed_face_0011_2233;
    let mut next = |m: u32| -> u32 {
        rng_state = crate::hash::splitmix64(rng_state);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "truncation is the intended behavior of this test PRNG"
        )]
        let low = rng_state as u32;
        low % m
    };

    let k: usize = 64;
    let seed: u64 = 42;
    let band_size: usize = 4;
    let band_count: usize = 16;
    // With (bs=4, bc=16), threshold at 50% collision is ~0.5; pairs
    // with J >= 0.7 should collide most of the time.
    let threshold: f64 = 0.7;

    let mut n_above: usize = 0;
    let mut n_retrieved: usize = 0;

    let n_pairs: usize = 200;
    for _ in 0..n_pairs {
        let alpha: u32 = 40;
        // Build a large base set, then perturb it slightly to make a
        // "high similarity" partner in most iterations.
        let len = 10 + (next(10) as usize);
        let base: alloc::vec::Vec<u32> = (0..len).map(|_| next(alpha)).collect();
        // Randomly drop or add a small number of elements to the partner.
        let mut partner: alloc::vec::Vec<u32> = base.clone();
        for _ in 0..(next(4) as usize) {
            partner.push(next(alpha));
        }
        if next(2) == 0 && !partner.is_empty() {
            partner.pop();
        }

        let true_j = true_jaccard(&base, &partner);
        if true_j < threshold {
            continue;
        }
        n_above += 1;

        let sa = MinHashSketch::from_iter(k, seed, base.iter().copied());
        let sb = MinHashSketch::from_iter(k, seed, partner.iter().copied());
        let mut idx = LshIndex::new(band_size, band_count);
        idx.insert(1, &sa);
        let cands = idx.query_candidates(&sb);
        if cands.contains(&1) {
            n_retrieved += 1;
        }
    }

    // A meaningful sample size is required to trust the recall estimate.
    assert!(
        n_above >= 10,
        "not enough high-similarity pairs to run the check: {n_above}; \
         adjust the perturbation logic if this fails"
    );

    #[allow(clippy::cast_precision_loss, reason = "small counters")]
    let recall = n_retrieved as f64 / n_above as f64;
    // Theoretical minimum for J=threshold=0.7 with (bs=4, bc=16):
    // 1 - (1 - 0.7^4)^16 ≈ 0.998. Well-behaved sketches easily clear
    // 0.9; the 0.6 floor tolerates finite-k variance and the fact that
    // some retrieved pairs sit just above threshold.
    assert!(
        recall >= 0.6,
        "LSH recall on high-similarity pairs = {recall} (below 0.6 floor); \
         a broken band-hash or configuration would drive this near zero."
    );
}
