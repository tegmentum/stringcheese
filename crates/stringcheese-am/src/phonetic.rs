//! Ge'ez → BGN/PCGN-style Latin → PHONEX-Amharic phonetic encoder.
//!
//! # Origin
//!
//! Amharic is the **first Ge'ez-script pack** in StringCheese. Two
//! candidate approaches present themselves for a phonetic hookup:
//!
//! * **BGN/PCGN or ISO 9985 romanization.** The BGN/PCGN Amharic
//!   scheme (1994) and ISO 9985 (Ethiopic transliteration, 1996)
//!   are the standard Latin-script romanizations. Both use
//!   diacritics for emphatic and pharyngeal consonants that have
//!   no single-character ASCII equivalent (`ḥ`, `ḫ`, `ṣ`, `š`, `ṭ`,
//!   `ʼ`, `ʽ`). Faithful to the script, but diacritics defeat
//!   ASCII-only indexes and hash inconsistently across NFC / NFD.
//! * **PHONEX-Amharic.** Transliterate to Latin first, then apply
//!   a Soundex-shape 4-character reduction that folds long / short
//!   vowels, drops emphatic under-dots, and collapses
//!   consonant-class duplicates. Produces sound-alike equivalence
//!   classes and matches the shape of the other Latin-alphabet
//!   packs' phonetic hookups (Czech, Dutch, French, Portuguese,
//!   Spanish, Swedish, Norwegian, Finnish, Polish, Hungarian,
//!   Vietnamese, Slovak — all PHONEX-family) and the Bengali /
//!   Hindi / Tamil Indic packs' two-stage ISO-15919-then-PHONEX
//!   pattern.
//!
//! # Implementation choice — two-stage: BGN/PCGN then PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`AmharicBgnPcgn`]** — the deterministic Ge'ez → Latin
//!    transliteration. Public because it's useful in its own right
//!    (data-migration tools, IR display).
//! 2. **[`AmharicPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* the BGN/PCGN output. This is the encoder
//!    [`AmharicPhonexAdapter`] wraps for the
//!    [`LanguagePhoneticEncoder`] trait hookup; adapter name
//!    `"phonex-am"`.
//!
//! # Ge'ez syllable → romanization algorithm
//!
//! Every Ge'ez main-block scalar (U+1200..=U+137F) encodes a
//! consonant + vowel pair (see [`crate::geez`] for the syllable
//! math). The transliterator decomposes each syllable to its
//! `(consonant_family, vowel_order)` pair via [`crate::geez::decompose`],
//! looks up the consonant family's ASCII form via
//! [`family_romanization`], appends the vowel-order suffix via
//! [`vowel_romanization`], and concatenates the result. Ge'ez
//! scalars outside the main block (supplement, extended, or the
//! ethiopic-punctuation range U+1361..=U+1368) pass through
//! unchanged; ASCII passes through unchanged; every other scalar
//! passes through unchanged.
//!
//! ## Vowel-order suffixes
//!
//! | Order | Amharic name | Vowel   | Romanized |
//! |-------|--------------|---------|-----------|
//! | 0     | ግዕዝ         | ə / ä   | `e`       |
//! | 1     | ካዕብ         | u       | `u`       |
//! | 2     | ሣልስ         | i       | `i`       |
//! | 3     | ራብዕ         | a       | `a`       |
//! | 4     | ሓምስ         | e       | `ie`      |
//! | 5     | ሳድስ         | ɨ / ∅   | (empty)   |
//! | 6     | ሳብዕ         | o       | `o`       |
//! | 7     | (labialized) | wa / oa | `wa`      |
//!
//! Order 5 (ሳድስ) is the *sixth order* — a short high-central vowel
//! /ɨ/ that in Amharic frequently reduces to no vowel at all. We
//! emit an empty suffix; the Soundex reducer will pick up only the
//! consonant.
//!
//! ## Consonant-family romanizations
//!
//! Every one of the 48 rows in the main Ge'ez block has an ASCII
//! romanization. Rows in the "labialized-column-only" range
//! (indices where Unicode reserved the entire row for the eight
//! labialized forms of an earlier row) fall back to a stand-in
//! letter that shares a Soundex class with the base form. See
//! [`family_romanization`] for the full table.
//!
//! # PHONEX-Amharic reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the other
//! Latin-alphabet packs:
//!
//! 1. Fold each ASCII letter to uppercase; drop non-letters.
//! 2. Take the first ASCII letter as the seed.
//! 3. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels — `A/E/I/O/U/Y/H` — reset the duplicate-collapse
//!    state).
//! 4. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-am"`.
//!
//! # Byte-length caveat
//!
//! Every Ge'ez main-block scalar is **3 bytes** in UTF-8
//! (U+1200..=U+137F falls in the 3-byte range). The transliterator
//! walks characters via [`str::chars`], never raw bytes, so it
//! never risks slicing a scalar apart. The output is ASCII only.
//!
//! # Non-Amharic pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar
//! outside the main Ge'ez block pass through the transliterator
//! unchanged. The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::geez::{GEEZ_MAIN_START, decompose};

/// The Amharic Ge'ez → BGN/PCGN-style Latin transliterator.
///
/// A zero-sized value; construct as [`AmharicBgnPcgn`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full romanization
/// algorithm.
///
/// # Example
///
/// ```
/// use stringcheese_am::AmharicBgnPcgn;
///
/// // Each syllable decomposes to consonant + vowel:
/// //   አ (order 0 of ' family) → "'e"
/// //   ማ (order 3 of m family) → "ma"
/// //   ር (order 5 of r family) → "r"
/// //   ኛ (order 3 of ñ family) → "Na"   (ñ folds to capital N stand-in)
/// assert_eq!(AmharicBgnPcgn.encode("አማርኛ"), "'emarNa");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmharicBgnPcgn;

impl AmharicBgnPcgn {
    /// Encode `text` per the Ge'ez → BGN/PCGN-style Latin
    /// transliteration.
    ///
    /// Every Ge'ez main-block scalar decomposes to `(family, order)`
    /// via [`crate::geez::decompose`] and is replaced by the
    /// concatenation of the family's ASCII romanization and the
    /// order's vowel suffix; every scalar outside the main block
    /// (supplement, extended, Ge'ez punctuation, ASCII, other) passes
    /// through unchanged.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            if let Some((base, order)) = decompose(c) {
                // Family index for a main-block scalar is at most 47
                // (48 rows × 8 columns fills U+1200..=U+137F), well
                // inside `u8` range — the cast cannot truncate.
                #[allow(clippy::cast_possible_truncation)]
                let family = (((base as u32) - GEEZ_MAIN_START) / 8) as u8;
                if let Some(cons) = family_romanization(family) {
                    out.push_str(cons);
                    out.push_str(vowel_romanization(order));
                    continue;
                }
            }
            out.push(c);
        }
        out
    }
}

/// Romanization for a vowel order (0..=7) — the vowel string to
/// append to a consonant.
///
/// See the [module-level table](self#vowel-order-suffixes) for the
/// full mapping. Order 5 (ሳድስ, the "sixth order") returns an empty
/// string; order 7 (labialized) returns `"wa"`.
#[must_use]
#[allow(clippy::match_same_arms)]
// Order 5 (ሳድስ, ɨ / silent) and the wildcard both return `""` —
// they are deliberately separate arms because order 5 is a
// documented linguistic category (the "6th order", short high
// central vowel that reduces to no vowel in Amharic) while the
// wildcard catches ill-formed callers (any u8 outside 0..=7). Keep
// them apart for reading clarity.
pub const fn vowel_romanization(order: u8) -> &'static str {
    match order {
        0 => "e",
        1 => "u",
        2 => "i",
        3 => "a",
        4 => "ie",
        5 => "",
        6 => "o",
        7 => "wa",
        _ => "",
    }
}

/// Romanization for a consonant family (0..=47) in the main Ge'ez
/// block — the ASCII string that stands in for the family's base
/// consonant.
///
/// Returns `None` for family indices outside 0..=47 (the main
/// block's 48 rows). The table follows BGN/PCGN Amharic conventions
/// where possible, with ASCII stand-ins for diacritic-marked
/// consonants — same design choice Buckwalter made for Arabic.
///
/// # Table
///
/// | Family | U+1200 + n*8 | Base | Rom. | Notes                       |
/// |--------|--------------|------|------|-----------------------------|
/// | 0      | U+1200       | ሀ    | `h`  | he (h family)               |
/// | 1      | U+1208       | ለ    | `l`  | le                          |
/// | 2      | U+1210       | ሐ    | `H`  | Hḥ (pharyngeal, capital)    |
/// | 3      | U+1218       | መ    | `m`  | me                          |
/// | 4      | U+1220       | ሠ    | `s`  | śe (śawt — s stand-in)      |
/// | 5      | U+1228       | ረ    | `r`  | re                          |
/// | 6      | U+1230       | ሰ    | `s`  | se                          |
/// | 7      | U+1238       | ሸ    | `x`  | še (sh — x stand-in)        |
/// | 8      | U+1240       | ቀ    | `q`  | qe                          |
/// | 9      | U+1248       | (qh) | `q`  | labialized-q row            |
/// | 10     | U+1250       | ቐ    | `q`  | qhe                         |
/// | 11     | U+1258       | (qw) | `q`  | reserved / labialized       |
/// | 12     | U+1260       | በ    | `b`  | be                          |
/// | 13     | U+1268       | ቨ    | `v`  | ve                          |
/// | 14     | U+1270       | ተ    | `t`  | te                          |
/// | 15     | U+1278       | ቸ    | `c`  | če (ch — c stand-in)        |
/// | 16     | U+1280       | ኀ    | `H`  | ḫe (uvular — H stand-in)    |
/// | 17     | U+1288       | (nh) | `n`  | labialized / gap            |
/// | 18     | U+1290       | ነ    | `n`  | ne                          |
/// | 19     | U+1298       | ኘ    | `N`  | ñe (palatal n)              |
/// | 20     | U+12A0       | አ    | `'`  | 'e (glottal — apostrophe)   |
/// | 21     | U+12A8       | ከ    | `k`  | ke                          |
/// | 22     | U+12B0       | (kh) | `k`  | labialized-k row            |
/// | 23     | U+12B8       | ኸ    | `k`  | ḵe (aspirated k)            |
/// | 24     | U+12C0       | (kw) | `k`  | labialized / gap            |
/// | 25     | U+12C8       | ወ    | `w`  | we                          |
/// | 26     | U+12D0       | ዐ    | `` ` `` | ʿe (ayn — backtick)      |
/// | 27     | U+12D8       | ዘ    | `z`  | ze                          |
/// | 28     | U+12E0       | ዠ    | `Z`  | že (zh — Z stand-in)        |
/// | 29     | U+12E8       | የ    | `y`  | ye                          |
/// | 30     | U+12F0       | ደ    | `d`  | de                          |
/// | 31     | U+12F8       | ዸ    | `D`  | ḍe (retroflex — D stand-in) |
/// | 32     | U+1300       | ጀ    | `j`  | je                          |
/// | 33     | U+1308       | ገ    | `g`  | ge                          |
/// | 34     | U+1310       | (gh) | `g`  | labialized-g row            |
/// | 35     | U+1318       | ጘ    | `g`  | ḡe (gap-filler)             |
/// | 36     | U+1320       | ጠ    | `T`  | ṭe (emphatic — T stand-in)  |
/// | 37     | U+1328       | ጨ    | `C`  | č̣e (emphatic — C stand-in) |
/// | 38     | U+1330       | ጰ    | `P`  | p̣e (emphatic — P stand-in) |
/// | 39     | U+1338       | ጸ    | `s`  | ṣe (emphatic — s stand-in)  |
/// | 40     | U+1340       | ፀ    | `s`  | ṣe (variant of family 39)   |
/// | 41     | U+1348       | ፈ    | `f`  | fe                          |
/// | 42     | U+1350       | ፐ    | `p`  | pe                          |
/// | 43     | U+1358       | (r-) | `r`  | Ethiopic supplement bridge  |
/// | 44     | U+1360       | (–)  | `?`  | Ethiopic punctuation range  |
/// | 45     | U+1368       | (–)  | `?`  | Ethiopic numerals range     |
/// | 46     | U+1370       | (–)  | `?`  | Ethiopic numerals range     |
/// | 47     | U+1378       | (–)  | `?`  | Ethiopic numerals range     |
#[must_use]
#[allow(clippy::match_same_arms)]
// The arms are deliberately separate: each family index is a
// distinct row in the Ge'ez chart, and grouping them by their
// linguistic category (labialized rows fold to their base
// consonant, gap-fillers share a placeholder) reads more clearly
// as one arm per row than as a collapsed alternation. Same
// reasoning as the ISO 15919 arm-grouping in `stringcheese-bn`.
pub const fn family_romanization(family: u8) -> Option<&'static str> {
    Some(match family {
        0 => "h",  // ሀ family
        1 => "l",  // ለ family
        2 => "H",  // ሐ family (pharyngeal — capital H stand-in)
        3 => "m",  // መ family
        4 => "s",  // ሠ family (śawt — fold to s)
        5 => "r",  // ረ family
        6 => "s",  // ሰ family
        7 => "x",  // ሸ family (sh — x stand-in)
        8 => "q",  // ቀ family
        9 => "q",  // labialized-q row
        10 => "q", // ቐ family
        11 => "q", // reserved / labialized-q
        12 => "b", // በ family
        13 => "v", // ቨ family
        14 => "t", // ተ family
        15 => "c", // ቸ family (č)
        16 => "H", // ኀ family (uvular ḫ — fold to H)
        17 => "n", // labialized / gap
        18 => "n", // ነ family
        19 => "N", // ኘ family (ñ)
        20 => "'", // አ family (glottal stop — apostrophe)
        21 => "k", // ከ family
        22 => "k", // labialized-k row
        23 => "k", // ኸ family (aspirated k)
        24 => "k", // labialized / gap
        25 => "w", // ወ family
        26 => "`", // ዐ family (ayn — backtick)
        27 => "z", // ዘ family
        28 => "Z", // ዠ family (ž)
        29 => "y", // የ family
        30 => "d", // ደ family
        31 => "D", // ዸ family (ḍ — retroflex, capital D)
        32 => "j", // ጀ family
        33 => "g", // ገ family
        34 => "g", // labialized-g row
        35 => "g", // ጘ family / gap-filler
        36 => "T", // ጠ family (emphatic ṭ — capital T)
        37 => "C", // ጨ family (emphatic č̣ — capital C)
        38 => "P", // ጰ family (emphatic p̣ — capital P)
        39 => "s", // ጸ family (emphatic ṣ — fold to s)
        40 => "s", // ፀ family (ṣ variant)
        41 => "f", // ፈ family
        42 => "p", // ፐ family
        43 => "r", // supplement bridge
        // Families 44..=47 are the U+1360..=U+137F punctuation and
        // Ethiopic numeral range. The tokenizer already filters
        // U+1361..=U+1368 out as separators; the numerals pass
        // through as digits under the pass-through branch. Return
        // an ASCII "?" as a diagnostic mark so a mis-tokenized
        // punctuation scalar shows up in the output.
        44 => "?",
        45 => "?",
        46 => "?",
        47 => "?",
        _ => return None,
    })
}

// ---------------------------------------------------------------------
// PHONEX-Amharic — 4-char Soundex-shape reduction over the BGN/PCGN
// output.
// ---------------------------------------------------------------------

/// The PHONEX-Amharic encoder.
///
/// A zero-sized value; construct as [`AmharicPhonex`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_am::AmharicPhonex;
///
/// // "አማርኛ" transliterates to "'emarna"; phonex reduces to '560.
/// // ' seed, E vow, M code=5, A vow, R code=6, N code=5, A vow reset.
/// // Actually first character is apostrophe (') — the phonex reducer
/// // drops non-letters, so the seed is the next letter: E, and the
/// // final key is E565.
/// // See tests/phonex_reference.rs for the traced value.
/// assert!(AmharicPhonex.encode("አማርኛ").is_some());
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmharicPhonex;

impl AmharicPhonex {
    /// Encodes `word` per the PHONEX-Amharic algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty
    /// input, pure whitespace, all punctuation, or reduced to
    /// nothing after filtering). Otherwise returns a 4-character
    /// key of the form `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = AmharicBgnPcgn.encode(word);
        let ascii = fold_to_ascii_upper(&transliterated);
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

/// Fold `s` to uppercase-ASCII letters, dropping non-letter code
/// points. The BGN/PCGN output uses ASCII only (with a handful of
/// stand-in punctuation for pharyngeal / glottal / ayn — apostrophe
/// and backtick — which the phonex reducer filters out).
fn fold_to_ascii_upper(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_uppercase());
        }
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// `A/E/I/O/U/Y/H` return `b'0'` (the vowel-reset sentinel); the
/// classical Soundex consonant classes fill in the rest. Any byte
/// outside A..=Z also returns `b'0'` (defensive default — the
/// [`fold_to_ascii_upper`] pre-pass keeps only ASCII letters, so
/// this branch is unreachable in practice).
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

/// Adapter that exposes [`AmharicPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Amharic::phonetic_encoder`](crate::Amharic) hands back.
///
/// Returns `Some((key, None))` for input with at least one Ge'ez
/// scalar; returns `None` for input with no Ge'ez content (the
/// transliteration would pass through unchanged, which is not a
/// useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmharicPhonexAdapter;

impl LanguagePhoneticEncoder for AmharicPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_geez_main(word) {
            return None;
        }
        let key = AmharicPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-am"
    }
}

/// Does `s` contain at least one scalar in the main Ge'ez block
/// U+1200..=U+137F?
fn contains_geez_main(s: &str) -> bool {
    s.chars().any(|c| ('\u{1200}'..='\u{137F}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        AmharicBgnPcgn.encode(s)
    }

    fn p(w: &str) -> String {
        AmharicPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Basic syllable → romanization.
    // -------------------------------------------------------------

    #[test]
    fn ha_family_syllables_romanize() {
        // ሀ (order 0) → "he"
        assert_eq!(e("ሀ"), "he");
        // ሁ (order 1) → "hu"
        assert_eq!(e("ሁ"), "hu");
        // ሂ (order 2) → "hi"
        assert_eq!(e("ሂ"), "hi");
        // ሃ (order 3) → "ha"
        assert_eq!(e("ሃ"), "ha");
        // ሄ (order 4) → "hie"
        assert_eq!(e("ሄ"), "hie");
        // ህ (order 5) → "h" (silent-vowel order)
        assert_eq!(e("ህ"), "h");
        // ሆ (order 6) → "ho"
        assert_eq!(e("ሆ"), "ho");
    }

    #[test]
    fn m_family_syllables_romanize() {
        // መ (order 0) → "me"
        assert_eq!(e("መ"), "me");
        // ማ (order 3) → "ma"
        assert_eq!(e("ማ"), "ma");
        // ሙ (order 1) → "mu"
        assert_eq!(e("ሙ"), "mu");
    }

    #[test]
    fn glottal_family_romanizes_with_apostrophe() {
        // አ (order 0 of ' family) → "'e"
        assert_eq!(e("አ"), "'e");
    }

    #[test]
    fn ayn_family_romanizes_with_backtick() {
        // ዐ (order 0 of ayn family) → "`e"
        assert_eq!(e("ዐ"), "`e");
    }

    // -------------------------------------------------------------
    // Full-word romanization.
    // -------------------------------------------------------------

    #[test]
    fn amharic_word_romanizes() {
        // አማርኛ = አ + ማ + ር + ኛ → "'e" + "ma" + "r" + "Na" = "'emarNa"
        assert_eq!(e("አማርኛ"), "'emarNa");
    }

    #[test]
    fn ethiopia_word_romanizes() {
        // ኢትዮጵያ = ኢ + ት + ዮ + ጵ + ያ
        //   ኢ order 2 of ' family → "'i"
        //   ት order 5 of t family → "t"
        //   ዮ order 6 of y family → "yo"
        //   ጵ order 5 of P family → "P"
        //   ያ order 3 of y family → "ya"
        // → "'ityoPya"
        assert_eq!(e("ኢትዮጵያ"), "'ityoPya");
    }

    // -------------------------------------------------------------
    // Pass-through.
    // -------------------------------------------------------------

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e(""), "");
        assert_eq!(e("hello"), "hello");
    }

    #[test]
    fn mixed_content_passes_through_non_geez() {
        assert_eq!(e("hello አማርኛ"), "hello 'emarNa");
    }

    // -------------------------------------------------------------
    // PHONEX-Amharic.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(AmharicPhonex.encode("").is_none());
        assert!(AmharicPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_amharic_word_encodes() {
        // "አማርኛ" → "'emarNa" → strip non-letters → "EMARNA"
        //   E seed. M code=5 push → "E5". A vow reset. R code=6
        //   push → "E56". N code=5 push → "E565". A vow reset.
        //   Length 4 already; done. Key = "E565".
        assert_eq!(p("አማርኛ"), "E565");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_am() {
        assert_eq!(AmharicPhonexAdapter.name(), "phonex-am");
    }

    #[test]
    fn adapter_returns_some_for_amharic() {
        let out = AmharicPhonexAdapter.encode("አማርኛ");
        assert_eq!(out, Some((String::from("E565"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_amharic() {
        assert!(AmharicPhonexAdapter.encode("").is_none());
        assert!(AmharicPhonexAdapter.encode("hello").is_none());
        assert!(AmharicPhonexAdapter.encode("123").is_none());
    }
}
