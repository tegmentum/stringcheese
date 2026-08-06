//! Ukkonen-style banded OSA kernel with early termination.
//!
//! Given a cutoff `k`, the kernel is allowed to abandon a comparison as soon
//! as it can prove the true distance exceeds `k`. The two soundness facts
//! that make pruning safe for plain Levenshtein carry over to OSA:
//!
//! * `d[i][j] ≥ |i − j|`. Any cell whose row/column offset from the main
//!   diagonal already exceeds `k` therefore has distance greater than `k`
//!   and cannot lie on any path with total cost `≤ k`.
//! * Along an optimal path from `(0, 0)` to `(m, n)`, DP values are
//!   monotonically non-decreasing. If every reachable cell of the current
//!   row already exceeds `k`, the answer at `(m, n)` will too.
//!
//! The transposition branch reaches to `d[i-2][j-2]`. Since `(i-2, j-2)`
//! has the same signed offset from the main diagonal as `(i, j)`, an
//! in-band current cell always has an in-band transposition source — so
//! the symmetric band of width `2k + 1` around the main diagonal remains
//! the correct pruning window for OSA.
//!
//! Cells outside the band are held at a `SENTINEL` value that combines
//! saturating-safely with `+1` so they cannot win the three-way (or
//! four-way) minimum.

use stringcheese_core::{BoundedDistance, Distance, Workspace};

use crate::workspace::OsaWorkspace;

/// Sentinel value representing "unreachable / out of band".
///
/// Combined with `saturating_add`, arithmetic on the sentinel stays at the
/// sentinel, so an out-of-band cell can never win the four-way minimum that
/// picks the next row's value.
const SENTINEL: u32 = u32::MAX;

/// Computes the Optimal String Alignment distance between `a` and `b` with
/// an early-termination cutoff, using three rolling rows backed by `ws`.
///
/// Returns [`BoundedDistance::Within`] with the exact distance if it is
/// `≤ cutoff`, otherwise [`BoundedDistance::Exceeded`] with the cutoff
/// value that was crossed.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the DP recurrence is universally written with `a`, `b`, `m`, `n`, `i`, `j`, `k`; renaming would put a translation layer between the code and its textbook derivation"
)]
#[allow(
    clippy::similar_names,
    reason = "`prev_off` and `prev2_off` name the two rows above the current row (i-1 and i-2); the pair-shaped naming is the clearest way to express that they are the two components of a three-row rolling window"
)]
pub fn distance_banded_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    cutoff: u32,
    ws: &mut OsaWorkspace,
) -> BoundedDistance<u32> {
    // OSA is symmetric, so we can choose the shorter side as the inner
    // dimension without changing the answer. That keeps the scratch buffer
    // at `3 · (min(m, n) + 1)` cells regardless of argument order.
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = long.len();
    let n = short.len();

    // The distance is at least |m − n|. If that already exceeds the cutoff
    // we can answer without any DP.
    if u64::try_from(m.abs_diff(n)).expect("length difference exceeds u64") > u64::from(cutoff) {
        return BoundedDistance::Exceeded { cutoff };
    }

    // Fast paths for one-sided empties. The length-difference check above
    // guarantees `max(m, n) ≤ cutoff` in these cases.
    if m == 0 {
        return BoundedDistance::Within(Distance::new(0));
    }
    if n == 0 {
        let d = u32::try_from(m).expect("input length exceeds u32::MAX");
        return BoundedDistance::Within(Distance::new(d));
    }

    let radius = usize::try_from(cutoff).unwrap_or(usize::MAX);
    let stride = n + 1;
    ws.ensure_capacity(3 * stride);
    let buf = ws.buffer_mut(3 * stride);

    // Row 0: `d[0][j] = j` inside the band, SENTINEL outside. This row
    // lives in physical slot `(0 % 3) * stride = 0`.
    for (j, cell) in buf[..=n].iter_mut().enumerate() {
        *cell = if j <= radius {
            u32::try_from(j).expect("input length exceeds u32::MAX")
        } else {
            SENTINEL
        };
    }

    for i in 1..=m {
        let i_u32 = u32::try_from(i).expect("input length exceeds u32::MAX");
        let curr_off = (i % 3) * stride;
        let prev_off = ((i + 3 - 1) % 3) * stride;
        let prev2_off = ((i + 3 - 2) % 3) * stride;

        let j_start = i.saturating_sub(radius);
        let j_end = n.min(i.saturating_add(radius));

        // Reset the current row to SENTINEL so out-of-band cells left over
        // from a previous rotation cannot leak into this row's `left`
        // neighbor read.
        for c in &mut buf[curr_off..curr_off + stride] {
            *c = SENTINEL;
        }

        // Leftmost column: `d[i][0] = i` — inside the band iff `i ≤ radius`.
        let mut row_min = SENTINEL;
        if j_start == 0 {
            buf[curr_off] = i_u32;
            row_min = i_u32;
        }

        let j_lo = j_start.max(1);
        for j in j_lo..=j_end {
            let cost = u32::from(long[i - 1] != short[j - 1]);
            let deletion = buf[prev_off + j].saturating_add(1);
            let insertion = buf[curr_off + (j - 1)].saturating_add(1);
            let substitution = buf[prev_off + (j - 1)].saturating_add(cost);
            let mut best = deletion.min(insertion).min(substitution);

            if i >= 2 && j >= 2 && long[i - 1] == short[j - 2] && long[i - 2] == short[j - 1] {
                let transposition = buf[prev2_off + (j - 2)].saturating_add(1);
                best = best.min(transposition);
            }

            buf[curr_off + j] = best;
            if best < row_min {
                row_min = best;
            }
        }

        // Because row minima are non-decreasing across rows, once every
        // reachable cell of the current row already exceeds the cutoff, the
        // final answer cannot fit under the cutoff either.
        if row_min > cutoff {
            return BoundedDistance::Exceeded { cutoff };
        }
    }

    let final_d = buf[(m % 3) * stride + n];
    if final_d > cutoff {
        BoundedDistance::Exceeded { cutoff }
    } else {
        BoundedDistance::Within(Distance::new(final_d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osa::full_matrix::distance_full_matrix;

    fn banded(a: &[u8], b: &[u8], k: u32) -> BoundedDistance<u32> {
        let mut ws = OsaWorkspace::new();
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
    fn adjacent_transposition_within_cutoff() {
        // OSA scores an adjacent transposition as one; the band must
        // reach the `d[i-2][j-2]` transposition source even for tight `k`.
        assert_eq!(
            banded(b"ab", b"ba", 1),
            BoundedDistance::Within(Distance::new(1))
        );
    }

    #[test]
    fn ca_abc_within_cutoff_three() {
        assert_eq!(
            banded(b"ca", b"abc", 3),
            BoundedDistance::Within(Distance::new(3))
        );
    }

    #[test]
    fn ca_abc_exceeds_cutoff_two() {
        // Distinguishes OSA from full Damerau — Damerau would fit under 2.
        assert_eq!(
            banded(b"ca", b"abc", 2),
            BoundedDistance::Exceeded { cutoff: 2 }
        );
    }

    #[test]
    fn length_difference_alone_exceeds_cutoff() {
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
            (b"ab", b"ba"),
            (b"ca", b"abc"),
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
        let pairs: &[(&[u8], &[u8])] = &[
            (b"abcdefgh", b"abcdefgh"),
            (b"abcdefgh", b"abcdefgi"),
            (b"abcdefgh", b"abzdefgh"),
            (b"abcdefgh", b"xxxxxxxx"),
            (b"abcd", b"badc"),
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
