//! Portable scalar Buzhash slice-batch reference.
//!
//! Always compiled — this is the byte-identical correctness anchor
//! every arch-specific backend is differentially tested against. See
//! the [module docs][super] for the SIMD-tree contract.

use crate::fingerprint::buzhash::BUZ_TABLE;

/// Portable single-`u64` Buzhash digest of a byte slice.
///
/// Byte-for-byte identical to
///
/// ```ignore
/// let mut h = Buzhash::new(window);
/// for &b in bytes { h.roll(b); }
/// h.digest()
/// ```
///
/// The eviction byte at position `i` (once the window has begun to
/// overflow) is read from `bytes[i - window]` directly, matching the
/// scalar implementation's circular-buffer semantics without allocating
/// one here.
///
/// A `window` of zero is legal but degenerate — every roll is a no-op
/// and the digest is always the identity zero.
#[inline]
#[must_use]
pub fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if window == 0 {
        return 0;
    }

    // `window mod 64` folded once; rotating a `u64` by `k` bits is
    // equivalent to rotating by `k mod 64`, and this keeps the eviction
    // rotate cheap even when `window > 64`. `window % 64` is always in
    // `0..64` and fits in a `u32`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "`window % 64` is in 0..64 and always fits in a u32"
    )]
    let window_rot = (window % 64) as u32;

    let mut state: u64 = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        let new_contrib = BUZ_TABLE[byte as usize];
        if i >= window {
            let leaving = bytes[i - window];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(window_rot);
            state = state.rotate_left(1) ^ leaving_contrib ^ new_contrib;
        } else {
            state = state.rotate_left(1) ^ new_contrib;
        }
    }
    state
}
