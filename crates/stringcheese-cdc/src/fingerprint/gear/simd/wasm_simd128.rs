//! wasm SIMD128-gated Gear-hash slice-batch backend for `wasm32`.
//!
//! Compiled only on `wasm32` targets and only when the `simd128`
//! target-feature is enabled at compile time. Unlike `x86_64` and
//! `aarch64`, wasm has no runtime CPU-feature detection: whether the
//! SIMD opcodes are legal is a property of the wasm engine executing
//! the module. Callers control the choice via
//! `RUSTFLAGS=-C target-feature=+simd128` at build time, and the
//! dispatcher in [`super`] compiles this path in or out with a matching
//! `#[cfg(target_feature = "simd128")]` gate.
//!
//! # Kernel shape
//!
//! Gear's `state = (state << 1) + G[byte]` recurrence is strictly
//! sequential, so this backend consumes the byte slice sequentially
//! inside the wasm SIMD128 target-feature context. See the
//! [module docs][super] for why the initial cut ships the same core
//! under wasm SIMD128 rather than a hand-written wide-block kernel — a
//! bit-parallel replacement is documented follow-up work.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` for parity with the sibling
//! SSE2/AVX2/NEON backends' `#[target_feature]`-gated signature, even
//! though on wasm the target feature is a compile-time property rather
//! than a runtime precondition. On `wasm32` with
//! `target_feature = "simd128"` this function is unconditionally safe
//! to call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// wasm SIMD128-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// See the module-level safety note — on `wasm32 + simd128` this is
/// unconditionally safe.
#[target_feature(enable = "simd128")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    scalar::digest_of_slice(bytes)
}
