//! Shared helpers for the arch-specific Jaro SIMD backends.
//!
//! Each of the three arch-specific backends
//! ([`super::x86_sse2`], [`super::x86_avx2`], [`super::aarch64_neon`])
//! reuses the same packed matched-position bitmap and the same
//! bit-extraction routine to project a contiguous window of the bitmap
//! into a 16-bit / 32-bit mask that can be `AND`ed against a SIMD byte
//! compare result. Hoisting them here keeps the arch backends focused on
//! the intrinsic-specific parts (load, broadcast, `cmpeqb`, movemask) and
//! avoids re-deriving the bitmap layout in three places.

use alloc::vec;
use alloc::vec::Vec;

/// Packed bit-vector, one bit per position, backed by a flat `Vec<u64>`.
///
/// This is the same shape as the scalar SIMD-shaped kernel's private
/// [`super::scalar`] `Bitmap`; the arch backends need their own local
/// copy because that one is `struct-private`. Sharing the layout keeps
/// [`Self::read_bits`] usable across all backends.
pub(super) struct Bitmap {
    words: Vec<u64>,
    bits: usize,
}

impl Bitmap {
    /// Allocate a bitmap for `bits` positions, all initially unset.
    #[inline]
    pub(super) fn new(bits: usize) -> Self {
        // `bits.div_ceil(64)` is the number of 64-bit words required.
        // We keep one guard word past the end so `read_bits` at
        // `abs_start = bits - 1` never needs a bounds check for the
        // second word of a straddling extraction.
        let words = bits.div_ceil(64) + 1;
        Self {
            words: vec![0u64; words],
            bits,
        }
    }

    /// Returns `true` iff bit `i` is set. Panics on out-of-range `i` in
    /// debug builds; release keeps the check off the hot path.
    #[inline]
    pub(super) fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.bits);
        (self.words[i >> 6] >> (i & 63)) & 1 == 1
    }

    /// Sets bit `i`.
    #[inline]
    pub(super) fn set(&mut self, i: usize) {
        debug_assert!(i < self.bits);
        self.words[i >> 6] |= 1u64 << (i & 63);
    }

    /// Reads `count` consecutive bits starting at absolute position
    /// `abs_start`, packed into a `u64` with the LSB corresponding to
    /// bit `abs_start`.
    ///
    /// `count` must be `<= 64`; bits past the end of the bitmap read
    /// as zero (the guard word in [`Self::new`] makes this branchless).
    #[inline]
    pub(super) fn read_bits(&self, abs_start: usize, count: usize) -> u64 {
        debug_assert!(count <= 64);
        let word_idx = abs_start >> 6;
        let bit_off = abs_start & 63;
        // The guard word past the end guarantees `word_idx + 1` is a
        // valid index whenever `word_idx` is; both loads are always
        // in-bounds without a branch.
        let lo = self.words[word_idx] >> bit_off;
        let hi = if bit_off == 0 {
            0
        } else {
            self.words[word_idx + 1] << (64 - bit_off)
        };
        let full = lo | hi;
        if count >= 64 {
            full
        } else {
            full & ((1u64 << count) - 1)
        }
    }
}

/// Shared inner scalar fallback used when the input pair is below the
/// minimum SIMD length. Every arch backend calls this on the short-input
/// branch so the dispatcher's amenability check stays one-liner.
///
/// Uses the SIMD-shaped scalar reference directly.
#[inline]
pub(super) fn scalar_fallback(a: &[u8], b: &[u8]) -> f64 {
    super::scalar::similarity(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_bits_matches_scalar_get() {
        let mut bm = Bitmap::new(200);
        for &i in &[0usize, 3, 63, 64, 65, 127, 128, 199] {
            bm.set(i);
        }
        // Full-word alignment.
        assert_eq!(bm.read_bits(0, 64) & 1, 1);
        assert_eq!((bm.read_bits(0, 64) >> 3) & 1, 1);
        assert_eq!((bm.read_bits(0, 64) >> 63) & 1, 1);
        // Straddling read.
        assert_eq!((bm.read_bits(60, 8) >> 4) & 1, 1); // bit 64
        assert_eq!((bm.read_bits(60, 8) >> 5) & 1, 1); // bit 65
        // Reading past the end zeros the tail.
        assert_eq!(bm.read_bits(199, 8), 1);
    }
}
