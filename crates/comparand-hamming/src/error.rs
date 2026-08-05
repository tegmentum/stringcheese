//! Error type for the fallible Hamming entry points.
//!
//! The [`LengthMismatch`] type is the only error Hamming can raise on
//! well-formed inputs. It carries both operand lengths so a diagnostic can
//! print them without a second pass over the inputs.

use core::fmt;

/// The two input sequences to a Hamming comparison had different lengths.
///
/// Hamming distance is only defined for equal-length sequences. The
/// [`Hamming::try_distance`] and [`Hamming::try_distance_within`] entry points
/// return this error rather than silently redefining the algorithm.
///
/// The type is `Copy` and holds only its two lengths; it does not borrow the
/// original inputs, so it can safely outlive them.
///
/// # `std` interaction
///
/// A [`std::error::Error`] implementation is available only when the crate is
/// built with the `std` feature. Under `--no-default-features` this type
/// still exists and still implements [`Display`], but not `Error` — because
/// the `Error` trait itself lives in `std`.
///
/// [`Hamming::try_distance`]: crate::algorithm::Hamming::try_distance
/// [`Hamming::try_distance_within`]: crate::algorithm::Hamming::try_distance_within
/// [`Display`]: core::fmt::Display
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LengthMismatch {
    /// The length of the left operand.
    pub left: usize,
    /// The length of the right operand.
    pub right: usize,
}

impl fmt::Display for LengthMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Hamming distance is undefined for unequal-length inputs: left has {left} items, right has {right}",
            left = self.left,
            right = self.right,
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LengthMismatch {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_both_lengths() {
        let e = LengthMismatch { left: 3, right: 5 };
        let msg = std::format!("{e}");
        assert!(msg.contains('3'), "display should include the left length: {msg}");
        assert!(msg.contains('5'), "display should include the right length: {msg}");
    }

    #[test]
    fn equality_and_hash_are_structural() {
        let a = LengthMismatch { left: 1, right: 2 };
        let b = LengthMismatch { left: 1, right: 2 };
        let c = LengthMismatch { left: 2, right: 1 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[cfg(feature = "std")]
    #[test]
    fn implements_std_error_under_std() {
        // Statically confirm the trait bound holds under `std`.
        fn assert_error<E: std::error::Error>(_: &E) {}
        let e = LengthMismatch { left: 0, right: 1 };
        assert_error(&e);
    }
}
