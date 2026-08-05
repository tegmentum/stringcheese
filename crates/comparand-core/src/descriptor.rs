//! Stable identification of algorithm variants and their published sources.
//!
//! Many algorithm names in the field cover multiple incompatible
//! definitions. "Damerau–Levenshtein" is used for at least two distinct
//! distances; "Jaro–Winkler" has several matching-window conventions;
//! "Soundex" has several regional and refined variants; and so on.
//!
//! Every implementation in Comparand carries an [`AlgorithmDescriptor`]
//! that pins down which specific definition it implements, when the
//! definition was published, and where it can be looked up. Golden test
//! cases reference variants by descriptor rather than by common name — so a
//! "Damerau-Levenshtein" case cannot silently be validated against the
//! wrong variant.

use core::fmt;

/// A stable identifier for a specific algorithm implementation and variant.
///
/// Two implementations may share a [`AlgorithmFamily`] but differ in
/// [`VariantId`]; two implementations with matching family and variant are
/// expected to produce identical output for every input.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AlgorithmDescriptor {
    /// The algorithm family the implementation belongs to.
    pub family: AlgorithmFamily,
    /// A stable variant identifier within the family.
    pub variant: VariantId,
    /// The descriptor's version. Behavior-preserving implementation changes
    /// bump the patch number; behavior-changing changes bump minor or major.
    pub version: DescriptorVersion,
    /// The source that defines this variant's semantics.
    pub source: DefinitionSource,
}

impl AlgorithmDescriptor {
    /// Constructs an algorithm descriptor.
    #[inline]
    #[must_use]
    pub const fn new(
        family: AlgorithmFamily,
        variant: VariantId,
        version: DescriptorVersion,
        source: DefinitionSource,
    ) -> Self {
        Self {
            family,
            variant,
            version,
            source,
        }
    }
}

/// The broad family of algorithms an implementation belongs to.
///
/// Variants within a family compute the same conceptual quantity but may
/// disagree on edge cases, normalization, or definitional choices. Two
/// implementations in different families are not interchangeable.
///
/// This enum is `#[non_exhaustive]`; new families may be added in
/// backwards-compatible releases.
#[non_exhaustive]
#[allow(
    missing_docs,
    reason = "variants are named after their canonical algorithms; per-variant documentation is deferred until each algorithm's implementation lands"
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AlgorithmFamily {
    // — Edit distance —
    Hamming,
    Levenshtein,
    WeightedLevenshtein,
    DamerauLevenshtein,
    OptimalStringAlignment,
    LongestCommonSubsequence,
    LongestCommonSubstring,

    // — Alignment —
    NeedlemanWunsch,
    SmithWaterman,
    AffineGap,

    // — String similarity —
    Jaro,
    JaroWinkler,

    // — Set / n-gram similarity —
    Dice,
    Jaccard,
    OverlapCoefficient,
    Cosine,
    WeightedJaccard,
    ContainmentSimilarity,

    // — Phonetic —
    Soundex,
    RefinedSoundex,
    Metaphone,
    DoubleMetaphone,
    Nysiis,
    MatchRating,
    Cologne,
    Caverphone,
    DaitchMokotoff,
    BeiderMorse,

    // — Search —
    RabinKarp,
    KnuthMorrisPratt,
    BoyerMoore,
    Horspool,
    TwoWaySearch,
    AhoCorasick,

    // — Fingerprint / chunking —
    RabinFingerprint,
    PolynomialRollingHash,
    Buzhash,
    GearHash,
    RabinCdc,
    FastCdc,
}

/// A stable, human-readable identifier for a variant within an algorithm
/// family.
///
/// Variant identifiers are `&'static str` slugs such as
/// `"unit-cost-unicode-scalars"`, `"restricted"`, `"unrestricted"`, or
/// `"prefix-limit-4"`. They are referenced by golden datasets so that a
/// "Damerau-Levenshtein" case cannot silently be run against the wrong
/// variant.
///
/// Slug conventions:
///
/// * lowercase kebab-case
/// * describes the *distinguishing* choice, not the family name
/// * stable across patch and minor releases
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariantId(pub &'static str);

impl fmt::Display for VariantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A three-component version tag for algorithm descriptors.
///
/// This is not the crate's own semantic version — it identifies the version
/// of the *implementation* of a particular variant, allowing
/// behavior-preserving implementation changes (patch) to be distinguished
/// from behavior-changing ones (minor or major).
///
/// * `major` — a change in the algorithm's observable output for at least
///   one input the previous version handled.
/// * `minor` — a widening of the algorithm's accepted inputs, or a new
///   optional feature that does not change output for previous inputs.
/// * `patch` — an implementation change that preserves observable output
///   for every input.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DescriptorVersion {
    /// Bumped when the algorithm's observable output changes for at least
    /// one input the previous version handled.
    pub major: u16,
    /// Bumped when the algorithm accepts strictly more inputs than before,
    /// or gains an optional feature that does not change output for
    /// previously-valid inputs.
    pub minor: u16,
    /// Bumped for implementation changes that preserve observable output
    /// for every input.
    pub patch: u16,
}

impl DescriptorVersion {
    /// Constructs a descriptor version.
    #[inline]
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for DescriptorVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The published source that an implementation follows.
///
/// This is intentionally coarse; the fields hold `&'static str` so
/// descriptors can be constructed in `const` context and used in
/// no-alloc builds.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DefinitionSource {
    /// A published academic paper.
    Paper {
        /// The paper's title.
        title: &'static str,
        /// The paper's authors, formatted for a citation.
        authors: &'static str,
        /// Publication year.
        year: u16,
    },
    /// A formal standard document.
    Standard {
        /// The standard's canonical name (e.g. `"Unicode 15.0"`).
        name: &'static str,
    },
    /// A widely used reference implementation.
    ReferenceImplementation {
        /// The reference implementation's canonical identifier.
        name: &'static str,
    },
    /// Derived independently from the algorithm's mathematical definition,
    /// without imitating a specific implementation.
    IndependentlyDerived,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_const_constructible() {
        const D: AlgorithmDescriptor = AlgorithmDescriptor::new(
            AlgorithmFamily::Levenshtein,
            VariantId("unit-cost-unicode-scalars"),
            DescriptorVersion::new(0, 1, 0),
            DefinitionSource::Paper {
                title: "Binary codes capable of correcting deletions, insertions, and reversals",
                authors: "V. I. Levenshtein",
                year: 1966,
            },
        );
        assert_eq!(D.variant.0, "unit-cost-unicode-scalars");
        assert_eq!(D.version.major, 0);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn version_display() {
        extern crate alloc;
        use alloc::format;
        assert_eq!(format!("{}", DescriptorVersion::new(1, 2, 3)), "1.2.3");
        assert_eq!(format!("{}", VariantId("unit-cost")), "unit-cost");
    }
}
