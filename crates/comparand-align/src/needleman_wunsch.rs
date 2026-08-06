//! Needleman-Wunsch global sequence alignment.
//!
//! Computes the maximum-score alignment of two sequences end-to-end. Every
//! symbol in both inputs appears in the resulting alignment, either paired
//! with a symbol from the other input or aligned to a gap.
//!
//! # Algorithm
//!
//! * When the scoring scheme reports `gap_open == gap_extend`, the algorithm
//!   uses the original single-matrix DP of Needleman and Wunsch (1970).
//! * When the two gap costs differ, the algorithm switches to a three-
//!   matrix (`M`, `X`, `Y`) DP due to Gotoh (1982). The three matrices
//!   track, respectively, alignments ending in a match, alignments ending
//!   in a gap in `b`, and alignments ending in a gap in `a`. Amortizing
//!   the higher opening cost across multi-symbol gaps requires this
//!   separation.
//!
//! # Complexity
//!
//! Score-only: `O(m * n)` time, `O(n)` space (rolling row).
//! With edit-script backtrace: `O(m * n)` time, `O(m * n)` space (full
//! matrix / matrices for the trace).

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass, Score,
    VariantId,
};

use crate::edit_script::{Alignment, EditOp};
use crate::scoring::{ScoringScheme, is_affine};
use crate::workspace::AlignmentWorkspace;

/// Sentinel used in place of negative infinity in DP cells that are
/// unreachable under a given transition rule.
///
/// `i32::MIN / 4` leaves ample headroom for repeated `+ gap_open` /
/// `+ gap_extend` additions without underflowing.
const NEG_INF: i32 = i32::MIN / 4;

/// The Needleman-Wunsch global-alignment algorithm handle.
///
/// Parameterized by any [`ScoringScheme`]. When the scheme is affine
/// (i.e. `gap_open != gap_extend`) the handle uses the three-matrix Gotoh
/// DP; otherwise it uses the simpler single-matrix DP. Both branches share
/// the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NeedlemanWunsch<S: ScoringScheme> {
    /// The scoring scheme governing match / mismatch / gap costs.
    pub scoring: S,
}

impl<S: ScoringScheme> NeedlemanWunsch<S> {
    /// Descriptor for the linear-gap variant.
    ///
    /// Returned by [`Self::descriptor`] whenever
    /// `scoring.gap_open() == scoring.gap_extend()`. The variant slug
    /// `"linear-gap-generic-eq"` names both the DP path taken and the
    /// generic-`T: Eq` interface.
    pub const LINEAR_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::NeedlemanWunsch,
        variant: VariantId("linear-gap-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "A general method applicable to the search for similarities in the amino acid sequence of two proteins",
            authors: "S. B. Needleman, C. D. Wunsch",
            year: 1970,
        },
    };

    /// Descriptor for the affine-gap variant.
    ///
    /// Returned by [`Self::descriptor`] whenever
    /// `scoring.gap_open() != scoring.gap_extend()`. Attribution points to
    /// Gotoh's 1982 improvement even though the containing family is
    /// still [`AlgorithmFamily::NeedlemanWunsch`] — the affine variant is
    /// the same problem under a richer cost model.
    pub const AFFINE_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::NeedlemanWunsch,
        variant: VariantId("affine-gap-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "An improved algorithm for matching biological sequences",
            authors: "O. Gotoh",
            year: 1982,
        },
    };

    /// Construct a Needleman-Wunsch aligner with the given scoring scheme.
    #[must_use]
    pub const fn new(scoring: S) -> Self {
        Self { scoring }
    }

    /// Return the descriptor corresponding to this configuration.
    ///
    /// Runtime dispatch on `scoring.gap_open() != scoring.gap_extend()`
    /// selects between [`Self::LINEAR_DESCRIPTOR`] and
    /// [`Self::AFFINE_DESCRIPTOR`].
    #[must_use]
    pub fn descriptor(&self) -> AlgorithmDescriptor {
        if is_affine(&self.scoring) {
            Self::AFFINE_DESCRIPTOR
        } else {
            Self::LINEAR_DESCRIPTOR
        }
    }

    /// Metric class this algorithm belongs to.
    ///
    /// Always [`MetricClass::Score`] — alignment scores do not satisfy the
    /// metric axioms.
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Score
    }

    /// Compute the alignment score of `a` and `b`.
    ///
    /// Allocates a fresh workspace; for repeated calls, prefer
    /// [`Self::score_with_workspace`].
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    #[must_use]
    pub fn score<T: Eq>(&self, a: &[T], b: &[T]) -> Score<i32> {
        let mut ws = AlignmentWorkspace::new();
        self.score_with_workspace(a, b, &mut ws)
    }

    /// Compute the alignment score, reusing a caller-supplied workspace.
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    pub fn score_with_workspace<T: Eq>(
        &self,
        a: &[T],
        b: &[T],
        ws: &mut AlignmentWorkspace,
    ) -> Score<i32> {
        if is_affine(&self.scoring) {
            score_affine(&self.scoring, a, b, ws)
        } else {
            score_linear(&self.scoring, a, b, ws)
        }
    }

    /// Compute the alignment score together with the reconstructed edit
    /// script.
    ///
    /// Uses the Full-mode DP (one matrix for linear-gap; three for affine)
    /// and backtraces from `(m, n)` to `(0, 0)`.
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    #[must_use]
    pub fn align<T: Eq + Clone>(&self, a: &[T], b: &[T]) -> Alignment<T> {
        let mut ws = AlignmentWorkspace::new();
        self.align_with_workspace(a, b, &mut ws)
    }

    /// Compute the alignment and reconstructed edit script, reusing a
    /// caller-supplied workspace.
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    pub fn align_with_workspace<T: Eq + Clone>(
        &self,
        a: &[T],
        b: &[T],
        ws: &mut AlignmentWorkspace,
    ) -> Alignment<T> {
        if is_affine(&self.scoring) {
            align_affine(&self.scoring, a, b, ws)
        } else {
            align_linear(&self.scoring, a, b, ws)
        }
    }
}

// ---------------------------------------------------------------------------
// Linear-gap score-only kernel (rolling row).
// ---------------------------------------------------------------------------

#[allow(
    clippy::many_single_char_names,
    reason = "i, j, m, n, s, gap are the standard names for this textbook DP"
)]
fn score_linear<S: ScoringScheme, T: Eq>(
    scheme: &S,
    a: &[T],
    b: &[T],
    ws: &mut AlignmentWorkspace,
) -> Score<i32> {
    let m = a.len();
    let n = b.len();
    let gap = scheme.gap_open();

    let row = ws.score_buffer(n + 1);

    for (j, cell) in row.iter_mut().enumerate() {
        *cell = to_i32(j) * gap;
    }

    for i in 1..=m {
        let mut diag = row[0];
        row[0] = to_i32(i) * gap;
        for j in 1..=n {
            let up = row[j];
            let left = row[j - 1];
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let new_val = (diag + s).max(up + gap).max(left + gap);
            diag = up;
            row[j] = new_val;
        }
    }

    Score::new(row[n])
}

// ---------------------------------------------------------------------------
// Linear-gap align kernel (full matrix + backtrace).
// ---------------------------------------------------------------------------

#[allow(
    clippy::many_single_char_names,
    reason = "i, j, m, n, s, gap are the standard names for this textbook DP"
)]
fn align_linear<S, T>(scheme: &S, a: &[T], b: &[T], ws: &mut AlignmentWorkspace) -> Alignment<T>
where
    S: ScoringScheme,
    T: Eq + Clone,
{
    let m = a.len();
    let n = b.len();
    let gap = scheme.gap_open();
    let stride = n + 1;
    let cells = (m + 1) * stride;

    let mat = ws.m_matrix(cells);
    let idx = |i: usize, j: usize| i * stride + j;

    mat[0] = 0;
    for i in 1..=m {
        mat[idx(i, 0)] = to_i32(i) * gap;
    }
    for j in 1..=n {
        mat[idx(0, j)] = to_i32(j) * gap;
    }

    for i in 1..=m {
        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let diag = mat[idx(i - 1, j - 1)] + s;
            let up = mat[idx(i - 1, j)] + gap;
            let left = mat[idx(i, j - 1)] + gap;
            mat[idx(i, j)] = diag.max(up).max(left);
        }
    }

    let score = mat[idx(m, n)];

    let mut script: Vec<EditOp<T>> = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        let cur = mat[idx(i, j)];
        // Try diagonal (match/substitute) first when both indices are
        // positive: preferring diagonal keeps runs of aligned symbols
        // contiguous rather than splitting them across gap paths that also
        // reach the same cell value.
        if i > 0 && j > 0 {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            if cur == mat[idx(i - 1, j - 1)] + s {
                script.push(if a[i - 1] == b[j - 1] {
                    EditOp::Match {
                        a: a[i - 1].clone(),
                        b: b[j - 1].clone(),
                    }
                } else {
                    EditOp::Substitute {
                        a: a[i - 1].clone(),
                        b: b[j - 1].clone(),
                    }
                });
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && cur == mat[idx(i - 1, j)] + gap {
            script.push(EditOp::Delete {
                a: a[i - 1].clone(),
            });
            i -= 1;
            continue;
        }
        // Remaining option: gap in `a` (insert from `b`).
        debug_assert!(j > 0);
        script.push(EditOp::Insert {
            b: b[j - 1].clone(),
        });
        j -= 1;
    }

    script.reverse();

    Alignment {
        score,
        script,
        a_start: 0,
        b_start: 0,
    }
}

// ---------------------------------------------------------------------------
// Affine-gap score-only kernel (rolling row over M, X, Y).
// ---------------------------------------------------------------------------

#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "Standard names + a single fused body match the published Gotoh 1982 derivation"
)]
fn score_affine<S: ScoringScheme, T: Eq>(
    scheme: &S,
    a: &[T],
    b: &[T],
    ws: &mut AlignmentWorkspace,
) -> Score<i32> {
    let m = a.len();
    let n = b.len();
    let open = scheme.gap_open();
    let extend = scheme.gap_extend();
    let stride = n + 1;

    let buf = ws.score_buffer(6 * stride);
    let (prev, curr) = buf.split_at_mut(3 * stride);
    let (prev_m, prev_rest) = prev.split_at_mut(stride);
    let (prev_x, prev_y) = prev_rest.split_at_mut(stride);
    let (curr_m, curr_rest) = curr.split_at_mut(stride);
    let (curr_x, curr_y) = curr_rest.split_at_mut(stride);

    // Row 0 (prev).
    prev_m[0] = 0;
    prev_x[0] = NEG_INF;
    prev_y[0] = NEG_INF;
    for j in 1..=n {
        prev_m[j] = NEG_INF;
        prev_x[j] = NEG_INF;
        prev_y[j] = open + to_i32(j - 1) * extend;
    }

    for i in 1..=m {
        curr_m[0] = NEG_INF;
        curr_x[0] = open + to_i32(i - 1) * extend;
        curr_y[0] = NEG_INF;

        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let best_diag = prev_m[j - 1].max(prev_x[j - 1]).max(prev_y[j - 1]);
            curr_m[j] = best_diag + s;
            curr_x[j] = (prev_m[j] + open)
                .max(prev_x[j] + extend)
                .max(prev_y[j] + open);
            curr_y[j] = (curr_m[j - 1] + open)
                .max(curr_x[j - 1] + open)
                .max(curr_y[j - 1] + extend);
        }

        prev_m.copy_from_slice(curr_m);
        prev_x.copy_from_slice(curr_x);
        prev_y.copy_from_slice(curr_y);
    }

    let final_score = prev_m[n].max(prev_x[n]).max(prev_y[n]);
    Score::new(final_score)
}

// ---------------------------------------------------------------------------
// Affine-gap align kernel (three full matrices + Gotoh backtrace).
// ---------------------------------------------------------------------------

/// The layer of the Gotoh three-matrix DP the backtrace is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// `M` — match / substitute cell.
    M,
    /// `X` — gap in `b` (deletion from `a`).
    X,
    /// `Y` — gap in `a` (insertion from `b`).
    Y,
}

#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "Standard names + a single fused body match the published Gotoh 1982 derivation"
)]
fn align_affine<S, T>(scheme: &S, a: &[T], b: &[T], ws: &mut AlignmentWorkspace) -> Alignment<T>
where
    S: ScoringScheme,
    T: Eq + Clone,
{
    let m = a.len();
    let n = b.len();
    let open = scheme.gap_open();
    let extend = scheme.gap_extend();
    let stride = n + 1;
    let cells = (m + 1) * stride;

    let (mat_m, mat_x, mat_y) = ws.full_matrices(cells);
    let idx = |i: usize, j: usize| i * stride + j;

    // Initialize.
    mat_m[0] = 0;
    mat_x[0] = NEG_INF;
    mat_y[0] = NEG_INF;
    for i in 1..=m {
        mat_m[idx(i, 0)] = NEG_INF;
        mat_x[idx(i, 0)] = open + to_i32(i - 1) * extend;
        mat_y[idx(i, 0)] = NEG_INF;
    }
    for j in 1..=n {
        mat_m[idx(0, j)] = NEG_INF;
        mat_x[idx(0, j)] = NEG_INF;
        mat_y[idx(0, j)] = open + to_i32(j - 1) * extend;
    }

    for i in 1..=m {
        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let ij = idx(i, j);

            let dm = mat_m[idx(i - 1, j - 1)];
            let dx = mat_x[idx(i - 1, j - 1)];
            let dy = mat_y[idx(i - 1, j - 1)];
            mat_m[ij] = dm.max(dx).max(dy) + s;

            let um = mat_m[idx(i - 1, j)];
            let ux = mat_x[idx(i - 1, j)];
            let uy = mat_y[idx(i - 1, j)];
            mat_x[ij] = (um + open).max(ux + extend).max(uy + open);

            let lm = mat_m[idx(i, j - 1)];
            let lx = mat_x[idx(i, j - 1)];
            let ly = mat_y[idx(i, j - 1)];
            mat_y[ij] = (lm + open).max(ly + extend).max(lx + open);
        }
    }

    let final_m = mat_m[idx(m, n)];
    let final_x = mat_x[idx(m, n)];
    let final_y = mat_y[idx(m, n)];
    let score = final_m.max(final_x).max(final_y);

    // Determine starting layer at (m, n).
    let mut layer = pick_layer(final_m, final_x, final_y);

    let mut script: Vec<EditOp<T>> = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        // Boundary snap: if only one dimension remains, only one layer is
        // reachable. This lets us keep the loop shape uniform even at the
        // top row / leftmost column.
        if i == 0 {
            layer = Layer::Y;
        } else if j == 0 {
            layer = Layer::X;
        }

        match layer {
            Layer::M => {
                debug_assert!(i > 0 && j > 0);
                let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
                let dm = mat_m[idx(i - 1, j - 1)];
                let dx = mat_x[idx(i - 1, j - 1)];
                let dy = mat_y[idx(i - 1, j - 1)];
                debug_assert_eq!(mat_m[idx(i, j)], dm.max(dx).max(dy) + s);
                let _ = s;

                script.push(if a[i - 1] == b[j - 1] {
                    EditOp::Match {
                        a: a[i - 1].clone(),
                        b: b[j - 1].clone(),
                    }
                } else {
                    EditOp::Substitute {
                        a: a[i - 1].clone(),
                        b: b[j - 1].clone(),
                    }
                });
                layer = pick_layer(dm, dx, dy);
                i -= 1;
                j -= 1;
            }
            Layer::X => {
                debug_assert!(i > 0);
                script.push(EditOp::Delete {
                    a: a[i - 1].clone(),
                });
                let um = mat_m[idx(i - 1, j)] + open;
                let ux = mat_x[idx(i - 1, j)] + extend;
                let uy = mat_y[idx(i - 1, j)] + open;
                layer = pick_layer(um, ux, uy);
                i -= 1;
            }
            Layer::Y => {
                debug_assert!(j > 0);
                script.push(EditOp::Insert {
                    b: b[j - 1].clone(),
                });
                let lm = mat_m[idx(i, j - 1)] + open;
                let lx = mat_x[idx(i, j - 1)] + open;
                let ly = mat_y[idx(i, j - 1)] + extend;
                layer = pick_layer(lm, lx, ly);
                j -= 1;
            }
        }
    }

    script.reverse();

    Alignment {
        score,
        script,
        a_start: 0,
        b_start: 0,
    }
}

/// Return the layer with the highest score, preferring `M` over `X` over `Y`
/// on ties. Ties do not affect the score; they only affect which of several
/// co-optimal edit scripts is reported.
#[inline]
fn pick_layer(m: i32, x: i32, y: i32) -> Layer {
    if m >= x && m >= y {
        Layer::M
    } else if x >= y {
        Layer::X
    } else {
        Layer::Y
    }
}

/// Convert a non-negative `usize` index into an `i32`, panicking if it does
/// not fit. Alignment operations do not target sequences long enough for
/// this to fire in practice.
#[inline]
fn to_i32(n: usize) -> i32 {
    i32::try_from(n).expect("alignment input longer than i32::MAX symbols")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::{AffineGap, LinearGap};

    #[test]
    fn descriptor_switches_on_scheme_shape() {
        let nw_linear = NeedlemanWunsch::new(LinearGap::simple());
        assert_eq!(nw_linear.descriptor().variant.0, "linear-gap-generic-eq");

        let nw_affine = NeedlemanWunsch::new(AffineGap::default_affine());
        assert_eq!(nw_affine.descriptor().variant.0, "affine-gap-generic-eq");
    }

    #[test]
    fn class_is_score() {
        assert_eq!(NeedlemanWunsch::<LinearGap>::class(), MetricClass::Score);
    }

    #[test]
    fn identical_scores_length_times_match() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let a: &[u8] = b"AAAA";
        assert_eq!(nw.score(a, a).into_inner(), 4);
    }

    #[test]
    fn empty_pair_scores_zero() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let empty: &[u8] = b"";
        assert_eq!(nw.score(empty, empty).into_inner(), 0);
    }

    #[test]
    fn linear_and_affine_agree_when_open_equals_extend() {
        // AffineGap with open == extend is behaviorally linear.
        let linear = NeedlemanWunsch::new(LinearGap {
            match_score: 2,
            mismatch_score: -1,
            gap_penalty: -1,
        });
        let affine = NeedlemanWunsch::new(AffineGap {
            match_score: 2,
            mismatch_score: -1,
            gap_open: -1,
            gap_extend: -1,
        });
        let a: &[u8] = b"ACGTACGT";
        let c: &[u8] = b"AGGTACCT";
        assert_eq!(linear.score(a, c), affine.score(a, c));
    }

    #[test]
    fn align_reconstructs_original_inputs() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let a: &[u8] = b"GATTACA";
        let b: &[u8] = b"GCATGCU";
        let al = nw.align(a, b);
        assert_eq!(al.extract_a(), a);
        assert_eq!(al.extract_b(), b);
        // Global alignment always starts at (0, 0).
        assert_eq!(al.a_start, 0);
        assert_eq!(al.b_start, 0);
    }

    #[test]
    fn align_affine_reconstructs_original_inputs() {
        let nw = NeedlemanWunsch::new(AffineGap::default_affine());
        let a: &[u8] = b"AAAABBBB";
        let b: &[u8] = b"AACCCCBBBB";
        let al = nw.align(a, b);
        assert_eq!(al.extract_a(), a);
        assert_eq!(al.extract_b(), b);
    }

    #[test]
    fn score_and_align_agree() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        for (a, b) in [
            (&b"ACGT"[..], &b"ACGT"[..]),
            (b"", b""),
            (b"AAAA", b""),
            (b"", b"BBBB"),
            (b"AGTC", b"GATC"),
        ] {
            assert_eq!(nw.score(a, b).into_inner(), nw.align(a, b).score);
        }
    }

    #[test]
    fn score_and_align_agree_affine() {
        let nw = NeedlemanWunsch::new(AffineGap::default_affine());
        for (a, b) in [
            (&b"ACGTACGT"[..], &b"ACGT"[..]),
            (b"", b"AAA"),
            (b"AAAA", b""),
            (b"AABBAABB", b"AAAABBBB"),
        ] {
            assert_eq!(nw.score(a, b).into_inner(), nw.align(a, b).score);
        }
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let mut ws = AlignmentWorkspace::new();
        let a: &[u8] = b"AAAA";
        let b: &[u8] = b"AAAB";
        let s1 = nw.score_with_workspace(a, b, &mut ws);
        // Second call reuses same workspace.
        let s2 = nw.score_with_workspace(a, b, &mut ws);
        assert_eq!(s1, s2);
        let s3 = nw.score(a, b);
        assert_eq!(s1, s3);
    }

    #[test]
    fn symmetric_scheme_yields_symmetric_score() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        let a: &[u8] = b"ACGTACGT";
        let b: &[u8] = b"CGTATTC";
        assert_eq!(nw.score(a, b), nw.score(b, a));
    }

    #[test]
    fn affine_gap_prefers_one_long_gap_over_many_short_gaps() {
        // With very large open and tiny extend, one long gap of length 4
        // should cost open + 3*extend, cheaper than 4 separate gaps
        // (4*open).
        let scheme = AffineGap {
            match_score: 1,
            mismatch_score: -10,
            gap_open: -10,
            gap_extend: -1,
        };
        let nw = NeedlemanWunsch::new(scheme);
        let a: &[u8] = b"AAAA";
        let b: &[u8] = b"AABBBBAA";
        // The optimal alignment: AA----AA vs AABBBBAA. That's 4 matches +
        // one gap of length 4. Score = 4*1 + (-10 + 3*-1) = 4 - 13 = -9.
        assert_eq!(nw.score(a, b).into_inner(), -9);
    }
}
