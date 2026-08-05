//! Result types for sequence comparisons.
//!
//! Comparand deliberately avoids returning bare numeric values from
//! comparison operations. Each result type encodes the *direction* and
//! *scale* of the quantity it carries, so a `Distance` cannot be silently
//! treated as a `Similarity`, and a `NormalizedDistance` cannot be silently
//! treated as an unbounded distance.
//!
//! The library also does not commit to a global rule such as
//! `distance = 1 - similarity`. When such a conversion is meaningful for a
//! specific algorithm and normalization policy, it must be expressed
//! explicitly at the call site.

use core::fmt;

/// A distance between two sequences.
///
/// Lower values indicate greater similarity; a distance of zero indicates
/// the two sequences are considered identical under the metric's semantics.
///
/// The numeric type `T` is chosen by the algorithm implementation:
///
/// * Unit-cost integer edit distances use integer types — typically `u32`,
///   which is portable across native and Wasm targets.
/// * Weighted or continuous distances use floating-point types.
///
/// # Layout
///
/// `#[repr(transparent)]` — a `Distance<T>` has exactly the same layout as
/// its underlying `T`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Distance<T = u32>(T);

impl<T> Distance<T> {
    /// Wraps a raw numeric value as a distance.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric value, consuming the distance.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Returns a reference to the underlying numeric value.
    #[inline]
    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Display> fmt::Display for Distance<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A similarity between two sequences.
///
/// Higher values indicate greater similarity. Unlike [`NormalizedSimilarity`],
/// this type does not guarantee any particular range — the algorithm's
/// documentation is authoritative. Where a bounded `[0.0, 1.0]` similarity
/// is required, use [`NormalizedSimilarity`] together with the appropriate
/// constructor.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Similarity<T = f64>(T);

impl<T> Similarity<T> {
    /// Wraps a raw numeric value as a similarity.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric value, consuming the similarity.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Returns a reference to the underlying numeric value.
    #[inline]
    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Display> fmt::Display for Similarity<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A raw alignment or scoring value that is neither a distance nor a
/// similarity in the canonical sense.
///
/// Used by algorithms such as Smith–Waterman, Needleman–Wunsch, and
/// probabilistic linkage where the numeric value's meaning depends on the
/// scoring scheme rather than on distance-or-similarity semantics.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Score<T = f64>(T);

impl<T> Score<T> {
    /// Wraps a raw numeric value as a score.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric value, consuming the score.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Returns a reference to the underlying numeric value.
    #[inline]
    pub const fn as_inner(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Display> fmt::Display for Score<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A distance normalized to the closed interval `[0.0, 1.0]`.
///
/// A value of `0.0` indicates identity under the applied normalization
/// policy; `1.0` indicates maximal dissimilarity. Construction via
/// [`NormalizedDistance::new`] enforces the range invariant — non-finite
/// values and values outside the interval are rejected.
///
/// The specific policy that produced a value (max-length, sum-length,
/// algorithmic-maximum, custom) is recorded separately via
/// [`crate::normalization::NormalizationPolicy`]; this type itself only
/// guarantees the range, not the interpretation.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NormalizedDistance(f64);

impl NormalizedDistance {
    /// Constructs a normalized distance, returning `None` if the value is
    /// non-finite or outside the closed interval `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use comparand_core::result::NormalizedDistance;
    /// assert!(NormalizedDistance::new(0.0).is_some());
    /// assert!(NormalizedDistance::new(1.0).is_some());
    /// assert!(NormalizedDistance::new(0.5).is_some());
    /// assert!(NormalizedDistance::new(-0.1).is_none());
    /// assert!(NormalizedDistance::new(1.1).is_none());
    /// assert!(NormalizedDistance::new(f64::NAN).is_none());
    /// assert!(NormalizedDistance::new(f64::INFINITY).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Constructs a normalized distance without checking the range invariant.
    ///
    /// The invariant is not memory-safety-related, so this constructor is
    /// safe. Callers that violate the invariant will produce nonsensical
    /// results downstream (in indexes, thresholds, and reports) but will not
    /// trigger undefined behavior.
    #[inline]
    #[must_use]
    pub const fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    /// Returns the underlying floating-point value.
    #[inline]
    #[must_use]
    pub const fn into_inner(self) -> f64 {
        self.0
    }

    /// Returns the underlying floating-point value.
    #[inline]
    #[must_use]
    pub const fn as_inner(&self) -> &f64 {
        &self.0
    }

    /// The identity distance (`0.0`).
    pub const IDENTITY: Self = Self(0.0);

    /// The maximum distance (`1.0`).
    pub const MAXIMUM: Self = Self(1.0);
}

impl fmt::Display for NormalizedDistance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A similarity normalized to the closed interval `[0.0, 1.0]`.
///
/// A value of `1.0` indicates identity; `0.0` indicates maximal
/// dissimilarity. Construction via [`NormalizedSimilarity::new`] enforces
/// the range invariant.
///
/// Comparand does not automatically convert between [`NormalizedDistance`]
/// and `NormalizedSimilarity`. The `1 - x` identity holds only when both
/// values were produced under the same normalization policy for the same
/// pair of inputs — a property the type system cannot verify.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NormalizedSimilarity(f64);

impl NormalizedSimilarity {
    /// Constructs a normalized similarity, returning `None` if the value is
    /// non-finite or outside the closed interval `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use comparand_core::result::NormalizedSimilarity;
    /// assert!(NormalizedSimilarity::new(0.0).is_some());
    /// assert!(NormalizedSimilarity::new(1.0).is_some());
    /// assert!(NormalizedSimilarity::new(1.0001).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Constructs a normalized similarity without checking the range
    /// invariant. See [`NormalizedDistance::new_unchecked`] for the safety
    /// caveat.
    #[inline]
    #[must_use]
    pub const fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    /// Returns the underlying floating-point value.
    #[inline]
    #[must_use]
    pub const fn into_inner(self) -> f64 {
        self.0
    }

    /// Returns the underlying floating-point value.
    #[inline]
    #[must_use]
    pub const fn as_inner(&self) -> &f64 {
        &self.0
    }

    /// The minimum similarity (`0.0`).
    pub const MINIMUM: Self = Self(0.0);

    /// The identity similarity (`1.0`).
    pub const IDENTITY: Self = Self(1.0);
}

impl fmt::Display for NormalizedSimilarity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The result of a distance computation performed with a maximum-distance
/// cutoff.
///
/// Cutoff-aware algorithms may terminate as soon as they can prove the true
/// distance exceeds the requested bound. In that case the exact distance is
/// not computed — the caller only learns that it is strictly greater than
/// the cutoff, along with the cutoff value that was exceeded.
///
/// This is a distinct type from [`Distance`] so callers cannot accidentally
/// treat an "exceeded cutoff" result as if it were a true distance of the
/// cutoff value. It is also distinct from `Option<Distance<T>>` because the
/// two carry different meanings — `None` would suggest an absent result, but
/// here the result is present and specific: the distance exceeds a known
/// bound.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundedDistance<T = u32> {
    /// The distance was computed exactly and lies within (less than or equal
    /// to) the requested cutoff.
    Within(Distance<T>),
    /// The true distance is strictly greater than the requested cutoff. The
    /// cutoff value that was exceeded is carried for diagnostic purposes.
    Exceeded {
        /// The cutoff value the true distance exceeded.
        cutoff: T,
    },
}

impl<T: Copy> BoundedDistance<T> {
    /// Returns the exact distance if within the cutoff, otherwise `None`.
    #[inline]
    #[must_use]
    pub fn within(self) -> Option<Distance<T>> {
        match self {
            Self::Within(d) => Some(d),
            Self::Exceeded { .. } => None,
        }
    }

    /// Returns `true` if the computation terminated early because the true
    /// distance exceeds the requested cutoff.
    #[inline]
    #[must_use]
    pub const fn is_exceeded(&self) -> bool {
        matches!(self, Self::Exceeded { .. })
    }

    /// Returns `true` if the computation produced an exact distance within
    /// the cutoff.
    #[inline]
    #[must_use]
    pub const fn is_within(&self) -> bool {
        matches!(self, Self::Within(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_transparency() {
        let d: Distance<u32> = Distance::new(7);
        assert_eq!(d.into_inner(), 7);
        assert_eq!(*Distance::new(3u32).as_inner(), 3);
    }

    #[test]
    fn normalized_distance_rejects_out_of_range() {
        assert!(NormalizedDistance::new(0.0).is_some());
        assert!(NormalizedDistance::new(1.0).is_some());
        assert!(NormalizedDistance::new(0.5).is_some());
        assert!(NormalizedDistance::new(-0.0).is_some()); // -0.0 is in range
        assert!(NormalizedDistance::new(-0.000_001).is_none());
        assert!(NormalizedDistance::new(1.000_001).is_none());
        assert!(NormalizedDistance::new(f64::NAN).is_none());
        assert!(NormalizedDistance::new(f64::INFINITY).is_none());
        assert!(NormalizedDistance::new(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn normalized_distance_constants() {
        // Compare bit patterns to sidestep the pedantic float-equality lint —
        // the constants are exact, so exact-bits comparison is what we want.
        assert_eq!(
            NormalizedDistance::IDENTITY.into_inner().to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            NormalizedDistance::MAXIMUM.into_inner().to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn bounded_distance_states() {
        let within = BoundedDistance::Within(Distance::<u32>::new(2));
        assert!(within.is_within());
        assert!(!within.is_exceeded());
        assert_eq!(within.within(), Some(Distance::new(2)));

        let exceeded: BoundedDistance<u32> = BoundedDistance::Exceeded { cutoff: 5 };
        assert!(exceeded.is_exceeded());
        assert!(!exceeded.is_within());
        assert_eq!(exceeded.within(), None);
    }
}
