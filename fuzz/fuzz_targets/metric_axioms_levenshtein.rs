//! Property fuzz target: metric axioms for Levenshtein.
//!
//! Unit-cost Levenshtein is declared as a true metric — every axiom in the
//! declaration must hold on every input:
//!
//! * **Identity.** `d(x, x) = 0`.
//! * **Symmetry.** `d(x, y) = d(y, x)`.
//! * **Non-negativity.** `d(x, y) >= 0` — enforced by the `u32` output type.
//! * **Triangle inequality.** `d(x, z) <= d(x, y) + d(y, z)`, for all `y`.
//!
//! The triangle inequality is what makes Levenshtein usable as a BK-tree
//! metric; a violation invalidates every index built on that assumption. It
//! is also the hardest axiom to bug-proof by inspection, which makes
//! generated inputs the right way to keep it honest.
//!
//! The rolling-rows kernel is used (rather than the full-matrix oracle) so
//! this target's iteration rate is not bottlenecked by the oracle's
//! `O(m * n)` space. A separate `levenshtein_differential` target already
//! keeps the two kernels in sync.

#![no_main]

use stringcheese_levenshtein::{LevenshteinWorkspace, distance_rolling_rows_with_workspace};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fn distance(a: &[u8], b: &[u8], ws: &mut LevenshteinWorkspace) -> u32 {
    distance_rolling_rows_with_workspace(a, b, ws).into_inner()
}

fuzz_target!(|data: &[u8]| {
    let (x, y, z) = common::split3(data);
    let mut ws = LevenshteinWorkspace::new();

    // Identity: d(x, x) == 0.
    assert_eq!(
        distance(x, x, &mut ws),
        0,
        "Levenshtein identity violated: d(x,x) != 0 on {x:?}",
    );

    // Symmetry: d(x, y) == d(y, x).
    let d_xy = distance(x, y, &mut ws);
    let d_yx = distance(y, x, &mut ws);
    assert_eq!(
        d_xy, d_yx,
        "Levenshtein symmetry violated: d(x,y)={d_xy}, d(y,x)={d_yx} on ({x:?}, {y:?})",
    );

    // Triangle inequality: d(x, z) <= d(x, y) + d(y, z).
    let d_xz = distance(x, z, &mut ws);
    let d_yz = distance(y, z, &mut ws);
    // Use saturating_add to avoid a spurious panic if the sum ever overflows
    // u32 — for splitter-capped inputs the true sum is at most `2 * MAX_SIDE`,
    // but saturating stays safe under any future cap change.
    let bound = d_xy.saturating_add(d_yz);
    assert!(
        d_xz <= bound,
        "Levenshtein triangle inequality violated: \
         d(x,z)={d_xz} > d(x,y)+d(y,z)={d_xy}+{d_yz}={bound} on ({x:?}, {y:?}, {z:?})",
    );
});
