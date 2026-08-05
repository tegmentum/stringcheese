//! Schema, oracles, and generators for Comparand's golden-case validation
//! corpus.
//!
//! A *golden case* pairs an input with an expected output for a specific
//! algorithm variant, along with the case's provenance. The corpus is
//! consumed by Comparand's own test suites; because it is a separately
//! versioned deliverable, it is also intended to be usable by other
//! sequence-comparison libraries to check their outputs against ours.
//!
//! # Subsystems
//!
//! This crate is organized into three cooperating subsystems, each with
//! its own module:
//!
//! * [`GoldenCase`] and friends (at the crate root) define the schema for
//!   hand-authored golden cases and the [`FloatExpectation`] comparison
//!   policy every floating-point expected value must declare.
//! * [`oracle`] defines the [`Oracle`] contract — a slow, straightforward,
//!   deliberately unoptimized reference implementation — and the
//!   [`differential_check`] / [`differential_check_bounded`] drivers that
//!   sweep a candidate implementation against an oracle over a stream of
//!   inputs.
//! * [`generator`] provides [`exhaustive_over_alphabet`] and
//!   [`exhaustive_pairs`]: the exhaustive small-alphabet enumeration that
//!   feeds those differential sweeps. [`count_sequences`] gives the
//!   closed-form size of the enumeration up front.
//! * [`differential`] provides [`DifferentialResult`] and
//!   [`DifferenceClassification`] — the classification vocabulary the
//!   design document commits to for triaging observed disagreements.
//!
//! # Layers
//!
//! This crate defines only the **schema types** and the **validation
//! infrastructure**. The actual corpus data — canonical examples,
//! exhaustive-oracle outputs, phonetic multilingual sets — is added
//! incrementally under `crates/comparand-corpus/data/` as algorithms come
//! online.
//!
//! # Sources
//!
//! Every golden case must record *how* its expected output was determined
//! (see [`GoldenSource`]). This lets a discrepancy investigation
//! distinguish "we disagree with a paper" from "we disagree with an
//! implementation we chose to imitate" from "we disagree with the output
//! of our own reference oracle."
//!
//! # Floating-point
//!
//! Floating-point expected values require an explicit comparison policy
//! (see [`FloatExpectation`]). Comparand rejects the practice of comparing
//! doubles for exact equality by default — every floating-point golden
//! case must declare its tolerance regime.
//!
//! # `no_std`
//!
//! The schema types, the [`Oracle`] trait, and [`DifferentialResult`] all
//! compile without `alloc`. The [`generator`] module and the
//! [`differential_check`] driver require the `alloc` feature because they
//! return `Vec`s.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

pub mod differential;
pub mod oracle;

#[cfg(feature = "alloc")]
pub mod generator;

pub use differential::{DifferenceClassification, DifferentialResult};
pub use oracle::{Disagreement, Oracle};

#[cfg(feature = "alloc")]
pub use generator::{
    ExhaustiveIter, count_sequences, exhaustive_over_alphabet, exhaustive_pairs,
};
#[cfg(feature = "alloc")]
pub use oracle::{differential_check, differential_check_bounded};

use comparand_core::AlgorithmDescriptor;

/// A single golden test case: paired input, expected output, and
/// provenance.
///
/// `I` is the input type (typically a pair of sequences); `O` is the
/// expected output type (usually a specific result type from
/// [`comparand_core::result`]).
#[derive(Debug, Clone)]
pub struct GoldenCase<I, O> {
    /// A stable identifier for the case, hierarchically named for
    /// organization — for example, `"levenshtein/basic/kitten-sitting"`.
    pub id: &'static str,
    /// The algorithm variant this case validates. A case that references
    /// a different variant than the one under test is a schema error, not
    /// a silent mismatch.
    pub descriptor: AlgorithmDescriptor,
    /// The paired input to the algorithm.
    pub input: I,
    /// The expected output.
    pub expected: O,
    /// Where this case's expected output was obtained from.
    pub source: GoldenSource,
    /// Human-readable notes about what the case exercises.
    pub notes: &'static str,
    /// Free-form tags for filtering test runs — for example
    /// `["basic", "insertion", "substitution"]`.
    pub tags: &'static [&'static str],
}

/// How a golden case's expected output was determined.
///
/// The classification is used when a discrepancy is investigated: a
/// disagreement with a `PublishedPaper` case is much stronger evidence of a
/// Comparand defect than a disagreement with a `ReferenceImplementation`
/// case, which may itself carry the bug.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GoldenSource {
    /// The expected output was published in an academic paper.
    PublishedPaper {
        /// Full citation suitable for a bibliography entry.
        citation: &'static str,
    },
    /// The expected output comes from a formal standard.
    Standard {
        /// The standard's canonical name.
        name: &'static str,
    },
    /// The expected output was produced by an independent reference
    /// implementation and manually verified.
    ReferenceImplementation {
        /// Name and, ideally, version of the reference implementation.
        name: &'static str,
    },
    /// The expected output was produced by an exhaustive oracle
    /// implementation over a small input domain.
    ExhaustiveOracle,
    /// The expected output was derived by hand from the algorithm's
    /// definition, without consulting another implementation.
    IndependentlyDerived,
    /// A regression case captured from a previously discovered bug.
    Regression {
        /// Issue identifier or bug-tracker link.
        issue: &'static str,
    },
}

/// The comparison policy for a floating-point expected value.
///
/// Floating-point algorithms rarely admit exact bitwise agreement across
/// platforms — different rounding modes, different intermediate precisions,
/// and different `libm` implementations all matter. Every golden case that
/// carries a floating-point expected value must specify how the observed
/// value should be compared to it.
///
/// Use [`FloatExpectation::matches`] to evaluate the policy against an
/// observed value.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FloatExpectation {
    /// The observed value must equal the expected value bit-for-bit
    /// (encoded as an IEEE 754 `u64` bit pattern).
    ExactBits {
        /// The expected `f64` encoded as its raw `u64` bit pattern.
        value: u64,
    },
    /// `|observed - value| <= tolerance` (`NaN` never matches).
    Absolute {
        /// The expected value.
        value: f64,
        /// The maximum absolute deviation permitted.
        tolerance: f64,
    },
    /// `|observed - value| / |value| <= tolerance`, or exact equality when
    /// `value == 0.0` (in which case any nonzero deviation fails).
    Relative {
        /// The expected value.
        value: f64,
        /// The maximum relative deviation permitted.
        tolerance: f64,
    },
    /// `observed` and `value` are within `maximum_ulps` units in the last
    /// place. Values with different signs never satisfy the ULP policy
    /// unless they are both zero.
    Ulps {
        /// The expected value.
        value: f64,
        /// The maximum ULP distance permitted.
        maximum_ulps: u32,
    },
}

impl FloatExpectation {
    /// Returns `true` if `observed` satisfies the expectation.
    #[must_use]
    pub fn matches(self, observed: f64) -> bool {
        match self {
            Self::ExactBits { value } => observed.to_bits() == value,
            Self::Absolute { value, tolerance } => {
                if observed.is_nan() || value.is_nan() {
                    false
                } else {
                    (observed - value).abs() <= tolerance
                }
            }
            Self::Relative { value, tolerance } => {
                if observed.is_nan() || value.is_nan() {
                    false
                } else if value == 0.0 {
                    observed == 0.0
                } else {
                    (observed - value).abs() / value.abs() <= tolerance
                }
            }
            Self::Ulps {
                value,
                maximum_ulps,
            } => ulps_between(observed, value) <= u64::from(maximum_ulps),
        }
    }
}

/// Computes the ULP distance between two `f64` values.
///
/// Returns `u64::MAX` for `NaN` inputs and for values with different signs
/// (except when both are zero). Assumes IEEE 754 `binary64`.
#[allow(
    clippy::float_cmp,
    reason = "IEEE 754 equality is exactly what we want here: it treats +0.0 and -0.0 as equal (correct: zero ULPs apart) and rejects any pair of finite floats whose bit patterns differ (correct: nonzero ULPs)."
)]
fn ulps_between(a: f64, b: f64) -> u64 {
    if a.is_nan() || b.is_nan() {
        return u64::MAX;
    }
    if a == b {
        // Handles the `+0.0 == -0.0` case correctly — no ULPs apart.
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_expectation() {
        let e = FloatExpectation::Absolute {
            value: 1.0,
            tolerance: 0.01,
        };
        assert!(e.matches(1.0));
        assert!(e.matches(1.005));
        assert!(e.matches(0.995));
        assert!(!e.matches(1.02));
        assert!(!e.matches(f64::NAN));
    }

    #[test]
    fn relative_expectation() {
        let e = FloatExpectation::Relative {
            value: 100.0,
            tolerance: 0.001,
        };
        assert!(e.matches(100.0));
        assert!(e.matches(100.05));
        assert!(e.matches(99.95));
        assert!(!e.matches(101.0));
    }

    #[test]
    fn relative_at_zero_requires_exact() {
        let e = FloatExpectation::Relative {
            value: 0.0,
            tolerance: 0.5,
        };
        assert!(e.matches(0.0));
        assert!(e.matches(-0.0));
        assert!(!e.matches(0.0001));
    }

    #[test]
    fn exact_bits_expectation() {
        let e = FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        };
        assert!(e.matches(1.0));
        assert!(!e.matches(1.0 + f64::EPSILON));
    }

    #[test]
    fn ulps_expectation() {
        let e = FloatExpectation::Ulps {
            value: 1.0,
            maximum_ulps: 1,
        };
        assert!(e.matches(1.0));
        // The float immediately above 1.0 is exactly 1 ULP away.
        let next = f64::from_bits(1.0_f64.to_bits() + 1);
        assert!(e.matches(next));
        // Two ULPs away should fail.
        let two_above = f64::from_bits(1.0_f64.to_bits() + 2);
        assert!(!e.matches(two_above));
    }

    #[test]
    fn ulps_across_zero_uses_max() {
        // +0.0 and -0.0 compare equal, so ULP distance is zero.
        let e = FloatExpectation::Ulps {
            value: 0.0,
            maximum_ulps: 0,
        };
        assert!(e.matches(-0.0));
        // But +1e-300 vs -1e-300 have opposite signs — should never match.
        let e = FloatExpectation::Ulps {
            value: 1e-300,
            maximum_ulps: u32::MAX,
        };
        assert!(!e.matches(-1e-300));
    }

    #[test]
    fn ulps_nan_never_matches() {
        let e = FloatExpectation::Ulps {
            value: f64::NAN,
            maximum_ulps: u32::MAX,
        };
        assert!(!e.matches(f64::NAN));
        assert!(!e.matches(0.0));
    }
}
