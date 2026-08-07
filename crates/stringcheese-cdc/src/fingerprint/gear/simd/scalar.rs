//! Portable scalar Gear-hash slice-batch reference.
//!
//! Always compiled — this is the byte-identical correctness anchor every
//! arch-specific backend is differentially tested against. See the
//! [module docs][super] for the SIMD-tree contract.

use crate::fingerprint::gear::GEAR_TABLE;

/// Portable single-`u64` Gear digest of a byte slice.
///
/// Byte-for-byte identical to
///
/// ```ignore
/// let mut h = GearHash::new(64);
/// for &b in bytes { h.roll(b); }
/// h.state()
/// ```
///
/// The naming mirrors the arch-specific siblings' `digest_of_slice`
/// entry points so a hand-written wide-block replacement can drop in
/// without changing this module's API.
#[inline]
#[must_use]
pub fn digest_of_slice(bytes: &[u8]) -> u64 {
    let mut state: u64 = 0;
    for &b in bytes {
        // Wrapping shift-and-add is the whole recurrence — see the
        // [`RollingHash::roll`][crate::fingerprint::RollingHash::roll]
        // implementation on [`GearHash`][crate::fingerprint::gear::GearHash].
        state = (state << 1).wrapping_add(GEAR_TABLE[b as usize]);
    }
    state
}
