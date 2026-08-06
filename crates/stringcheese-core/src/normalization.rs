//! Explicit policies for converting raw distances into a bounded range.
//!
//! A raw Levenshtein distance of `3` means one thing for `"cat"` /
//! `"cot"` and something else entirely for `"encyclopedia"` /
//! `"encyclopædia"`. Turning a raw distance into a value in `[0, 1]`
//! therefore requires a choice — one this crate refuses to make silently.
//!
//! This module defines the *policy* only. Applying a policy to convert a
//! [`crate::result::Distance`] into a [`crate::result::NormalizedDistance`]
//! is the responsibility of algorithm-specific code, because different
//! algorithms have different natural upper bounds (`max(len)`,
//! `len_a + len_b`, `c * max(len)` for a scoring scheme with per-cell
//! maximum cost `c`, and so on).

/// A convention for converting a raw distance into a value in `[0.0, 1.0]`.
///
/// This enum is `#[non_exhaustive]`; new policies may be added in
/// backwards-compatible releases.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NormalizationPolicy {
    /// Divide the raw distance by `max(len_left, len_right)`. The most
    /// common convention for unit-cost edit distances.
    ByMaxLength,
    /// Divide by `len_left + len_right`. Yields values in `[0, 1]` for
    /// unit-cost edit distances by a different route than `ByMaxLength`,
    /// with different behavior when the two inputs have very different
    /// lengths.
    BySumLength,
    /// Divide by an algorithm-specific theoretical upper bound. For
    /// unit-cost Levenshtein this coincides with [`Self::ByMaxLength`]; for
    /// a scoring scheme with per-cell maximum cost `c` this would be
    /// `c * max(len)`.
    ByAlgorithmicMaximum,
    /// A caller-defined normalizing constant. The caller is responsible for
    /// ensuring the constant is greater than or equal to any raw distance
    /// the inputs in question could produce.
    Custom,
}
