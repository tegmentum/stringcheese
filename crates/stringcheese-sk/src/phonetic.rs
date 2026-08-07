//! A Slovak-tuned Soundex-family phonetic encoder — PHONEX-Slovak.
//!
//! # Origin
//!
//! Slovak, like Czech and Dutch, lacks a single canonical published
//! phonetic encoder the way English has Soundex or German has Kölner
//! Phonetik. Two candidate approaches present themselves:
//!
//! * **Diacritic-strip transliteration to ASCII.** Slovak uses Latin
//!   letters with diacritics; a diacritic-strip pass produces a
//!   deterministic ASCII rendering (`kráľ → kral`, `späť → spat`,
//!   `žltý → zlty`). This is a *transliteration*, not a *phonetic
//!   encoder* — two words that sound alike don't necessarily encode
//!   alike (there are no Soundex-style consonant merges).
//! * **PHONEX-Slovak.** A Dutch / Portuguese / Spanish / Czech-style
//!   Soundex-shape encoder with Slovak-tuned preprocessing (haček
//!   fold including the Slovak-specific `ľ`, long-vowel fold
//!   including `ĺ` / `ŕ`, `ä → E`, `ô → O`, silent `h`, `ch → X`)
//!   over a Slovak-tuned classification table. Produces sound-alike
//!   equivalence classes and matches the shape of the other
//!   Latin-alphabet packs' phonetic hookups.
//!
//! # Implementation choice — PHONEX-Slovak
//!
//! This module ships a **PHONEX-Slovak** encoder for **consistency
//! with the other Latin-alphabet language packs** (Dutch, Portuguese,
//! Spanish, French, Czech all ship PHONEX-family encoders). Concretely,
//! the algorithm is a 4-character `<letter><digit><digit><digit>`
//! Soundex-shape key with Slovak-tuned preprocessing and classification:
//!
//! 1. **Uppercase and fold Slovak-specific letters.** Under Slovak's
//!    orthography-vs-phonology relationship:
//!    * **Haček (caron) consonants** map to Latin placeholders in the
//!      same phonetic class as the base letter, so `č → C`, `š → S`,
//!      `ž → Z`, `ď → D`, `ť → T`, `ň → N`, and — Slovak-specific —
//!      `ľ → L`. (Each haček variant participates in the same
//!      Soundex class as its base: `č/c` in the sibilant/affricate
//!      class, `š/s/z/ž` in the sibilant class, `ď/t/ť/d` in the
//!      dental class, `ň/n` in the nasal class, `ľ/l` in the liquid
//!      class.)
//!    * **Long vowels** fold to their short counterparts (`á → A`,
//!      `é → E`, `í → I`, `ó → O`, `ú → U`, `ý → Y`); the syllabic
//!      long consonants fold to their base letter (`ĺ → L`, `ŕ → R`).
//!      Vowels are dropped in the Soundex-shape encoding anyway
//!      (they belong to the zero class), so this fold's practical
//!      effect is on the *seed* letter: a word beginning with `Ú` and
//!      one beginning with `U` produce the same key.
//!    * **Slovak-specific `ä → E`.** The Slovak `ä` is phonetically
//!      an open-front vowel /æ/, closer to /e/ than to /a/. Folding
//!      to `E` puts it in the vowel class (dropped in the
//!      Soundex-shape encoding) but resolves the seed-letter case
//!      correctly: a word beginning with `Ä` encodes with an `E`
//!      seed, matching the phonetic contour.
//!    * **Slovak-specific `ô → O`.** The Slovak `ô` orthographically
//!      marks the diphthong /uo/; the fold to `O` matches the
//!      long-vowel convention (drop the diacritic; keep the base
//!      vowel).
//! 2. **Silent `H`.** Slovak `h` is a voiced glottal fricative /ɦ/
//!    which for Soundex-shape purposes we drop (matching the Dutch /
//!    Spanish / Portuguese / Czech packs' silent-H convention). This
//!    is linguistically simplified but keeps the encoder consistent
//!    with the sibling Latin packs and conservative on discrimination.
//! 3. **Digraph `ch`.** Slovak treats `ch` as a single letter for
//!    collation, spelled as two ASCII scalars. The encoder folds `CH`
//!    to a velar placeholder `X` (which lives in the velar class 2)
//!    so `chlieb` and `xlieb` encode alike.
//! 4. **`RR → R` and duplicate-consonant collapse.** Consecutive
//!    equal Soundex codes always collapse (the standard Soundex
//!    duplicate-drop rule); the `RR → R` preprocessing step is a
//!    special case for the Slovak / Latin convention.
//! 5. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below; drop
//!    the zero class (vowels); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach
//!    length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X (velars; includes CH-fold X) |
//! | 3    | D T (dental stops; includes haček-fold `ď→D`, `ť→T`) |
//! | 4    | L (includes Slovak-specific `ľ→L` and `ĺ→L`) |
//! | 5    | M N (nasals; includes haček-fold `ň→N`) |
//! | 6    | R (liquid; includes long-syllabic `ŕ→R`) |
//! | 7    | S Z (sibilants; includes haček-fold `š→S`, `ž→Z`) |
//! | 0    | A E I O U Y (dropped as vowels); H is stripped in preprocessing |
//!
//! # Adapter name
//!
//! `"phonex-sk"` — chosen for consistency with the other Latin-
//! alphabet packs' PHONEX adapters (`phonex-nl`, `phonex-pt`,
//! `phonex-es`, `phonex-fr`, `phonex-cs`). Callers who need the
//! alternative transliteration-only approach can compose their own
//! diacritic-strip pass; the PHONEX-Slovak encoder is intended for
//! sound-alike record linkage.
//!
//! # Slovak-specific letters vs. Czech
//!
//! The Slovak fold table adds:
//!
//! * `ľ → L` (Slovak-only palatal lateral)
//! * `ĺ → L` (Slovak-only long syllabic *l*)
//! * `ŕ → R` (Slovak-only long syllabic *r*)
//! * `ä → E` (Slovak-only open-e vowel)
//! * `ô → O` (Slovak-only diphthong marker)
//!
//! And omits (Slovak does not have these letters):
//!
//! * `ř → R` (Czech-only fricative-trill)
//! * `ě → E` (Czech-only palatalizing vowel)
//! * `ů → U` (Czech-only ring-over-u)
//!
//! # Deferred to a follow-up wave
//!
//! * **Diacritic-strip Slovak transliteration adapter.** A parallel
//!   `iso-9-sk` adapter that returns a diacritic-stripped ASCII
//!   rendering would suit library-catalog interop. Shipped as an
//!   alternative encoder, not a replacement.
//! * **Métaphone Slovak / Slavic Metaphone.** A variable-length
//!   encoder with better discrimination; heavier to reference-test.
//! * **True `h → ɦ` handling.** Slovak `h` is not truly silent; a
//!   more accurate encoder would place it in the velar class
//!   alongside `ch`. The current silent-H convention prioritizes
//!   cross-Latin-pack consistency over Slovak-phonology accuracy.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Slovak PHONEX encoder.
///
/// A zero-sized value; construct as [`SlovakPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_sk::SlovakPhonex;
///
/// let key = SlovakPhonex.encode("Novák").unwrap();
/// // N seed, O vowel (reset), V code=1, A vowel (reset), K code=2
/// //   → "N" + "1" + "2" → "N12" pad → "N120".
/// assert_eq!(key, "N120");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlovakPhonex;

impl SlovakPhonex {
    /// Encodes `word` per the PHONEX-Slovak algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to
    /// nothing after silent-`H` stripping). Otherwise returns a
    /// 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state.
                last_code = b'0';
                continue;
            }
            if code == last_code {
                continue;
            }
            out.push(code as char);
            last_code = code;
            if out.len() == 4 {
                break;
            }
        }
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess `word` into uppercase-ASCII letters after Slovak digraph
/// and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII (drops non-letter code points).
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions on the ASCII form.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph substitutions.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'C', b'H') => {
                    out.push('X');
                    i += 2;
                    continue;
                }
                (b'R', b'R') => {
                    out.push('R');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* silent — drop */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Slovak-specific letter folds.
    // Long vowels fold to their short counterparts.
    // Haček consonants fold to their base-letter placeholder.
    // Slovak-only letters: `ľ→L`, `ĺ→L`, `ŕ→R`, `ä→E`, `ô→O`.
    // Long vowels fold to their short counterparts. Slovak-only
    // `ä` (open-front vowel /æ/, closer to `e` than `a`) folds to
    // `E`, so it shares the arm with `é/É`. Slovak-only `ô` (the
    // diphthong /uo/) folds to `O`, so it shares the arm with `ó/Ó`.
    // Slovak-only `ľ/Ľ` (palatal lateral) and `ĺ/Ĺ` (long syllabic l)
    // both fold to `L`, so they share the arm with the ASCII `L` path.
    // Slovak-only `ŕ/Ŕ` (long syllabic r) folds to `R`.
    let folded = match c {
        'á' | 'Á' => 'A',
        'é' | 'É' | 'ä' | 'Ä' => 'E',
        'í' | 'Í' => 'I',
        'ó' | 'Ó' | 'ô' | 'Ô' => 'O',
        'ú' | 'Ú' => 'U',
        'ý' | 'Ý' => 'Y',
        // Haček consonants — fold to base placeholder.
        'č' | 'Č' => 'C',
        'š' | 'Š' => 'S',
        'ž' | 'Ž' => 'Z',
        'ď' | 'Ď' => 'D',
        'ť' | 'Ť' => 'T',
        'ň' | 'Ň' => 'N',
        // Slovak-specific palatal / long syllabic l — both fold to L.
        'ľ' | 'Ľ' | 'ĺ' | 'Ĺ' => 'L',
        // Slovak-specific long syllabic r.
        'ŕ' | 'Ŕ' => 'R',
        _ => return None,
    };
    Some(folded)
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'K' | b'G' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        _ => b'0',
    }
}

/// Adapter that exposes [`SlovakPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Slovak::phonetic_encoder`](crate::Slovak) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlovakPhonexAdapter;

impl LanguagePhoneticEncoder for SlovakPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        SlovakPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-sk"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        SlovakPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(SlovakPhonex.encode("").is_none());
        assert!(SlovakPhonex.encode("   ").is_none());
        assert!(SlovakPhonex.encode("---").is_none());
    }

    #[test]
    fn haceks_fold_to_base_letters() {
        // č/c share the class.
        assert_eq!(p("čech"), p("cech"));
        // š/s share:
        assert_eq!(p("šum"), p("sum"));
        // ž/z share:
        assert_eq!(p("žena"), p("zena"));
        // ď/d, ť/t, ň/n:
        assert_eq!(p("deti"), p("děti".replace('ě', "e").as_str()));
        assert_eq!(p("ťuk"), p("tuk"));
        assert_eq!(p("kôň"), p("kon"));
    }

    #[test]
    fn slovak_palatal_l_folds_to_l() {
        // ľ (Slovak-only) folds to L, so kráľ encodes like kral.
        assert_eq!(p("kráľ"), p("kral"));
    }

    #[test]
    fn slovak_long_syllabics_fold_to_base() {
        // ĺ → L, ŕ → R. `stĺp` encodes like `stlp`; `vŕba` like `vrba`.
        assert_eq!(p("stĺp"), p("stlp"));
        assert_eq!(p("vŕba"), p("vrba"));
    }

    #[test]
    fn slovak_ae_folds_to_e_not_a() {
        // ä → E (Slovak-specific; open-e sound). `späť` and `spet`
        //   encode alike; `späť` and `spat` need not.
        assert_eq!(p("späť"), p("spet"));
    }

    #[test]
    fn slovak_o_circumflex_folds_to_o() {
        // ô → O. `kôň` and `kon` encode alike.
        assert_eq!(p("kôň"), p("kon"));
    }

    #[test]
    fn long_vowels_fold_to_short() {
        // Long-to-short vowel folds — seed and interior vowels alike
        // reduce to the short counterpart.
        assert_eq!(p("útok"), p("utok"));
        assert_eq!(p("Íra"), p("Ira"));
    }

    #[test]
    fn ch_digraph_encodes_as_velar() {
        // "chlieb" → preprocess CH → X: "XLIEB".
        //   X seed last=2. L code=4 push → "X4" last=4. I vow reset.
        //   E vow reset. B code=1 push → "X41" last=1. Pad → "X410".
        assert_eq!(p("chlieb"), "X410");
    }

    #[test]
    fn silent_h_is_stripped() {
        // "hora" → preprocess H drops → "ORA".
        //   O seed. R code=6 push → "O6". A vow reset. Pad → "O600".
        assert_eq!(p("hora"), "O600");
        // Compare to "ora" (same, no H).
        assert_eq!(p("hora"), p("ora"));
    }

    #[test]
    fn common_slovak_surnames() {
        // Novák: N seed. O vow reset. V code=1 push → N1. A vow reset.
        //   K code=2 push → N12. Pad → N120.
        assert_eq!(p("Novák"), "N120");
        // Kováč: K seed. O vow reset. V code=1 push → K1. A vow reset.
        //   C code=2 push → K12. Pad → K120.
        assert_eq!(p("Kováč"), "K120");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("NOVÁK"), p("novák"));
        assert_eq!(p("ŽENA"), p("žena"));
        assert_eq!(p("KRÁĽ"), p("kráľ"));
        assert_eq!(p("KÔŇ"), p("kôň"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ne"), "N000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Abba" — A seed. B code=1 push → "A1". B dup drop. A vow
        //   reset. Pad → "A100".
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_sk() {
        assert_eq!(SlovakPhonexAdapter.name(), "phonex-sk");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(SlovakPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = SlovakPhonexAdapter.encode("Novák").unwrap();
        assert_eq!(primary, "N120");
        assert!(alt.is_none());
    }
}
