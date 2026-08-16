//! Property-based differential tests for the SIMD OSA backend.
//!
//! Compiled only under `--features simd` and only on non-wasm hosts (the
//! `proptest` transitive dep is not wasm-friendly, matching the pattern
//! used by the sibling [`crate::damerau::property_tests`] module).
//!
//! The claim under test is bit-for-bit agreement between the SIMD entry
//! points and the crate's `full_matrix` oracle. If any SIMD backend
//! disagrees with the oracle on any generated input, the whole story of
//! "SIMD as a transparent accelerator" is broken; this file is the
//! guard.

use proptest::prelude::*;

use crate::damerau::osa::full_matrix::distance_full_matrix;
use crate::damerau::osa::simd;
use crate::damerau::osa::simd::scalar;

/// Byte-slice strategy over the full byte alphabet, capped at 128 bytes.
///
/// 128 bytes crosses both the single-word Myers boundary (m ≤ 64) and
/// the SSE2/NEON/wasm-SIMD wide-block boundary (m > 64), so both paths
/// of the bit-parallel OSA are exercised on those three backends by the
/// same test.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=128)
}

/// Byte-slice strategy capped at 256 bytes — the AVX2 wide-block-256
/// path only fires for `128 < m ≤ 256`, so the `arb_bytes` strategy
/// leaves it uncovered. This strategy exercises the AVX2 four-lane
/// bit-parallel OSA end-to-end.
#[cfg(target_arch = "x86_64")]
fn arb_bytes_wide_avx2() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=256)
}

/// Long-side strategy — up to 512 bytes — to catch any state that might
/// only misbehave after many column iterations.
fn arb_bytes_long() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=512)
}

/// Small-alphabet strategy — a three-symbol byte alphabet — that raises
/// the frequency of adjacent-transposition matches. Small alphabets are
/// the stress case for OSA's transposition branch.
fn arb_bytes_small_alphabet() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'b'), Just(b'c')], 0..=128)
}

/// Small-alphabet strategy up to 256 bytes — same shape as
/// [`arb_bytes_small_alphabet`] but stretched into the AVX2 wide-block
/// range so the four-lane Hyyrö carry propagation is stressed with
/// frequent transposition matches.
#[cfg(target_arch = "x86_64")]
fn arb_bytes_small_alphabet_wide() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'b'), Just(b'c')], 0..=256)
}

proptest! {
    /// The runtime-dispatching [`simd::distance`] must agree with the
    /// full-matrix oracle on every generated input pair.
    #[test]
    fn dispatcher_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let observed = simd::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "simd::distance disagreed with oracle");
    }

    /// The scalar SIMD-shaped OSA backend must agree with the
    /// full-matrix oracle; this is the anchor every arch-specific
    /// backend chains through.
    #[test]
    fn scalar_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let observed = scalar::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed with oracle");
    }

    /// Long-side coverage: many column iterations against a long text.
    #[test]
    fn scalar_matches_oracle_on_long_text(
        pattern in arb_bytes(),
        text in arb_bytes_long(),
    ) {
        let observed = scalar::distance(&pattern, &text);
        let expected = distance_full_matrix(&pattern, &text);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed on long text");
    }

    /// Small-alphabet coverage: transposition branch fires often, so a
    /// bug in that branch would surface here more readily than on the
    /// random-byte strategy.
    #[test]
    fn scalar_matches_oracle_small_alphabet(
        a in arb_bytes_small_alphabet(),
        b in arb_bytes_small_alphabet(),
    ) {
        let observed = scalar::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed on small-alphabet input");
    }

    /// Symmetry: OSA is symmetric in its inputs. The SIMD kernel picks
    /// one side to iterate over, so a symmetry violation would indicate
    /// that the choice matters (a bug).
    #[test]
    fn scalar_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            scalar::distance(&a, &b),
            scalar::distance(&b, &a),
            "scalar SIMD-shaped disagreed with itself under argument-swap"
        );
    }

    /// The AVX2 backend, when available on the host, must agree with
    /// the scalar SIMD-shaped kernel on every input.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "avx2 disagreed with scalar");
    }

    /// SSE2 differential — SSE2 is baseline on x86_64.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn sse2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("sse2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd_result = unsafe { simd::x86_sse2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "sse2 disagreed with scalar");
    }

    /// NEON differential — NEON is baseline on aarch64.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd_result = unsafe { simd::aarch64_neon::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "neon disagreed with scalar");
    }

    /// AVX2 wide-block-256 coverage: `arb_bytes_wide_avx2` reaches into
    /// the `128 < m ≤ 256` band that only the AVX2 backend handles in a
    /// single register. Without this strategy the four-lane Hyyrö path
    /// would never be entered from proptest.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_wide_block_matches_scalar(
        a in arb_bytes_wide_avx2(),
        b in arb_bytes_wide_avx2(),
    ) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "avx2 wide-block disagreed with scalar");
    }

    /// AVX2 wide-block, small-alphabet flavour: transposition branch
    /// fires frequently at high `m`.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_wide_block_small_alphabet_matches_scalar(
        a in arb_bytes_small_alphabet_wide(),
        b in arb_bytes_small_alphabet_wide(),
    ) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(
            simd_result, scalar_result,
            "avx2 wide-block disagreed with scalar on small-alphabet input"
        );
    }
}

/// Explicit canonical adjacent-transposition pairs, exercised at
/// arbitrary lengths that reach every backend's wide-block band.
///
/// The tuples embed the classical single-transposition witnesses
/// (`"MARTHA"`/`"MARHTA"`, `"ab"`/`"ba"`, `"abcdef"`/`"abcdfe"`) into
/// longer padded inputs so the transposition falls at the wide-block
/// boundary as well as inside a single word, and long random-tail pairs
/// (m up to 300) that stretch the AVX2 four-lane state and the
/// SSE2/NEON/wasm-SIMD two-lane state through many column iterations.
///
/// Every case is asserted through both the runtime dispatcher and the
/// scalar backend so any disagreement is unambiguously flagged.
#[cfg(test)]
mod adjacent_transposition_wide_block {
    use super::*;

    /// Repeat `pad` until `n` bytes are produced. Used to build long
    /// prefixes and suffixes that stretch a canonical transposition pair
    /// across the wide-block boundary.
    fn pad(seed: &[u8], n: usize) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::with_capacity(n);
        while out.len() < n {
            let need = n - out.len();
            let take = need.min(seed.len());
            out.extend_from_slice(&seed[..take]);
        }
        out
    }

    fn distinct_symbol_seed(seed: u32) -> alloc::vec::Vec<u8> {
        // Rotate through a 16-byte cycle whose base bytes are all
        // pairwise distinct and disjoint from the transposition
        // witnesses (avoid ASCII letters used in "MARTHA"/"MARHTA"
        // and 'a'/'b' pairs).
        let base: [u8; 16] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        // `seed & 0xFF` is by construction in range for `u8`; we use
        // `try_from` on the masked value to keep the workspace's
        // `cast_possible_truncation` lint quiet without an allow.
        let seed_lo = u8::try_from(seed & 0xFF).expect("mask by 0xFF bounds to u8");
        (0..16u8)
            .map(|i| base[usize::from(i.wrapping_add(seed_lo) & 0x0F)])
            .collect()
    }

    /// Every canonical adjacent-transposition pair must match the
    /// full-matrix oracle under the runtime dispatcher, at padded
    /// lengths that reach the scalar-word, SSE2/NEON/wasm-SIMD
    /// wide-block, and AVX2 wide-block bands.
    #[test]
    fn martha_marhta_at_every_backend_length() {
        let witnesses: &[(&[u8], &[u8])] = &[
            (b"MARTHA", b"MARHTA"),
            (b"ab", b"ba"),
            (b"abcdef", b"abcdfe"),
            (b"kitten", b"sittign"),
        ];
        // 32 (below OSA_MIN_LEN), 65 (into scalar-word wide-block),
        // 96 (mid SSE2/NEON wide-block), 128 (SSE2/NEON boundary),
        // 200 (mid AVX2 wide-block), 256 (AVX2 boundary), 300 (past
        // AVX2 — delegates to scalar rolling-rows for AVX2).
        let lengths: &[usize] = &[32, 65, 96, 128, 200, 256, 300];
        for (needle, twisted) in witnesses {
            for &len in lengths {
                if needle.len() >= len || twisted.len() >= len {
                    continue;
                }
                let prefix_len = (len - needle.len()) / 2;
                let seed = distinct_symbol_seed(u32::try_from(len).unwrap());
                let prefix = pad(&seed, prefix_len);
                let suffix_seed = distinct_symbol_seed(u32::try_from(len).unwrap() ^ 0x9E37);
                let suffix_len = len - prefix.len() - needle.len();
                let suffix = pad(&suffix_seed, suffix_len);

                let mut a = alloc::vec::Vec::with_capacity(len);
                a.extend_from_slice(&prefix);
                a.extend_from_slice(needle);
                a.extend_from_slice(&suffix);
                let mut b = alloc::vec::Vec::with_capacity(len);
                b.extend_from_slice(&prefix);
                b.extend_from_slice(twisted);
                b.extend_from_slice(&suffix);

                let expected = distance_full_matrix(&a, &b);
                let observed = simd::distance(&a, &b);
                assert_eq!(
                    observed, expected,
                    "dispatcher disagreed with oracle at len={len} on witness={needle:?} vs {twisted:?}"
                );
                let scalar_observed = scalar::distance(&a, &b);
                assert_eq!(
                    scalar_observed, expected,
                    "scalar backend disagreed with oracle at len={len} on witness={needle:?} vs {twisted:?}"
                );
            }
        }
    }

    /// Long random-tail inputs (m ∈ {150, 200, 256, 300}) that force
    /// the wide-block Hyyrö loops through many column iterations. Uses
    /// a deterministic xorshift so failures reproduce.
    #[test]
    fn long_input_pairs_match_oracle_through_dispatcher() {
        let lengths: &[usize] = &[150, 200, 256, 300];
        for &m in lengths {
            let mut state: u64 = 0xDEAD_BEEF_CAFE_F00D_u64.wrapping_add(m as u64);
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let a: alloc::vec::Vec<u8> = (0..m).map(|_| (next() & 0xff) as u8).collect();
            let n = m + 17;
            let b: alloc::vec::Vec<u8> = (0..n).map(|_| (next() & 0xff) as u8).collect();

            let expected = distance_full_matrix(&a, &b);
            let observed = simd::distance(&a, &b);
            assert_eq!(
                observed, expected,
                "dispatcher disagreed with oracle at long-input length m={m}"
            );
        }
    }
}
