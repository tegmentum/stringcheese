//! Randomised property tests for the alignment kernels.
//!
//! The properties fall into three groups:
//!
//! 1. **Score-shape invariants** — non-negativity of Smith-Waterman,
//!    symmetry of Needleman-Wunsch under symmetric scoring, the affine ==
//!    linear equivalence when `gap_open == gap_extend`.
//! 2. **Score arithmetic** — appending a matching symbol to both inputs
//!    increases NW's score by exactly `match_score` when the scheme
//!    satisfies `match_score >= 2 * gap_penalty`.
//! 3. **Edit-script correctness** — the reconstructed script, applied to
//!    `a`, reproduces `b`. This is the critical property that separates a
//!    correct backtrace from one that accidentally agrees on the total
//!    score but scrambles the operations.
//!
//! Strategies use a three-symbol alphabet so matches are common enough to
//! exercise runs of aligned pairs; longer alphabets would leave the DP
//! stuck in mismatch-dominated regimes and starve the interesting cases.

use alloc::vec::Vec;
use proptest::prelude::*;

use crate::edit_script::{Alignment, EditOp};
use crate::needleman_wunsch::NeedlemanWunsch;
use crate::scoring::{AffineGap, LinearGap, ScoringScheme};
use crate::smith_waterman::SmithWaterman;

/// Whether the previous script op was part of a run of Insert or Delete
/// operations. Used by [`script_score`] to know whether to charge
/// `gap_open` (transition into a gap) or `gap_extend` (continuation).
enum GapKind {
    /// The previous op was not part of a gap run.
    None,
    /// The previous op was `Insert`.
    InsertRun,
    /// The previous op was `Delete`.
    DeleteRun,
}

fn arb_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..20)
}

fn arb_short_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..10)
}

/// Independently apply the script to `a` and verify it reconstructs `b`.
fn script_reconstructs(al: &Alignment<u8>, a: &[u8], b: &[u8]) -> bool {
    // For a global alignment, extract_a() == a and extract_b() == b.
    // For a local alignment, extract_a() == a[a_start..a_end] and
    // extract_b() == b[b_start..b_end].
    let a_end = al.a_end();
    let b_end = al.b_end();
    if a_end > a.len() || b_end > b.len() {
        return false;
    }
    al.extract_a() == a[al.a_start..a_end] && al.extract_b() == b[al.b_start..b_end]
}

/// Recompute the alignment score from the script under a scheme, walking
/// runs of consecutive Insert/Delete ops so open/extend transitions are
/// charged correctly.
fn script_score<S: ScoringScheme>(scheme: &S, al: &Alignment<u8>) -> i32 {
    let mut score = 0i32;
    // Track the previous op so we know whether to charge open or extend
    // for the current gap symbol.
    let mut prev = GapKind::None;

    for op in &al.script {
        match op {
            EditOp::Match { .. } => {
                score += scheme.match_score();
                prev = GapKind::None;
            }
            EditOp::Substitute { .. } => {
                score += scheme.mismatch_score();
                prev = GapKind::None;
            }
            EditOp::Delete { .. } => {
                score += match prev {
                    GapKind::DeleteRun => scheme.gap_extend(),
                    _ => scheme.gap_open(),
                };
                prev = GapKind::DeleteRun;
            }
            EditOp::Insert { .. } => {
                score += match prev {
                    GapKind::InsertRun => scheme.gap_extend(),
                    _ => scheme.gap_open(),
                };
                prev = GapKind::InsertRun;
            }
        }
    }
    score
}

proptest! {
    // ---- Needleman-Wunsch -------------------------------------------------

    /// The score of any two sequences under a symmetric scheme is invariant
    /// under swapping the arguments.
    #[test]
    fn nw_linear_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        prop_assert_eq!(nw.score(&a, &b), nw.score(&b, &a));
    }

    #[test]
    fn nw_affine_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        let nw = NeedlemanWunsch::new(AffineGap::default_affine());
        prop_assert_eq!(nw.score(&a, &b), nw.score(&b, &a));
    }

    /// Identical inputs score `len * match_score` (linear only — with
    /// affine there is nothing to open).
    #[test]
    fn nw_identical_scores_length_times_match(xs in arb_bytes()) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        // match_score = 1, so score is just the length.
        let expected = i32::try_from(xs.len()).unwrap();
        prop_assert_eq!(nw.score(&xs, &xs).into_inner(), expected);
    }

    /// Appending a match to both inputs raises the NW linear score by
    /// exactly `match_score` when the scheme satisfies
    /// `match_score >= 2 * gap_penalty` (holds for LinearGap::simple).
    #[test]
    fn nw_append_match_increases_score_by_match(
        a in arb_bytes(),
        b in arb_bytes(),
        c in 0u8..3,
    ) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let base = nw.score(&a, &b).into_inner();
        let mut a2 = a.clone();
        a2.push(c);
        let mut b2 = b.clone();
        b2.push(c);
        let after = nw.score(&a2, &b2).into_inner();
        prop_assert_eq!(after, base + 1);
    }

    /// A `LinearGap` scheme and an `AffineGap` scheme with matching
    /// match/mismatch/gap and `open == extend` produce identical scores.
    #[test]
    fn linear_gap_matches_affine_with_equal_open_and_extend(
        a in arb_bytes(),
        b in arb_bytes(),
    ) {
        let linear = LinearGap { match_score: 2, mismatch_score: -1, gap_penalty: -1 };
        let affine = AffineGap {
            match_score: 2, mismatch_score: -1, gap_open: -1, gap_extend: -1,
        };
        let nw_l = NeedlemanWunsch::new(linear);
        let nw_a = NeedlemanWunsch::new(affine);
        prop_assert_eq!(nw_l.score(&a, &b), nw_a.score(&a, &b));
    }

    /// `align` and `score` agree on the score.
    #[test]
    fn nw_score_and_align_agree(a in arb_short_bytes(), b in arb_short_bytes()) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        prop_assert_eq!(nw.score(&a, &b).into_inner(), nw.align(&a, &b).score);
    }

    /// The edit script produced by NW (global) reconstructs both inputs
    /// exactly.
    #[test]
    fn nw_linear_edit_script_reconstructs_inputs(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let al = nw.align(&a, &b);
        prop_assert_eq!(al.a_start, 0);
        prop_assert_eq!(al.b_start, 0);
        prop_assert!(script_reconstructs(&al, &a, &b));
    }

    #[test]
    fn nw_affine_edit_script_reconstructs_inputs(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let nw = NeedlemanWunsch::new(AffineGap::default_affine());
        let al = nw.align(&a, &b);
        prop_assert_eq!(al.a_start, 0);
        prop_assert_eq!(al.b_start, 0);
        prop_assert!(script_reconstructs(&al, &a, &b));
    }

    /// The recomputed script score, walked with open/extend semantics,
    /// equals the reported alignment score.
    #[test]
    fn nw_linear_script_score_matches_reported(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let scheme = LinearGap::simple();
        let nw = NeedlemanWunsch::new(scheme);
        let al = nw.align(&a, &b);
        prop_assert_eq!(script_score(&scheme, &al), al.score);
    }

    #[test]
    fn nw_affine_script_score_matches_reported(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let scheme = AffineGap::default_affine();
        let nw = NeedlemanWunsch::new(scheme);
        let al = nw.align(&a, &b);
        prop_assert_eq!(script_score(&scheme, &al), al.score);
    }

    // ---- Smith-Waterman ---------------------------------------------------

    /// SW score is always non-negative — the empty local alignment (0) is
    /// always available.
    #[test]
    fn sw_linear_score_is_non_negative(a in arb_bytes(), b in arb_bytes()) {
        let sw = SmithWaterman::new(LinearGap::simple());
        prop_assert!(sw.score(&a, &b).into_inner() >= 0);
    }

    #[test]
    fn sw_affine_score_is_non_negative(a in arb_bytes(), b in arb_bytes()) {
        let sw = SmithWaterman::new(AffineGap::default_affine());
        prop_assert!(sw.score(&a, &b).into_inner() >= 0);
    }

    /// SW is also symmetric under a symmetric scheme.
    #[test]
    fn sw_linear_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        let sw = SmithWaterman::new(LinearGap::simple());
        prop_assert_eq!(sw.score(&a, &b), sw.score(&b, &a));
    }

    #[test]
    fn sw_score_and_align_agree(a in arb_short_bytes(), b in arb_short_bytes()) {
        let sw = SmithWaterman::new(LinearGap::simple());
        prop_assert_eq!(sw.score(&a, &b).into_inner(), sw.align(&a, &b).score);
    }

    /// The local edit script's extract_a/extract_b match the substrings
    /// indexed by a_start/b_start.
    #[test]
    fn sw_linear_edit_script_reconstructs_substrings(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let sw = SmithWaterman::new(LinearGap::simple());
        let al = sw.align(&a, &b);
        prop_assert!(script_reconstructs(&al, &a, &b));
    }

    #[test]
    fn sw_affine_edit_script_reconstructs_substrings(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let sw = SmithWaterman::new(AffineGap::default_affine());
        let al = sw.align(&a, &b);
        prop_assert!(script_reconstructs(&al, &a, &b));
    }

    /// The recomputed script score equals the reported SW score.
    #[test]
    fn sw_linear_script_score_matches_reported(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let scheme = LinearGap::simple();
        let sw = SmithWaterman::new(scheme);
        let al = sw.align(&a, &b);
        prop_assert_eq!(script_score(&scheme, &al), al.score);
    }

    /// SW score >= NW score whenever NW is positive. If NW is negative or
    /// zero, SW is at least 0 (already covered by non-negativity), so we
    /// only assert the stronger clause conditionally.
    #[test]
    fn sw_score_dominates_positive_nw(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
    ) {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let sw = SmithWaterman::new(LinearGap::simple());
        let nw_s = nw.score(&a, &b).into_inner();
        let sw_s = sw.score(&a, &b).into_inner();
        prop_assert!(sw_s >= nw_s.max(0));
    }
}

// ---- One plain #[test] outside the proptest macro ---------------------------
//
// The empty/empty case is easy to get wrong (off-by-one in a rolling-row
// setup, or an early-return that reports the wrong sentinel). Assert it
// once, cheaply, without the harness overhead.

#[test]
fn nw_empty_pair_scores_zero() {
    let nw = NeedlemanWunsch::new(LinearGap::simple());
    assert_eq!(nw.score::<u8>(b"", b"").into_inner(), 0);
}

#[test]
fn sw_empty_pair_scores_zero() {
    let sw = SmithWaterman::new(LinearGap::simple());
    assert_eq!(sw.score::<u8>(b"", b"").into_inner(), 0);
}
