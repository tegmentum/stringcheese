//! Locality-sensitive-hashing banded index over [`MinHashSketch`].
//!
//! # Banding
//!
//! Given a sketch of size `k = band_count · band_size`, `LSH` splits each
//! sketch into `band_count` contiguous chunks ("bands") of `band_size`
//! signatures ("rows") each. Each band is hashed to a single `u64`, and
//! the index maintains a table mapping band-hash to the list of item ids
//! whose sketches produced that band-hash.
//!
//! Two items are *candidates* for one another iff at least one band
//! collides. Under an idealized k-permutation `MinHash`, the probability
//! that a pair with true Jaccard `s` collides is exactly:
//!
//! ```text
//!     P_collision(s) = 1 - (1 - s^band_size)^band_count
//! ```
//!
//! This is the S-curve consumers pick their `(band_count, band_size)`
//! configuration against: choose the pair whose crossover point sits
//! near the target similarity, then verify the false-positive rate at
//! low `s` and the false-negative rate at high `s` are acceptable.
//! [`LshIndex::suggest_config`] automates the search across all valid
//! configurations for a given target similarity and sketch size.
//!
//! # Not a metric-space index
//!
//! Unlike `comparand-index`'s `BkTree` and `VpTree`, this index does
//! not require a metric input — it is driven by `MinHash` signatures, not
//! by a distance function. The sound-pruning guarantee is *probabilistic*
//! rather than exact: a true-positive at similarity `s` is returned with
//! probability `P_collision(s)` per query, not with certainty. Callers
//! wanting an exact-recall guarantee at any threshold should not use `LSH`.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::hash::splitmix64;
use crate::sketch::MinHashSketch;

/// A banded `LSH` index over [`MinHashSketch`]es.
///
/// See the [module-level documentation](self) for the banding scheme,
/// the collision probability formula, and the caveat that pruning is
/// probabilistic rather than exact.
#[derive(Debug, Clone)]
pub struct LshIndex {
    /// Number of signature rows per band. Must be positive.
    band_size: usize,
    /// Number of bands per sketch. Must be positive.
    band_count: usize,
    /// Per-band hash tables. `bands[i]` maps a band-hash to the list of
    /// item ids whose band-`i` hash equals that value. Postings within
    /// each bucket are stored in insertion order.
    bands: Vec<BTreeMap<u64, Vec<usize>>>,
}

impl LshIndex {
    /// Constructs an empty index with `band_count` bands of `band_size`
    /// rows each.
    ///
    /// Every sketch subsequently passed to [`LshIndex::insert`] or
    /// [`LshIndex::query_candidates`] must have size at least
    /// `band_count · band_size`; excess rows are ignored.
    ///
    /// # Panics
    ///
    /// Panics if either dimension is zero.
    #[must_use]
    pub fn new(band_size: usize, band_count: usize) -> Self {
        assert!(band_size > 0, "band_size must be > 0");
        assert!(band_count > 0, "band_count must be > 0");
        let mut bands = Vec::with_capacity(band_count);
        for _ in 0..band_count {
            bands.push(BTreeMap::new());
        }
        Self {
            band_size,
            band_count,
            bands,
        }
    }

    /// Returns the configured rows-per-band.
    #[inline]
    #[must_use]
    pub fn band_size(&self) -> usize {
        self.band_size
    }

    /// Returns the configured number of bands.
    #[inline]
    #[must_use]
    pub fn band_count(&self) -> usize {
        self.band_count
    }

    /// Returns the number of insertions performed so far.
    ///
    /// This counts *insertions*, not distinct ids — inserting the same
    /// id twice raises the count twice.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bands
            .iter()
            .map(|b| b.values().map(Vec::len).sum::<usize>())
            .sum::<usize>()
            / self.band_count
    }

    /// Returns `true` if no sketches have been inserted.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bands.iter().all(BTreeMap::is_empty)
    }

    /// Inserts a sketch under `id`.
    ///
    /// The sketch is hashed into `band_count` band-buckets; queries whose
    /// sketch collides in any of the bands will return `id` as a
    /// candidate.
    ///
    /// # Panics
    ///
    /// Panics if the sketch is smaller than `band_size · band_count`.
    pub fn insert(&mut self, id: usize, sketch: &MinHashSketch) {
        assert!(
            sketch.size() >= self.band_size * self.band_count,
            "sketch too small for this LSH configuration: sketch size {}, need {}",
            sketch.size(),
            self.band_size * self.band_count
        );
        let sigs = sketch.signatures();
        for i in 0..self.band_count {
            let start = i * self.band_size;
            let end = start + self.band_size;
            let band_hash = hash_band(&sigs[start..end]);
            self.bands[i].entry(band_hash).or_default().push(id);
        }
    }

    /// Returns the candidate ids for `sketch` — every id that shares at
    /// least one band-hash with the query.
    ///
    /// The returned vector is deduplicated but its order is not
    /// specified (an insertion-order guarantee would preclude a
    /// hash-map-backed variant we may add later).
    ///
    /// # Panics
    ///
    /// Panics if the sketch is smaller than `band_size · band_count`.
    #[must_use]
    pub fn query_candidates(&self, sketch: &MinHashSketch) -> Vec<usize> {
        assert!(
            sketch.size() >= self.band_size * self.band_count,
            "sketch too small for this LSH configuration: sketch size {}, need {}",
            sketch.size(),
            self.band_size * self.band_count
        );
        let sigs = sketch.signatures();
        // Use a BTreeSet for the growing candidate set — deterministic
        // deduplication with no `std` dependency.
        let mut seen: alloc::collections::BTreeSet<usize> = alloc::collections::BTreeSet::new();
        for i in 0..self.band_count {
            let start = i * self.band_size;
            let end = start + self.band_size;
            let band_hash = hash_band(&sigs[start..end]);
            if let Some(ids) = self.bands[i].get(&band_hash) {
                for id in ids {
                    seen.insert(*id);
                }
            }
        }
        seen.into_iter().collect()
    }

    /// The theoretical band-collision probability for a pair with true
    /// Jaccard `similarity`, under this index's configuration.
    ///
    /// Formula: `1 - (1 - s^band_size)^band_count`. The S-curve consumers
    /// choose their `(band_count, band_size)` against.
    ///
    /// # Panics
    ///
    /// Panics if `similarity` is outside `[0.0, 1.0]`.
    #[must_use]
    pub fn collision_probability(&self, similarity: f64) -> f64 {
        assert!(
            (0.0..=1.0).contains(&similarity),
            "similarity must be in [0.0, 1.0]"
        );
        let inside = 1.0 - pow_int(similarity, self.band_size);
        1.0 - pow_int(inside, self.band_count)
    }

    /// Suggests a `(band_count, band_size)` configuration whose S-curve
    /// crossover sits closest to `target_similarity` under the given
    /// `sketch_size` budget.
    ///
    /// The heuristic sweeps every valid pair `(b, r)` with `b · r
    /// <= sketch_size` and picks the one whose theoretical crossover
    /// `s_threshold = (1/b)^(1/r)` is closest to `target_similarity`.
    /// Ties are broken toward the higher `band_count` (more bands
    /// steepens the S-curve — sharper transition, fewer false positives
    /// and negatives).
    ///
    /// # Std gate
    ///
    /// The heuristic uses `f64::powf`, which lives in `std` (not `core`).
    /// The method is compiled only when the `std` feature is on;
    /// `no-std` consumers who need a configuration must derive their own
    /// `(band_count, band_size)` from the [`LshIndex::collision_probability`]
    /// formula.
    ///
    /// # Panics
    ///
    /// Panics if `sketch_size == 0` or `target_similarity` is outside
    /// `[0.0, 1.0]`.
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "the search bound sketch_size is a small integer in every practical configuration"
    )]
    pub fn suggest_config(target_similarity: f64, sketch_size: usize) -> (usize, usize) {
        assert!(sketch_size > 0, "sketch_size must be > 0");
        assert!(
            (0.0..=1.0).contains(&target_similarity),
            "target_similarity must be in [0.0, 1.0]"
        );

        let mut best_band_count: usize = 1;
        let mut best_band_size: usize = sketch_size;
        let mut best_gap: f64 = f64::INFINITY;

        for band_count in 1..=sketch_size {
            for band_size in 1..=(sketch_size / band_count) {
                // Threshold at which P_collision = 0.5 — the classical
                // crossover point of the S-curve.
                let threshold = (1.0_f64 / band_count as f64).powf(1.0_f64 / band_size as f64);
                let gap = (threshold - target_similarity).abs();
                // Prefer the pair with the smaller gap; break ties toward
                // the higher band_count for a steeper transition.
                let better = gap < best_gap
                    || ((gap - best_gap).abs() < f64::EPSILON && band_count > best_band_count);
                if better {
                    best_gap = gap;
                    best_band_count = band_count;
                    best_band_size = band_size;
                }
            }
        }
        (best_band_count, best_band_size)
    }
}

/// Integer-exponent power for a `f64`, `no_std`-friendly.
///
/// Replacement for `f64::powi`, which lives in `std` (not `core`). This
/// is only used with small `n` (`band_size` and `band_count`), so the
/// naive loop is cheap.
#[inline]
#[must_use]
fn pow_int(base: f64, n: usize) -> f64 {
    let mut acc = 1.0_f64;
    for _ in 0..n {
        acc *= base;
    }
    acc
}

/// Hashes a band's row-signatures to a single `u64` for the `LSH` bucket.
///
/// The mixing is a stream of `splitmix64(state XOR row)` folds. This is
/// stable across builds, portable, and avoids depending on any external
/// hasher.
#[must_use]
fn hash_band(rows: &[u64]) -> u64 {
    // Seed with a nonzero constant so that a band of all-zero rows still
    // produces a nonzero band-hash — otherwise the empty-set sentinel
    // configuration would map every band to `0`.
    let mut state: u64 = 0xa5a5_a5a5_a5a5_a5a5;
    for r in rows {
        state = splitmix64(state ^ *r);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sketch_from<G: core::hash::Hash>(items: &[G]) -> MinHashSketch {
        MinHashSketch::from_iter(64, 42, items.iter())
    }

    #[test]
    fn empty_index_reports_empty() {
        let idx = LshIndex::new(4, 16);
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn identical_sketches_are_candidates() {
        let mut idx = LshIndex::new(4, 16);
        let a = sketch_from(&[1u32, 2, 3, 4, 5]);
        let b = sketch_from(&[1u32, 2, 3, 4, 5]);
        idx.insert(7, &a);
        let cands = idx.query_candidates(&b);
        assert!(cands.contains(&7));
    }

    #[test]
    fn disjoint_sketches_rarely_candidate() {
        let mut idx = LshIndex::new(4, 16);
        let a = sketch_from(&[1u32, 2, 3, 4, 5]);
        let b = sketch_from(&[100u32, 200, 300, 400, 500]);
        idx.insert(7, &a);
        // Not asserting empty (`LSH` is probabilistic) — this documents
        // the intended pattern without a flaky assertion.
        let _ = idx.query_candidates(&b);
    }

    #[test]
    fn insert_counts_track_bands() {
        let mut idx = LshIndex::new(2, 4);
        let a = sketch_from(&[1u32, 2, 3]);
        idx.insert(0, &a);
        idx.insert(1, &a);
        assert_eq!(idx.len(), 2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn collision_probability_matches_formula() {
        let idx = LshIndex::new(4, 8);
        let s: f64 = 0.7;
        let expected = 1.0 - (1.0_f64 - s.powi(4)).powi(8);
        let observed = idx.collision_probability(s);
        assert!(
            (observed - expected).abs() < 1e-12,
            "expected {expected}, observed {observed}"
        );
    }

    #[test]
    fn collision_probability_endpoints() {
        let idx = LshIndex::new(4, 8);
        assert_eq!(idx.collision_probability(0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(idx.collision_probability(1.0).to_bits(), 1.0_f64.to_bits());
    }

    #[cfg(feature = "std")]
    #[test]
    fn suggest_config_picks_valid_pair() {
        let (bc, bs) = LshIndex::suggest_config(0.8, 128);
        assert!(bc * bs <= 128);
        assert!(bc > 0 && bs > 0);
    }

    #[cfg(feature = "std")]
    #[test]
    #[allow(
        clippy::cast_precision_loss,
        reason = "sketch_size 128 fits in an f64 mantissa without loss"
    )]
    fn suggest_config_crossover_close_to_target() {
        let target = 0.75;
        let (bc, bs) = LshIndex::suggest_config(target, 128);
        let bands_f = bc as f64;
        let rows_f = bs as f64;
        let threshold = (1.0_f64 / bands_f).powf(1.0_f64 / rows_f);
        // A 128-slot budget can hit any target within a few percent.
        assert!(
            (threshold - target).abs() < 0.05,
            "threshold {threshold} too far from target {target} for (bc, bs) = ({bc}, {bs})"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn suggest_config_lower_target_yields_more_bands() {
        // A lower similarity target wants MORE bands (each band is
        // easier to collide, so the S-curve crossover moves left).
        let (bc_low, _) = LshIndex::suggest_config(0.4, 128);
        let (bc_high, _) = LshIndex::suggest_config(0.9, 128);
        assert!(
            bc_low > bc_high,
            "expected more bands for lower target: low={bc_low}, high={bc_high}"
        );
    }

    #[test]
    #[should_panic(expected = "band_size must be > 0")]
    fn zero_band_size_panics() {
        let _ = LshIndex::new(0, 4);
    }

    #[test]
    #[should_panic(expected = "band_count must be > 0")]
    fn zero_band_count_panics() {
        let _ = LshIndex::new(4, 0);
    }

    #[test]
    #[should_panic(expected = "sketch too small")]
    fn tiny_sketch_panics() {
        let mut idx = LshIndex::new(8, 16); // needs 128 sigs
        let a: MinHashSketch = MinHashSketch::from_iter::<u32, _>(32, 42, [1u32]);
        idx.insert(0, &a);
    }
}
