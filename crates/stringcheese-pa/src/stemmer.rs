//! [`LightPunjabiStemmer`] — a light Punjabi (Eastern, Gurmukhi
//! script) suffix stripper.
//!
//! # Scope
//!
//! Punjabi morphology is comparatively simple next to the
//! agglutinative Sanskrit-inherited Hindi verb complex or Tamil's
//! case + person + tense stack. Nouns inflect for number (singular /
//! plural) and case (direct / oblique), with a fem-vs-masc distinction
//! reflected in the suffix vowel; verbs stack a small closed set of
//! participial suffixes (imperfective `-ਦਾ / ਦੀ / ਦੇ`, perfective
//! `-ਿਆ / ੀ / ੇ / ੀਆਂ`) onto the verb root, followed by a separate
//! auxiliary (`ਹੈ`, `ਹਾਂ`, `ਹਨ`, `ਸੀ`, ...) as its own orthographic
//! word.
//!
//! There is **no canonical Snowball Punjabi algorithm** — Snowball's
//! catalogue does not list Punjabi. The community reference is
//! Kumar & Josan (2010) *A stemming algorithm for Punjabi language*
//! and Gupta (2013)'s successor work; this module ships a
//! **deliberately conservative** rule-based subset covering the
//! high-frequency case + plural + verb-personal endings. It is
//! intended to be a starter — a downstream `stringcheese-pa-morph`
//! crate would ship the full analyzer.
//!
//! # Suffix table
//!
//! One pass, longest-match-wins, over the following (all Gurmukhi):
//!
//! | Suffix   | Chars | Class                                     |
//! |----------|-------|-------------------------------------------|
//! | `ੀਆਂ`   | 3     | fem plural / perfective 3pl-f              |
//! | `ਿਆਂ`   | 3     | oblique plural (masc / fem)                |
//! | `ਦਾ`    | 2     | imperfective participle masc sg            |
//! | `ਦੀ`    | 2     | imperfective participle fem sg             |
//! | `ਦੇ`    | 2     | imperfective participle masc pl / obl      |
//! | `ਾਂ`    | 2     | plural marker (ā + bindi)                  |
//! | `ਆਂ`    | 2     | plural marker after vowel stem             |
//! | `ਿਆ`    | 2     | perfective 3sg-m (i + ā)                   |
//! | `ੇ`     | 1     | oblique sg masc / perf 3pl-m               |
//! | `ੀ`     | 1     | fem sg / perf 3sg-f                        |
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **Free-standing postpositions (`ਦਾ`, `ਦੀ`, `ਦੇ`, `ਨੂੰ`,
//!   `ਵਿੱਚ`, `ਤੋਂ`, `ਨਾਲ`, `ਲਈ`).** These are almost always
//!   written as *separate* orthographic words in modern Punjabi
//!   text; they're handled by the stopword list. The `ਦਾ / ਦੀ / ਦੇ`
//!   entries in the suffix table above target the fused **verbal**
//!   imperfective participle, not the free-standing postposition
//!   (the tokenizer never delivers a postposition and a noun in a
//!   single token, so ambiguity is moot at the stemmer boundary).
//! * **Copula / auxiliary forms.** `ਹੈ`, `ਹਾਂ`, `ਹਨ`, `ਹੋ`, `ਸੀ`,
//!   `ਸਨ` — all in the stopword list.
//! * **Standalone tippi `ੰ`, bindi `ਂ`, addak `ੱ`.** These are
//!   *modifiers* that inflect a preceding scalar (tippi / bindi
//!   nasalize a vowel, addak geminates a following consonant); they
//!   are not inflectional endings and are never stripped alone.
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte offsets —
//! and matched at the *scalar-slice tail* of the input's `Vec<char>`
//! form. Gurmukhi scalars are 3 UTF-8 bytes each, so byte-level
//! arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a two-scalar suffix
//! standing alone (`ਦਾ` alone, `ਹਾਂ` alone).
//!
//! # Deliberate limitations (documented, not bugs)
//!
//! - **Free postpositions written as separate words are not seen
//!   by this stemmer.** `ਮੁੰਡੇ ਦਾ` (of the boy) is two
//!   whitespace-separated tokens; only `ਮੁੰਡੇ` reaches the stemmer,
//!   and its `-ੇ` oblique marker is what gets stripped.
//! - **No stem-final vowel restoration.** Fem-noun plural
//!   `ਕੁੜੀਆਂ` (girls) — the natural lemma is `ਕੁੜੀ` (girl), not
//!   `ਕੁੜ`. Longest-match-wins strips `-ੀਆਂ` (3 scalars) leaving
//!   `ਕੁੜ`, which normalizes both singular and plural (singular
//!   `ਕੁੜੀ` strips `-ੀ` to same `ਕੁੜ`) — trades lemma-fidelity for
//!   equivalence-class consistency. Good enough for IR.
//! - **No prefix stripping.** Punjabi nominal prefixes are rare
//!   (mostly Perso-Arabic loans); leaving them intact keeps the
//!   stem recognizable.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)`
//!   return equal outputs.
//! - **Converges under repeated application.** Each successful
//!   strip shortens the stem by at least one scalar and the
//!   suffix table has finite entries, so iterated application
//!   terminates.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`]
//!   when no rule fires.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
const MIN_STEM_CHARS: usize = 2;

// Named Gurmukhi scalars used by the suffix table. Named constants
// keep the table below readable and prevent fat-fingered codepoint
// literals across visually-similar matras.
const BINDI: char = '\u{0A02}'; // ਂ
const V_AA: char = '\u{0A06}'; // ਆ
const MATRA_AA: char = '\u{0A3E}'; // ਾ
const MATRA_I: char = '\u{0A3F}'; // ਿ
const MATRA_II: char = '\u{0A40}'; // ੀ
const MATRA_E: char = '\u{0A47}'; // ੇ
const C_DA: char = '\u{0A26}'; // ਦ

/// Suffix table, sorted longest-first. Every entry is a
/// scalar-sequence (`&'static [char]`) rather than a byte string — the
/// stemmer matches on `Vec<char>` tails, never raw bytes.
///
/// Longest-match-wins is enforced by iterating the table in order and
/// returning on the first match, and by ordering the table strictly
/// by descending scalar length.
const SUFFIXES: &[&[char]] = &[
    // ---------------------------------------------------------------
    // 3-scalar plural / perfective-plural markers.
    // ---------------------------------------------------------------
    &[MATRA_II, V_AA, BINDI], // ੀਆਂ — fem plural / perf 3pl-f
    &[MATRA_I, V_AA, BINDI],  // ਿਆਂ — oblique plural
    // ---------------------------------------------------------------
    // 2-scalar imperfective-participle endings.
    // ---------------------------------------------------------------
    &[C_DA, MATRA_AA], // ਦਾ — imperfective masc sg
    &[C_DA, MATRA_II], // ਦੀ — imperfective fem sg
    &[C_DA, MATRA_E],  // ਦੇ — imperfective masc pl / obl
    // ---------------------------------------------------------------
    // 2-scalar plural / perfective-singular markers.
    // ---------------------------------------------------------------
    &[MATRA_AA, BINDI], // ਾਂ — plural marker (after consonant stem)
    &[V_AA, BINDI],     // ਆਂ — plural marker (after vowel stem)
    &[MATRA_I, V_AA],   // ਿਆ — perf 3sg-m
    // ---------------------------------------------------------------
    // 1-scalar case / perfective markers.
    // ---------------------------------------------------------------
    &[MATRA_E],  // ੇ — obl sg masc / perf 3pl-m
    &[MATRA_II], // ੀ — fem sg / perf 3sg-f
];

/// The light Punjabi suffix stripper.
///
/// A zero-sized value; construct as [`LightPunjabiStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_pa::LightPunjabiStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural marker -ਾਂ.
/// assert_eq!(LightPunjabiStemmer.stem("ਘਰਾਂ"), "ਘਰ");
/// // Strip the imperfective participle -ਦਾ.
/// assert_eq!(LightPunjabiStemmer.stem("ਬੋਲਦਾ"), "ਬੋਲ");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightPunjabiStemmer.stem("ਪੰਜਾਬ"), "ਪੰਜਾਬ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightPunjabiStemmer;

impl LightPunjabiStemmer {
    /// Stems `word` per the light Punjabi rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Gurmukhi letter is 3 bytes in UTF-8, so any code that mixed
        // byte offsets with character-boundary logic would corrupt
        // boundaries — we work strictly in char space.
        let chars: Vec<char> = word.chars().collect();

        if chars.len() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        for &suffix in SUFFIXES {
            if suffix.len() > chars.len() {
                continue;
            }
            let stem_len = chars.len() - suffix.len();
            if chars[stem_len..] != *suffix {
                continue;
            }
            if stem_len < MIN_STEM_CHARS {
                // Over-strip guard fired — keep looking for a shorter
                // suffix (there may not be one, in which case the
                // input passes through unchanged).
                continue;
            }
            // Match! Rebuild the stem as a `String`.
            let stem: String = chars[..stem_len].iter().collect();
            return Cow::Owned(stem);
        }

        Cow::Borrowed(word)
    }
}

impl Stemmer for LightPunjabiStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightPunjabiStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Plural markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_plural_marker_aan() {
        // ਘਰਾਂ → ਘਰ (houses).
        assert_eq!(s("ਘਰਾਂ"), "ਘਰ");
    }

    #[test]
    fn strips_fem_plural_marker_iiaan() {
        // ਕੁੜੀਆਂ → ਕੁੜ (girls — normalizes with singular ਕੁੜੀ → ਕੁੜ).
        assert_eq!(s("ਕੁੜੀਆਂ"), "ਕੁੜ");
    }

    #[test]
    fn strips_oblique_plural_iaan() {
        // ਬੱਚਿਆਂ → ਬੱਚ (of children).
        assert_eq!(s("ਬੱਚਿਆਂ"), "ਬੱਚ");
    }

    // -------------------------------------------------------------
    // Case markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_oblique_sg_e() {
        // ਮੁੰਡੇ → ਮੁੰਡ (of the boy / boys direct).
        assert_eq!(s("ਮੁੰਡੇ"), "ਮੁੰਡ");
    }

    #[test]
    fn strips_fem_sg_ii() {
        // ਕੁੜੀ → ਕੁੜ (girl).
        assert_eq!(s("ਕੁੜੀ"), "ਕੁੜ");
    }

    // -------------------------------------------------------------
    // Imperfective participle endings.
    // -------------------------------------------------------------

    #[test]
    fn strips_imperfective_masc_sg_daa() {
        // ਬੋਲਦਾ → ਬੋਲ (speaks, masc sg).
        assert_eq!(s("ਬੋਲਦਾ"), "ਬੋਲ");
    }

    #[test]
    fn strips_imperfective_fem_sg_dii() {
        // ਬੋਲਦੀ → ਬੋਲ (speaks, fem sg).
        assert_eq!(s("ਬੋਲਦੀ"), "ਬੋਲ");
    }

    #[test]
    fn strips_imperfective_masc_pl_de() {
        // ਬੋਲਦੇ → ਬੋਲ (speak, masc pl).
        assert_eq!(s("ਬੋਲਦੇ"), "ਬੋਲ");
    }

    // -------------------------------------------------------------
    // Perfective/aorist endings.
    // -------------------------------------------------------------

    #[test]
    fn strips_perfective_3sg_masc_ia() {
        // ਬੋਲਿਆ → ਬੋਲ (spoke, 3sg-m).
        assert_eq!(s("ਬੋਲਿਆ"), "ਬੋਲ");
    }

    #[test]
    fn strips_perfective_3sg_fem_ii() {
        // ਬੋਲੀ → ਬੋਲ (spoke, 3sg-f).
        assert_eq!(s("ਬੋਲੀ"), "ਬੋਲ");
    }

    #[test]
    fn strips_perfective_3pl_masc_e() {
        // ਬੋਲੇ → ਬੋਲ (spoke, 3pl-m).
        assert_eq!(s("ਬੋਲੇ"), "ਬੋਲ");
    }

    #[test]
    fn strips_perfective_3pl_fem_iiaan() {
        // ਬੋਲੀਆਂ → ਬੋਲ (spoke, 3pl-f).
        assert_eq!(s("ਬੋਲੀਆਂ"), "ਬੋਲ");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_iiaan_over_ii() {
        // ਕੁੜੀਆਂ — ੀਆਂ (3) beats bare ੀ (1).
        assert_eq!(s("ਕੁੜੀਆਂ"), "ਕੁੜ");
    }

    #[test]
    fn longest_match_wins_iaan_over_aan() {
        // ਬੱਚਿਆਂ — ਿਆਂ (3) beats bare ਾਂ (2, doesn't even match here).
        assert_eq!(s("ਬੱਚਿਆਂ"), "ਬੱਚ");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("ਪੰਜਾਬ"), "ਪੰਜਾਬ");
        assert_eq!(s("ਘਰ"), "ਘਰ");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in [
            "ਘਰਾਂ",
            "ਕੁੜੀਆਂ",
            "ਬੱਚਿਆਂ",
            "ਮੁੰਡੇ",
            "ਕੁੜੀ",
            "ਬੋਲਦਾ",
            "ਬੋਲਦੀ",
            "ਬੋਲਿਆ",
            "ਬੋਲੀਆਂ",
        ] {
            let once = LightPunjabiStemmer.stem(w).into_owned();
            let twice = LightPunjabiStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "ਦਾ" alone — 2 chars, stripping ਦਾ leaves 0 → guarded.
        assert_eq!(s("ਦਾ"), "ਦਾ");
        // "ਦੇ" alone — 2 chars, stripping ਦੇ leaves 0 → guarded;
        // also stripping bare ੇ leaves 1 → still guarded.
        assert_eq!(s("ਦੇ"), "ਦੇ");
        // "ਹਾਂ" — 3 chars, stripping ਾਂ leaves 1 → guarded.
        assert_eq!(s("ਹਾਂ"), "ਹਾਂ");
        // "ੇ" alone — 1 char, MIN_STEM_CHARS short-circuit.
        assert_eq!(s("ੇ"), "ੇ");
    }

    #[test]
    fn tippi_bindi_addak_never_stripped_alone() {
        // Tippi (ੰ U+0A70) — nasalization sign; never in the suffix
        // table so never touched.
        assert_eq!(s("ਪੰਜਾਬ"), "ਪੰਜਾਬ");
        // Bindi (ਂ U+0A02) — inside ਹੈਂ (are, 2sg): no suffix in the
        // table ends in bare bindi, so no strip.
        assert_eq!(s("ਹੈਂ"), "ਹੈਂ");
        // Addak (ੱ U+0A71) — inside ਪੱਕਾ; would only strip -ਾ if it
        // were in the table (it's not); the full 2-scalar -ਦਾ / -ਦੀ /
        // -ਦੇ requires a preceding ਦ, so ਪੱਕਾ stays.
        assert_eq!(s("ਪੱਕਾ"), "ਪੱਕਾ");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["ਪੰਜਾਬ", "ਘਰਾਂ", "ਕੁੜੀਆਂ", "hello", "ਬੋਲਦਾ"]
        {
            let out = LightPunjabiStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightPunjabiStemmer.stem("ਪੰਜਾਬ");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightPunjabiStemmer.stem("ਘਰਾਂ");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
