//! Fuzz target: alignment score-only vs. score-with-traceback agreement.
//!
//! An alignment kernel is really two paths through the same DP: a
//! `score_only` fast path that keeps a rolling row and reports the
//! final cell value, and an `align` slow path that retains the full
//! matrix, backtraces from the corner, and re-emits both a score and
//! an edit script. A defect in either path — off-by-one on the row
//! rebase, a gap direction flipped in the backtrace, a boundary cell
//! initialized with the wrong sign — would silently produce two
//! different scores for the same `(a, b, scheme)` triple. That is the
//! exact class of bug this target is designed to surface.
//!
//! # Property
//!
//! For arbitrary short byte strings `a` and `b` under the textbook
//! linear-gap scheme (`LinearGap::simple()`), all three of the
//! following agree, both for Needleman-Wunsch and Smith-Waterman:
//!
//! 1. The score reported by `score(a, b)` (rolling-row DP).
//! 2. The `Alignment::score` field reported by `align(a, b)` (full
//!    matrix + backtrace).
//! 3. The score recomputed from the returned edit script — one
//!    `match_score` per [`EditOp::Match`], one `mismatch_score` per
//!    [`EditOp::Substitute`], one `gap_penalty` per [`EditOp::Insert`]
//!    or [`EditOp::Delete`].
//!
//! Property 3 is the strong invariant: it asserts that the reported
//! score is a genuine sum of the reported traceback, not merely two
//! independent DP results that happen to converge on the same
//! integer.
//!
//! # Input
//!
//! The libFuzzer byte stream feeds an [`arbitrary::Unstructured`]
//! reader which derives two short byte strings, each bounded to
//! `[0, 64]` bytes via [`Unstructured::int_in_range`]. The bounds
//! keep every fuzz iteration well under the libFuzzer per-input
//! time budget (a 64×64 DP fill is ~4k cell updates, dwarfed by
//! libFuzzer's per-input overhead).
//!
//! # Scope
//!
//! Only the linear-gap scheme is checked here. The affine-gap DP
//! carries its own three-matrix layout and a separate traceback with
//! Gotoh's layer state; a dedicated affine target is the natural
//! follow-up. See the crate-level `nw_score_affine` /
//! `nw_align_linear` bench groups for the parallel API entry points.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use stringcheese_align::{EditOp, LinearGap, NeedlemanWunsch, SmithWaterman};

/// Maximum byte length of each generated input string. 64 is comfortably
/// larger than any hand-crafted seed and keeps each fuzz iteration's DP
/// fill under 5k cell updates.
const MAX_INPUT_LEN: usize = 64;

/// Two short byte strings; both required by the aligner call and
/// bounded through `Unstructured::int_in_range`. The derive would
/// default `Vec::arbitrary` to unbounded length, which libFuzzer
/// would grow into MB-sized blobs; hand-implementing `Arbitrary`
/// keeps each iteration cheap.
#[derive(Debug, Clone)]
struct FuzzPair {
    a: Vec<u8>,
    b: Vec<u8>,
}

impl<'a> Arbitrary<'a> for FuzzPair {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let a_len = u.int_in_range(0..=MAX_INPUT_LEN)?;
        let mut a = Vec::with_capacity(a_len);
        for _ in 0..a_len {
            a.push(u.arbitrary()?);
        }
        let b_len = u.int_in_range(0..=MAX_INPUT_LEN)?;
        let mut b = Vec::with_capacity(b_len);
        for _ in 0..b_len {
            b.push(u.arbitrary()?);
        }
        Ok(Self { a, b })
    }
}

/// Recompute the score of an edit script under a linear-gap scheme.
///
/// This is the third leg of the three-way agreement check: rather
/// than comparing two DP results to each other, it walks the reported
/// traceback and re-derives what the score *must* be if every op is
/// scored under the same scheme the DP used. Any disagreement between
/// this sum and the reported `Alignment::score` proves the traceback
/// and the score are out of sync.
fn score_from_script(scheme: LinearGap, script: &[EditOp<u8>]) -> i32 {
    let mut sum: i32 = 0;
    for op in script {
        match op {
            EditOp::Match { .. } => sum += scheme.match_score,
            EditOp::Substitute { .. } => sum += scheme.mismatch_score,
            EditOp::Insert { .. } | EditOp::Delete { .. } => sum += scheme.gap_penalty,
        }
    }
    sum
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(pair) = FuzzPair::arbitrary(&mut u) else {
        // Not enough bytes to satisfy the bounded decoder — benign.
        return;
    };
    let a = pair.a.as_slice();
    let b = pair.b.as_slice();

    let scheme = LinearGap::simple();

    // Needleman-Wunsch (global). All three legs must agree.
    let nw = NeedlemanWunsch::new(scheme);
    let nw_score_only = nw.score(a, b).into_inner();
    let nw_alignment = nw.align(a, b);
    let nw_from_script = score_from_script(scheme, &nw_alignment.script);
    assert_eq!(
        nw_score_only, nw_alignment.score,
        "NW score-only vs. align().score disagree on ({a:?}, {b:?})"
    );
    assert_eq!(
        nw_alignment.score, nw_from_script,
        "NW align().score vs. sum-over-script disagree on ({a:?}, {b:?})"
    );

    // Smith-Waterman (local). The reconstructed script covers only
    // the aligned substring, but the same score-vs-sum invariant
    // holds: the reported score is a genuine sum of the reported ops.
    let sw = SmithWaterman::new(scheme);
    let sw_score_only = sw.score(a, b).into_inner();
    let sw_alignment = sw.align(a, b);
    let sw_from_script = score_from_script(scheme, &sw_alignment.script);
    assert_eq!(
        sw_score_only, sw_alignment.score,
        "SW score-only vs. align().score disagree on ({a:?}, {b:?})"
    );
    assert_eq!(
        sw_alignment.score, sw_from_script,
        "SW align().score vs. sum-over-script disagree on ({a:?}, {b:?})"
    );
});
