//! Ukkonen-style banded Levenshtein kernel with early termination.
//!
//! The banding strategy is Ukkonen's (1985) — the observation that any
//! optimal edit path stays within a band of width `2k + 1` around the main
//! diagonal when the total edit cost is bounded by `k`. See the crate-level
//! `References` section for the full citation.
//!
//! Given a cutoff `k`, the kernel is allowed to abandon a comparison as soon
//! as it can prove the true distance exceeds `k`. Two facts make the pruning
//! sound:
//!
//! * `d[i][j] ≥ |i − j|`. Any cell whose row/column offset from the main
//!   diagonal already exceeds `k` therefore has distance greater than `k` and
//!   cannot lie on any path with total cost `≤ k`.
//! * Along an optimal path from `(0, 0)` to `(m, n)`, the DP values are
//!   monotonically non-decreasing (each edit contributes a non-negative
//!   cost). If every reachable cell of the current row already exceeds `k`,
//!   the answer at `(m, n)` will too.
//!
//! Together these give the classical **symmetric band of width `2k + 1`**
//! around the main diagonal: cells outside the band are treated as an
//! infinity sentinel via saturating arithmetic, so they never win a
//! three-way minimum. The kernel touches `O(k · min(m, n))` cells rather
//! than the full `O(m · n)`.
//!
//! # Trade-offs
//!
//! The band this kernel uses is *symmetric*: `|i − j| ≤ k`. A tighter band
//! keyed on `|m − n|` is possible — the search only needs to reach the
//! endpoint `(m, n)` — but the symmetric formulation keeps the loop bounds
//! simple and stays `O(k)` in width, which is what the crate's spec asks for.

use stringcheese_core::{BoundedDistance, Distance, Workspace};

use crate::levenshtein::workspace::LevenshteinWorkspace;

/// Sentinel value representing "unreachable / out of band".
///
/// Combined with `saturating_add`, arithmetic on the sentinel stays at the
/// sentinel, so an out-of-band cell can never win the three-way minimum that
/// picks the next row's value.
const SENTINEL: u32 = u32::MAX;

/// Computes the unit-cost Levenshtein distance between `a` and `b` with an
/// early-termination cutoff.
///
/// Returns [`BoundedDistance::Within`] with the exact distance if it is
/// `≤ cutoff`, otherwise [`BoundedDistance::Exceeded`] with the cutoff value
/// that was crossed.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the DP recurrence is universally written with `a`, `b`, `m`, `n`, `i`, `j`, `k`; renaming would put a translation layer between the code and its textbook derivation"
)]
pub fn distance_banded_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    cutoff: u32,
    ws: &mut LevenshteinWorkspace,
) -> BoundedDistance<u32> {
    let m = a.len();
    let n = b.len();

    // The distance is at least |m − n|. If that already exceeds the cutoff,
    // we can answer without any DP.
    if u64::try_from(m.abs_diff(n)).expect("length difference exceeds u64") > u64::from(cutoff) {
        return BoundedDistance::Exceeded { cutoff };
    }

    // Fast paths for one-sided empties. The length-difference check above
    // guarantees `max(m, n) ≤ cutoff` in these cases.
    if m == 0 {
        let d = u32::try_from(n).expect("input length exceeds u32::MAX");
        return BoundedDistance::Within(Distance::new(d));
    }
    if n == 0 {
        let d = u32::try_from(m).expect("input length exceeds u32::MAX");
        return BoundedDistance::Within(Distance::new(d));
    }

    let radius = usize::try_from(cutoff).unwrap_or(usize::MAX);
    ws.ensure_capacity(n + 1);
    let row = ws.buffer_mut(n + 1);

    // Initial row (i = 0): d[0][j] = j inside the band, sentinel outside.
    for (j, cell) in row.iter_mut().enumerate() {
        *cell = if j <= radius {
            u32::try_from(j).expect("input length exceeds u32::MAX")
        } else {
            SENTINEL
        };
    }

    for i in 1..=m {
        let i_u32 = u32::try_from(i).expect("input length exceeds u32::MAX");
        let j_start = i.saturating_sub(radius);
        let j_end = n.min(i.saturating_add(radius));

        // `prev_diag` is d[i-1][j-1] for the cell about to be computed; it
        // slides one column right on every iteration.
        let mut prev_diag: u32;
        let mut row_min: u32;

        if j_start == 0 {
            // Cell (i, 0) is inside the band (i ≤ radius). Its value is `i`.
            prev_diag = row[0];
            row[0] = i_u32;
            row_min = i_u32;
        } else {
            // Cell (i, 0) is outside the band. `row[0]` still holds
            // d[i-1][0] = i-1 from the previous row — its value does not
            // matter for correctness because no in-band cell of this row
            // reads it (the leftmost cell to compute is at j = j_start > 0).
            // The `prev_diag` for the leftmost in-band cell is d[i-1][j_start-1],
            // which sits at the *left edge* of the previous row's band and is
            // therefore a valid computed value.
            prev_diag = row[j_start - 1];
            row_min = SENTINEL;
        }

        // Compute d[i][j] for j from max(j_start, 1) through j_end.
        let j_lo = j_start.max(1);
        for j in j_lo..=j_end {
            let old_row_j = row[j]; // = d[i-1][j], sentinel if out of previous band
            let cost = u32::from(a[i - 1] != b[j - 1]);

            // For the leftmost cell in a shifted band (j == j_start with
            // j_start > 0), cell (i, j-1) is outside this row's band; treat
            // it as sentinel so the insertion candidate cannot win.
            let left = if j == j_start && j_start > 0 {
                SENTINEL
            } else {
                row[j - 1]
            };

            let deletion = old_row_j.saturating_add(1);
            let insertion = left.saturating_add(1);
            let substitution = prev_diag.saturating_add(cost);

            let curr = deletion.min(insertion).min(substitution);

            prev_diag = old_row_j;
            row[j] = curr;
            if curr < row_min {
                row_min = curr;
            }
        }

        // Reset the cell immediately to the right of the band so the next
        // row's read of `row[j_end + 1]` (as d[i][j_end + 1]) sees a
        // sentinel rather than a stale value from an earlier iteration.
        if j_end < n {
            row[j_end + 1] = SENTINEL;
        }

        // Because row minima are non-decreasing across rows (any cell in
        // row i+1 is at least the minimum of the two cells above it), a row
        // whose minimum already exceeds the cutoff cannot lead to a
        // sub-cutoff final answer.
        if row_min > cutoff {
            return BoundedDistance::Exceeded { cutoff };
        }
    }

    let final_d = row[n];
    if final_d > cutoff {
        BoundedDistance::Exceeded { cutoff }
    } else {
        BoundedDistance::Within(Distance::new(final_d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levenshtein::full_matrix::distance_full_matrix;

    fn banded(a: &[u8], b: &[u8], k: u32) -> BoundedDistance<u32> {
        let mut ws = LevenshteinWorkspace::new();
        distance_banded_with_workspace(a, b, k, &mut ws)
    }

    #[test]
    fn within_cutoff_returns_exact_distance() {
        assert_eq!(
            banded(b"kitten", b"sitting", 5),
            BoundedDistance::Within(Distance::new(3))
        );
    }

    #[test]
    fn exceeding_cutoff_returns_exceeded() {
        assert_eq!(
            banded(b"kitten", b"sitting", 2),
            BoundedDistance::Exceeded { cutoff: 2 }
        );
    }

    #[test]
    fn length_difference_alone_exceeds_cutoff() {
        // |m − n| = 5, cutoff = 4, so the length difference itself already
        // exceeds the cutoff. The kernel should short-circuit without any DP.
        assert_eq!(
            banded(b"short", b"much-longer", 4),
            BoundedDistance::Exceeded { cutoff: 4 }
        );
    }

    #[test]
    fn cutoff_zero_only_accepts_identical() {
        assert_eq!(
            banded(b"abc", b"abc", 0),
            BoundedDistance::Within(Distance::new(0))
        );
        assert_eq!(
            banded(b"abc", b"abd", 0),
            BoundedDistance::Exceeded { cutoff: 0 }
        );
    }

    #[test]
    fn empty_pair_is_within() {
        assert_eq!(
            banded(b"", b"", 0),
            BoundedDistance::Within(Distance::new(0))
        );
    }

    #[test]
    fn one_side_empty() {
        assert_eq!(
            banded(b"", b"abcd", 4),
            BoundedDistance::Within(Distance::new(4))
        );
        assert_eq!(
            banded(b"abcd", b"", 3),
            BoundedDistance::Exceeded { cutoff: 3 }
        );
    }

    #[test]
    fn wide_cutoff_matches_full_matrix() {
        let pairs: &[(&[u8], &[u8])] = &[
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"", b"nonempty"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            (b"totally", b"different"),
            (b"aaaaa", b"aaaba"),
            (b"a", b"a"),
        ];
        for (a, b) in pairs {
            let expected = distance_full_matrix(a, b);
            let observed = banded(a, b, 100);
            assert_eq!(
                observed,
                BoundedDistance::Within(Distance::new(expected)),
                "banded (wide cutoff) disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn tight_cutoff_matches_full_matrix() {
        // Compare banded outputs at varied cutoffs against the oracle.
        let pairs: &[(&[u8], &[u8])] = &[
            (b"abcdefgh", b"abcdefgh"),
            (b"abcdefgh", b"abcdefgi"),
            (b"abcdefgh", b"abzdefgh"),
            (b"abcdefgh", b"xxxxxxxx"),
        ];
        for (a, b) in pairs {
            let expected = distance_full_matrix(a, b);
            for k in 0..=10u32 {
                let observed = banded(a, b, k);
                if expected <= k {
                    assert_eq!(
                        observed,
                        BoundedDistance::Within(Distance::new(expected)),
                        "band k={k} on ({a:?}, {b:?}) expected Within({expected})"
                    );
                } else {
                    assert_eq!(
                        observed,
                        BoundedDistance::Exceeded { cutoff: k },
                        "band k={k} on ({a:?}, {b:?}) expected Exceeded"
                    );
                }
            }
        }
    }
}
