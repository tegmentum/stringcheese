//! The [`PhoneticEncoder`] trait and its metadata sidekicks.
//!
//! A [`PhoneticEncoder`] is the *encode* half of a phonetic match. It takes an
//! input string and produces a phonetic key of a type it declares. The
//! *compare* half — deciding whether two keys mean the same input pronunciation
//! matched — lives on [`crate::PhoneticMatcher`], which composes an encoder
//! with a key-equality (or, in a future release, key-Levenshtein) predicate.
//!
//! # Why the trait is minimal
//!
//! Comparand's design document describes a richer phonetic-subsystem API
//! (multi-key `PhoneticCodes`, phoneme-sequence output, `MatchMode`
//! configuration on the matcher). The 0.1 delivery lands the minimum surface
//! required to make Soundex, Double Metaphone, and NYSIIS usable:
//!
//! * A [`PhoneticEncoder`] with an associated [`PhoneticEncoder::Key`] type
//!   so single-key algorithms (Soundex, NYSIIS) can return `String` and
//!   multi-key algorithms (Double Metaphone) can return a small struct.
//! * A [`PhoneticEncoder::descriptor`] that pins the variant per Comparand's
//!   [algorithm-variant registry][reg].
//! * A [`PhoneticEncoder::applicability`] that declares the languages,
//!   scripts, and regions the encoder was designed for.
//!
//! The full multi-key `PhoneticCodes` type and the phoneme-sequence
//! representation described in the design document are deferred to a
//! non-breaking 0.2 extension — the trait signature is designed so an
//! associated `Output` type (a superset of `Key`) can replace `Key` without
//! breaking single-key call sites.
//!
//! [reg]: comparand_core::AlgorithmDescriptor

use comparand_core::AlgorithmDescriptor;

/// A phonetic encoder: input string in, phonetic key out.
///
/// The associated [`Key`](Self::Key) type is typically `String` for a
/// single-key algorithm (Soundex, NYSIIS) or a small struct for a multi-key
/// algorithm ([`crate::DoubleMetaphoneKey`]).
///
/// Implementations declare themselves as one of Comparand's known algorithm
/// variants via [`descriptor`](Self::descriptor). Two implementations with
/// the same descriptor are expected to produce identical output for every
/// input; that guarantee is what makes golden cases interchangeable across
/// implementations.
pub trait PhoneticEncoder {
    /// The type of key this encoder produces.
    ///
    /// A single-key encoder returns `String` (or an equivalent owned
    /// string-like type). A multi-key encoder returns a small struct
    /// containing the primary key and, optionally, one or more alternates.
    type Key;

    /// Encodes `input` to a phonetic key.
    ///
    /// The result is defined only for the encoder's declared
    /// [`applicability`](Self::applicability). Feeding input outside the
    /// declared applicability is not automatically prevented — the type
    /// system cannot know a string's language — but the returned key may be
    /// meaningless. See the encoder's own documentation for its behavior on
    /// input that lies outside its declared scope.
    fn encode(&self, input: &str) -> Self::Key;

    /// Returns the algorithm descriptor identifying this encoder.
    ///
    /// Golden cases and multi-encoder pipelines reference the descriptor
    /// rather than the encoder's concrete type so that two implementations
    /// of the same variant are interchangeable.
    fn descriptor(&self) -> AlgorithmDescriptor;

    /// Returns the languages, scripts, and regions this encoder was
    /// designed for.
    ///
    /// The default implementation returns [`Applicability::UNSPECIFIED`],
    /// but every encoder in this crate overrides it with a specific
    /// applicability value.
    fn applicability(&self) -> Applicability {
        Applicability::UNSPECIFIED
    }
}

/// A BCP 47 language subtag as a plain `&'static str` (e.g. `"en"`, `"de"`,
/// `"fr"`).
///
/// The type is a newtype wrapper for the same reasons as
/// [`comparand_core::VariantId`]: it makes language declarations searchable
/// (grep for `LanguageTag("en")` picks up every English-only encoder in one
/// pass) and prevents an [`Applicability`]'s language field from being
/// confused with its script or region fields.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LanguageTag(pub &'static str);

/// An ISO 15924 script code as a plain `&'static str` (e.g. `"Latn"`,
/// `"Cyrl"`, `"Hani"`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScriptTag(pub &'static str);

/// An ISO 3166-1 alpha-2 region code as a plain `&'static str` (e.g. `"US"`,
/// `"GB"`, `"DE"`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionTag(pub &'static str);

/// The languages, scripts, and regions a phonetic encoder was designed for.
///
/// Every encoder in this crate exposes an [`Applicability`] via
/// [`PhoneticEncoder::applicability`]. The value is descriptive, not
/// prescriptive: it does not stop callers from encoding input in an
/// undeclared language, but it makes the mismatch inspectable and it feeds
/// explainability output that reports *why* a particular pair of inputs was
/// (or was not) matched.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Applicability {
    /// The languages the encoder was designed for.
    pub languages: &'static [LanguageTag],
    /// The scripts the encoder was designed for.
    pub scripts: &'static [ScriptTag],
    /// The regions the encoder was designed for; empty if the encoder is
    /// not region-specific.
    pub regions: &'static [RegionTag],
    /// Free-form notes about the encoder's design intent.
    pub notes: &'static str,
}

impl Applicability {
    /// A sentinel applicability used only by the [`PhoneticEncoder`] trait's
    /// default method implementation, when an encoder forgets to override
    /// [`applicability`](PhoneticEncoder::applicability).
    ///
    /// Every encoder shipped by this crate provides an explicit applicability
    /// — this sentinel exists so that a third-party encoder that forgets to
    /// declare one produces a legible "unspecified" value rather than a
    /// misleading English-only default.
    pub const UNSPECIFIED: Self = Self {
        languages: &[],
        scripts: &[],
        regions: &[],
        notes: "unspecified — the encoder did not declare its applicability",
    };

    /// Convenience constructor for encoders that were designed for a single
    /// language in a single script, without a specific region.
    #[inline]
    #[must_use]
    pub const fn single(
        language: &'static [LanguageTag],
        script: &'static [ScriptTag],
        notes: &'static str,
    ) -> Self {
        Self {
            languages: language,
            scripts: script,
            regions: &[],
            notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unspecified_applicability_has_empty_lists() {
        let a = Applicability::UNSPECIFIED;
        assert!(a.languages.is_empty());
        assert!(a.scripts.is_empty());
        assert!(a.regions.is_empty());
    }

    #[test]
    fn single_constructor_leaves_regions_empty() {
        const LANGS: &[LanguageTag] = &[LanguageTag("en")];
        const SCRIPTS: &[ScriptTag] = &[ScriptTag("Latn")];
        let a = Applicability::single(LANGS, SCRIPTS, "test");
        assert_eq!(a.languages, LANGS);
        assert_eq!(a.scripts, SCRIPTS);
        assert!(a.regions.is_empty());
        assert_eq!(a.notes, "test");
    }

    #[test]
    fn tag_types_are_orderable() {
        // Ord is asserted via a compile-time sort — the tags need to be
        // usable as `BTreeMap` keys.
        let mut v = [LanguageTag("en"), LanguageTag("de"), LanguageTag("fr")];
        v.sort();
        assert_eq!(v[0], LanguageTag("de"));
    }
}
