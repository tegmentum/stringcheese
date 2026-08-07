//! Shared SIMD dispatch helpers used by the byte-slice SIMD backends.
//!
//! Every SIMD sub-tree in this crate (`levenshtein::simd`, `jaro::simd`,
//! `damerau::osa::simd`) needs the same viability guard for its byte-slice
//! entry point: both inputs must be `u8`-shaped by construction, and the
//! shorter side must clear a per-algorithm minimum length below which the
//! per-call setup cost of the SIMD backend (Peq-table build, per-symbol
//! bitmap scan, or similar) outweighs the algorithmic win. Hoisting that
//! one-line helper here is a small win in its own right, but it also
//! documents the amenability contract in one place so downstream callers
//! can reason about SIMD-vs-scalar dispatch without having to read three
//! near-identical helpers.
//!
//! # Why the helper is shared and the dispatchers are not
//!
//! The runtime CPU-feature dispatchers in each SIMD module look nearly
//! identical, but they call `unsafe fn` backends with different signatures
//! (`u32` distance, `f64` similarity) and different documentation
//! requirements (each `SAFETY:` comment names the CPU-feature precondition
//! its own caller upheld). Hoisting the dispatcher body would require a
//! generic-over-fn-pointer wrapper that obscured those safety comments,
//! for no real gain — the per-module dispatcher is 15-20 lines of
//! `is_x86_feature_detected!` / `is_aarch64_feature_detected!` chained
//! calls, each documented in place. This module therefore only exports
//! the amenability heuristic, and the per-arch dispatch stays inside
//! each algorithm's SIMD sub-tree.
//!
//! # `#[cfg(feature = "simd")]`
//!
//! This module compiles only when the crate's `simd` feature is enabled.
//! Under `--no-default-features` and `--no-default-features --features
//! alloc` it is not compiled at all — the SIMD sub-trees themselves are
//! also gated on `simd`, so nothing references the helper in those
//! configurations.

/// Returns `true` iff the input pair is a good candidate for a byte-oriented
/// SIMD kernel with the given minimum-length threshold.
///
/// Both inputs are byte-oriented by construction (the caller is on the
/// `&[u8]` API entry point). The shorter side must be at least `min_len`
/// bytes long — below that threshold the per-call setup cost of a SIMD
/// backend (Peq-table build, per-symbol bitmap scan, etc.) dominates the
/// algorithmic win and the scalar path is faster.
///
/// The `min_len` argument is per-algorithm: Levenshtein's Myers backend
/// picks 32 (small Peq build, tight inner loop), Jaro's window-scan
/// backend picks a similar threshold because its per-symbol setup is
/// dominated by the same 256-entry bitmap build, and OSA reuses
/// Levenshtein's threshold because the two DPs share the same block
/// structure. See each SIMD module for the tuned constant it passes.
#[inline]
#[must_use]
pub fn is_byte_amenable(a: &[u8], b: &[u8], min_len: usize) -> bool {
    a.len().min(b.len()) >= min_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_pair() {
        assert!(!is_byte_amenable(b"", b"", 1));
        assert!(!is_byte_amenable(b"", b"anything", 1));
        assert!(!is_byte_amenable(b"anything", b"", 1));
    }

    #[test]
    fn accepts_exact_threshold() {
        let ten = &[0u8; 10];
        assert!(is_byte_amenable(ten, ten, 10));
        assert!(!is_byte_amenable(ten, ten, 11));
    }

    #[test]
    fn uses_shorter_side() {
        let short = &[0u8; 8];
        let long = &[0u8; 128];
        assert!(!is_byte_amenable(short, long, 16));
        assert!(is_byte_amenable(short, long, 8));
    }

    #[test]
    fn zero_threshold_accepts_anything() {
        assert!(is_byte_amenable(b"", b"", 0));
        assert!(is_byte_amenable(b"a", b"b", 0));
    }
}
