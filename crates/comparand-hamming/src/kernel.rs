//! The Hamming-distance kernels.
//!
//! Two entry points live here: [`hamming_distance`] computes the exact
//! distance, and [`hamming_distance_within`] adds early termination once a
//! running mismatch count exceeds a caller-supplied cutoff.
//!
//! Both kernels are pure — no allocation, no scratch buffers, no interior
//! mutability. Callers who cannot statically prove equal-length inputs
//! should use the fallible [`Hamming::try_distance`] /
//! [`Hamming::try_distance_within`] wrappers on the algorithm handle rather
//! than these kernels directly; the kernels here panic on a length mismatch
//! to match the trait-implementation contract.
//!
//! [`Hamming::try_distance`]: crate::algorithm::Hamming::try_distance
//! [`Hamming::try_distance_within`]: crate::algorithm::Hamming::try_distance_within

use comparand_core::{BoundedDistance, Distance};

/// Computes the Hamming distance between two equal-length slices.
///
/// The distance is the number of positions at which the two slices differ
/// under `T`'s [`Eq`] implementation.
///
/// # Panics
///
/// Panics if `left.len() != right.len()`. Callers that cannot statically
/// establish equal length should call [`Hamming::try_distance`] instead,
/// which returns a [`LengthMismatch`] rather than panicking.
///
/// # Overflow
///
/// The distance is returned as [`Distance<u32>`]. On the rare hypothetical
/// input longer than `u32::MAX`, the returned distance saturates at
/// `u32::MAX` — which is still a correct-in-direction distance (an upper
/// bound on the true count) rather than a wraparound to a small number.
///
/// [`Hamming::try_distance`]: crate::algorithm::Hamming::try_distance
/// [`LengthMismatch`]: crate::error::LengthMismatch
#[inline]
#[must_use]
pub fn hamming_distance<T: Eq>(left: &[T], right: &[T]) -> Distance<u32> {
    assert_eq!(
        left.len(),
        right.len(),
        "hamming_distance requires equal-length inputs (got {} and {})",
        left.len(),
        right.len(),
    );

    // `usize -> u32` may saturate for pathological input lengths above
    // `u32::MAX`. Saturation preserves the direction of the inequality
    // downstream (the true distance is at most the returned value).
    let count = left
        .iter()
        .zip(right.iter())
        .filter(|(a, b)| a != b)
        .count();
    Distance::new(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Computes the Hamming distance with early termination when the running
/// mismatch count exceeds `cutoff`.
///
/// Returns [`BoundedDistance::Within`] carrying the exact distance if it is
/// less than or equal to `cutoff`; returns [`BoundedDistance::Exceeded`]
/// carrying the cutoff value if the true distance is strictly greater. The
/// two variants are distinct so a caller cannot silently mistake an
/// early-terminated result for an exact one.
///
/// # Panics
///
/// Panics if `left.len() != right.len()`, matching [`hamming_distance`].
/// Callers that cannot statically establish equal length should call
/// [`Hamming::try_distance_within`] instead.
///
/// [`Hamming::try_distance_within`]: crate::algorithm::Hamming::try_distance_within
#[inline]
#[must_use]
pub fn hamming_distance_within<T: Eq>(
    left: &[T],
    right: &[T],
    cutoff: u32,
) -> BoundedDistance<u32> {
    assert_eq!(
        left.len(),
        right.len(),
        "hamming_distance_within requires equal-length inputs (got {} and {})",
        left.len(),
        right.len(),
    );

    let mut count: u32 = 0;
    for (a, b) in left.iter().zip(right.iter()) {
        if a != b {
            // Increment first, then compare. This ordering keeps the counter
            // usable as the exact distance when we fall out of the loop
            // without ever exceeding the cutoff.
            count = count.saturating_add(1);
            if count > cutoff {
                return BoundedDistance::Exceeded { cutoff };
            }
        }
    }
    BoundedDistance::Within(Distance::new(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(hamming_distance::<u8>(b"", b"").into_inner(), 0);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(hamming_distance(b"abcdef", b"abcdef").into_inner(), 0);
    }

    #[test]
    fn all_differ_equals_length() {
        assert_eq!(hamming_distance(b"abc", b"xyz").into_inner(), 3);
    }

    #[test]
    #[should_panic(expected = "equal-length")]
    fn panics_on_unequal_length_exact() {
        let _ = hamming_distance(b"abc", b"abcd");
    }

    #[test]
    #[should_panic(expected = "equal-length")]
    fn panics_on_unequal_length_bounded() {
        let _ = hamming_distance_within(b"abc", b"abcd", 5);
    }

    #[test]
    fn bounded_within_reports_exact() {
        let d = hamming_distance_within(b"karolin", b"kathrin", 5);
        assert_eq!(d, BoundedDistance::Within(Distance::new(3)));
    }

    #[test]
    fn bounded_exceeded_carries_cutoff() {
        let d = hamming_distance_within(b"karolin", b"kathrin", 2);
        assert_eq!(d, BoundedDistance::Exceeded { cutoff: 2 });
    }

    #[test]
    fn bounded_at_cutoff_is_within() {
        // Distance = 3, cutoff = 3 -> Within(3) (cutoff is inclusive).
        let d = hamming_distance_within(b"karolin", b"kathrin", 3);
        assert_eq!(d, BoundedDistance::Within(Distance::new(3)));
    }
}
