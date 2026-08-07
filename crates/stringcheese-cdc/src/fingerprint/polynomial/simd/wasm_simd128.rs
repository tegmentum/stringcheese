//! wasm SIMD128-gated polynomial-hash slice-batch backend for `wasm32`.
//!
//! Compiled only on `wasm32` and only when the `simd128` target-feature
//! is enabled at compile time. See the [module docs][super] and the
//! parallel Gear wasm backend at
//! [`crate::fingerprint::gear::simd::wasm_simd128`] for the compile-time
//! gating model and the shared `unsafe` policy.
//!
//! Polynomial-hash's Mersenne-61 modular arithmetic requires a
//! 64×64 → 128 multiply for every byte; wasm SIMD128 does not surface
//! that primitive on integer lanes, so this backend cannot bit-parallel
//! the recurrence and is shipped as the scalar core under the wasm
//! SIMD128 compile-time flag. It preserves API uniformity across the
//! four hashes so downstream code can dispatch through
//! [`super::digest_of_slice`] regardless of hash family without a
//! per-arch conditional.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// wasm SIMD128-gated polynomial-hash digest of a byte slice.
///
/// # Safety
///
/// See the module-level safety note — on `wasm32 + simd128` this is
/// unconditionally safe.
#[target_feature(enable = "simd128")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    scalar::digest_of_slice(window, bytes)
}
