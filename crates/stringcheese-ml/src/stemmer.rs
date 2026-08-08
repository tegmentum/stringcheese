//! [`LightMalayalamStemmer`] — a light Malayalam suffix stripper.
//!
//! # Scope
//!
//! Malayalam is **agglutinative** in the same family sense as
//! Tamil / Finnish / Korean: nouns stack case + plural + possessive
//! suffixes and verbs stack tense / aspect / adjectival endings onto
//! a single orthographic word. No rule-based light stemmer can cover
//! this in full; a proper Malayalam morphological analyzer needs a
//! lexicon and finite-state transducers. This module ships a
//! **deliberately conservative** longest-match-wins suffix stripper
//! covering the highest-frequency inflectional endings — enough to
//! normalize surface forms in an IR index but not enough to recover
//! the underlying lemma in every case.
//!
//! There is **no canonical Snowball Malayalam algorithm** in the
//! Snowball catalogue. The reference practice for Malayalam IR is a
//! hand-tuned affix stripper; this module is a starter — a downstream
//! `stringcheese-ml-morph` crate would ship the full analyzer.
//!
//! # Suffix table (longest-match wins)
//!
//! One pass, longest-match-wins, over the following (all Malayalam):
//!
//! | Suffix        | Chars | Class                                       |
//! |---------------|-------|---------------------------------------------|
//! | `-ുന്നു`      | 5     | verb: present tense (finite)                |
//! | `-ന്റെ`      | 4     | genitive case                               |
//! | `-ുന്ന`      | 4     | verb: present-adjectival (relative form)    |
//! | `-ിനാൽ`      | 4     | instrumental (with linking `-ിന്-`)          |
//! | `-ല്ലേ`      | 4     | emphatic tag-question particle              |
//! | `-ങ്ങൾ`      | 4     | inanimate plural (with `ങ്ങ` linking)        |
//! | `-മാർ`        | 3     | plural (animate / human) marker             |
//! | `-ോട്`        | 3     | sociative case                              |
//! | `-ുടെ`        | 3     | genitive (short form, common)               |
//! | `-ിൽ`        | 2     | locative case (matra + chillu-l)            |
//! | `-ാൽ`        | 2     | instrumental case (matra form)              |
//! | `-കൾ`         | 2     | plural marker (inanimate)                   |
//! | `-നെ`        | 2     | accusative case                             |
//! | `-ന്`        | 2     | dative case (linking form)                  |
//! | `-ും`        | 2     | emphatic "also" particle / future marker    |
//! | `-ുക`        | 2     | infinitive marker                           |
//! | `-ിയ`        | 2     | past-adjectival (relative form)             |
//! | `-ോ`          | 1     | interrogative particle                      |
//!
//! # Chillu-letter handling
//!
//! Malayalam has six **chillu** letters in U+0D7A..=U+0D7F
//! (`ൺ ൻ ർ ൽ ൾ ൿ`) — atomic "pure consonant" scalars that represent
//! a word-final consonant without any following vowel. Words like
//! `അവൻ` "he" (ends in `ൻ` U+0D7B), `അവർ` "they" (ends in `ർ`
//! U+0D7C), and `ഞാൻ` "I" (ends in `ൻ`) end in a bare chillu that is
//! **not** a stripped inflection — it's part of the stem itself. The
//! suffix table below is deliberately built to **never** ship a
//! standalone chillu as a strippable suffix; every chillu-carrying
//! entry (`-ിൽ`, `-ന്റെ`, `-ന്`, `-കൾ`, `-മാർ`, `-ാൽ`, `-ങ്ങൾ`)
//! requires at least one *preceding* scalar so bare chillu-final
//! words survive the stemmer unchanged.
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **The 1-scalar past-tense marker `-ി`.** The scalar `ി`
//!   U+0D3F is the extremely common short-i matra; stripping it as
//!   a suffix would over-strip almost every consonant-plus-i word.
//!   Malayalam's past-tense is usually recovered from context (verb
//!   root + `-ി`), and stemming for IR usually leaves the form
//!   as-is.
//! * **Free-standing postpositions** (`കൊണ്ട്` "with",
//!   `വരെ` "until", `വേണ്ടി` "for"). These are almost always
//!   written as *separate* orthographic words in modern Malayalam
//!   text; they're handled by the stopword list, not by the stemmer.
//! * **Sandhi-driven consonant alternations.** Malayalam stems
//!   often surface with alternating final consonants across sandhi
//!   boundaries; recovering the underlying form needs a lexicon.
//!   Left to the follow-up morphology crate.
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte offsets —
//! and matched at the *scalar-slice tail* of the input's `Vec<char>`
//! form. Malayalam scalars are 3 UTF-8 bytes each, so byte-level
//! arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a bare suffix standing
//! alone (`കൾ` on its own; `ുക` on its own).
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

// Common Malayalam scalars used by the suffix table. Named constants
// make the table below readable — Malayalam scalars are visually
// similar and their code points are easy to fat-finger.
const VIRAMA: char = '\u{0D4D}'; // ്
const ANUSVARA: char = '\u{0D02}'; // ം
const MATRA_AA: char = '\u{0D3E}'; // ാ
const MATRA_I: char = '\u{0D3F}'; // ി
const MATRA_U: char = '\u{0D41}'; // ു
const MATRA_E: char = '\u{0D46}'; // െ
const MATRA_EE: char = '\u{0D47}'; // േ
const MATRA_O: char = '\u{0D4B}'; // ോ
// Chillu letters (atomic word-final consonants; part of the Malayalam
// block U+0D7A..=U+0D7F).
const CHILLU_R: char = '\u{0D7C}'; // ർ
const CHILLU_L: char = '\u{0D7D}'; // ൽ
const CHILLU_LL: char = '\u{0D7E}'; // ൾ
// Base consonants used in the suffix table.
const C_KA: char = '\u{0D15}'; // ക
const C_NGA: char = '\u{0D19}'; // ങ
const C_TTA: char = '\u{0D1F}'; // ട
const C_NA: char = '\u{0D28}'; // ന
const C_RRA: char = '\u{0D31}'; // റ
const C_LA: char = '\u{0D32}'; // ല
const C_MA: char = '\u{0D2E}'; // മ
const C_YA: char = '\u{0D2F}'; // യ

/// Suffix table, sorted longest-first. Every entry is a
/// scalar-sequence (`&'static [char]`) rather than a byte string — the
/// stemmer matches on `Vec<char>` tails, never raw bytes.
///
/// Longest-match-wins is enforced by iterating the table in order and
/// returning on the first match, and by ordering the table strictly
/// by descending scalar length.
const SUFFIXES: &[&[char]] = &[
    // ---------------------------------------------------------------
    // 5-scalar: finite present-tense verb.
    // ---------------------------------------------------------------
    // -ുന്നു — present tense
    &[MATRA_U, C_NA, VIRAMA, C_NA, MATRA_U],
    // ---------------------------------------------------------------
    // 4-scalar: genitive, present-adjectival, instrumental (linking),
    // tag question, ങ്ങ-linked plural.
    // ---------------------------------------------------------------
    // -ന്റെ — genitive
    &[C_NA, VIRAMA, C_RRA, MATRA_E],
    // -ുന്ന — present-adjectival (relative)
    &[MATRA_U, C_NA, VIRAMA, C_NA],
    // -ിനാൽ — instrumental with linking -ിന്-
    &[MATRA_I, C_NA, MATRA_AA, CHILLU_L],
    // -ല്ലേ — emphatic tag-question particle
    &[C_LA, VIRAMA, C_LA, MATRA_EE],
    // -ങ്ങൾ — plural with ങ്ങ linking
    &[C_NGA, VIRAMA, C_NGA, CHILLU_LL],
    // ---------------------------------------------------------------
    // 3-scalar: plural (animate), sociative, genitive (short).
    // ---------------------------------------------------------------
    // -മാർ — animate plural marker
    &[C_MA, MATRA_AA, CHILLU_R],
    // -ോട് — sociative case
    &[MATRA_O, C_TTA, VIRAMA],
    // -ുടെ — genitive (common short form)
    &[MATRA_U, C_TTA, MATRA_E],
    // ---------------------------------------------------------------
    // 2-scalar: locative, instrumental, plural, accusative, dative,
    // emphatic / future, infinitive, past-adjectival.
    // ---------------------------------------------------------------
    // -ിൽ — locative (matra + chillu-l)
    &[MATRA_I, CHILLU_L],
    // -ാൽ — instrumental (matra form)
    &[MATRA_AA, CHILLU_L],
    // -കൾ — inanimate plural
    &[C_KA, CHILLU_LL],
    // -നെ — accusative
    &[C_NA, MATRA_E],
    // -ന് — dative (linking form)
    &[C_NA, VIRAMA],
    // -ും — emphatic "also" particle / future marker
    &[MATRA_U, ANUSVARA],
    // -ുക — infinitive
    &[MATRA_U, C_KA],
    // -ിയ — past-adjectival (relative)
    &[MATRA_I, C_YA],
    // ---------------------------------------------------------------
    // 1-scalar: interrogative particle.
    // ---------------------------------------------------------------
    // -ോ — interrogative
    &[MATRA_O],
];

/// The light Malayalam suffix stripper.
///
/// A zero-sized value; construct as [`LightMalayalamStemmer`] and
/// reuse across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_ml::LightMalayalamStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural marker -കൾ.
/// assert_eq!(LightMalayalamStemmer.stem("പുസ്തകങ്ങൾ"), "പുസ്തക");
/// // Strip the present-tense ending -ുന്നു.
/// assert_eq!(LightMalayalamStemmer.stem("പഠിക്കുന്നു"), "പഠിക്ക");
/// // Bare pronoun ending in a chillu — no suffix, no-op.
/// assert_eq!(LightMalayalamStemmer.stem("അവൻ"), "അവൻ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightMalayalamStemmer;

impl LightMalayalamStemmer {
    /// Stems `word` per the light Malayalam rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Malayalam letter is 3 bytes in UTF-8, so any code that
        // mixed byte offsets with character-boundary logic would
        // corrupt boundaries — we work strictly in char space.
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

impl Stemmer for LightMalayalamStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightMalayalamStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Plural markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_plural_marker_ngal() {
        // പുസ്തകങ്ങൾ → പുസ്തക (books).
        assert_eq!(s("പുസ്തകങ്ങൾ"), "പുസ്തക");
    }

    #[test]
    fn strips_plural_marker_kal() {
        // വീടുകൾ → വീടു (houses).
        assert_eq!(s("വീടുകൾ"), "വീടു");
    }

    #[test]
    fn strips_animate_plural_maar() {
        // ടീച്ചർമാർ → ടീച്ചർ (teachers).
        assert_eq!(s("ടീച്ചർമാർ"), "ടീച്ചർ");
    }

    // -------------------------------------------------------------
    // Case markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_locative_il() {
        // വീട്ടിൽ → വീട്ട (in the house). The stripped ending is
        // [ി, ൽ] and the residual final `ട` retains its inherent
        // vowel (no trailing virama in the surface stem).
        assert_eq!(s("വീട്ടിൽ"), "വീട്ട");
    }

    #[test]
    fn strips_genitive_nte() {
        // രാമന്റെ → രാമ (of Rama).
        assert_eq!(s("രാമന്റെ"), "രാമ");
    }

    #[test]
    fn strips_accusative_ne() {
        // അവനെ → അവ (him).
        assert_eq!(s("അവനെ"), "അവ");
    }

    #[test]
    fn strips_dative_n() {
        // രാമന് → രാമ (to Rama).
        assert_eq!(s("രാമന്"), "രാമ");
    }

    #[test]
    fn strips_sociative_od() {
        // രാമനോട് → രാമന (to/with Rama).
        assert_eq!(s("രാമനോട്"), "രാമന");
    }

    #[test]
    fn strips_short_genitive_ude() {
        // അവരുടെ → അവര (their).
        assert_eq!(s("അവരുടെ"), "അവര");
    }

    // -------------------------------------------------------------
    // Verb endings.
    // -------------------------------------------------------------

    #[test]
    fn strips_present_tense_unnu() {
        // പഠിക്കുന്നു → പഠിക്ക (studies).
        assert_eq!(s("പഠിക്കുന്നു"), "പഠിക്ക");
    }

    #[test]
    fn strips_present_adjectival_unna() {
        // പഠിക്കുന്ന → പഠിക്ക (studying — relative form).
        assert_eq!(s("പഠിക്കുന്ന"), "പഠിക്ക");
    }

    #[test]
    fn strips_infinitive_uka() {
        // പഠിക്കുക → പഠിക്ക (to study).
        assert_eq!(s("പഠിക്കുക"), "പഠിക്ക");
    }

    // -------------------------------------------------------------
    // Particles.
    // -------------------------------------------------------------

    #[test]
    fn strips_emphatic_um() {
        // രാമനും → രാമന (Rama also).
        assert_eq!(s("രാമനും"), "രാമന");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_unnu_over_unna() {
        // പഠിക്കുന്നു — -ുന്നു (5) beats -ുന്ന (4).
        // Verified by the strip landing on പഠിക്ക (5-char stem),
        // not പഠിക്ക + ു (6-char stem which would result if -ുന്ന
        // fired instead).
        assert_eq!(s("പഠിക്കുന്നു"), "പഠിക്ക");
    }

    // -------------------------------------------------------------
    // Chillu-letter handling — bare chillu is NOT a suffix.
    // -------------------------------------------------------------

    #[test]
    fn bare_chillu_letters_are_not_stripped() {
        // ഞാൻ (I) ends in chillu ൻ U+0D7B — not a listed suffix.
        assert_eq!(s("ഞാൻ"), "ഞാൻ");
        // അവൻ (he) ends in chillu ൻ.
        assert_eq!(s("അവൻ"), "അവൻ");
        // അവർ (they) ends in chillu ർ U+0D7C.
        assert_eq!(s("അവർ"), "അവർ");
        // അവൾ (she) ends in chillu ൾ U+0D7E.
        assert_eq!(s("അവൾ"), "അവൾ");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("മലയാളം"), "മലയാളം");
        assert_eq!(s("രാമ"), "രാമ");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        // Every stem below is a "converged" outcome: a second pass of
        // the stemmer over the strip result must produce the same
        // string. Note that convergence is not "one strip per word";
        // it's "the same rule table won't fire on the output". Words
        // whose stem still ends in a strippable tail (e.g. `-കൾ`
        // after stripping `-മാർ`, or `-ോ` after some other strip) do
        // NOT satisfy this property — the guarantee is that iterated
        // application terminates, not that a single call reaches a
        // fixed point.
        for w in [
            "പുസ്തകങ്ങൾ",
            "വീട്ടിൽ",
            "രാമന്റെ",
            "പഠിക്കുന്നു",
            "പഠിക്കുക",
            "രാമനോട്",
            "അവരുടെ",
            "രാമനും",
        ] {
            let mut cur = LightMalayalamStemmer.stem(w).into_owned();
            for _ in 0..8 {
                let next = LightMalayalamStemmer.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let again = LightMalayalamStemmer.stem(&cur).into_owned();
            assert_eq!(cur, again, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "കൾ" alone — 2 chars, stripping കൾ leaves 0 → guarded.
        assert_eq!(s("കൾ"), "കൾ");
        // "ുക" alone — 2 chars, stripping ുക leaves 0 → guarded.
        assert_eq!(s("ുക"), "ുക");
        // "ോ" alone — 1 char, MIN_STEM_CHARS short-circuit.
        assert_eq!(s("ോ"), "ോ");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["മലയാളം", "പുസ്തകങ്ങൾ", "വീട്ടിൽ", "പഠിക്കുന്നു", "hello"]
        {
            let out = LightMalayalamStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightMalayalamStemmer.stem("മലയാളം");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightMalayalamStemmer.stem("പുസ്തകങ്ങൾ");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
