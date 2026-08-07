//! Portable scalar Rabin-fingerprint slice-batch reference.
//!
//! Always compiled — the byte-identical correctness anchor every
//! arch-specific backend is differentially tested against. See the
//! [module docs][super] for the SIMD-tree contract.
//!
//! The polynomial-reduction machinery (`SHIFT_TABLE`, roll-out table
//! build) lives in the streaming
//! [`RabinFingerprint`];
//! this reference drives that type directly rather than duplicating the
//! `GF(2)` arithmetic so the two paths cannot silently diverge under
//! future edits. A hand-written wide-block SIMD kernel would inline
//! its own state and table access — the API shape here is stable
//! regardless.

use crate::fingerprint::RollingHash;
use crate::fingerprint::rabin::RabinFingerprint;

/// Portable single-`u64` Rabin-fingerprint digest of a byte slice.
///
/// Byte-for-byte identical to
///
/// ```ignore
/// let mut h = RabinFingerprint::new(window);
/// for &b in bytes { h.roll(b); }
/// h.digest()
/// ```
///
/// The implementation *is* that loop; a hand-written wide-block SIMD
/// replacement would inline the polynomial arithmetic to avoid the
/// per-instance table allocation, but the correctness contract is
/// bit-for-bit against this reference.
#[inline]
#[must_use]
pub fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    let mut h = RabinFingerprint::new(window);
    for &b in bytes {
        h.roll(b);
    }
    h.digest()
}
