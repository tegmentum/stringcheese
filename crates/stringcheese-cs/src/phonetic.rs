//! A Czech-tuned Soundex-family phonetic encoder — PHONEX-Czech.
//!
//! # Origin
//!
//! Czech, like Dutch, lacks a single canonical published phonetic
//! encoder the way English has Soundex or German has Kölner Phonetik.
//! Two candidate approaches present themselves for a Czech phonetic
//! hookup:
//!
//! * **ISO 9-style transliteration to ASCII.** Czech uses Latin
//!   letters with diacritics; a diacritic-strip pass produces a
//!   deterministic ASCII rendering (`křížek → krizek`, `žluťoučký →
//!   zlutoucky`). This is a *transliteration*, not a *phonetic
//!   encoder* — two words that sound alike don't necessarily encode
//!   alike (there are no Soundex-style consonant merges).
//! * **PHONEX-Czech.** A Dutch/Portuguese/Spanish-style Soundex-shape
//!   encoder with Czech-tuned preprocessing (haček fold, long-vowel
//!   fold, silent `h`) over a Czech-tuned classification table.
//!   Produces sound-alike equivalence classes and matches the shape
//!   of the other Latin-alphabet packs' phonetic hookups.
//!
//! # Implementation choice — PHONEX-Czech
//!
//! This module ships a **PHONEX-Czech** encoder for **consistency
//! with the other Latin-alphabet language packs** (Dutch, Portuguese,
//! Spanish, French all ship PHONEX-family encoders). Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>`
//! Soundex-shape key with Czech-tuned preprocessing and classification:
//!
//! 1. **Uppercase and fold Czech-specific letters.** Under Czech's
//!    orthography-vs-phonology relationship:
//!    * **Haček (caron) consonants** map to Latin placeholders in the
//!      same phonetic class as the base letter, so `č → C`, `š → S`,
//!      `ž → Z`, `ř → R`, `ď → D`, `ť → T`, `ň → N`. (Each haček
//!      variant participates in the same Soundex class as its base:
//!      `č/c` in the sibilant/affricate class, `š/s/z/ž` in the
//!      sibilant class, `ř/r` in the liquid class, `ď/t/ť/d` in the
//!      dental class, `ň/n` in the nasal class.)
//!    * **Long vowels** fold to their short counterparts (`á → A`,
//!      `é → E`, `í → I`, `ó → O`, `ú → U`, `ý → Y`), the ring-over-u
//!      folds to `U` (`ů → U`), and the e-caron folds to `E`
//!      (`ě → E`). Vowels are dropped in the Soundex-shape encoding
//!      anyway (they belong to the zero class), so this fold's
//!      practical effect is on the *seed* letter: a word beginning
//!      with `Ú` and one beginning with `U` produce the same key.
//! 2. **Silent `H`.** Czech `h` is a voiced glottal fricative /ɦ/
//!    which for Soundex-shape purposes we drop (matching the Dutch /
//!    Spanish / Portuguese packs' silent-H convention). This is
//!    linguistically simplified — Czech `h` is not truly silent — but
//!    keeps the encoder consistent with the sibling Latin packs and
//!    conservative on discrimination (dropping `h` errs toward false
//!    matches rather than false splits).
//! 3. **Digraph `ch`.** Czech treats `ch` as a single letter for
//!    collation, spelled as two ASCII scalars. The encoder folds `CH`
//!    to a velar placeholder `X` (which lives in the velar class 2)
//!    so `chleba` and `xleba` encode alike.
//! 4. **`RR → R` and duplicate-consonant collapse.** Consecutive
//!    equal Soundex codes always collapse (the standard Soundex
//!    duplicate-drop rule); the `RR → R` preprocessing step is a
//!    special case for the Czech / Latin convention.
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
//! | 4    | L |
//! | 5    | M N (nasals; includes haček-fold `ň→N`) |
//! | 6    | R (liquid; includes haček-fold `ř→R`) |
//! | 7    | S Z (sibilants; includes haček-fold `š→S`, `ž→Z`) |
//! | 0    | A E I O U Y (dropped as vowels); H is stripped in preprocessing |
//!
//! # Adapter name
//!
//! `"phonex-cs"` — chosen for consistency with the other Latin-
//! alphabet packs' PHONEX adapters (`phonex-nl`, `phonex-pt`,
//! `phonex-es`, `phonex-fr`). Callers who need the alternative
//! transliteration-only approach can compose their own diacritic-
//! strip pass; the PHONEX-Czech encoder is intended for sound-alike
//! record linkage.
//!
//! # Deferred to a follow-up wave
//!
//! * **ISO 9-style Czech transliteration adapter.** A parallel
//!   `iso-9-cs` adapter that returns a diacritic-stripped ASCII
//!   rendering would suit library-catalog interop. Shipped as an
//!   alternative encoder, not a replacement.
//! * **Métaphone Czech / Slavic Metaphone.** A variable-length
//!   encoder with better discrimination; heavier to reference-test.
//! * **True `h → ɦ` handling.** Czech `h` is not truly silent — a
//!   more accurate encoder would place it in the velar class
//!   alongside `ch` (with the two folded to the same code). The
//!   current silent-H convention prioritizes cross-Latin-pack
//!   consistency over Czech-phonology accuracy.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Czech PHONEX encoder.
///
/// A zero-sized value; construct as [`CzechPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_cs::CzechPhonex;
///
/// let key = CzechPhonex.encode("Novák").unwrap();
/// // N seed, O vowel (reset), V code=1, A vowel (reset), K code=2
/// //   → "N" + "1" + "2" → "N12" pad → "N120".
/// assert_eq!(key, "N120");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CzechPhonex;

impl CzechPhonex {
    /// Encodes `word` per the PHONEX-Czech algorithm.
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

/// Preprocess `word` into uppercase-ASCII letters after Czech digraph
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
    // Czech-specific letter folds.
    // Long vowels fold to their short counterparts; `ů → U`.
    // Haček consonants fold to their base-letter placeholder.
    let folded = match c {
        // Long vowels + ring-over-u.
        'á' | 'Á' => 'A',
        'é' | 'ě' | 'É' | 'Ě' => 'E',
        'í' | 'Í' => 'I',
        'ó' | 'Ó' => 'O',
        'ú' | 'ů' | 'Ú' | 'Ů' => 'U',
        'ý' | 'Ý' => 'Y',
        // Haček consonants — fold to base placeholder.
        'č' | 'Č' => 'C',
        'š' | 'Š' => 'S',
        'ž' | 'Ž' => 'Z',
        'ř' | 'Ř' => 'R',
        'ď' | 'Ď' => 'D',
        'ť' | 'Ť' => 'T',
        'ň' | 'Ň' => 'N',
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

/// Adapter that exposes [`CzechPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Czech::phonetic_encoder`](crate::Czech) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CzechPhonexAdapter;

impl LanguagePhoneticEncoder for CzechPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        CzechPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-cs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        CzechPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(CzechPhonex.encode("").is_none());
        assert!(CzechPhonex.encode("   ").is_none());
        assert!(CzechPhonex.encode("---").is_none());
    }

    #[test]
    fn haceks_fold_to_base_letters() {
        // č/c share the class (both start with C).
        //   "čech" → CECH → preprocess CH → X, but seed is C. So
        //   "CECH" → C seed last=2. E vow reset. C code=2 push? Wait —
        //   we're at the CH position. bytes after preprocess: [C, E, X].
        //   C seed last=2. E vow reset. X code=2 push → "CX" last=2.
        //   Cap: only 2 chars, pad → "C200".
        // "cech" → CECH → same preprocess → "CEX" → "C200".
        assert_eq!(p("čech"), p("cech"));
        // š/s share:
        assert_eq!(p("šum"), p("sum"));
        // ž/z share:
        assert_eq!(p("žena"), p("zena"));
        // ř/r share:
        assert_eq!(p("řeka"), p("reka"));
        // ď/d, ť/t, ň/n:
        assert_eq!(p("děti"), p("deti"));
        assert_eq!(p("ťuk"), p("tuk"));
        assert_eq!(p("kůň"), p("kun"));
    }

    #[test]
    fn long_vowels_fold_to_short() {
        // Long-to-short vowel folds — seed and interior vowels alike
        // reduce to the short counterpart. The interior fold is
        // invisible in Soundex-shape output (vowels drop) but the
        // seed fold shows up in position 0.
        assert_eq!(p("útok"), p("utok"));
        assert_eq!(p("ódes"), p("odes"));
        assert_eq!(p("Íra"), p("Ira"));
    }

    #[test]
    fn u_variants_fold_together() {
        // ů → U (ring-over-u).
        //   "dům" → DUM → D seed last=3. U vow reset. M code=5. → "D5" → "D500".
        //   "duom" → same by ASCII path? Actually duom has u,o,m — but
        //   the point is dům and dum encode alike.
        //   Also ú → U: úřad and urad.
        assert_eq!(p("dům"), p("dum"));
        assert_eq!(p("úřad"), p("urad"));
    }

    #[test]
    fn ch_digraph_encodes_as_velar() {
        // "chleba" → preprocess CH → X: "XLEBA".
        //   X seed last=2. L code=4 push → "X4" last=4. E vow reset.
        //   B code=1 push → "X41" last=1. A vow reset. Cap: 3, pad → "X410".
        assert_eq!(p("chleba"), "X410");
        // "nechci" → NECHCI → preprocess CH → X: "NEXCI".
        //   N seed last=5. E vow reset. X code=2 push → "N2" last=2.
        //   C code=2 dup drop. I vow reset. Cap: 2, pad → "N200".
        assert_eq!(p("nechci"), "N200");
    }

    #[test]
    fn silent_h_is_stripped() {
        // "hora" → preprocess H drops → "ORA".
        //   O seed last=0. R code=6 push → "O6" last=6. A vow reset.
        //   Pad → "O600".
        assert_eq!(p("hora"), "O600");
        // Compare to "ora" (same, no H).
        assert_eq!(p("hora"), p("ora"));
    }

    #[test]
    fn common_czech_surnames() {
        // Novák: N seed last=5. O vow reset. V code=1 push → "N1"
        //   last=1. A vow reset. K code=2 push → "N12" last=2. Pad
        //   → "N120".
        assert_eq!(p("Novák"), "N120");
        // Svoboda: S seed last=7. V code=1 push → "S1" last=1. O vow
        //   reset. B code=1 push (not dup — last was reset to 0) →
        //   "S11" wait: after V push, last_code=1. Then O vow → reset
        //   last_code=0. Then B code=1 (not 0, not equal to last=0
        //   after reset). Push → "S11" wait that's 3 chars total.
        //   Actually let me recount: S(seed), then chars are V,O,B,O,D,A.
        //   S seed → "S" last=7.
        //   V code=1 (not dup with 7). Push → "S1" last=1.
        //   O vow → reset last=0.
        //   B code=1. Not 0, not equal to 0. Push → "S11" last=1.
        //   O vow → reset last=0.
        //   D code=3. Push → "S113" last=3. Cap 4. Stop.
        //   Result "S113".
        assert_eq!(p("Svoboda"), "S113");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("NOVÁK"), p("novák"));
        assert_eq!(p("ŽENA"), p("žena"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ne"), "N000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Abba" — A seed last=0. B code=1 push → "A1" last=1. B dup
        //   drop. A vow reset. Pad → "A100".
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_cs() {
        assert_eq!(CzechPhonexAdapter.name(), "phonex-cs");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(CzechPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = CzechPhonexAdapter.encode("Novák").unwrap();
        assert_eq!(primary, "N120");
        assert!(alt.is_none());
    }
}
