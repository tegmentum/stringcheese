//! Shared input-splitting helpers for fuzz targets.
//!
//! libfuzzer hands the target a single `&[u8]`, but the algorithms we exercise
//! take two (or three) independent sequences. These helpers carve the input
//! into pieces without leaving structural bias — the first byte is consumed
//! as the split index so the fuzzer can learn to steer it.
//!
//! Every target caps its per-side input to keep an individual iteration
//! cheap: full-matrix Levenshtein is `O(m * n)` time and space, so 128 bytes
//! per side already touches 16k cells per call. libFuzzer expects millions
//! of iterations per second, and staying small is the difference between the
//! fuzzer exploring branches and the fuzzer waiting on DP tables.
//!
//! Included from each fuzz target with `#[path = "common.rs"] mod common;`
//! so cargo-fuzz does not try to build it as a target of its own (only
//! files registered as `[[bin]]` entries in Cargo.toml are targets).

#![allow(dead_code, reason = "not every fuzz target uses every helper here")]

/// Per-side input cap. See the module docs for the rationale.
///
/// 128 bytes keeps a full-matrix Levenshtein call at roughly `128 * 128 = 16k`
/// cells — small enough to run at high iteration rate but large enough to
/// exercise the interior of the DP recurrence (not just boundary rows).
pub const MAX_SIDE: usize = 128;

/// Splits `data` into two sub-slices using the first byte as the split index.
///
/// The convention: the first byte of `data` is consumed and used, modulo the
/// remaining length, as the length of the first side. Everything after that
/// belongs to the second side. Both sides are capped to [`MAX_SIDE`] bytes.
///
/// The empty and single-byte cases return two empty slices — libFuzzer will
/// quickly grow the input past the degenerate cases on its own.
pub fn split2(data: &[u8]) -> (&[u8], &[u8]) {
    if data.len() < 2 {
        return (&[], &[]);
    }
    let rest = &data[1..];
    let split = usize::from(data[0]) % (rest.len() + 1);
    let (a, b) = rest.split_at(split);
    (&a[..a.len().min(MAX_SIDE)], &b[..b.len().min(MAX_SIDE)])
}

/// Splits `data` into three sub-slices using the first two bytes as split
/// indices.
///
/// Same convention as [`split2`], repeated once for the middle boundary. The
/// three-way split feeds the metric-axiom targets (`x`, `y`, `z` for the
/// triangle inequality).
pub fn split3(data: &[u8]) -> (&[u8], &[u8], &[u8]) {
    if data.len() < 3 {
        return (&[], &[], &[]);
    }
    let rest = &data[2..];
    let s1 = usize::from(data[0]) % (rest.len() + 1);
    let (x, tail) = rest.split_at(s1);
    let s2 = usize::from(data[1]) % (tail.len() + 1);
    let (y, z) = tail.split_at(s2);
    (
        &x[..x.len().min(MAX_SIDE)],
        &y[..y.len().min(MAX_SIDE)],
        &z[..z.len().min(MAX_SIDE)],
    )
}

/// Splits `data` into two equal-length halves.
///
/// If `data.len()` is odd the trailing byte is dropped. Used by the Hamming
/// symmetry target, which requires equal-length inputs to exercise the
/// infallible entry points at all.
pub fn equal_halves(data: &[u8]) -> (&[u8], &[u8]) {
    let cap = 2 * MAX_SIDE;
    let data = &data[..data.len().min(cap)];
    let half = data.len() / 2;
    let a = &data[..half];
    let b = &data[half..2 * half];
    (a, b)
}
