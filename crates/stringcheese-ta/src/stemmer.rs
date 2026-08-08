//! [`LightTamilStemmer`] — a light Tamil suffix stripper.
//!
//! # Scope
//!
//! Tamil is **agglutinative** in the same family sense as Finnish and
//! Korean: nouns stack case + plural + possessive suffixes and verbs
//! stack tense / person / number endings onto a single orthographic
//! word. No rule-based light stemmer can cover this in full; a proper
//! Tamil morphological analyzer needs a lexicon and finite-state
//! transducers. This module ships a **deliberately conservative**
//! longest-match-wins suffix stripper covering the highest-frequency
//! inflectional endings — enough to normalize surface forms in an IR
//! index but not enough to recover the underlying lemma in every
//! case.
//!
//! There is **no canonical Snowball Tamil algorithm** in the Snowball
//! catalogue. The reference practice for Tamil IR is a hand-tuned
//! affix stripper; this module is a starter — a downstream
//! `stringcheese-ta-morph` crate would ship the full analyzer.
//!
//! # Suffix table (longest-match wins)
//!
//! One pass, longest-match-wins, over the following (all Tamil):
//!
//! | Suffix                      | Chars | Class                          |
//! |-----------------------------|-------|--------------------------------|
//! | `-கிறீர்கள்` / `கிறார்கள்` | 9    | verb: 2p-pl / 3p-pl present    |
//! | `-கிறேன்`                  | 6    | verb: 1p-sg present            |
//! | `-கிறாய்`                   | 6    | verb: 2p-sg present            |
//! | `-கிறார்`                   | 6    | verb: 3p-polite present        |
//! | `-கிறோம்`                  | 6    | verb: 1p-pl present            |
//! | `-கின்று`                  | 6    | present-tense marker           |
//! | `-உடன்`                     | 4    | sociative case (independent u) |
//! | `-கிறு`                     | 4    | present-tense marker           |
//! | `-கள்`                       | 3    | plural marker                  |
//! | `-ஆல்` / `-ால்`            | 3    | instrumental case              |
//! | `-இன்` / `-ின்`             | 3    | genitive case                  |
//! | `-இல்` / `-ில்`             | 3    | locative case                  |
//! | `-ஓடு` / `-ோடு`           | 3    | sociative case                 |
//! | `-வரை`                       | 3    | allative case                  |
//! | `-கு`                         | 2    | dative case                    |
//! | `-ஐ` / `-ை`                | 1    | accusative case                |
//! | `-ஆ`                          | 1    | interrogative particle         |
//! | `-ஏ`                          | 1    | interrogative / emphatic       |
//! | `-வ`                          | 1    | future-tense marker (bare v)   |
//!
//! Where a suffix begins with an independent-vowel scalar (e.g. `-ஆல்`
//! begins with `ஆ` U+0B86, `-இன்` begins with `இ` U+0B87) the surface
//! form after a consonant stem uses the corresponding **matra** —
//! `ா` U+0BBE for `ஆ`, `ி` U+0BBF for `இ`, `ோ` U+0BCB for `ஓ`. Both
//! spellings are shipped in the table so the stemmer normalizes both
//! surface variants of the same case marker.
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **Past-tense verb morphology.** Tamil past-tense infixes
//!   (`-த்த்-`, `-ன்ன்-`, `-ண்ட்-`, `-இன்-`) appear *before* the
//!   personal ending and vary by verb class; stripping them safely
//!   needs a lexicon (they interact with sandhi at the stem
//!   boundary). Left to the follow-up morphology crate.
//! * **Standalone matras / independent vowels at word-end.**
//!   Word-final `ா` U+0BBE, `ே` U+0BC7, `ூ` U+0BC2, `ௌ` U+0BCC are
//!   overwhelmingly part of the surface *stem* (many Tamil words
//!   end in a legitimate vowel matra); stripping them would
//!   over-strip more often than help. The interrogative particles
//!   are shipped as their **independent-vowel** forms only — a
//!   modest recall loss traded for a large precision gain.
//! * **Grantha sandhi.** Tamil Grantha loans (`ஜ`, `ஷ`, `ஸ`,
//!   `ஹ`) participate in modified sandhi in Sanskrit-loaned
//!   compounds; not handled.
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte offsets —
//! and matched at the *scalar-slice tail* of the input's `Vec<char>`
//! form. Tamil scalars are 3 UTF-8 bytes each, so byte-level
//! arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a bare suffix standing
//! alone (`கள்` on its own; `கு` on its own).
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

// Common Tamil scalars used by the suffix table. Named constants make
// the table below readable — Tamil scalars are visually similar and
// their code points are easy to fat-finger.
const PULLI: char = '\u{0BCD}'; // ் virama / pulli
const MATRA_AA: char = '\u{0BBE}'; // ா
const MATRA_I: char = '\u{0BBF}'; // ி
const MATRA_II: char = '\u{0BC0}'; // ீ
const MATRA_U: char = '\u{0BC1}'; // ு
const MATRA_E: char = '\u{0BC7}'; // ே
const MATRA_AI: char = '\u{0BC8}'; // ை
const MATRA_O: char = '\u{0BCB}'; // ோ
const V_AA: char = '\u{0B86}'; // ஆ
const V_I: char = '\u{0B87}'; // இ
const V_E: char = '\u{0B8F}'; // ஏ
const V_AI: char = '\u{0B90}'; // ஐ
const V_U: char = '\u{0B89}'; // உ
const V_O: char = '\u{0B93}'; // ஓ
const C_KA: char = '\u{0B95}'; // க
const C_TTA: char = '\u{0B9F}'; // ட
const C_NA_ALV: char = '\u{0BA9}'; // ன (alveolar n)
const C_MA: char = '\u{0BAE}'; // ம
const C_YA: char = '\u{0BAF}'; // ய
const C_RA: char = '\u{0BB0}'; // ர
const C_RRA: char = '\u{0BB1}'; // ற
const C_LA: char = '\u{0BB2}'; // ல
const C_LLA: char = '\u{0BB3}'; // ள
const C_VA: char = '\u{0BB5}'; // வ

/// Suffix table, sorted longest-first. Every entry is a
/// scalar-sequence (`&'static [char]`) rather than a byte string — the
/// stemmer matches on `Vec<char>` tails, never raw bytes.
///
/// Longest-match-wins is enforced by iterating the table in order and
/// returning on the first match, and by ordering the table strictly
/// by descending scalar length.
const SUFFIXES: &[&[char]] = &[
    // ---------------------------------------------------------------
    // 9-scalar verb endings (2p-pl / 3p-pl present indicative).
    // ---------------------------------------------------------------
    // கிறீர்கள் — you (plural) present
    &[
        C_KA, MATRA_I, C_RRA, MATRA_II, C_RA, PULLI, C_KA, C_LLA, PULLI,
    ],
    // கிறார்கள் — they present
    &[
        C_KA, MATRA_I, C_RRA, MATRA_AA, C_RA, PULLI, C_KA, C_LLA, PULLI,
    ],
    // ---------------------------------------------------------------
    // 6-scalar verb personal endings and tense marker.
    // ---------------------------------------------------------------
    // கிறேன் — I present
    &[C_KA, MATRA_I, C_RRA, MATRA_E, C_NA_ALV, PULLI],
    // கிறாய் — you (familiar) present
    &[C_KA, MATRA_I, C_RRA, MATRA_AA, C_YA, PULLI],
    // கிறார் — he/she (polite) present
    &[C_KA, MATRA_I, C_RRA, MATRA_AA, C_RA, PULLI],
    // கிறோம் — we present
    &[C_KA, MATRA_I, C_RRA, MATRA_O, C_MA, PULLI],
    // கின்று — present-tense marker (formal / literary)
    &[C_KA, MATRA_I, C_NA_ALV, PULLI, C_RRA, MATRA_U],
    // ---------------------------------------------------------------
    // 4-scalar case marker and tense marker.
    // ---------------------------------------------------------------
    // உடன் — sociative (with) — independent-vowel form
    &[V_U, C_TTA, C_NA_ALV, PULLI],
    // கிறு — present-tense marker (bare, no personal ending)
    &[C_KA, MATRA_I, C_RRA, MATRA_U],
    // ---------------------------------------------------------------
    // 3-scalar plural marker and case markers.
    // ---------------------------------------------------------------
    // கள் — plural marker
    &[C_KA, C_LLA, PULLI],
    // ஆல் — instrumental (independent-vowel form)
    &[V_AA, C_LA, PULLI],
    // ால் — instrumental (matra form after consonant)
    &[MATRA_AA, C_LA, PULLI],
    // இன் — genitive (independent-vowel form)
    &[V_I, C_NA_ALV, PULLI],
    // ின் — genitive (matra form)
    &[MATRA_I, C_NA_ALV, PULLI],
    // இல் — locative (independent-vowel form)
    &[V_I, C_LA, PULLI],
    // ில் — locative (matra form)
    &[MATRA_I, C_LA, PULLI],
    // ஓடு — sociative (independent-vowel form)
    &[V_O, C_TTA, MATRA_U],
    // ோடு — sociative (matra form)
    &[MATRA_O, C_TTA, MATRA_U],
    // வரை — allative (until / up to)
    &[C_VA, C_RA, MATRA_AI],
    // ---------------------------------------------------------------
    // 2-scalar dative.
    // ---------------------------------------------------------------
    // கு — dative marker
    &[C_KA, MATRA_U],
    // ---------------------------------------------------------------
    // 1-scalar particles and future marker.
    // ---------------------------------------------------------------
    // ை — accusative matra
    &[MATRA_AI],
    // ஐ — accusative (independent form)
    &[V_AI],
    // ஆ — interrogative particle
    &[V_AA],
    // ஏ — interrogative / emphatic
    &[V_E],
    // வ — future-tense marker (bare)
    &[C_VA],
];

/// The light Tamil suffix stripper.
///
/// A zero-sized value; construct as [`LightTamilStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_ta::LightTamilStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural marker -கள்.
/// assert_eq!(LightTamilStemmer.stem("மாணவர்கள்"), "மாணவர்");
/// // Strip the 1p-sg present ending -கிறேன்.
/// assert_eq!(LightTamilStemmer.stem("பேசுகிறேன்"), "பேசு");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightTamilStemmer.stem("தமிழ்"), "தமிழ்");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightTamilStemmer;

impl LightTamilStemmer {
    /// Stems `word` per the light Tamil rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Tamil letter is 3 bytes in UTF-8, so any code that mixed
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

impl Stemmer for LightTamilStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightTamilStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Plural marker.
    // -------------------------------------------------------------

    #[test]
    fn strips_plural_marker_kal() {
        // மாணவர்கள் → மாணவர் (students).
        assert_eq!(s("மாணவர்கள்"), "மாணவர்");
    }

    // -------------------------------------------------------------
    // Case markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_locative_il_matra() {
        // வீட்டில் → வீட்ட (in the house).
        assert_eq!(s("வீட்டில்"), "வீட்ட");
    }

    #[test]
    fn strips_instrumental_aal_matra() {
        // மரத்தால் → மரத்த (by the tree).
        assert_eq!(s("மரத்தால்"), "மரத்த");
    }

    #[test]
    fn strips_dative_ku() {
        // அவனுக்கு → அவனுக் (to him).
        assert_eq!(s("அவனுக்கு"), "அவனுக்");
    }

    #[test]
    fn strips_genitive_in_matra() {
        // நண்பனின் → நண்பன (of the friend).
        assert_eq!(s("நண்பனின்"), "நண்பன");
    }

    // -------------------------------------------------------------
    // Verb personal endings.
    // -------------------------------------------------------------

    #[test]
    fn strips_verb_1p_singular_kiren() {
        // பேசுகிறேன் → பேசு (I speak).
        assert_eq!(s("பேசுகிறேன்"), "பேசு");
    }

    #[test]
    fn strips_verb_2p_singular_kiray() {
        // பேசுகிறாய் → பேசு (you speak).
        assert_eq!(s("பேசுகிறாய்"), "பேசு");
    }

    #[test]
    fn strips_verb_3p_polite_kirar() {
        // பேசுகிறார் → பேசு (he/she polite present).
        assert_eq!(s("பேசுகிறார்"), "பேசு");
    }

    #[test]
    fn strips_verb_1p_plural_kirom() {
        // பேசுகிறோம் → பேசு (we speak).
        assert_eq!(s("பேசுகிறோம்"), "பேசு");
    }

    #[test]
    fn strips_verb_3p_plural_kirarkal() {
        // பேசுகிறார்கள் → பேசு (they speak).
        assert_eq!(s("பேசுகிறார்கள்"), "பேசு");
    }

    #[test]
    fn strips_verb_2p_plural_kirirkal() {
        // பேசுகிறீர்கள் → பேசு (you plural speak).
        assert_eq!(s("பேசுகிறீர்கள்"), "பேசு");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_kirarkal_over_kal() {
        // பேசுகிறார்கள் — கிறார்கள் (9) beats bare கள் (3).
        assert_eq!(s("பேசுகிறார்கள்"), "பேசு");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("தமிழ்"), "தமிழ்");
        assert_eq!(s("நான்"), "நான்");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in [
            "மாணவர்கள்",
            "வீட்டில்",
            "மரத்தால்",
            "அவனுக்கு",
            "நண்பனின்",
            "பேசுகிறேன்",
            "பேசுகிறார்கள்",
        ] {
            let once = LightTamilStemmer.stem(w).into_owned();
            let twice = LightTamilStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "கள்" alone — 3 chars, stripping கள் leaves 0 → guarded.
        assert_eq!(s("கள்"), "கள்");
        // "கு" alone — 2 chars, stripping கு leaves 0 → guarded.
        assert_eq!(s("கு"), "கு");
        // "ஐ" alone — 1 char, MIN_STEM_CHARS short-circuit.
        assert_eq!(s("ஐ"), "ஐ");
        // "ஆ" alone — 1 char, MIN_STEM_CHARS short-circuit.
        assert_eq!(s("ஆ"), "ஆ");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["தமிழ்", "மாணவர்கள்", "வீட்டில்", "பேசுகிறேன்", "hello"]
        {
            let out = LightTamilStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightTamilStemmer.stem("தமிழ்");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightTamilStemmer.stem("மாணவர்கள்");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
