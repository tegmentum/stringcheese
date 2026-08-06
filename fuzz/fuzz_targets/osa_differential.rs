//! Differential fuzz target: full-matrix vs rolling-rows vs banded OSA
//! (Optimal String Alignment, "restricted Damerau-Levenshtein").
//!
//! Same shape as the Levenshtein differential target, applied to OSA's three
//! kernels. The transposition branch reaches two rows back, so the rolling
//! implementation keeps three rows in scratch and the banded implementation
//! must ensure its pruning window admits the transposition source. A bug in
//! either of those additions is a bug the oracle's textbook DP would not
//! share.

#![no_main]

use stringcheese_core::BoundedDistance;
use stringcheese_compare::damerau::OsaWorkspace;
use stringcheese_compare::damerau::osa::{
    distance_banded_with_workspace, distance_full_matrix, distance_rolling_rows_with_workspace,
};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (a, b) = common::split2(data);

    // Oracle.
    let d_full = distance_full_matrix(a, b);

    let mut ws = OsaWorkspace::new();
    let d_rolling = distance_rolling_rows_with_workspace(a, b, &mut ws).into_inner();
    assert_eq!(
        d_full, d_rolling,
        "OSA rolling-rows disagreed with full-matrix oracle on ({a:?}, {b:?})"
    );

    // OSA(a, b) <= max(|a|, |b|) under unit costs. A cutoff of MAX_SIDE * 2
    // therefore always accepts the true distance for splitter-capped inputs.
    let cutoff = u32::try_from(common::MAX_SIDE * 2).expect("MAX_SIDE fits in u32");
    let d_banded = distance_banded_with_workspace(a, b, cutoff, &mut ws);
    match d_banded {
        BoundedDistance::Within(d) => assert_eq!(
            d.into_inner(),
            d_full,
            "OSA banded disagreed with full-matrix oracle on ({a:?}, {b:?})",
        ),
        BoundedDistance::Exceeded { cutoff: c } => panic!(
            "OSA banded (cutoff={c}) claimed exceedance for true distance \
             {d_full} on ({a:?}, {b:?})",
        ),
    }
});
