//! Differential fuzz target: full-matrix (oracle) vs production kernel for
//! full unrestricted Damerau-Levenshtein.
//!
//! The two kernels compute the same Lowrance-Wagner recurrence but locate
//! the transposition source through structurally independent code paths —
//! the oracle scans backward through the input; the production kernel
//! consults a `HashMap<&T, usize>` maintained across the outer loop.
//! Agreement across arbitrary inputs is much stronger evidence of the
//! recurrence's correctness than any single kernel being fast.

#![no_main]

use stringcheese_damerau::DamerauWorkspace;
use stringcheese_damerau::damerau::{distance_full_matrix, distance_production_with_workspace};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (a, b) = common::split2(data);

    let d_full = distance_full_matrix(a, b);

    let mut ws = DamerauWorkspace::new();
    let d_prod = distance_production_with_workspace(a, b, &mut ws).into_inner();

    assert_eq!(
        d_full, d_prod,
        "Damerau production kernel disagreed with full-matrix oracle on ({a:?}, {b:?})",
    );
});
