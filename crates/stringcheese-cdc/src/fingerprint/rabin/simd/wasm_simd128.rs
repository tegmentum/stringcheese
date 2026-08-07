//! wasm SIMD128-gated Rabin-fingerprint slice-batch backend for `wasm32`.
//!
//! Compiled only on `wasm32` and only when the `simd128` target-feature
//! is enabled at compile time. See the [module docs][super] and the
//! parallel Gear wasm backend at
//! [`crate::fingerprint::gear::simd::wasm_simd128`] for the compile-time
//! gating model and the shared `unsafe` policy.
//!
//! # ISA constraint — no carry-less multiply on wasm SIMD128
//!
//! The [x86_sse2][super::x86_sse2], [x86_avx2][super::x86_avx2], and
//! [aarch64_neon][super::aarch64_neon] backends ship a real
//! vectorized Rabin kernel built on hardware carry-less multiply
//! (`pclmulqdq`, `vpclmulqdq`, `PMULL` respectively). wasm SIMD128
//! does **not** surface any `GF(2)` carry-less multiply primitive on
//! integer lanes — there is no wasm counterpart to `pclmulqdq` or
//! `vmull_p64` today. A software carry-less multiply built from
//! `u64x2` XOR-accumulate would cost more than the scalar recurrence
//! it displaces on any current wasm engine, so this backend is
//! shipped as the scalar core under the wasm SIMD128 compile-time
//! flag. The `#[target_feature(enable = "simd128")]` context still
//! lets the compiler generate SIMD-flavoured code for surrounding
//! utility work.
//!
//! Byte-identical output to the streaming reference is preserved by
//! delegating to the same portable `RabinFingerprint`-driven scalar
//! loop the other backends fall back to when their runtime feature
//! check fails.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// wasm SIMD128-gated Rabin-fingerprint digest of a byte slice.
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
