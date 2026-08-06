//! Unicode normalization — NFC, NFD, NFKC, NFKD.
//!
//! The Unicode Standard defines four *normal forms* that make otherwise
//! equivalent text comparable for byte-level equality:
//!
//! | Form | Decomposition | Compose after | Equivalence |
//! |------|---------------|---------------|-------------|
//! | NFD  | canonical     | no            | canonical   |
//! | NFC  | canonical     | yes           | canonical   |
//! | NFKD | compatibility | no            | compatibility |
//! | NFKC | compatibility | yes           | compatibility |
//!
//! - **Canonical equivalence** treats byte sequences that differ only in
//!   the composition of otherwise-equivalent code points as equal — for
//!   example, `"é"` (U+00E9) and `"e\u{0301}"` (`e` + combining acute)
//!   normalize to the same NFC output.
//! - **Compatibility equivalence** additionally collapses forms that share
//!   the same abstract character but differ in presentation — for
//!   example, the ligature `"ﬁ"` (U+FB01) becomes `"fi"` under NFKC.
//!
//! For string comparison work, **NFC** is almost always the right default:
//! it is the form the web produces and consumes, and it preserves the
//! original character forms the user typed. **NFKC** is appropriate when
//! visual variants should compare equal (typography differences,
//! full-width vs half-width forms). NFD and NFKD are useful when a
//! downstream stage needs to inspect combining marks separately (for
//! example, [`crate::diacritics`] uses NFD internally).
//!
//! Reference: [Unicode Standard Annex #15](https://www.unicode.org/reports/tr15/).
//!
//! # References
//!
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.
//!   URL: <https://www.unicode.org/versions/Unicode15.0.0/>
//! * Unicode Standard Annex #15. *Unicode Normalization Forms*. URL:
//!   <https://www.unicode.org/reports/tr15/> — the specification the four
//!   normal forms in this module implement.

use alloc::string::String;
use unicode_normalization::UnicodeNormalization;

/// Returns `input` in Unicode Normalization Form C (canonical
/// composition).
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::nfc;
/// // "e" + combining acute → precomposed "é" (U+00E9).
/// assert_eq!(nfc("e\u{0301}"), "\u{00E9}");
/// // Already-NFC input is returned unchanged.
/// assert_eq!(nfc("hello"), "hello");
/// ```
#[must_use]
pub fn nfc(input: &str) -> String {
    input.nfc().collect()
}

/// Returns `input` in Unicode Normalization Form D (canonical
/// decomposition).
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::nfd;
/// // Precomposed "é" (U+00E9) → "e" + combining acute (U+0301).
/// assert_eq!(nfd("\u{00E9}"), "e\u{0301}");
/// ```
#[must_use]
pub fn nfd(input: &str) -> String {
    input.nfd().collect()
}

/// Returns `input` in Unicode Normalization Form KC (compatibility
/// composition).
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::nfkc;
/// // The ligature "ﬁ" (U+FB01) is decomposed to "fi" under NFKC.
/// assert_eq!(nfkc("\u{FB01}"), "fi");
/// ```
#[must_use]
pub fn nfkc(input: &str) -> String {
    input.nfkc().collect()
}

/// Returns `input` in Unicode Normalization Form KD (compatibility
/// decomposition).
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::nfkd;
/// // Superscript "²" (U+00B2) decomposes to "2" under compatibility.
/// assert_eq!(nfkd("\u{00B2}"), "2");
/// ```
#[must_use]
pub fn nfkd(input: &str) -> String {
    input.nfkd().collect()
}

/// Names one of the four Unicode normalization forms, for policy-driven
/// code that selects a form at runtime (for example, the
/// [`PreprocessingPipeline`](crate::preprocessing::PreprocessingPipeline)
/// builder).
///
/// This is **not** the same concept as
/// [`stringcheese_core::NormalizationPolicy`], which names how a raw
/// [`Distance<T>`](stringcheese_core::Distance) is scaled into `[0.0, 1.0]`.
/// The two never collide; see
/// [`docs/design/preprocessing-pipeline.md § Cross-references`](https://github.com/tegmentum/stringcheese/blob/main/docs/design/preprocessing-pipeline.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Normalization {
    /// Canonical composition — the web-default form. See [`nfc`].
    Nfc,
    /// Canonical decomposition. See [`nfd`].
    Nfd,
    /// Compatibility composition — collapses visual variants. See
    /// [`nfkc`].
    Nfkc,
    /// Compatibility decomposition. See [`nfkd`].
    Nfkd,
}

impl Normalization {
    /// Applies this normalization form to `input`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use stringcheese_unicode::Normalization;
    /// assert_eq!(Normalization::Nfc.apply("e\u{0301}"), "\u{00E9}");
    /// ```
    #[must_use]
    pub fn apply(self, input: &str) -> String {
        match self {
            Self::Nfc => nfc(input),
            Self::Nfd => nfd(input),
            Self::Nfkc => nfkc(input),
            Self::Nfkd => nfkd(input),
        }
    }

    /// A short human-readable label for this form, for explainability
    /// output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nfc => "NFC",
            Self::Nfd => "NFD",
            Self::Nfkc => "NFKC",
            Self::Nfkd => "NFKD",
        }
    }
}

impl core::fmt::Display for Normalization {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_composes_combining_sequence() {
        assert_eq!(nfc("e\u{0301}"), "\u{00E9}");
    }

    #[test]
    fn nfd_decomposes_precomposed() {
        assert_eq!(nfd("\u{00E9}"), "e\u{0301}");
    }

    #[test]
    fn nfkc_expands_ligature() {
        assert_eq!(nfkc("\u{FB01}"), "fi");
    }

    #[test]
    fn nfkd_decomposes_superscript() {
        assert_eq!(nfkd("\u{00B2}"), "2");
    }

    #[test]
    fn ascii_is_stable_under_all_forms() {
        for s in ["", "hello", "A quick brown fox."] {
            assert_eq!(nfc(s), s);
            assert_eq!(nfd(s), s);
            assert_eq!(nfkc(s), s);
            assert_eq!(nfkd(s), s);
        }
    }

    #[test]
    fn normalization_enum_apply_matches_direct_calls() {
        let s = "café";
        assert_eq!(Normalization::Nfc.apply(s), nfc(s));
        assert_eq!(Normalization::Nfd.apply(s), nfd(s));
        assert_eq!(Normalization::Nfkc.apply(s), nfkc(s));
        assert_eq!(Normalization::Nfkd.apply(s), nfkd(s));
    }

    #[test]
    fn normalization_enum_display() {
        assert_eq!(Normalization::Nfc.to_string(), "NFC");
        assert_eq!(Normalization::Nfd.to_string(), "NFD");
        assert_eq!(Normalization::Nfkc.to_string(), "NFKC");
        assert_eq!(Normalization::Nfkd.to_string(), "NFKD");
    }
}
