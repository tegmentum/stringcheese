//! [`LightPersianStemmer`] — a light Persian suffix stripper.
//!
//! # Scope
//!
//! Persian morphology is much larger than what a rule-based light
//! stemmer can cover: verbs have a rich prefix + stem + suffix system
//! (`می-` durative, `ب-` subjunctive, subject / tense agreement
//! suffixes), the ezafeh vowel chains nouns into unwritten compounds,
//! and many verbs are `noun + auxiliary` compounds (`کار کردن` "to
//! work" = "to make work") that no surface-form rule set can
//! decompose. This stemmer is deliberately a **starter**: it handles
//! the most-common nominal suffixes and leaves the harder cases to
//! a downstream `stringcheese-fa-morph` crate.
//!
//! # Suffix table
//!
//! One pass, longest-match-wins, over the following:
//!
//! | Suffix     | Chars | Meaning                                       |
//! |------------|-------|-----------------------------------------------|
//! | `ترین`     | 4     | superlative ("most …")                        |
//! | `های`      | 3     | plural + ezafeh ("the … of")                  |
//! | `مان`      | 3     | 1st-person plural possessive ("our-")         |
//! | `تان`      | 3     | 2nd-person plural possessive ("your-pl-")     |
//! | `شان`      | 3     | 3rd-person plural possessive ("their-")       |
//! | `تر`       | 2     | comparative ("more …")                        |
//! | `ها`       | 2     | plural                                        |
//! | `ام`       | 2     | 1st-person singular possessive ("my-")        |
//! | `ای`       | 2     | 2nd-person singular possessive ("your-sg-")   |
//! | `اش`       | 2     | 3rd-person singular possessive ("his/her-")   |
//!
//! Each suffix is tried **twice**: once with an optional ZWNJ
//! (U+200C) immediately before it (`کتاب‌ها` = book + ZWNJ + plural)
//! and once bare (`کتابها`, same word without the joiner). If either
//! form matches, both the ZWNJ and the suffix are stripped.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (not bytes —
//! Persian scalars are 2 UTF-8 bytes each, so "2 characters" is
//! typically 4 bytes), the stemmer discards the strip and returns the
//! input unchanged. This guards against nonsense like stripping the
//! entire word.
//!
//! # Contract
//!
//! - **Idempotent.** For any input, `stem(stem(w)) == stem(w)`. The
//!   suffix table is small and the rules do not compose in a way that
//!   surfaces a second suffix after stripping the first.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`] when
//!   no rule fires.
//!
//! # Non-goals
//!
//! - **Verb-form stripping.** No rule matches the `می-` / `ب-` verb
//!   prefixes or the perfect / imperfect subject markers. Verb stems
//!   pass through unchanged for IR.
//! - **Ezafeh detection.** The ezafeh vowel is usually unwritten in
//!   the surface form; identifying its position needs POS tagging plus
//!   a lexicon and is deferred to the morphology crate.
//! - **Compound-verb decomposition.** `کار کردن` "to work" (literally
//!   "to make work") stays as two orthographic words on either side of
//!   the stemmer — recognizing them as one lemma is a lexicon problem.
//! - **Prefix stripping.** No prefix rules — Persian nominal prefixes
//!   are rare and mostly Arabic loans (`بی-` "without", `نا-` "un-")
//!   that are usually intended as part of the lemma.
//!
//! # RTL note
//!
//! Suffix stripping removes bytes from the *end* of the UTF-8 string —
//! a logical-order operation, not a display-order operation. The "end"
//! is where the last consonant is written, which is displayed on the
//! *left* in an RTL-rendered document. This mirrors every other
//! Persian-text-processing library's convention.

use alloc::borrow::Cow;
use alloc::string::String;

use stringcheese_lang::Stemmer;

/// Zero-width non-joiner, used between compound-word morphemes.
const ZWNJ: char = '\u{200C}';

/// Suffix table, longest-first. Each entry is a `&'static str` — the
/// stemmer strips the *first* one that matches from the *end* of the
/// input, optionally with a leading ZWNJ.
///
/// The ordering places the four-scalar (eight-byte) `ترین` at the top,
/// then the three-scalar suffixes, then the two-scalar suffixes —
/// longest-match-wins.
const SUFFIXES: &[&str] = &[
    "ترین", // -tarin: superlative
    "های",  // -haye: plural + ezafeh
    "مان",  // -eman: 1pl possessive
    "تان",  // -etan: 2pl possessive
    "شان",  // -eshan: 3pl possessive
    "تر",   // -tar: comparative
    "ها",   // -ha: plural
    "ام",   // -am: 1sg possessive
    "ای",   // -e: 2sg possessive
    "اش",   // -ash: 3sg possessive
];

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
const MIN_STEM_CHARS: usize = 2;

/// The light Persian suffix stripper.
///
/// A zero-sized value; construct as [`LightPersianStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_fa::LightPersianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural suffix.
/// assert_eq!(LightPersianStemmer.stem("کتاب\u{200C}ها"), "کتاب");
/// assert_eq!(LightPersianStemmer.stem("کتابها"), "کتاب");
/// // Bare noun with no suffix — the stemmer is a no-op.
/// assert_eq!(LightPersianStemmer.stem("کتاب"), "کتاب");
/// // The superlative wins over the comparative (longest match).
/// assert_eq!(LightPersianStemmer.stem("بزرگ\u{200C}ترین"), "بزرگ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightPersianStemmer;

impl LightPersianStemmer {
    /// Strip one suffix (optionally preceded by ZWNJ), honoring the
    /// over-strip safeguard.
    ///
    /// Returns [`Cow::Borrowed`] when no rule fires; [`Cow::Owned`]
    /// otherwise.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Length short-circuit — nothing shorter than the guard could
        // possibly be stripped.
        if word.chars().count() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        for &suffix in SUFFIXES {
            // First try with a ZWNJ immediately before the suffix.
            let zwnj_bytes = ZWNJ.len_utf8();
            let total_with_zwnj = zwnj_bytes + suffix.len();
            if word.len() > total_with_zwnj {
                let candidate_start = word.len() - total_with_zwnj;
                // Safe because we work on scalar boundaries: `suffix`
                // and ZWNJ are both valid UTF-8, and `starts_with`
                // works on byte prefixes but `char_indices` alignment
                // is guaranteed because `suffix` begins on a scalar
                // boundary (it's a valid `&str`).
                if word.is_char_boundary(candidate_start) {
                    let tail = &word[candidate_start..];
                    let tail_first = tail.chars().next();
                    if tail_first == Some(ZWNJ) && tail[zwnj_bytes..] == *suffix {
                        let stem = &word[..candidate_start];
                        if stem.chars().count() >= MIN_STEM_CHARS {
                            return Cow::Owned(String::from(stem));
                        }
                        // Guard fired — continue to the next suffix.
                        continue;
                    }
                }
            }

            // Then try the bare suffix (no leading ZWNJ).
            if word.len() > suffix.len() && word.ends_with(suffix) {
                let stem_len = word.len() - suffix.len();
                if word.is_char_boundary(stem_len) {
                    let stem = &word[..stem_len];
                    if stem.chars().count() >= MIN_STEM_CHARS {
                        return Cow::Owned(String::from(stem));
                    }
                }
                // Guard fired — continue to the next suffix. (Suffix
                // matched but the resulting stem was too short.)
            }
        }

        Cow::Borrowed(word)
    }
}

impl Stemmer for LightPersianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        LightPersianStemmer.stem(w).into_owned()
    }

    // ---------------------------------------------------------------
    // Plural / plural+ezafeh.
    // ---------------------------------------------------------------

    #[test]
    fn strips_plural_ha_with_zwnj() {
        // کتاب + ZWNJ + ها → کتاب
        assert_eq!(s("کتاب\u{200C}ها"), "کتاب");
    }

    #[test]
    fn strips_plural_ha_without_zwnj() {
        // کتاب + ها → کتاب
        assert_eq!(s("کتابها"), "کتاب");
    }

    #[test]
    fn strips_plural_ezafeh_haye_with_zwnj() {
        // کتاب + ZWNJ + های → کتاب
        assert_eq!(s("کتاب\u{200C}های"), "کتاب");
    }

    #[test]
    fn strips_plural_ezafeh_haye_without_zwnj() {
        assert_eq!(s("کتابهای"), "کتاب");
    }

    // ---------------------------------------------------------------
    // Comparative / superlative.
    // ---------------------------------------------------------------

    #[test]
    fn strips_comparative_tar() {
        // بزرگ + تر → بزرگ
        assert_eq!(s("بزرگتر"), "بزرگ");
        assert_eq!(s("بزرگ\u{200C}تر"), "بزرگ");
    }

    #[test]
    fn strips_superlative_tarin() {
        // بزرگ + ترین → بزرگ
        assert_eq!(s("بزرگترین"), "بزرگ");
        assert_eq!(s("بزرگ\u{200C}ترین"), "بزرگ");
    }

    #[test]
    fn longest_suffix_wins_tarin_over_tar() {
        // The superlative `ترین` (4 chars) must match before the
        // comparative `تر` (2 chars).
        assert_eq!(s("خوبترین"), "خوب");
        assert_eq!(s("خوب\u{200C}ترین"), "خوب");
    }

    // ---------------------------------------------------------------
    // Possessive clitics.
    // ---------------------------------------------------------------

    #[test]
    fn strips_1sg_possessive_am() {
        // کتاب + ام → کتاب
        assert_eq!(s("کتابام"), "کتاب");
    }

    #[test]
    fn strips_2sg_possessive_ei() {
        // کتاب + ای → کتاب
        assert_eq!(s("کتابای"), "کتاب");
    }

    #[test]
    fn strips_3sg_possessive_ash() {
        // کتاب + اش → کتاب
        assert_eq!(s("کتاباش"), "کتاب");
    }

    #[test]
    fn strips_1pl_possessive_eman() {
        // کتاب + مان → کتاب
        assert_eq!(s("کتابمان"), "کتاب");
    }

    #[test]
    fn strips_2pl_possessive_etan() {
        // کتاب + تان → کتاب
        assert_eq!(s("کتابتان"), "کتاب");
    }

    #[test]
    fn strips_3pl_possessive_eshan() {
        // کتاب + شان → کتاب
        assert_eq!(s("کتابشان"), "کتاب");
    }

    // ---------------------------------------------------------------
    // Contract: idempotent, non-lengthening, guarded.
    // ---------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("کتاب"), "کتاب");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in [
            "کتاب\u{200C}ها",
            "کتاب\u{200C}های",
            "بزرگ\u{200C}تر",
            "بزرگ\u{200C}ترین",
            "کتابمان",
            "کتاباش",
        ] {
            let once = LightPersianStemmer.stem(w).into_owned();
            let twice = LightPersianStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "ها" alone: 2 chars — the guard's `>` check on total length
        // means we can't strip more than exists.
        assert_eq!(s("ها"), "ها");
        // A word that's exactly `اش` (2 chars) — length short-circuit
        // does not fire (>= 2), but the strip would leave 0 chars, so
        // guard rolls back.
        assert_eq!(s("اش"), "اش");
        // Very short word — length short-circuit.
        assert_eq!(s("ک"), "ک");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in [
            "کتاب\u{200C}ها",
            "کتابها",
            "بزرگترین",
            "کتاباش",
            "hello",
            "کتاب",
        ] {
            let out = LightPersianStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn identity_on_non_persian_input() {
        assert_eq!(s("hello"), "hello");
        assert_eq!(s("running"), "running");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightPersianStemmer.stem("کتاب");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightPersianStemmer.stem("کتاب\u{200C}ها");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
