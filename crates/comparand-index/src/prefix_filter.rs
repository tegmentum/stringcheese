//! The [`length_filter`] helper: a Jaccard-friendly length bound.
//!
//! Length filtering is the cheapest and most widely used pre-filter for
//! set-similarity candidate generation. Given a query with `Q` distinct
//! items (grams, tokens, whatever the set contains) and a Jaccard threshold
//! `θ ∈ (0, 1]`, an item with `L` distinct items can meet or exceed `θ`
//! only when `θ · Q ≤ L ≤ Q / θ`.
//!
//! # Derivation
//!
//! For sets `A`, `B` with `|A| = L`, `|B| = Q`, the Jaccard similarity is
//! `|A ∩ B| / |A ∪ B|`. The intersection is bounded above by `min(L, Q)`;
//! the union is bounded below by `max(L, Q)`. Therefore
//! `J(A, B) ≤ min(L, Q) / max(L, Q)`, and demanding `J(A, B) ≥ θ` forces
//! `min(L, Q) / max(L, Q) ≥ θ`. Rewriting for the case `L ≤ Q` gives
//! `L / Q ≥ θ`, i.e. `L ≥ θ · Q`; the case `L > Q` gives `Q / L ≥ θ`, i.e.
//! `L ≤ Q / θ`. Combining the two, `θ · Q ≤ L ≤ Q / θ`.
//!
//! The bound is *tight in the sense of soundness* — it never excludes an
//! item that could actually meet the threshold — but not *complete*: many
//! items with lengths in that range will still fall short of `θ` after the
//! full similarity is computed. Length filtering is a pre-filter, not a
//! decision.
//!
//! # Rounding
//!
//! We return a `RangeInclusive<u32>` so downstream code can iterate or
//! range-check directly. The lower end rounds *up* (`ceil(θ · Q)`) and the
//! upper end rounds *down* (`floor(Q / θ)`); both roundings preserve
//! soundness because rounding toward `Q` in either direction can only
//! *widen* the accepted range, never narrow it. When `θ = 0.0` the range
//! degenerates to `0..=u32::MAX` — every item passes. When `θ ≤ 0.0` or
//! `θ > 1.0` (invalid Jaccard thresholds), the range is `0..=u32::MAX` too,
//! on the "reject nothing" side: an out-of-range threshold is a caller bug
//! and we prefer to keep the pre-filter transparent rather than silently
//! filter items out.
//!
//! # References
//!
//! * Sarawagi, S., & Kirpal, A. (2004). "Efficient set joins on similarity
//!   predicates." *Proceedings of the 2004 ACM SIGMOD international
//!   conference on Management of data*, 743-754.
//!   <https://doi.org/10.1145/1007568.1007652> — presents the
//!   length-filter bound used here in the context of set-similarity joins.

use core::ops::RangeInclusive;

/// Returns the range of item lengths that could possibly meet a Jaccard
/// threshold `threshold` for a query of length `query_len`.
///
/// See the [module-level documentation][crate::prefix_filter] for the
/// derivation and the rounding convention.
///
/// # Panics
///
/// Does not panic. Non-finite thresholds and thresholds outside `(0.0, 1.0]`
/// are treated conservatively — the returned range is `0..=u32::MAX` and
/// nothing is filtered out. Callers who want stricter validation should
/// inspect the threshold before calling.
#[must_use]
pub fn length_filter(query_len: u32, threshold: f64) -> RangeInclusive<u32> {
    if !threshold.is_finite() || threshold <= 0.0 || threshold > 1.0 {
        return 0..=u32::MAX;
    }
    let q = f64::from(query_len);
    // ceil(θ · Q) and floor(Q / θ), computed without `f64::ceil`/`floor`
    // so the crate stays `no_std`. For non-negative values `x`, `x as u32`
    // truncates toward zero (i.e. floor), and saturates at both ends.
    let lo_f = threshold * q;
    let hi_f = q / threshold;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the guard branches clamp to a well-defined u32 range"
    )]
    let lo_u = if lo_f <= 0.0 {
        0u32
    } else if lo_f >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        // ceil(x) = floor(x) + (x is not integral ? 1 : 0).
        let floor = lo_f as u32;
        if lo_f > f64::from(floor) {
            floor.saturating_add(1)
        } else {
            floor
        }
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the guard branches clamp to a well-defined u32 range"
    )]
    let hi_u = if hi_f <= 0.0 {
        0u32
    } else if hi_f >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        hi_f as u32
    };
    lo_u..=hi_u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_one_forces_equal_length() {
        assert_eq!(length_filter(10, 1.0), 10..=10);
        assert_eq!(length_filter(0, 1.0), 0..=0);
    }

    #[test]
    fn threshold_half_gives_symmetric_range() {
        // θ = 0.5, Q = 10 → 5 ≤ L ≤ 20.
        assert_eq!(length_filter(10, 0.5), 5..=20);
    }

    #[test]
    fn out_of_range_thresholds_accept_everything() {
        assert_eq!(length_filter(10, 0.0), 0..=u32::MAX);
        assert_eq!(length_filter(10, -0.1), 0..=u32::MAX);
        assert_eq!(length_filter(10, 1.5), 0..=u32::MAX);
        assert_eq!(length_filter(10, f64::NAN), 0..=u32::MAX);
        assert_eq!(length_filter(10, f64::INFINITY), 0..=u32::MAX);
    }

    #[test]
    fn ceil_and_floor_rounding_are_conservative() {
        // θ = 0.7, Q = 10 → 7 ≤ L ≤ ~14.28 → 7..=14.
        assert_eq!(length_filter(10, 0.7), 7..=14);
    }
}
