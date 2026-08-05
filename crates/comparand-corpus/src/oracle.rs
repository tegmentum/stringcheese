//! The [`Oracle`] contract and differential-testing driver.
//!
//! An *oracle* is a slow, straightforward, deliberately unoptimized
//! implementation whose only responsibility is to be correct — it is the
//! reference every optimized implementation is checked against. An oracle
//! prioritizes clarity over performance to the point that, if the algorithm
//! is simple enough, the oracle could plausibly be executed on paper.
//!
//! Oracles are deliberately distinct from [`comparand_core::DistanceMetric`]
//! and related traits. A `DistanceMetric` participates in the library's
//! runtime API — it may be an optimized bit-parallel implementation, a
//! cutoff-aware banded variant, or a SIMD backend. An [`Oracle`] is a
//! validation artifact: it never ships in production paths, it never claims
//! to be efficient, and it is the ground truth against which the runtime
//! implementations are exercised.
//!
//! The [`differential_check`] and [`differential_check_bounded`] helpers
//! sweep a candidate implementation over a sequence of inputs and collect
//! every disagreement with the oracle. Combined with
//! [`crate::generator`], this gives complete coverage over small domains.

use comparand_core::AlgorithmDescriptor;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// A reference implementation of an algorithm, written for correctness
/// rather than speed.
///
/// An `Oracle` is the ground-truth answer for its declared
/// [`AlgorithmDescriptor`]. Optimized implementations of the same variant
/// must agree with the oracle on every input — this is the contract that
/// [`differential_check`] enforces.
///
/// Oracles are held to a stricter simplicity standard than production
/// implementations. Where a production implementation may use bit-parallel
/// tricks, hand-rolled SIMD, or cutoff-aware banding, the oracle typically
/// implements the algorithm's textbook recurrence directly — a form so
/// simple it could be executed on paper for small inputs. Any bug that
/// hides in the runtime implementation is unlikely to also be present in
/// an implementation this direct.
///
/// # Contract
///
/// Implementations must be pure: repeated calls with equal inputs must
/// return equal outputs and must have no observable side effects. This is
/// what makes the oracle usable as a reference.
pub trait Oracle<Input: ?Sized, Output> {
    /// Computes the reference output for `input`.
    ///
    /// Performance is not a concern here; correctness is. The result must
    /// depend only on `input` and the oracle's declared descriptor.
    fn compute(&self, input: &Input) -> Output;

    /// Returns the algorithm descriptor this oracle implements.
    ///
    /// Two oracles with equal descriptors must produce equal outputs for
    /// equal inputs. This is what makes the descriptor a meaningful
    /// identity — it pins down *which* algorithm and *which* variant of it
    /// the oracle is a reference for.
    fn descriptor(&self) -> AlgorithmDescriptor;
}

/// A single disagreement between an oracle and a candidate implementation.
///
/// The struct carries enough context to reproduce the disagreement and to
/// classify it downstream — the offending input, both outputs, and the
/// descriptors of both implementations.
#[derive(Debug, Clone)]
pub struct Disagreement<Input, Output> {
    /// The input on which the oracle and candidate disagreed.
    pub input: Input,
    /// The oracle's output — treated as ground truth.
    pub oracle_output: Output,
    /// The candidate implementation's output.
    pub candidate_output: Output,
    /// The oracle's algorithm descriptor.
    pub oracle_descriptor: AlgorithmDescriptor,
    /// The candidate implementation's algorithm descriptor. Provided by
    /// the caller because a candidate expressed as a bare closure carries
    /// no descriptor of its own.
    pub candidate_descriptor: AlgorithmDescriptor,
}

/// Runs `candidate` against every input in `inputs` and returns every
/// disagreement with the oracle.
///
/// The candidate is passed as a closure because callers routinely want to
/// validate a not-yet-crate-worthy variant, a scalar/SIMD switch, or an
/// inline optimization. Because a bare closure has no descriptor of its
/// own, the candidate's descriptor is supplied separately.
///
/// This function traverses the entire input iterator. For enormous input
/// spaces, prefer [`differential_check_bounded`] to bail out after a fixed
/// number of failures.
///
/// # Type parameters
///
/// * `I` — input type; must be `Clone` so a disagreement can carry the
///   offending input by value for reproduction.
/// * `O` — output type; compared for equality via [`PartialEq`].
/// * `Or` — oracle type.
/// * `Cand` — candidate closure type.
/// * `Inputs` — the input iterable.
#[cfg(feature = "alloc")]
pub fn differential_check<I, O, Or, Cand, Inputs>(
    oracle: &Or,
    candidate: Cand,
    candidate_descriptor: AlgorithmDescriptor,
    inputs: Inputs,
) -> Vec<Disagreement<I, O>>
where
    Or: Oracle<I, O>,
    Cand: Fn(&I) -> O,
    Inputs: IntoIterator<Item = I>,
    I: Clone,
    O: PartialEq + Clone,
{
    differential_check_bounded(oracle, candidate, candidate_descriptor, inputs, usize::MAX)
}

/// Like [`differential_check`] but stops after `max_disagreements`
/// disagreements have been collected.
///
/// Useful when the input space is astronomically large — a single systemic
/// bug can otherwise fill memory with essentially the same failure repeated
/// millions of times. `max_disagreements` of `0` returns an empty vector
/// without consulting the candidate.
#[cfg(feature = "alloc")]
pub fn differential_check_bounded<I, O, Or, Cand, Inputs>(
    oracle: &Or,
    candidate: Cand,
    candidate_descriptor: AlgorithmDescriptor,
    inputs: Inputs,
    max_disagreements: usize,
) -> Vec<Disagreement<I, O>>
where
    Or: Oracle<I, O>,
    Cand: Fn(&I) -> O,
    Inputs: IntoIterator<Item = I>,
    I: Clone,
    O: PartialEq + Clone,
{
    let mut disagreements: Vec<Disagreement<I, O>> = Vec::new();
    if max_disagreements == 0 {
        return disagreements;
    }
    let oracle_descriptor = oracle.descriptor();
    for input in inputs {
        let oracle_output = oracle.compute(&input);
        let candidate_output = candidate(&input);
        if oracle_output != candidate_output {
            disagreements.push(Disagreement {
                input: input.clone(),
                oracle_output,
                candidate_output,
                oracle_descriptor,
                candidate_descriptor,
            });
            if disagreements.len() >= max_disagreements {
                break;
            }
        }
    }
    disagreements
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;
    use comparand_core::{AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId};

    const TEST_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Levenshtein,
        VariantId("test-variant"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::IndependentlyDerived,
    );

    const CANDIDATE_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Levenshtein,
        VariantId("test-candidate"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::IndependentlyDerived,
    );

    /// An oracle that always returns zero — deliberately trivial so its
    /// correctness is unmistakable in the test.
    struct AlwaysZero;

    impl Oracle<u32, u32> for AlwaysZero {
        fn compute(&self, _input: &u32) -> u32 {
            0
        }

        fn descriptor(&self) -> AlgorithmDescriptor {
            TEST_DESCRIPTOR
        }
    }

    #[test]
    fn differential_check_reports_every_disagreement() {
        let oracle = AlwaysZero;
        let inputs = vec![1_u32, 2, 3, 4, 5];
        let disagreements =
            differential_check(&oracle, |_input: &u32| 1_u32, CANDIDATE_DESCRIPTOR, inputs);
        assert_eq!(disagreements.len(), 5);
        for d in &disagreements {
            assert_eq!(d.oracle_output, 0);
            assert_eq!(d.candidate_output, 1);
            assert_eq!(d.oracle_descriptor, TEST_DESCRIPTOR);
            assert_eq!(d.candidate_descriptor, CANDIDATE_DESCRIPTOR);
        }
    }

    #[test]
    fn differential_check_bounded_stops_at_cap() {
        let oracle = AlwaysZero;
        let inputs = vec![1_u32, 2, 3, 4, 5];
        let disagreements = differential_check_bounded(
            &oracle,
            |_input: &u32| 1_u32,
            CANDIDATE_DESCRIPTOR,
            inputs,
            2,
        );
        assert_eq!(disagreements.len(), 2);
    }

    #[test]
    fn differential_check_reports_nothing_when_candidate_agrees() {
        let oracle = AlwaysZero;
        let inputs = vec![1_u32, 2, 3];
        let disagreements =
            differential_check(&oracle, |_input: &u32| 0_u32, CANDIDATE_DESCRIPTOR, inputs);
        assert!(disagreements.is_empty());
    }

    #[test]
    fn differential_check_bounded_with_zero_cap_returns_empty() {
        let oracle = AlwaysZero;
        let inputs = vec![1_u32, 2, 3];
        let disagreements = differential_check_bounded(
            &oracle,
            |_input: &u32| 1_u32,
            CANDIDATE_DESCRIPTOR,
            inputs,
            0,
        );
        assert!(disagreements.is_empty());
    }
}
