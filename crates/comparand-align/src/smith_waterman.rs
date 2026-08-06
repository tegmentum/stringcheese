//! Smith-Waterman local sequence alignment.
//!
//! Finds the highest-scoring alignment between a substring of `a` and a
//! substring of `b`. Unlike Needleman-Wunsch, the DP floors negative
//! partial scores to zero, which lets the alignment restart anywhere and
//! effectively discards low-scoring flanks. The reported alignment is the
//! substring pair that maximizes the score.
//!
//! # Algorithm
//!
//! * When the scoring scheme reports `gap_open == gap_extend`, the algorithm
//!   uses the original single-matrix DP of Smith and Waterman (1981).
//! * When the two gap costs differ, the algorithm uses a three-matrix
//!   (`M`, `X`, `Y`) DP adapted from Gotoh (1982) — the `M` matrix is
//!   floored to zero (a fresh local alignment may start at any cell) while
//!   `X` and `Y` are left unfloored so that the semantics of "currently in
//!   a gap" cannot be silently reset mid-gap.
//!
//! # Complexity
//!
//! Score-only: `O(m * n)` time, `O(n)` space (linear gap) or `O(3 * n)`
//! space (affine).
//! With edit-script backtrace: `O(m * n)` time and space.

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass, Score,
    VariantId,
};

use crate::edit_script::{Alignment, EditOp};
use crate::scoring::{ScoringScheme, is_affine};
use crate::workspace::AlignmentWorkspace;

/// Sentinel used in unreachable gap-state cells so a max-picking operation
/// naturally ignores them. `i32::MIN / 4` leaves ample headroom for
/// repeated `+ gap_open` / `+ gap_extend` additions.
const NEG_INF: i32 = i32::MIN / 4;

/// The Smith-Waterman local-alignment algorithm handle.
///
/// Parameterized by any [`ScoringScheme`]. When the scheme is affine
/// (i.e. `gap_open != gap_extend`) the handle uses the three-matrix Gotoh-
/// adapted DP; otherwise it uses the simpler single-matrix DP. Both
/// branches share the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SmithWaterman<S: ScoringScheme> {
    /// The scoring scheme governing match / mismatch / gap costs.
    pub scoring: S,
}

impl<S: ScoringScheme> SmithWaterman<S> {
    /// Descriptor for the linear-gap variant.
    pub const LINEAR_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::SmithWaterman,
        variant: VariantId("linear-gap-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Identification of common molecular subsequences",
            authors: "T. F. Smith, M. S. Waterman",
            year: 1981,
        },
    };

    /// Descriptor for the affine-gap variant.
    ///
    /// The base local-alignment algorithm is still Smith-Waterman 1981; the
    /// three-matrix construction that makes the affine gap tractable is
    /// Gotoh 1982. We attribute the variant to Gotoh because that paper is
    /// what makes this DP shape unambiguous.
    pub const AFFINE_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::SmithWaterman,
        variant: VariantId("affine-gap-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "An improved algorithm for matching biological sequences",
            authors: "O. Gotoh",
            year: 1982,
        },
    };

    /// Construct a Smith-Waterman aligner with the given scoring scheme.
    #[must_use]
    pub const fn new(scoring: S) -> Self {
        Self { scoring }
    }

    /// Return the descriptor corresponding to this configuration.
    #[must_use]
    pub fn descriptor(&self) -> AlgorithmDescriptor {
        if is_affine(&self.scoring) {
            Self::AFFINE_DESCRIPTOR
        } else {
            Self::LINEAR_DESCRIPTOR
        }
    }

    /// Metric class this algorithm belongs to.
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Score
    }

    /// Compute the local alignment score of `a` and `b`.
    ///
    /// Guaranteed to be `>= 0` — the "empty alignment" of score `0` is
    /// always available.
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    #[must_use]
    pub fn score<T: Eq>(&self, a: &[T], b: &[T]) -> Score<i32> {
        let mut ws = AlignmentWorkspace::new();
        self.score_with_workspace(a, b, &mut ws)
    }

    /// Compute the local alignment score, reusing a caller-supplied
    /// workspace.
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

    /// Compute the local alignment together with the reconstructed edit
    /// script and the start indices of the aligned substrings within `a`
    /// and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either input is longer than `i32::MAX` symbols.
    #[must_use]
    pub fn align<T: Eq + Clone>(&self, a: &[T], b: &[T]) -> Alignment<T> {
        let mut ws = AlignmentWorkspace::new();
        self.align_with_workspace(a, b, &mut ws)
    }

    /// Compute the local alignment and reconstructed edit script, reusing a
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
// Linear-gap score-only kernel (single-row rolling with running max).
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

    if m == 0 || n == 0 {
        return Score::new(0);
    }

    let row = ws.score_buffer(n + 1);
    for cell in row.iter_mut() {
        *cell = 0;
    }

    let mut best: i32 = 0;
    for i in 1..=m {
        let mut diag = row[0];
        row[0] = 0;
        for j in 1..=n {
            let up = row[j];
            let left = row[j - 1];
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let val = 0i32.max(diag + s).max(up + gap).max(left + gap);
            best = best.max(val);
            diag = up;
            row[j] = val;
        }
    }

    Score::new(best)
}

// ---------------------------------------------------------------------------
// Linear-gap align kernel (full matrix + backtrace from the argmax cell).
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

    if m == 0 || n == 0 {
        return Alignment {
            score: 0,
            script: Vec::new(),
            a_start: 0,
            b_start: 0,
        };
    }

    let stride = n + 1;
    let cells = (m + 1) * stride;

    let mat = ws.m_matrix(cells);
    let idx = |i: usize, j: usize| i * stride + j;

    for cell in mat.iter_mut() {
        *cell = 0;
    }

    let mut best: i32 = 0;
    let mut best_i: usize = 0;
    let mut best_j: usize = 0;

    for i in 1..=m {
        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let diag = mat[idx(i - 1, j - 1)] + s;
            let up = mat[idx(i - 1, j)] + gap;
            let left = mat[idx(i, j - 1)] + gap;
            let val = 0i32.max(diag).max(up).max(left);
            mat[idx(i, j)] = val;
            if val > best {
                best = val;
                best_i = i;
                best_j = j;
            }
        }
    }

    // No positive-scoring local alignment exists.
    if best == 0 {
        return Alignment {
            score: 0,
            script: Vec::new(),
            a_start: 0,
            b_start: 0,
        };
    }

    // Backtrace from (best_i, best_j) until we hit a zero cell.
    let mut script: Vec<EditOp<T>> = Vec::new();
    let mut i = best_i;
    let mut j = best_j;

    while i > 0 && j > 0 && mat[idx(i, j)] > 0 {
        let cur = mat[idx(i, j)];
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
        } else if cur == mat[idx(i - 1, j)] + gap {
            script.push(EditOp::Delete {
                a: a[i - 1].clone(),
            });
            i -= 1;
        } else {
            // cur == mat[idx(i, j-1)] + gap
            script.push(EditOp::Insert {
                b: b[j - 1].clone(),
            });
            j -= 1;
        }
    }

    script.reverse();

    Alignment {
        score: best,
        script,
        a_start: i,
        b_start: j,
    }
}

// ---------------------------------------------------------------------------
// Affine-gap score-only kernel (rolling row over M, X, Y).
// ---------------------------------------------------------------------------

#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "Standard names + a fused body match the published Gotoh 1982 derivation"
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

    if m == 0 || n == 0 {
        return Score::new(0);
    }

    let stride = n + 1;
    let buf = ws.score_buffer(6 * stride);
    let (prev, curr) = buf.split_at_mut(3 * stride);
    let (prev_m, prev_rest) = prev.split_at_mut(stride);
    let (prev_x, prev_y) = prev_rest.split_at_mut(stride);
    let (curr_m, curr_rest) = curr.split_at_mut(stride);
    let (curr_x, curr_y) = curr_rest.split_at_mut(stride);

    // Row 0: M is 0 (a fresh local alignment may start anywhere); X and Y
    // are -inf (gaps cannot exist without a preceding match).
    for j in 0..=n {
        prev_m[j] = 0;
        prev_x[j] = NEG_INF;
        prev_y[j] = NEG_INF;
    }

    let mut best: i32 = 0;
    for i in 1..=m {
        curr_m[0] = 0;
        curr_x[0] = NEG_INF;
        curr_y[0] = NEG_INF;

        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let best_diag = prev_m[j - 1].max(prev_x[j - 1]).max(prev_y[j - 1]);
            curr_m[j] = 0i32.max(best_diag + s);
            curr_x[j] = (prev_m[j] + open)
                .max(prev_x[j] + extend)
                .max(prev_y[j] + open);
            curr_y[j] = (curr_m[j - 1] + open)
                .max(curr_x[j - 1] + open)
                .max(curr_y[j - 1] + extend);
            best = best.max(curr_m[j]);
        }

        prev_m.copy_from_slice(curr_m);
        prev_x.copy_from_slice(curr_x);
        prev_y.copy_from_slice(curr_y);
    }

    Score::new(best)
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
    reason = "Standard names + a fused body match the published Gotoh 1982 derivation"
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

    if m == 0 || n == 0 {
        return Alignment {
            score: 0,
            script: Vec::new(),
            a_start: 0,
            b_start: 0,
        };
    }

    let stride = n + 1;
    let cells = (m + 1) * stride;

    let (mat_m, mat_x, mat_y) = ws.full_matrices(cells);
    let idx = |i: usize, j: usize| i * stride + j;

    // Initialize: M is 0 everywhere on the boundary; X and Y are -inf.
    for j in 0..=n {
        mat_m[idx(0, j)] = 0;
        mat_x[idx(0, j)] = NEG_INF;
        mat_y[idx(0, j)] = NEG_INF;
    }
    for i in 1..=m {
        mat_m[idx(i, 0)] = 0;
        mat_x[idx(i, 0)] = NEG_INF;
        mat_y[idx(i, 0)] = NEG_INF;
    }

    let mut best: i32 = 0;
    let mut best_i: usize = 0;
    let mut best_j: usize = 0;

    for i in 1..=m {
        for j in 1..=n {
            let s = scheme.pair_score(&a[i - 1], &b[j - 1]);
            let ij = idx(i, j);

            let dm = mat_m[idx(i - 1, j - 1)];
            let dx = mat_x[idx(i - 1, j - 1)];
            let dy = mat_y[idx(i - 1, j - 1)];
            mat_m[ij] = 0i32.max(dm.max(dx).max(dy) + s);

            let um = mat_m[idx(i - 1, j)];
            let ux = mat_x[idx(i - 1, j)];
            let uy = mat_y[idx(i - 1, j)];
            mat_x[ij] = (um + open).max(ux + extend).max(uy + open);

            let lm = mat_m[idx(i, j - 1)];
            let lx = mat_x[idx(i, j - 1)];
            let ly = mat_y[idx(i, j - 1)];
            mat_y[ij] = (lm + open).max(ly + extend).max(lx + open);

            if mat_m[ij] > best {
                best = mat_m[ij];
                best_i = i;
                best_j = j;
            }
        }
    }

    if best == 0 {
        return Alignment {
            score: 0,
            script: Vec::new(),
            a_start: 0,
            b_start: 0,
        };
    }

    // Backtrace from (best_i, best_j) in M until M reaches 0.
    let mut script: Vec<EditOp<T>> = Vec::new();
    let mut i = best_i;
    let mut j = best_j;
    let mut layer = Layer::M;

    loop {
        match layer {
            Layer::M => {
                if mat_m[idx(i, j)] == 0 {
                    break;
                }
                debug_assert!(i > 0 && j > 0);
                let dm = mat_m[idx(i - 1, j - 1)];
                let dx = mat_x[idx(i - 1, j - 1)];
                let dy = mat_y[idx(i - 1, j - 1)];

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
                if i == 0 {
                    break;
                }
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
                if j == 0 {
                    break;
                }
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
        score: best,
        script,
        a_start: i,
        b_start: j,
    }
}

/// Return the layer with the highest score, preferring `M` over `X` over `Y`
/// on ties. Ties do not affect the score.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::{AffineGap, LinearGap};

    #[test]
    fn descriptor_switches_on_scheme_shape() {
        let sw_linear = SmithWaterman::new(LinearGap::simple());
        assert_eq!(sw_linear.descriptor().variant.0, "linear-gap-generic-eq");
        let sw_affine = SmithWaterman::new(AffineGap::default_affine());
        assert_eq!(sw_affine.descriptor().variant.0, "affine-gap-generic-eq");
    }

    #[test]
    fn class_is_score() {
        assert_eq!(SmithWaterman::<LinearGap>::class(), MetricClass::Score);
    }

    #[test]
    fn score_is_never_negative() {
        let sw = SmithWaterman::new(LinearGap::simple());
        assert!(sw.score::<u8>(b"", b"").into_inner() >= 0);
        assert!(sw.score(b"AAA", b"BBB").into_inner() >= 0);
    }

    #[test]
    fn identical_scores_length_times_match() {
        let sw = SmithWaterman::new(LinearGap::simple());
        assert_eq!(sw.score(b"AAAA", b"AAAA").into_inner(), 4);
    }

    #[test]
    fn local_alignment_ignores_surrounding_garbage() {
        let sw = SmithWaterman::new(LinearGap::simple());
        // "ACGT" sits in the middle of both; the flanking symbols never
        // match. With match=1, mismatch=-1, gap=-1, the local alignment
        // scores 4 (the four "ACGT" matches).
        let a: &[u8] = b"XXACGTYY";
        let b: &[u8] = b"ZZACGTWW";
        assert_eq!(sw.score(a, b).into_inner(), 4);
    }

    #[test]
    fn align_records_start_indices_of_local_alignment() {
        let sw = SmithWaterman::new(LinearGap::simple());
        let a: &[u8] = b"XXACGTYY";
        let b: &[u8] = b"ZZACGTWW";
        let al = sw.align(a, b);
        assert_eq!(al.score, 4);
        // The aligned substring "ACGT" starts at index 2 in both inputs.
        assert_eq!(al.a_start, 2);
        assert_eq!(al.b_start, 2);
        assert_eq!(al.a_end(), 6);
        assert_eq!(al.b_end(), 6);
        assert_eq!(al.extract_a(), b"ACGT".to_vec());
        assert_eq!(al.extract_b(), b"ACGT".to_vec());
    }

    #[test]
    fn negative_only_pair_returns_empty_alignment() {
        let sw = SmithWaterman::new(LinearGap::simple());
        let al = sw.align(b"AAA", b"BBB");
        assert_eq!(al.score, 0);
        assert!(al.script.is_empty());
    }

    #[test]
    fn score_and_align_agree() {
        let sw = SmithWaterman::new(LinearGap::simple());
        for (a, b) in [
            (&b"ACGT"[..], &b"ACGT"[..]),
            (b"XXACGTYY", b"ZZACGTWW"),
            (b"AAAA", b"AAAA"),
            (b"AAA", b"BBB"),
            (b"", b"AAA"),
        ] {
            assert_eq!(sw.score(a, b).into_inner(), sw.align(a, b).score);
        }
    }

    #[test]
    fn score_and_align_agree_affine() {
        let sw = SmithWaterman::new(AffineGap::default_affine());
        for (a, b) in [
            (&b"ACGT"[..], &b"ACGT"[..]),
            (b"XXACGTYY", b"ZZACGTWW"),
            (b"AAAA", b"AAAA"),
            (b"AAA", b"BBB"),
        ] {
            assert_eq!(sw.score(a, b).into_inner(), sw.align(a, b).score);
        }
    }

    #[test]
    fn local_alignment_reconstructs_extracted_substrings() {
        let sw = SmithWaterman::new(LinearGap::simple());
        let a: &[u8] = b"AAABBBCCC";
        let b: &[u8] = b"XXAAABBBCCCYY";
        let al = sw.align(a, b);
        // The extracted a-side must match a[a_start..a_end].
        let a_end = al.a_end();
        let b_end = al.b_end();
        assert_eq!(al.extract_a(), a[al.a_start..a_end].to_vec());
        assert_eq!(al.extract_b(), b[al.b_start..b_end].to_vec());
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace() {
        let sw = SmithWaterman::new(LinearGap::simple());
        let mut ws = AlignmentWorkspace::new();
        let a: &[u8] = b"AACGT";
        let b: &[u8] = b"ACGTA";
        let s1 = sw.score_with_workspace(a, b, &mut ws);
        let s2 = sw.score_with_workspace(a, b, &mut ws);
        assert_eq!(s1, s2);
    }
}
