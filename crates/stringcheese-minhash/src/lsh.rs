//! Locality-sensitive hashing (LSH) banding over MinHash sketches.
//!
//! Given a corpus of N sketches, a naive "find near-duplicates"
//! search is `O(N²)` sketch comparisons. LSH banding turns that
//! into an approximate `O(N)` lookup by dividing each sketch into
//! `b` bands of `r` rows and hashing each band separately —
//! sketches that agree in any single band become candidates for
//! full Jaccard scoring.
//!
//! The `(b, r)` choice trades off recall against false-positive
//! rate. Rule of thumb: the S-curve inflection point is around
//! `s ≈ (1/b)^(1/r)`; choose `b · r ≤ width` and tune to whatever
//! Jaccard threshold matters for the pipeline.
//!
//! ## Example
//!
//! ```
//! use stringcheese_minhash::{Sketcher, lsh::band_signatures};
//!
//! let s = Sketcher::new(128).sketch(["foo", "bar", "baz"]);
//! // 16 bands of 8 rows each (128 = 16 · 8).
//! let bands = band_signatures(&s, 16, 8);
//! assert_eq!(bands.len(), 16);
//! ```

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

use ahash::AHasher;

use crate::sketch::Sketch;

/// Compute `b` band signatures for a sketch by splitting the
/// `width = b · r` min-hash vector into `b` contiguous bands of
/// `r` rows and hashing each band.
///
/// Two sketches share a bucket in band `i` iff their `i`-th band
/// signature matches — a much cheaper index key than the full
/// sketch. Callers wire an inverted index `HashMap<(band_idx,
/// band_sig), Vec<CorpusId>>` around this to get O(1) candidate
/// lookup.
///
/// # Panics
///
/// Panics when `b * r != sketch.width()` — the banding decomposition
/// must exactly cover the sketch.
#[must_use]
pub fn band_signatures(sketch: &Sketch, b: usize, r: usize) -> Vec<u64> {
    assert_eq!(
        b * r,
        sketch.width(),
        "b * r must equal sketch width; got {} * {} != {}",
        b,
        r,
        sketch.width(),
    );
    let mins = sketch.as_slice();
    (0..b)
        .map(|i| {
            let band = &mins[i * r..(i + 1) * r];
            // Hash both the band contents AND the band index — two
            // different bands with identical row values must
            // produce different signatures so they don't collide
            // in the LSH bucket table.
            let mut hasher = AHasher::default();
            i.hash(&mut hasher);
            for &m in band {
                m.hash(&mut hasher);
            }
            hasher.finish()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sketcher;

    #[test]
    fn identical_sketches_share_every_band() {
        let s1 = Sketcher::new(64).sketch(["a", "b", "c", "d"]);
        let s2 = Sketcher::new(64).sketch(["a", "b", "c", "d"]);
        let b1 = band_signatures(&s1, 8, 8);
        let b2 = band_signatures(&s2, 8, 8);
        assert_eq!(b1, b2);
    }

    #[test]
    fn different_sketches_diverge_in_bands() {
        let s1 = Sketcher::new(64).sketch(["a", "b", "c", "d"]);
        let s2 = Sketcher::new(64).sketch(["w", "x", "y", "z"]);
        let b1 = band_signatures(&s1, 8, 8);
        let b2 = band_signatures(&s2, 8, 8);
        let shared = b1.iter().zip(b2.iter()).filter(|(a, b)| a == b).count();
        // Disjoint 4-element sets under 8-row bands share zero
        // bands with overwhelming probability. Guard on <= 1 to
        // absorb a hypothetical single collision without turning
        // this into a flaky test.
        assert!(shared <= 1, "expected 0-1 shared bands, got {shared}");
    }

    #[test]
    #[should_panic(expected = "b * r must equal sketch width")]
    fn mismatched_band_shape_panics() {
        let s = Sketcher::new(64).sketch(["a"]);
        let _ = band_signatures(&s, 7, 10); // 70 != 64
    }
}
