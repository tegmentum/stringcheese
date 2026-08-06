//! Differential fuzz target: full-matrix vs rolling-rows vs banded
//! Levenshtein.
//!
//! The three kernels compute the same recurrence through structurally
//! different code paths — the full matrix is an oracle-shaped textbook DP,
//! the rolling-rows kernel collapses the matrix into a single in-place row,
//! and the banded kernel adds a symmetric-band pruning window on top with
//! saturating-arithmetic sentinels for out-of-band cells. Bugs in the two
//! optimized kernels are unlikely to be shared with the oracle unless the
//! recurrence itself is wrong, which the canonical test vectors already
//! rule out.
//!
//! The banded kernel is invoked with a cutoff that is provably wide enough
//! to accept every possible answer for the capped input size (Levenshtein
//! distance between two `n`-byte inputs is at most `max(len_a, len_b)`, and
//! both are capped at `MAX_SIDE = 128`). Any `Exceeded` return therefore
//! signals a bug in the banded pruning, not a legitimate early-termination.

#![no_main]

use stringcheese_core::BoundedDistance;
use stringcheese_compare::levenshtein::{
    LevenshteinWorkspace, distance_banded_with_workspace, distance_full_matrix,
    distance_rolling_rows_with_workspace,
};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (a, b) = common::split2(data);

    // Oracle: the textbook full-matrix DP. All other kernels are checked
    // against this.
    let d_full = distance_full_matrix(a, b);

    // Production rolling-rows kernel.
    let mut ws = LevenshteinWorkspace::new();
    let d_rolling = distance_rolling_rows_with_workspace(a, b, &mut ws).into_inner();
    assert_eq!(
        d_full, d_rolling,
        "levenshtein rolling-rows disagreed with full-matrix oracle on ({a:?}, {b:?})"
    );

    // Banded kernel with a cutoff that must accept the true distance —
    // Levenshtein(a, b) <= max(|a|, |b|), and both sides are capped by the
    // splitter to `MAX_SIDE`. `MAX_SIDE * 2` gives a comfortable margin.
    let cutoff = u32::try_from(common::MAX_SIDE * 2).expect("MAX_SIDE fits in u32");
    let d_banded = distance_banded_with_workspace(a, b, cutoff, &mut ws);
    match d_banded {
        BoundedDistance::Within(d) => assert_eq!(
            d.into_inner(),
            d_full,
            "levenshtein banded disagreed with full-matrix oracle on ({a:?}, {b:?})",
        ),
        BoundedDistance::Exceeded { cutoff: c } => panic!(
            "levenshtein banded (cutoff={c}) claimed exceedance for true distance \
             {d_full} on ({a:?}, {b:?})",
        ),
    }
});
