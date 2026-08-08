//! [`LightAmharicStemmer`] — a rule-based Amharic suffix stripper.
//!
//! # Scope
//!
//! Amharic is a **Semitic** language (like Arabic and Hebrew) with
//! root-and-pattern morphology at its core — a 3- or 4-consonant
//! root is interleaved with vowel patterns to derive verb stems.
//! Full morphological analysis needs the equivalent of a
//! `HornMorpho` / `AmMorpho` lexicon plus template matching against
//! a large derivational pattern table; no rule-based stemmer can
//! reproduce that in a per-crate language pack. On top of the root-
//! and-pattern core, however, Amharic surface morphology has a
//! productive **suffix system** — definite article, plural,
//! possessive clitics, object clitics — that peels off
//! deterministically from most nominal stems. That is what this
//! module strips.
//!
//! There is **no canonical Snowball Amharic algorithm**. Snowball's
//! catalogue lists neither Amharic nor any Ge'ez-script language;
//! the community references are the AAU Amharic corpus tools and
//! the Argaw & Asker 2007 "An Amharic Stemmer" paper. This module
//! ships a deliberately conservative rule-based subset, matching
//! the shape of the Hebrew and Arabic packs' light stemmers.
//!
//! # Suffix table
//!
//! Iterate until convergence, longest-match-wins per pass, over the
//! following (all Amharic in Ge'ez script). Every suffix is stored
//! as a **`&'static [char]`** — the stemmer works on `Vec<char>`
//! tails, never raw byte offsets. Every main-block Ge'ez scalar is
//! 3 bytes in UTF-8, so byte-level arithmetic would silently
//! corrupt boundaries.
//!
//! ## Definite article
//!
//! | Suffix | Meaning         |
//! |--------|-----------------|
//! | `-ው`   | the (m.)        |
//! | `-ዋ`   | the (f.)        |
//!
//! ## Plural
//!
//! | Suffix | Meaning         |
//! |--------|-----------------|
//! | `-ኦች`  | plural           |
//!
//! ## Possessive suffixes
//!
//! | Suffix | Meaning         |
//! |--------|-----------------|
//! | `-ዬ`   | my              |
//! | `-ህ`   | your (m.)       |
//! | `-ሽ`   | your (f.)       |
//! | `-ው`   | his             |
//! | `-ዋ`   | her             |
//! | `-ችን`  | our             |
//! | `-ችሁ`  | your (pl.)      |
//! | `-ችው`  | their           |
//!
//! ## Object suffixes
//!
//! | Suffix  | Meaning         |
//! |---------|-----------------|
//! | `-ኝ`    | me              |
//! | `-ህ`    | you (m.)        |
//! | `-ሽ`    | you (f.)        |
//! | `-ው`    | him             |
//! | `-ት`    | her             |
//! | `-ን`    | us              |
//! | `-ችሁ`   | you (pl.)       |
//! | `-ኣቸው`  | them            |
//!
//! Because several suffixes are **homographic across categories**
//! (e.g. `-ው` is both the masculine definite article, the "his"
//! possessive, and the "him" object suffix; `-ሽ` is both "your-f"
//! possessive and "you-f" object), the table dedupes internally —
//! each surface suffix appears exactly once even when it belongs
//! to multiple morphological categories.
//!
//! # Iterate-to-convergence
//!
//! Amharic can stack suffixes (e.g. plural + possessive:
//! `ልጆቻችን` "our children" = `ልጅ` + `-ኦች` + `-ችን`). The
//! stemmer therefore runs the suffix pass **repeatedly** until no
//! rule fires — same shape as `stringcheese_ar::Light10`. Each
//! successful strip shortens the stem by at least one scalar, and
//! the over-strip guard caps the tail, so iteration terminates in
//! at most `chars().count()` steps.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than **2 characters** (scalars — not
//! bytes; Ge'ez scalars are 3 bytes each, so "2 characters" is 6
//! bytes for pure-Amharic input), the stemmer discards the strip
//! and returns the current stem unchanged. This guards against
//! pathological inputs like a suffix standing alone.
//!
//! # Deliberate limitations (documented, not bugs)
//!
//! - **No root-and-pattern reversal.** Extracting the 3- or
//!   4-consonant Semitic root needs template matching against a
//!   large derivational pattern table plus a lexicon. This light
//!   stemmer strips *surface* suffixes only; a follow-up
//!   `stringcheese-am-morph` crate would ship the full analyzer.
//! - **No verb prefix stripping.** Amharic verbs carry subject
//!   prefixes (`እ-` 1sg, `ት-` 2sg / 3sgF, `ይ-` 3sgM, `ን-` 1pl).
//!   Stripping them off a bare form like `እወዳለሁ` "I love" would
//!   yield an ill-formed stem — verb templates entangle the prefix
//!   with the stem. The high-frequency subject-prefixed forms of
//!   the copula are handled by the stopword list instead.
//! - **Homograph collisions are accepted by design.** The
//!   masculine definite article `-ው` and the possessive "his" `-ው`
//!   are surface-identical; the stemmer strips the surface form
//!   without disambiguation. This is standard IR practice for a
//!   light stemmer.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)`
//!   return equal outputs.
//! - **Converges under repeated application.** Each successful
//!   strip shortens the stem by at least one scalar and the suffix
//!   table has finite entries, so the iterate-to-convergence loop
//!   terminates.
//! - **Idempotent.** After convergence, a second call to `stem` is
//!   a no-op.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`]
//!   when no rule fires.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Over-stripping guard: refuse to strip if the resulting stem
/// would have fewer than this many *characters* (scalars).
///
/// Ge'ez scalars are 3 UTF-8 bytes each, so the equivalent byte
/// floor for pure-Amharic input is 6 bytes; we count characters so
/// the guard works correctly for input that mixes ASCII too.
const MIN_STEM_CHARS: usize = 2;

/// Suffix table, sorted longest-first. Every entry is a
/// scalar-sequence (`&'static [char]`) rather than a byte string —
/// the stemmer matches on `Vec<char>` tails, never raw bytes.
///
/// Longest-match-wins is enforced by ordering the table with the
/// longest suffixes first. Duplicates across morphological
/// categories (e.g. `-ው` is definite article + possessive + object)
/// appear exactly once.
const SUFFIXES: &[&[char]] = &[
    // -----------------------------------------------------------------
    // Three-scalar suffix: `-ኣቸው` "them" (object).
    // -----------------------------------------------------------------
    &['ኣ', 'ቸ', 'ው'],
    // -----------------------------------------------------------------
    // Two-scalar suffixes.
    // -----------------------------------------------------------------
    &['ኦ', 'ች'], // -ኦች   plural
    &['ች', 'ን'], // -ችን   our (poss)
    &['ች', 'ሁ'], // -ችሁ   your-pl (poss) / you-pl (obj)
    &['ች', 'ው'], // -ችው   their (poss)
    // -----------------------------------------------------------------
    // Single-scalar suffixes. `-ው` and `-ሽ` and `-ህ` each cover
    // multiple morphological categories (definite article,
    // possessive, object) — the surface strip does not
    // disambiguate.
    // -----------------------------------------------------------------
    &['ው'], // -ው    the-m / his / him
    &['ዋ'], // -ዋ    the-f / her (poss)
    &['ዬ'], // -ዬ    my
    &['ህ'], // -ህ    your-m (poss) / you-m (obj)
    &['ሽ'], // -ሽ    your-f (poss) / you-f (obj)
    &['ኝ'], // -ኝ    me (obj)
    &['ት'], // -ት    her (obj)
    &['ን'], // -ን    us (obj)
];

/// The light Amharic suffix stripper.
///
/// A zero-sized value; construct as [`LightAmharicStemmer`] and
/// reuse across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_am::LightAmharicStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural marker `-ኦች`.
/// assert_eq!(LightAmharicStemmer.stem("ልጅኦች"), "ልጅ");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightAmharicStemmer.stem("ቤት"), "ቤት");
/// // Iterate to convergence: stacked -ኦች then -ችን.
/// assert_eq!(LightAmharicStemmer.stem("ልጅኦችችን"), "ልጅ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightAmharicStemmer;

impl LightAmharicStemmer {
    /// Stems `word` per the light Amharic rule set, iterating to
    /// convergence.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires on the first
    /// pass, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Ge'ez letter is 3 bytes in UTF-8, so any code that mixed
        // byte offsets with character-boundary logic would corrupt
        // boundaries — we work strictly in char space.
        let mut chars: Vec<char> = word.chars().collect();

        if chars.len() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        let mut changed = false;
        loop {
            let mut fired = false;
            for &suffix in SUFFIXES {
                if suffix.len() > chars.len() {
                    continue;
                }
                let stem_len = chars.len() - suffix.len();
                if chars[stem_len..] != *suffix {
                    continue;
                }
                if stem_len < MIN_STEM_CHARS {
                    // Over-strip guard fired — keep looking for a
                    // shorter suffix that leaves a viable stem.
                    continue;
                }
                // Match! Truncate the tail and try again from the
                // top of the table.
                chars.truncate(stem_len);
                fired = true;
                changed = true;
                break;
            }
            if !fired {
                break;
            }
        }

        if !changed {
            return Cow::Borrowed(word);
        }
        Cow::Owned(chars.into_iter().collect::<String>())
    }
}

impl Stemmer for LightAmharicStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightAmharicStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Definite article.
    // -------------------------------------------------------------

    #[test]
    fn strips_definite_article_masc() {
        // ቤት + ው → ቤት (the house).
        assert_eq!(s("ቤትው"), "ቤት");
    }

    #[test]
    fn strips_definite_article_fem() {
        // ልጅ + ዋ → ልጅ (the daughter).
        assert_eq!(s("ልጅዋ"), "ልጅ");
    }

    // -------------------------------------------------------------
    // Plural.
    // -------------------------------------------------------------

    #[test]
    fn strips_plural_ocxh() {
        // ልጅ + ኦች → ልጅ (children).
        assert_eq!(s("ልጅኦች"), "ልጅ");
    }

    // -------------------------------------------------------------
    // Possessive suffixes.
    // -------------------------------------------------------------

    #[test]
    fn strips_possessive_my_ye() {
        // ቤት + ዬ → ቤት (my house).
        assert_eq!(s("ቤትዬ"), "ቤት");
    }

    #[test]
    fn strips_possessive_our_cxin() {
        // ቤት + ችን → ቤት (our house).
        assert_eq!(s("ቤትችን"), "ቤት");
    }

    #[test]
    fn strips_possessive_their_cxiw() {
        // ቤት + ችው → ቤት (their house).
        assert_eq!(s("ቤትችው"), "ቤት");
    }

    // -------------------------------------------------------------
    // Object suffixes.
    // -------------------------------------------------------------

    #[test]
    fn strips_object_them_axcxew() {
        // ቤት + ኣቸው → ቤት (... them).
        assert_eq!(s("ቤትኣቸው"), "ቤት");
    }

    #[test]
    fn strips_object_me_nye() {
        // አየ + ኝ → አየ ("saw me" → "saw", crude).
        assert_eq!(s("አየኝ"), "አየ");
    }

    // -------------------------------------------------------------
    // Iterate-to-convergence: stacked suffixes.
    // -------------------------------------------------------------

    #[test]
    fn iterates_plural_plus_possessive() {
        // ልጅ + ኦች + ችን → ልጅ (our children).
        assert_eq!(s("ልጅኦችችን"), "ልጅ");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_ax_cxew_over_cxew() {
        // ኣቸው is a 3-scalar entry; without longest-match-wins the
        // 2-scalar ችው (their) would fire first and leave an errant
        // ኣ. Test that the 3-scalar fires first.
        assert_eq!(s("ቤትኣቸው"), "ቤት");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded, non-lengthening.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("አማርኛ"), "አማርኛ");
        assert_eq!(s("ኢትዮጵያ"), "ኢትዮጵያ");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in ["ቤትው", "ልጅዋ", "ልጅኦች", "ቤትዬ", "ቤትችን", "ቤትኣቸው", "ልጅኦችችን"]
        {
            let once = LightAmharicStemmer.stem(w).into_owned();
            let twice = LightAmharicStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "ው" alone — 1 char, length short-circuit.
        assert_eq!(s("ው"), "ው");
        // "ችን" alone — 2 chars, stripping ችን leaves 0 → guarded;
        // the shorter single-scalar suffix ን would also over-strip
        // to 1 char and is guarded too.
        assert_eq!(s("ችን"), "ችን");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["አማርኛ", "ቤትው", "ልጅኦችችን", "hello", "ቤትኣቸው"] {
            let out = LightAmharicStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightAmharicStemmer.stem("አማርኛ");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightAmharicStemmer.stem("ልጅኦች");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
