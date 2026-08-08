//! Thai → Royal-Thai-Romanization-tuned → PHONEX-Thai phonetic
//! encoder.
//!
//! # Origin
//!
//! Thai is the first Thai-script pack in StringCheese and — like
//! every other language with a non-Latin script that lacks a
//! widely-adopted native phonetic-key algorithm (Bengali, Hindi, Greek,
//! Vietnamese, Korean) — the pragmatic answer for a **phonetic key
//! generator** is a two-stage encoder that first folds the script to
//! a coarse Latin representation and then applies a Soundex-shape
//! reduction. That is what this module ships.
//!
//! The reference romanization is the **Royal Thai General System of
//! Transcription (RTGS)**, published by the Royal Institute of
//! Thailand and used on all Thai road signs and government
//! publications. RTGS is a *lossy* transcription (it drops tone,
//! merges long / short vowel length, and collapses several consonants
//! that are distinct in the script but pronounced the same); those
//! properties are actively desirable for a phonetic *key*, which
//! wants a stable equivalence-class representative rather than a
//! faithful one-to-one transliteration.
//!
//! # Implementation choice — two-stage: RTGS-family fold then PHONEX
//!
//! This module ships a single stage-collapsed encoder,
//! **PHONEX-Thai**, that internally:
//!
//! 1. **Drops every non-consonant scalar.** Tone marks, vowel marks,
//!    signs, and pre-vowels do not contribute to the phonetic key.
//!    (The four tone marks — mai ek / tho / tri / chattawa — encode
//!    lexical tone, which is a stronger discriminator than the key
//!    wants; equivalence classes should collapse across tone.)
//! 2. **Maps each remaining consonant to a Royal-Thai-Romanization
//!    family letter.** For example, both `ค` (kho khwai) and `ข`
//!    (kho khai) map to `K` — they are historical high-class vs.
//!    low-class /k/ in Thai script tradition but the same modern
//!    /kʰ/ phone. `ท` (tho thahan), `ธ` (tho thong), `ต` (to tao),
//!    and `ฏ` (to patak) all map to `T`. `ผ` and `พ` (pho phueng /
//!    pho phan) both map to `P`. The full table is documented below.
//! 3. **Runs a Soundex-shape 4-character reduction over the folded
//!    Latin.** The first Latin letter becomes the seed; subsequent
//!    letters classify by Soundex family, consecutive equal codes
//!    collapse, the result pads to 4 characters with `'0'`. This is
//!    the same PHONEX shape as the sibling packs
//!    (`stringcheese-{cs,da,es,et,fi,fr,hu,ko,nl,pl,pt,sk,sv,vi,bn}`).
//!
//! Adapter name: `"phonex-th"`.
//!
//! # Byte-length caveat
//!
//! Every Thai scalar (U+0E00..=U+0E7F) is encoded as **three UTF-8
//! bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). The encoder walks scalars via [`str::chars`],
//! never raw bytes, so it never risks slicing a Thai scalar apart.
//!
//! # The consonant → Latin family map
//!
//! | Thai | Name             | Family | Notes                    |
//! |------|------------------|--------|--------------------------|
//! | `ก`  | ko kai           | K      | /k/                      |
//! | `ข`  | kho khai         | K      | /kʰ/ historically high   |
//! | `ฃ`  | kho khuat        | K      | archaic /kʰ/             |
//! | `ค`  | kho khwai        | K      | /kʰ/ historically low    |
//! | `ฅ`  | kho khon         | K      | archaic /kʰ/             |
//! | `ฆ`  | kho rakhang      | K      | /kʰ/                     |
//! | `ง`  | ngo ngu          | N      | /ŋ/ (Soundex nasal)      |
//! | `จ`  | cho chan         | C      | /tɕ/                     |
//! | `ฉ`  | cho ching        | C      | /tɕʰ/                    |
//! | `ช`  | cho chang        | C      | /tɕʰ/                    |
//! | `ซ`  | so so            | S      | /s/                      |
//! | `ฌ`  | cho choe         | C      | /tɕʰ/                    |
//! | `ญ`  | yo ying          | Y      | /j/                      |
//! | `ฎ`  | do chada         | D      | /d/                      |
//! | `ฏ`  | to patak         | T      | /t/                      |
//! | `ฐ`  | tho than         | T      | /tʰ/                     |
//! | `ฑ`  | tho nangmontho   | T      | /tʰ/ or /d/              |
//! | `ฒ`  | tho phuthao      | T      | /tʰ/                     |
//! | `ณ`  | no nen           | N      | /n/                      |
//! | `ด`  | do dek           | D      | /d/                      |
//! | `ต`  | to tao           | T      | /t/                      |
//! | `ถ`  | tho thung        | T      | /tʰ/                     |
//! | `ท`  | tho thahan       | T      | /tʰ/                     |
//! | `ธ`  | tho thong        | T      | /tʰ/                     |
//! | `น`  | no nu            | N      | /n/                      |
//! | `บ`  | bo baimai        | B      | /b/                      |
//! | `ป`  | po pla           | P      | /p/                      |
//! | `ผ`  | pho phueng       | P      | /pʰ/                     |
//! | `ฝ`  | fo fa            | F      | /f/                      |
//! | `พ`  | pho phan         | P      | /pʰ/                     |
//! | `ฟ`  | fo fan           | F      | /f/                      |
//! | `ภ`  | pho samphao      | P      | /pʰ/                     |
//! | `ม`  | mo ma            | M      | /m/                      |
//! | `ย`  | yo yak           | Y      | /j/                      |
//! | `ร`  | ro rua           | R      | /r/                      |
//! | `ล`  | lo ling          | L      | /l/                      |
//! | `ว`  | wo waen          | W      | /w/                      |
//! | `ศ`  | so sala          | S      | /s/                      |
//! | `ษ`  | so ruesi         | S      | /s/                      |
//! | `ส`  | so sua           | S      | /s/                      |
//! | `ห`  | ho hip           | H      | /h/                      |
//! | `ฬ`  | lo chula         | L      | /l/                      |
//! | `อ`  | o ang            | A      | zero-onset / glottal     |
//! | `ฮ`  | ho nokhuk        | H      | /h/                      |
//!
//! # Soundex-shape classification table
//!
//! Runs after the fold. Same table as the sibling Latin-alphabet
//! packs' PHONEX-family encoders:
//!
//! | Class | Letters       |
//! |-------|---------------|
//! | 1     | B P F V W     |
//! | 2     | C K G Q J X   |
//! | 3     | D T           |
//! | 4     | L             |
//! | 5     | M N           |
//! | 6     | R             |
//! | 7     | S Z           |
//! | 0     | A E I O U Y H (dropped as vowels / silent) |
//!
//! Thai `ห` and `ฮ` (both /h/) drop out entirely under class 0, which
//! matches the Latin-pack precedent (silent `H`).
//!
//! # Non-goals
//!
//! - **Tone preservation.** Thai has five phonemic tones and PHONEX
//!   folds all five into one key. Callers who need tone-aware
//!   matching should compose a tone-preserving encoder alongside
//!   this one.
//! - **Full RTGS output.** RTGS is a *readable* romanization; PHONEX
//!   is a *phonetic key generator*. A future
//!   `stringcheese-th-rtgs` sibling could expose the full RTGS
//!   output as a distinct public API.
//! - **Faithful transliteration** (ISO 11940). ISO 11940 is a
//!   one-to-one Thai-script → Latin-with-diacritics transliteration
//!   for scholarly / library use. Out of scope for a phonetic key.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// Map a Thai base consonant to its Royal-Thai-Romanization family
/// letter (uppercase ASCII), collapsing consonant classes that share
/// a modern phone.
///
/// Returns `None` for scalars outside the Thai consonant range
/// U+0E01..=U+0E2E. See the [module-level docs](self) for the full
/// table.
///
/// The `match_same_arms` lint is suppressed because arms grouped by
/// consonant class (all velars → K, all sibilants → S, all labials
/// → P, all dentals → T, all nasals → N, etc.) deliberately map to
/// the same ASCII target — merging them would obscure the phonological
/// grouping, and the encoder's design intent is that each Thai
/// consonant folds *individually* to its family letter.
#[must_use]
#[allow(clippy::match_same_arms)]
pub const fn consonant_family(c: char) -> Option<char> {
    Some(match c {
        // Velars — all /k/ or /kʰ/ historically.
        '\u{0E01}' => 'K', // ก ko kai
        '\u{0E02}' => 'K', // ข kho khai
        '\u{0E03}' => 'K', // ฃ kho khuat (archaic)
        '\u{0E04}' => 'K', // ค kho khwai
        '\u{0E05}' => 'K', // ฅ kho khon (archaic)
        '\u{0E06}' => 'K', // ฆ kho rakhang
        // Velar nasal.
        '\u{0E07}' => 'N', // ง ngo ngu — /ŋ/
        // Palatal affricates.
        '\u{0E08}' => 'C', // จ cho chan
        '\u{0E09}' => 'C', // ฉ cho ching
        '\u{0E0A}' => 'C', // ช cho chang
        '\u{0E0B}' => 'S', // ซ so so
        '\u{0E0C}' => 'C', // ฌ cho choe
        // Palatal glide.
        '\u{0E0D}' => 'Y', // ญ yo ying
        // Retroflex-heritage stops (all modern /t/ or /tʰ/).
        '\u{0E0E}' => 'D', // ฎ do chada
        '\u{0E0F}' => 'T', // ฏ to patak
        '\u{0E10}' => 'T', // ฐ tho than
        '\u{0E11}' => 'T', // ฑ tho nangmontho
        '\u{0E12}' => 'T', // ฒ tho phuthao
        '\u{0E13}' => 'N', // ณ no nen — /n/
        // Dental stops and nasals.
        '\u{0E14}' => 'D', // ด do dek
        '\u{0E15}' => 'T', // ต to tao
        '\u{0E16}' => 'T', // ถ tho thung
        '\u{0E17}' => 'T', // ท tho thahan
        '\u{0E18}' => 'T', // ธ tho thong
        '\u{0E19}' => 'N', // น no nu
        // Labials.
        '\u{0E1A}' => 'B', // บ bo baimai
        '\u{0E1B}' => 'P', // ป po pla
        '\u{0E1C}' => 'P', // ผ pho phueng
        '\u{0E1D}' => 'F', // ฝ fo fa
        '\u{0E1E}' => 'P', // พ pho phan
        '\u{0E1F}' => 'F', // ฟ fo fan
        '\u{0E20}' => 'P', // ภ pho samphao
        '\u{0E21}' => 'M', // ม mo ma
        // Palatal glide (variant).
        '\u{0E22}' => 'Y', // ย yo yak
        // Liquids.
        '\u{0E23}' => 'R', // ร ro rua
        '\u{0E24}' => 'R', // ฤ ru (vowel — treated as R for coverage)
        '\u{0E25}' => 'L', // ล lo ling
        '\u{0E26}' => 'L', // ฦ lu (archaic vowel — treated as L)
        // Semivowel.
        '\u{0E27}' => 'W', // ว wo waen
        // Sibilants.
        '\u{0E28}' => 'S', // ศ so sala
        '\u{0E29}' => 'S', // ษ so ruesi
        '\u{0E2A}' => 'S', // ส so sua
        // Fricative + glottal.
        '\u{0E2B}' => 'H', // ห ho hip
        '\u{0E2C}' => 'L', // ฬ lo chula
        '\u{0E2D}' => 'A', // อ o ang — glottal / zero-onset
        '\u{0E2E}' => 'H', // ฮ ho nokhuk
        _ => return None,
    })
}

/// The PHONEX-Thai encoder.
///
/// A zero-sized value; construct as [`ThaiPhonex`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_th::ThaiPhonex;
///
/// // สวัสดี = ส ว ั ส ด ี. Drop vowel marks and tones.
/// // Consonant sequence: ส ว ส ด → S W S D → S seed, W code=1,
/// // S code=7, D code=3 → "SWSD" — length 4, done.
/// // But `SW` fold: S seed, W code=1 push → "S1", S code=7 push → "S17",
/// // D code=3 push → "S173". Truncated at 4.
/// assert_eq!(ThaiPhonex.encode("สวัสดี").unwrap(), "S173");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ThaiPhonex;

impl ThaiPhonex {
    /// Encodes `word` per the PHONEX-Thai algorithm.
    ///
    /// Returns `None` when `word` has no Thai-consonant content
    /// (empty input, pure whitespace, all punctuation, all Thai
    /// vowel marks / tone marks with no consonants). Otherwise
    /// returns a 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let folded = fold_thai_to_ascii(word);
        if folded.is_empty() {
            return None;
        }
        let bytes = folded.as_bytes();

        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel / silent — reset the duplicate-collapse
                // state.
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

/// Fold `s` to uppercase-ASCII letters:
///
/// - Thai consonants map through [`consonant_family`].
/// - Thai vowel marks, tone marks, signs, and pre-vowels drop.
/// - ASCII letters pass through (uppercased).
/// - Everything else drops.
fn fold_thai_to_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len() / 3 + 4);
    for c in s.chars() {
        if let Some(family) = consonant_family(c) {
            out.push(family);
            continue;
        }
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_uppercase());
        }
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the [module-level docs](self) for the full classification
/// table.
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

/// Adapter that exposes [`ThaiPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Thai::phonetic_encoder`](crate::Thai) hands back.
///
/// Returns `Some((key, None))` for input with at least one Thai
/// consonant; returns `None` for input with no Thai-consonant
/// content (empty, all whitespace, all punctuation, all vowel /
/// tone marks, or pure non-Thai text).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ThaiPhonexAdapter;

impl LanguagePhoneticEncoder for ThaiPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_thai_consonant(word) {
            return None;
        }
        let key = ThaiPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-th"
    }
}

/// Does `s` contain at least one Thai consonant scalar (U+0E01..=U+0E2E)?
fn contains_thai_consonant(s: &str) -> bool {
    s.chars().any(|c| ('\u{0E01}'..='\u{0E2E}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        ThaiPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Consonant family lookups.
    // -------------------------------------------------------------

    #[test]
    fn velars_all_fold_to_k() {
        for c in ['ก', 'ข', 'ฃ', 'ค', 'ฅ', 'ฆ'] {
            assert_eq!(consonant_family(c), Some('K'), "{c:?} should fold to K");
        }
    }

    #[test]
    fn dentals_all_fold_to_t_or_d() {
        assert_eq!(consonant_family('ต'), Some('T'));
        assert_eq!(consonant_family('ท'), Some('T'));
        assert_eq!(consonant_family('ธ'), Some('T'));
        assert_eq!(consonant_family('ถ'), Some('T'));
        assert_eq!(consonant_family('ด'), Some('D'));
    }

    #[test]
    fn sibilants_all_fold_to_s() {
        for c in ['ซ', 'ศ', 'ษ', 'ส'] {
            assert_eq!(consonant_family(c), Some('S'), "{c:?} should fold to S");
        }
    }

    #[test]
    fn labials_all_fold_to_correct_family() {
        assert_eq!(consonant_family('บ'), Some('B'));
        assert_eq!(consonant_family('ป'), Some('P'));
        assert_eq!(consonant_family('ผ'), Some('P'));
        assert_eq!(consonant_family('พ'), Some('P'));
        assert_eq!(consonant_family('ภ'), Some('P'));
        assert_eq!(consonant_family('ฟ'), Some('F'));
        assert_eq!(consonant_family('ฝ'), Some('F'));
        assert_eq!(consonant_family('ม'), Some('M'));
    }

    #[test]
    fn nasals_fold_to_n() {
        for c in ['ง', 'ณ', 'น'] {
            assert_eq!(consonant_family(c), Some('N'), "{c:?} should fold to N");
        }
    }

    #[test]
    fn non_consonant_returns_none() {
        // Vowel mark, tone mark, pre-vowel, ASCII letter.
        assert!(consonant_family('า').is_none());
        assert!(consonant_family('่').is_none());
        assert!(consonant_family('เ').is_none());
        assert!(consonant_family('a').is_none());
    }

    // -------------------------------------------------------------
    // Encoder behaviour.
    // -------------------------------------------------------------

    #[test]
    fn empty_input_returns_none() {
        assert!(ThaiPhonex.encode("").is_none());
        assert!(ThaiPhonex.encode("   ").is_none());
    }

    #[test]
    fn all_tone_marks_returns_none() {
        // Tone marks alone — no consonants, no key.
        assert!(ThaiPhonex.encode("่้๊๋").is_none());
    }

    #[test]
    fn bare_consonant_encodes_to_seed_plus_zeros() {
        // ก → K → K seed, no other letters → "K000".
        assert_eq!(p("ก"), "K000");
        // ส → S → "S000".
        assert_eq!(p("ส"), "S000");
    }

    #[test]
    fn tone_marks_and_vowel_marks_drop() {
        // กา = ก + า (vowel mark drops) → K → "K000".
        assert_eq!(p("กา"), "K000");
        // ก่า = ก + ่ (tone) + า → same as กา.
        assert_eq!(p("ก่า"), "K000");
    }

    #[test]
    fn consonant_family_folds_produce_same_key() {
        // ทท (tho thahan twice) and ตต (to tao twice) — both fold to
        // TT → collapse to T → "T000".
        assert_eq!(p("ทท"), "T000");
        assert_eq!(p("ตต"), "T000");
        assert_eq!(ThaiPhonex.encode("ทท"), ThaiPhonex.encode("ตต"));
    }

    #[test]
    fn sibilants_produce_same_key() {
        // ซ, ศ, ษ, ส all fold to S.
        let a = ThaiPhonex.encode("ซ").unwrap();
        let b = ThaiPhonex.encode("ศ").unwrap();
        let c = ThaiPhonex.encode("ษ").unwrap();
        let d = ThaiPhonex.encode("ส").unwrap();
        assert_eq!(a, "S000");
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(c, d);
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_th() {
        assert_eq!(ThaiPhonexAdapter.name(), "phonex-th");
    }

    #[test]
    fn adapter_returns_some_for_thai() {
        let out = ThaiPhonexAdapter.encode("สวัสดี");
        assert!(out.is_some(), "expected Some for Thai input");
        let (primary, alt) = out.unwrap();
        assert!(!primary.is_empty());
        assert!(alt.is_none());
    }

    #[test]
    fn adapter_returns_none_for_no_thai() {
        assert!(ThaiPhonexAdapter.encode("").is_none());
        assert!(ThaiPhonexAdapter.encode("hello").is_none());
        assert!(ThaiPhonexAdapter.encode("123").is_none());
    }
}
