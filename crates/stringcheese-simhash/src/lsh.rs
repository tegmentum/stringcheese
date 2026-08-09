//! Permutation-banding LSH over SimHash fingerprints.
//!
//! SimHash's Hamming-distance similarity admits an LSH scheme
//! similar to MinHash's: divide the fingerprint into `b` chunks
//! of `r` bits and use each chunk as a bucket key. Near-duplicate
//! fingerprints (small Hamming distance) share at least one chunk
//! with high probability.
//!
//! For fixed budget of `bits = 64` or `128`:
//!
//! - 64-bit: 4 bands of 16 bits, 8 bands of 8, or 16 bands of 4.
//! - 128-bit: 8 bands of 16, 16 bands of 8, or 32 bands of 4.
//!
//! Wider bands (larger `r`) → fewer false positives, more false
//! negatives. Tune `(b, r)` to whatever Hamming threshold matters
//! for the pipeline.

use alloc::vec::Vec;

use crate::sketch::{Sketch64, Sketch128};

/// Split a [`Sketch64`] into `b` bands of `r = 64 / b` bits and
/// return one `u64` key per band (the band's bits, zero-extended
/// into a u64 with the band index mixed in).
///
/// # Panics
///
/// Panics when `64 % b != 0` — bands must partition the fingerprint
/// evenly.
#[must_use]
pub fn bands_64(sketch: &Sketch64, b: usize) -> Vec<u64> {
    assert!(b > 0 && 64 % b == 0, "b must divide 64 evenly; got {b}");
    let r = 64 / b;
    let mask = if r == 64 { u64::MAX } else { (1u64 << r) - 1 };
    let bits = sketch.bits();
    (0..b)
        .map(|i| {
            let chunk = (bits >> (i * r)) & mask;
            // Mix the band index into the key so identical row
            // patterns in different bands don't collide in the
            // shared bucket table.
            chunk
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(i as u64)
        })
        .collect()
}

/// Split a [`Sketch128`] into `b` bands of `r = 128 / b` bits and
/// return one `u64` key per band.
///
/// # Panics
///
/// Panics when `128 % b != 0`.
#[must_use]
pub fn bands_128(sketch: &Sketch128, b: usize) -> Vec<u64> {
    assert!(b > 0 && 128 % b == 0, "b must divide 128 evenly; got {b}");
    let r = 128 / b;
    let (hi, lo) = sketch.bits();
    // Treat (hi, lo) as one 128-bit int; extract each r-bit chunk.
    (0..b)
        .map(|i| {
            let bit_offset = i * r;
            let chunk = if bit_offset >= 64 {
                (hi >> (bit_offset - 64)) & mask(r)
            } else if bit_offset + r <= 64 {
                (lo >> bit_offset) & mask(r)
            } else {
                // Chunk straddles the 64-bit boundary — glue the
                // tail of lo to the head of hi.
                let lo_bits = 64 - bit_offset;
                let lo_part = lo >> bit_offset;
                let hi_part = hi & mask(r - lo_bits);
                lo_part | (hi_part << lo_bits)
            };
            chunk
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(i as u64)
        })
        .collect()
}

fn mask(r: usize) -> u64 {
    if r == 64 { u64::MAX } else { (1u64 << r) - 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sketcher;

    #[test]
    fn identical_fingerprints_share_every_band() {
        let s1 = Sketcher::new().add_all(["a", "b", "c"]).finalize_64();
        let s2 = Sketcher::new().add_all(["a", "b", "c"]).finalize_64();
        assert_eq!(bands_64(&s1, 8), bands_64(&s2, 8));
    }

    #[test]
    fn different_fingerprints_diverge_across_bands() {
        // Take two disjoint bags with expected ~50% Hamming
        // agreement. Under 8 bands of 8 bits, the expected number
        // of matching bands is ~1 (0.5^8 = 0.004 chance any given
        // band matches → ~0.03 expected across 8 bands; loose
        // guard).
        let a: alloc::vec::Vec<alloc::string::String> =
            (0..16).map(|i| alloc::format!("a-{i}")).collect();
        let b: alloc::vec::Vec<alloc::string::String> =
            (0..16).map(|i| alloc::format!("z-{i}")).collect();
        let s1 = Sketcher::new()
            .add_all(a.iter().map(alloc::string::String::as_str))
            .finalize_64();
        let s2 = Sketcher::new()
            .add_all(b.iter().map(alloc::string::String::as_str))
            .finalize_64();
        let bs1 = bands_64(&s1, 8);
        let bs2 = bands_64(&s2, 8);
        let shared = bs1.iter().zip(bs2.iter()).filter(|(a, b)| a == b).count();
        assert!(shared <= 2, "expected <= 2 shared bands, got {shared}");
    }

    #[test]
    fn bands_128_partition_correctly() {
        let s = Sketcher::new().add_all(["a", "b", "c"]).finalize_128();
        // 16 bands of 8 bits, 8 bands of 16 bits — both partitions
        // divide 128 evenly.
        assert_eq!(bands_128(&s, 16).len(), 16);
        assert_eq!(bands_128(&s, 8).len(), 8);
    }

    #[test]
    #[should_panic(expected = "b must divide 64 evenly")]
    fn bad_band_count_64_panics() {
        let s = Sketcher::new().add("x").finalize_64();
        let _ = bands_64(&s, 7);
    }

    #[test]
    #[should_panic(expected = "b must divide 128 evenly")]
    fn bad_band_count_128_panics() {
        let s = Sketcher::new().add("x").finalize_128();
        let _ = bands_128(&s, 7);
    }
}
