//! [`LightHebrewStemmer`] — a light Hebrew suffix-and-prefix stripper.
//!
//! # Why "light"
//!
//! Hebrew morphology is *root-and-pattern* (like Arabic): a 3- or
//! 4-letter consonantal root (`כתב` "write", `למד` "learn", `שמר`
//! "guard", ...) is inflected by inserting vowels and derivational
//! affixes into a template. Recovering the root from a surface form is
//! not a simple string rewrite — it needs template matching against a
//! large derivational pattern table plus a lexicon (`HebMorph`, `MILA`,
//! `YAP`). None of that fits inside a per-crate language pack the
//! `stringcheese-he` charter allows.
//!
//! Instead, this module ships a *light* stemmer: a single prefix strip
//! plus a single suffix strip, longest-match-wins, with an over-strip
//! safeguard. It captures the common IR wins — stripping the definite
//! article (`ה-`), coordinating particles (`ו-`, `ב-`, `כ-`, `ל-`,
//! `מ-`, `ש-`), plural endings (`-ים`, `-ות`), possessive suffixes,
//! and past-tense verb suffixes — without pretending to solve
//! root-and-pattern morphology.
//!
//! # Algorithm
//!
//! A two-pass strict prefix/suffix stripper. Normalization (via
//! [`crate::normalize::normalize`]) is expected to have run *before*
//! the stemmer sees the input; the rules below assume no niqqud and no
//! cantillation.
//!
//! ## Pass 1 — strip one prefix, longest-match-wins
//!
//! Try each of the following in order and strip the first that matches:
//!
//! | Prefix | Meaning                                    |
//! |--------|--------------------------------------------|
//! | `וה`   | *and-the-* (`ו-` + `ה-`)                   |
//! | `בה`   | *in-the-*                                  |
//! | `כה`   | *like-the-*                                |
//! | `לה`   | *to-the-*                                  |
//! | `מה`   | *from-the-* (also the interrogative "what") |
//! | `שה`   | *that-the-*                                |
//! | `וב`   | *and-in-*                                  |
//! | `וכ`   | *and-like-*                                |
//! | `ול`   | *and-to-*                                  |
//! | `ומ`   | *and-from-*                                |
//! | `וש`   | *and-that-*                                |
//! | `ה`    | *the-* (definite article)                  |
//! | `ו`    | *and-* (conjunction)                       |
//! | `ב`    | *in-*                                      |
//! | `כ`    | *like-*                                    |
//! | `ל`    | *to-*                                      |
//! | `מ`    | *from-*                                    |
//! | `ש`    | *that-*                                    |
//!
//! ## Pass 2 — strip one suffix, longest-match-wins
//!
//! Try each of the following in order and strip the first that matches:
//!
//! | Suffix | Meaning                                          |
//! |--------|--------------------------------------------------|
//! | `ותיהם` | possessive on fem plural (their-f-pl)           |
//! | `יהם`  | their-m (masc plural possessive)                 |
//! | `יהן`  | their-f                                           |
//! | `יכם`  | your-m-pl                                         |
//! | `יכן`  | your-f-pl                                         |
//! | `תם`   | you-mp past                                       |
//! | `נו`   | our / we-past                                     |
//! | `כם`   | your-mp                                           |
//! | `הם`   | their-m                                           |
//! | `הן`   | their-f                                           |
//! | `תי`   | I-past                                            |
//! | `ים`   | masc plural                                       |
//! | `ות`   | fem plural                                        |
//! | `ה`    | fem sg / her (attached)                           |
//! | `י`    | my                                                |
//! | `ך`    | your (sg attached, final kaf)                     |
//! | `ו`    | his / they-past                                   |
//! | `ת`    | you-sg past                                       |
//!
//! **Note the deliberate absence of bare `-ם` and `-ן`.** Both do
//! function as rare possessive / pronoun clitics, but stripping them
//! off common short words like `שלום` "peace" (ends in ם) or `גן`
//! "garden" (ends in ן) would over-strip. The two-letter `-הם` /
//! `-הן` combinations above catch the "their-" cases.
//!
//! ## Ambiguous single-letter prefix strips
//!
//! The seven single-letter prefixes (`ה ו ב כ ל מ ש`) are
//! aggressively stripped when the input passes the over-strip guard.
//! Words that happen to begin with one of these letters as a *root*
//! consonant — `בית` "house" starts with `ב`, `כתבתי` "I wrote" starts
//! with `כ` — are over-stripped by design. A lexicon-free light
//! stemmer cannot tell the two cases apart. A future
//! `stringcheese-he-morph` crate would ship the lexicon-based
//! alternative.
//!
//! ## Over-stripping safeguard
//!
//! If the resulting stem would have fewer than 2 *characters* (not
//! bytes — Hebrew scalars are 2 UTF-8 bytes each, so "2 characters" is
//! 4 bytes), the stemmer discards the strip and returns the input
//! unchanged. This guards against nonsense like stripping the entire
//! word.
//!
//! # Contract
//!
//! - **Idempotent on real Hebrew input.** For any valid Modern Hebrew
//!   surface form, `stem(stem(w)) == stem(w)` — the prefix and suffix
//!   tables are curated so that no real word's stem re-matches a
//!   table entry. **Adversarial input may re-strip:** the single-pass
//!   design (one prefix strip, one suffix strip, per call) means that
//!   a synthetic input engineered to nest affixes can be stripped
//!   further on a second call. Same design principle as
//!   `stringcheese_ar::Light10`'s handling of `الوقت`; iterating to
//!   a fixed point would over-strip real words whose leading letter
//!   happens to look like a prefix.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`] when
//!   no rule fires.
//!
//! # Non-goals
//!
//! - **Full root-and-pattern morphological analysis.** Extracting the
//!   3- or 4-letter consonantal root needs template matching against a
//!   large derivational pattern table plus a lexicon (`HebMorph`, `MILA`,
//!   `YAP`). This light stemmer is the well-established "good enough for
//!   IR" baseline; root extraction is deferred to a downstream
//!   `stringcheese-he-morph` crate.
//! - **Verb-binyan awareness.** The Hebrew verb inflects across seven
//!   *binyanim* (`pa'al`, `nif'al`, `pi'el`, `pu'al`, `hif'il`,
//!   `huf'al`, `hitpa'el`) with stem-changing morphology this stemmer
//!   does not attempt to reverse.
//! - **Final-form awareness.** The suffix table lists both final and
//!   non-final variants where the suffix can end in either — e.g. `-ך`
//!   (final kaf) for "your-sg". If the input has been passed through
//!   [`HebrewNormalizer::with_final_form_folding(true)`](crate::normalize::HebrewNormalizer::with_final_form_folding),
//!   the corresponding non-final letters must also be present in the
//!   table for the fold to be reversible. The current table is
//!   optimized for un-folded input; folded input may miss some suffix
//!   strips.
//!
//! # RTL note
//!
//! Prefix stripping removes bytes from the *start* of the UTF-8
//! string; suffix stripping removes bytes from the *end*. These are
//! logical-order operations, not display-order operations — the "start"
//! is where the first consonant is written, which is displayed on the
//! *right* in an RTL-rendered document. This mirrors every other
//! Hebrew-text-processing library's convention.

use alloc::borrow::Cow;
use alloc::string::String;

use stringcheese_lang::Stemmer;

/// Prefix table, longest-first. Each entry is a `&'static str` — the
/// stemmer strips the *first* one that matches from the *start* of the
/// input.
///
/// Two-letter (four-byte) combined-prefix variants come first so
/// longest-match-wins fires before the single-letter fallbacks.
const PREFIXES: &[&str] = &[
    // Two-letter combined prefixes (particle + definite article, or
    // conjunction + particle).
    "וה", // and-the-
    "בה", // in-the-
    "כה", // like-the-
    "לה", // to-the-
    "מה", // from-the- (also "what")
    "שה", // that-the-
    "וב", // and-in-
    "וכ", // and-like-
    "ול", // and-to-
    "ומ", // and-from-
    "וש", // and-that-
    // Single-letter prefixes.
    "ה", // the-
    "ו", // and-
    "ב", // in-
    "כ", // like-
    "ל", // to-
    "מ", // from-
    "ש", // that-
];

/// Suffix table, longest-first.
///
/// Longer suffixes ship first so longest-match-wins fires
/// deterministically.
const SUFFIXES: &[&str] = &[
    // Five-letter suffix (compound possessive on fem plural).
    "ותיהם",
    // Three-letter suffixes (possessive on plural stem).
    "יהם", // their-m
    "יהן", // their-f
    "יכם", // your-m-pl
    "יכן", // your-f-pl
    // Two-letter suffixes.
    "תם", // you-mp past
    "נו", // our / we-past
    "כם", // your-mp
    "הם", // their-m
    "הן", // their-f
    "תי", // I-past
    "ים", // masc plural
    "ות", // fem plural
    // Single-letter suffixes. Note the deliberate absence of bare
    // `-ם` and `-ן` (final mem / final nun): both do function as
    // rare possessive/pronoun clitics, but stripping them off common
    // short words like `שלום` "peace" (ends in ם) or `גן` "garden"
    // (ends in ן) over-strips. The two-letter `-הם` / `-הן`
    // combinations above catch the "their-" cases; a bare final-mem
    // / final-nun strip trades precision for recall we don't want.
    "ה", // fem sg / her
    "י", // my
    "ך", // your (final kaf attached)
    "ו", // his / they-past
    "ת", // you-sg past
];

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
///
/// Hebrew scalars are 2 UTF-8 bytes each, so the equivalent byte floor
/// for pure-Hebrew input is 4 bytes; we count characters so the guard
/// works correctly for input that includes ASCII too.
const MIN_STEM_CHARS: usize = 2;

/// The light Hebrew prefix-and-suffix stripper.
///
/// A zero-sized value; construct as [`LightHebrewStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the contract.
///
/// # Example
///
/// ```
/// use stringcheese_he::LightHebrewStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the definite article.
/// assert_eq!(LightHebrewStemmer.stem("הספר"), "ספר");
/// // Bare noun with no affixes — the stemmer is a no-op.
/// assert_eq!(LightHebrewStemmer.stem("ספר"), "ספר");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightHebrewStemmer;

impl LightHebrewStemmer {
    /// Strip one prefix and then one suffix, honoring the over-strip
    /// safeguard.
    ///
    /// Returns [`Cow::Borrowed`] when neither pass fires; [`Cow::Owned`]
    /// otherwise.
    ///
    /// # Contract
    ///
    /// - **Idempotent on real Hebrew input** — a second call on the
    ///   output is a no-op for any valid Modern Hebrew surface form.
    ///   See the [module-level docs](self#contract) for the
    ///   adversarial-input caveat.
    /// - **Non-lengthening.** Output is never longer than the input.
    /// - **Guarded.** A strip that would leave fewer than 2 characters
    ///   is rolled back.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Two-character floor short-circuit — nothing to strip.
        if word.chars().count() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        // Pass 1: strip one prefix (longest-match-wins).
        let after_prefix = strip_prefix(word);
        // Pass 2: strip one suffix from whatever pass 1 handed us.
        let after_suffix = strip_suffix(after_prefix.as_ref());

        if after_suffix == word {
            return Cow::Borrowed(word);
        }
        Cow::Owned(String::from(after_suffix.as_ref()))
    }
}

/// Strip the first matching prefix in [`PREFIXES`] from the *start* of
/// `word`, honoring the over-strip guard. Returns a `Cow` — borrowed
/// if no prefix fires, borrowed-slice otherwise.
fn strip_prefix(word: &str) -> Cow<'_, str> {
    for &prefix in PREFIXES {
        if word.len() > prefix.len() && word.starts_with(prefix) {
            let stem = &word[prefix.len()..];
            // Over-strip guard.
            if stem.chars().count() >= MIN_STEM_CHARS {
                return Cow::Borrowed(stem);
            }
            // Match found but guard fired — return the input unchanged
            // (longest-match-wins: don't fall through to a shorter
            // prefix once a match has been found and rolled back).
            return Cow::Borrowed(word);
        }
    }
    Cow::Borrowed(word)
}

/// Strip the first matching suffix in [`SUFFIXES`] from the *end* of
/// `word`, honoring the over-strip guard.
fn strip_suffix(word: &str) -> Cow<'_, str> {
    for &suffix in SUFFIXES {
        if word.len() > suffix.len() && word.ends_with(suffix) {
            let stem_len = word.len() - suffix.len();
            let stem = &word[..stem_len];
            if stem.chars().count() >= MIN_STEM_CHARS {
                return Cow::Borrowed(stem);
            }
            return Cow::Borrowed(word);
        }
    }
    Cow::Borrowed(word)
}

impl Stemmer for LightHebrewStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        LightHebrewStemmer.stem(w).into_owned()
    }

    // ---------------------------------------------------------------
    // Prefix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_definite_article() {
        // ה + ספר → ספר
        assert_eq!(s("הספר"), "ספר");
    }

    #[test]
    fn strips_and_the() {
        // וה + ספר → ספר
        assert_eq!(s("והספר"), "ספר");
    }

    #[test]
    fn strips_in_the() {
        assert_eq!(s("בהספר"), "ספר");
    }

    #[test]
    fn strips_like_the() {
        assert_eq!(s("כהספר"), "ספר");
    }

    #[test]
    fn strips_to_the() {
        assert_eq!(s("להספר"), "ספר");
    }

    #[test]
    fn strips_that_the() {
        assert_eq!(s("שהספר"), "ספר");
    }

    #[test]
    fn strips_bare_conjunction() {
        // ו + ספר → ספר
        assert_eq!(s("וספר"), "ספר");
    }

    #[test]
    fn strips_bare_preposition() {
        assert_eq!(s("בספר"), "ספר");
        assert_eq!(s("לספר"), "ספר");
    }

    #[test]
    fn longest_prefix_wins() {
        // Input starts with וה — the two-letter prefix must trip before
        // the one-letter ו or ה.
        assert_eq!(s("והילד"), "ילד");
    }

    // ---------------------------------------------------------------
    // Suffix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_masculine_plural_im() {
        // ספר + ים → ספר
        assert_eq!(s("ספרים"), "ספר");
    }

    #[test]
    fn strips_feminine_plural_ot() {
        // ילד + ות → ילד
        assert_eq!(s("ילדות"), "ילד");
    }

    #[test]
    fn strips_feminine_singular_h() {
        // ילד + ה → ילד
        assert_eq!(s("ילדה"), "ילד");
    }

    #[test]
    fn strips_first_person_possessive() {
        // ספר + י → ספר
        assert_eq!(s("ספרי"), "ספר");
    }

    #[test]
    fn strips_third_person_possessive() {
        // ספר + ו → ספר
        assert_eq!(s("ספרו"), "ספר");
    }

    #[test]
    fn strips_our_possessive() {
        // ספר + נו → ספר
        assert_eq!(s("ספרנו"), "ספר");
    }

    #[test]
    fn strips_i_past_ti() {
        // אמר + תי → אמר (I said). Choice of `אמר` (root starts with
        // א, not a prefix letter) avoids the single-letter-prefix
        // over-strip trap that affects, e.g., `כתבתי` — see the
        // module-level "ambiguous single-letter prefix" note.
        assert_eq!(s("אמרתי"), "אמר");
    }

    #[test]
    fn strips_their_m_hem() {
        // ספר + הם → ספר
        assert_eq!(s("ספרהם"), "ספר");
    }

    // ---------------------------------------------------------------
    // Combined prefix + suffix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_prefix_and_suffix_in_one_call() {
        // ה + ילד + ים → ילד
        assert_eq!(s("הילדים"), "ילד");
    }

    #[test]
    fn strips_wal_and_feminine_plural() {
        // וה + ילד + ות → ילד
        assert_eq!(s("והילדות"), "ילד");
    }

    #[test]
    fn strips_and_in_and_plural() {
        // וב + ספר + ים → ספר
        assert_eq!(s("ובספרים"), "ספר");
    }

    // ---------------------------------------------------------------
    // Contract: idempotent, non-lengthening, guarded.
    // ---------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        // Bare 3-consonant roots that do NOT begin with one of the
        // seven prefix letters. See the module-level "ambiguous
        // single-letter prefix" note for why words like `בית` (which
        // starts with the `ב` prefix letter) are aggressively
        // stripped by the light stemmer.
        assert_eq!(s("ספר"), "ספר");
        assert_eq!(s("אור"), "אור");
        assert_eq!(s("יום"), "יום");
        assert_eq!(s(""), "");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in ["הספר", "הילדים", "והילדות", "ספרים", "ילדה", "כתבתי"]
        {
            let once = LightHebrewStemmer.stem(w).into_owned();
            let twice = LightHebrewStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // Very short word — the length short-circuit fires.
        assert_eq!(s("ה"), "ה");
        assert_eq!(s("ו"), "ו");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["הספר", "והילדים", "ספרות", "ילדנו", "כתבתי", "hello"] {
            let out = LightHebrewStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn identity_on_non_hebrew_input() {
        // Nothing in the tables matches English — the stemmer is a
        // no-op for non-Hebrew input.
        assert_eq!(s("hello"), "hello");
        assert_eq!(s("running"), "running");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightHebrewStemmer.stem("ספר");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightHebrewStemmer.stem("הספר");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
