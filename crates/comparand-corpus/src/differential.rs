//! Classification vocabulary for differential-testing outcomes.
//!
//! Differential testing produces a stream of observations of the form
//! "implementation A said X, implementation B said Y for the same input."
//! Raw observations are useless without a classification: is this a bug we
//! should file, a genuine algorithmic-variant disagreement we should
//! document, or a floating-point rounding artifact well within tolerance?
//!
//! The [`DifferenceClassification`] enum enumerates the vocabulary the
//! Comparand design document commits to for this classification. Every
//! disagreement surfaced by the differential-testing harness must be
//! labelled with one of these variants before a release is cut — otherwise
//! the disagreement is unresolved, and the release is blocked.

use comparand_core::AlgorithmDescriptor;

/// A single differential-testing observation, classified.
///
/// This is the record produced after a differential-testing sweep has been
/// triaged: the disagreement itself, the algorithm it concerned, the
/// external implementation that was consulted, and the reviewer's
/// determination of what the disagreement means.
///
/// `case_id` and `implementation` are `&'static str` — deliberately so —
/// so that this type can be constructed and stored in `no_std`, no-`alloc`
/// contexts. Callers assembling these values dynamically (for example, out
/// of a JSON test manifest) should intern the strings into a static string
/// pool rather than borrowing owned `String`s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DifferentialResult {
    /// The algorithm variant the comparison was made against.
    pub algorithm: AlgorithmDescriptor,
    /// A stable identifier for the golden case or generated input that
    /// produced this observation. Same shape as [`crate::GoldenCase::id`]
    /// — a hierarchical slug such as
    /// `"levenshtein/basic/kitten-sitting"`.
    pub case_id: &'static str,
    /// A stable name for the external or internal implementation whose
    /// output was compared against Comparand's own. For example
    /// `"jellyfish-1.0.3"` or `"rapidfuzz-3.9.0"`.
    pub implementation: &'static str,
    /// The reviewer's classification of the disagreement.
    pub classification: DifferenceClassification,
}

/// The classification of a differential-testing disagreement.
///
/// This is the vocabulary the design document commits to for triaging
/// disagreements. Every field has an explicit meaning; when a disagreement
/// does not obviously fit any listed variant, that itself is a signal —
/// either the classification vocabulary needs an addition (this enum is
/// `#[non_exhaustive]`) or the disagreement is not actually understood.
///
/// # Design intent
///
/// Only the [`Agreement`] variant is a "no action needed" outcome. Every
/// other variant carries a specific consequence:
///
/// * [`ComparandDefect`] and [`ExternalImplementationBug`] point at fixable
///   bugs — in Comparand or upstream, respectively.
/// * [`DifferentAlgorithmVariant`], [`DifferentNormalization`], and
///   [`DifferentRepresentation`] mean the two implementations are answering
///   subtly different questions. The disagreement should be documented,
///   and the golden case should be re-tagged against a specific variant.
/// * [`FloatingPointTolerance`] means the disagreement is within the
///   variant's declared floating-point comparison policy — expected.
/// * [`UndefinedBehavior`] means the input drove one implementation into
///   territory neither definition specifies; a specification patch is
///   needed.
/// * [`SpecificationAmbiguity`] means the source paper or standard admits
///   both answers as valid; escalate to a variant split.
///
/// [`Agreement`]: DifferenceClassification::Agreement
/// [`ComparandDefect`]: DifferenceClassification::ComparandDefect
/// [`ExternalImplementationBug`]: DifferenceClassification::ExternalImplementationBug
/// [`DifferentAlgorithmVariant`]: DifferenceClassification::DifferentAlgorithmVariant
/// [`DifferentNormalization`]: DifferenceClassification::DifferentNormalization
/// [`DifferentRepresentation`]: DifferenceClassification::DifferentRepresentation
/// [`FloatingPointTolerance`]: DifferenceClassification::FloatingPointTolerance
/// [`UndefinedBehavior`]: DifferenceClassification::UndefinedBehavior
/// [`SpecificationAmbiguity`]: DifferenceClassification::SpecificationAmbiguity
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DifferenceClassification {
    /// No disagreement. The two implementations produced equal output for
    /// the input. Present in the vocabulary so that the same enum can
    /// summarize an entire sweep without a second "was there a
    /// disagreement at all" boolean.
    Agreement,
    /// Comparand's implementation was wrong; a bug should be filed and
    /// the golden case retained as a permanent regression.
    ComparandDefect,
    /// The external implementation was wrong; the disagreement is
    /// evidence for the upstream project, not for Comparand. Documented
    /// so that future comparative benchmarks do not repeatedly re-report
    /// the same known bug.
    ExternalImplementationBug,
    /// The two implementations follow different variants of the same
    /// algorithm family (for example restricted vs. unrestricted
    /// Damerau-Levenshtein). Both are correct within their own definition;
    /// the golden case should be tagged against a specific
    /// [`AlgorithmDescriptor`] so the disagreement is not re-flagged.
    DifferentAlgorithmVariant,
    /// The two implementations apply different normalization policies
    /// (for example divide-by-max-length vs. divide-by-sum-length for
    /// Levenshtein). The values disagree in magnitude even though the
    /// underlying raw distance is the same.
    DifferentNormalization,
    /// The two implementations operate on different representations of the
    /// same input — bytes vs. Unicode scalars vs. grapheme clusters. The
    /// disagreement disappears if the representations are aligned.
    DifferentRepresentation,
    /// The two floating-point outputs differ, but by less than the
    /// variant's declared tolerance
    /// (see [`crate::FloatExpectation`]). Not a bug — expected.
    FloatingPointTolerance,
    /// The input drove at least one implementation into behavior neither
    /// the algorithm's definition nor the implementations' documentation
    /// specifies. A specification patch is required before the
    /// classification can be revisited.
    UndefinedBehavior,
    /// The algorithm's source paper or standard admits both outputs as
    /// valid. The right response is to split the shared name into two
    /// distinct [`AlgorithmDescriptor`] variants, one per interpretation.
    SpecificationAmbiguity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use comparand_core::{
        AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
    };

    const D: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Levenshtein,
        VariantId("unit-cost-unicode-scalars"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::IndependentlyDerived,
    );

    #[test]
    fn differential_result_is_constructible_in_const_context() {
        // Whole struct isn't const-constructible today (AlgorithmDescriptor
        // is, but the enum discriminant carries no data). This just checks
        // ordinary constructibility.
        let r = DifferentialResult {
            algorithm: D,
            case_id: "levenshtein/basic/example",
            implementation: "test-impl-1.0",
            classification: DifferenceClassification::Agreement,
        };
        assert_eq!(r.classification, DifferenceClassification::Agreement);
        assert_eq!(r.case_id, "levenshtein/basic/example");
    }

    #[test]
    fn classification_variants_are_distinct() {
        // A quick smoke test: none of the variants collapse into another.
        assert_ne!(
            DifferenceClassification::ComparandDefect,
            DifferenceClassification::ExternalImplementationBug,
        );
        assert_ne!(
            DifferenceClassification::DifferentAlgorithmVariant,
            DifferenceClassification::DifferentNormalization,
        );
        assert_ne!(
            DifferenceClassification::FloatingPointTolerance,
            DifferenceClassification::UndefinedBehavior,
        );
    }
}
