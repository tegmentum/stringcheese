//! [`VietnameseStemmer`] — an identity-style canonicalizer.
//!
//! # Why no morphological stemming?
//!
//! Vietnamese is a **strictly analytic** language: verbs do not
//! conjugate for tense, aspect, mood, person, or number; nouns do not
//! decline for case, number, or gender; adjectives do not agree with
//! nouns. Grammatical relations are expressed by **word order and
//! function-word particles** (`đã` past, `đang` progressive, `sẽ`
//! future, `những` / `các` plurality, `hơn` / `nhất` comparative /
//! superlative) — not by morphological inflection *on* the content
//! word.
//!
//! There is therefore **no meaningful suffix or affix a Vietnamese
//! stemmer could strip** — every Vietnamese content word already
//! stands in its dictionary form. Every Snowball-family stemmer
//! algorithm we've studied for other languages targets exactly the
//! inflectional apparatus Vietnamese does not have.
//!
//! # What this "stemmer" does instead
//!
//! It **canonicalizes** the input. Two Vietnamese strings that
//! represent the same word may differ in Unicode composition — one
//! in NFC (precomposed vowel-plus-diacritic scalars), the other in
//! NFD (base letter + combining marks). Both should stem to the same
//! canonical key so a downstream IR pipeline that hashes stems or
//! compares them for equality doesn't miss matches.
//!
//! The implementation is therefore a single NFC pass, delegated to
//! [`crate::normalize::VietnameseNormalizer`]. The output is an owned
//! [`String`] wrapped in a [`Cow::Owned`] when the input required
//! recomposition; a [`Cow::Borrowed`] on the input otherwise (the
//! common case — most Vietnamese input on the web is already NFC).
//!
//! # Why not just call `normalize::normalize` directly?
//!
//! The pack pattern reserves the [`Language::stem`] slot for the
//! per-language canonical key. Every other pack in the workspace
//! (English → Porter, French → Snowball French, Portuguese →
//! Snowball Portuguese, …) fills that slot with a per-language
//! algorithm. Filling it with the NFC canonicalization for
//! Vietnamese matches the pattern: the caller writes `lang.stem(w)`
//! and gets back the Vietnamese canonical form, without having to
//! know that Vietnamese has no morphological stemming.
//!
//! [`Language::stem`]: stringcheese_lang::Language::stem
//!
//! # Contract
//!
//! - **Idempotent.** For any input, `stem(stem(w)) == stem(w)`. NFC
//!   is idempotent by construction; the second pass is a byte-level
//!   no-op.
//! - **Non-lengthening in practice.** NFC never grows the output for
//!   Vietnamese input — every Vietnamese vowel-plus-diacritic
//!   combination has a precomposed NFC scalar in the BMP that is
//!   equal or shorter in UTF-8 bytes than its NFD form.
//! - **Preserves input on already-NFC no-op.** Returns
//!   [`Cow::Borrowed`] when the input is already in NFC form and no
//!   rewriting was needed.
//! - **Deterministic.** Same input, same output; no randomness.
//!
//! # Non-goals
//!
//! - **Morphological analysis.** There is no morphology to analyse.
//! - **Diacritic stripping.** Aggressive tone-mark or full-diacritic
//!   folds are the normalizer's job, not the stemmer's — the stemmer
//!   is the *canonical, lossless* fingerprint.
//! - **Lemmatization / compound decomposition.** Reducing
//!   `học sinh` "student" to a canonical lemma requires a
//!   Vietnamese lexicon; out of scope for the offline pack.

use alloc::borrow::Cow;

use stringcheese_lang::Stemmer;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::is_nfc;

/// The Vietnamese identity-style canonicalizer.
///
/// A zero-sized unit value; construct as [`VietnameseStemmer`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the rationale — Vietnamese
/// is analytic, no morphological stemming is meaningful, so the
/// "stemmer" is a lossless NFC canonicalizer.
///
/// # Example
///
/// ```
/// use stringcheese_lang::Stemmer;
/// use stringcheese_vi::VietnameseStemmer;
///
/// // Already-NFC input: no rewriting.
/// assert_eq!(VietnameseStemmer.stem("được"), "được");
/// // NFD input: recomposed to NFC.
/// assert_eq!(VietnameseStemmer.stem("e\u{0302}\u{0323}"), "ệ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VietnameseStemmer;

impl VietnameseStemmer {
    /// Canonicalize `word` to its NFC form.
    ///
    /// Returns the canonical form as a [`Cow`]. If the input is
    /// already NFC (the common case for Vietnamese input on the web),
    /// the returned `Cow` borrows the input without allocating.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Fast path: already-NFC input requires no rewriting.
        if is_nfc(word) {
            return Cow::Borrowed(word);
        }
        Cow::Owned(word.nfc().collect())
    }
}

impl Stemmer for VietnameseStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        VietnameseStemmer::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        VietnameseStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_input_is_borrowed() {
        let out = VietnameseStemmer.stem("");
        assert_eq!(out, "");
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn already_nfc_input_is_borrowed_and_unchanged() {
        for w in ["được", "học sinh", "và", "cà phê", "Việt Nam", "hello"] {
            let out = VietnameseStemmer.stem(w);
            assert_eq!(out, w, "{w:?} was rewritten");
            assert!(
                matches!(out, Cow::Borrowed(_)),
                "{w:?} allocated on already-NFC input"
            );
        }
    }

    #[test]
    fn nfd_input_is_recomposed_to_nfc() {
        // `e` + circumflex + dot-below → precomposed `ệ`.
        assert_eq!(s("e\u{0302}\u{0323}"), "ệ");
        // `a` + breve + grave → precomposed `ằ`.
        assert_eq!(s("a\u{0306}\u{0300}"), "ằ");
        // `o` + circumflex + hook-above → precomposed `ổ`.
        assert_eq!(s("o\u{0302}\u{0309}"), "ổ");
    }

    #[test]
    fn idempotent() {
        for w in [
            "được",
            "học sinh",
            "e\u{0302}\u{0323}",
            "a\u{0306}\u{0300}",
            "hello",
            "",
        ] {
            let once = VietnameseStemmer.stem(w).into_owned();
            let twice = VietnameseStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn output_never_longer_than_input() {
        // Vietnamese NFC forms are never longer in UTF-8 bytes than
        // their NFD forms; the stemmer's output is bounded by the
        // input.
        for w in [
            "được",
            "học sinh đọc sách",
            "e\u{0302}\u{0323}",
            "a\u{0306}\u{0300}",
            "hello",
        ] {
            let out = VietnameseStemmer.stem(w);
            assert!(
                out.len() <= w.len(),
                "stem grew on {w:?}: {} → {}",
                w.len(),
                out.len()
            );
        }
    }
}
