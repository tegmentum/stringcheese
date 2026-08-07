//! A Turkish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Turkish has no widely established phonetic encoder in the Soundex /
//! Métaphone / PHONEX family. This is deliberate: Turkish orthography
//! is **highly phonetic** — the modern Latin alphabet was designed in
//! 1928 explicitly to be a near-1:1 mapping to the language's phoneme
//! inventory. Simple case folding plus accent-fold (dotted / dotless
//! `I`, `ç → c`, `ğ → g`, `ö → o`, `ş → s`, `ü → u`) is already close
//! to a phonetic key.
//!
//! # Implementation choice
//!
//! This module ships a **light PHONEX-Turkish** encoder: a 4-character
//! Soundex-shape key with Turkish-tuned preprocessing but a
//! deliberately conservative classification table. The design
//! prioritizes:
//!
//! 1. **Turkic case-fold correctness.** Uppercase `I` folds to `ı`,
//!    then to ASCII `I`; uppercase `İ` folds to `i`. Getting this
//!    right is the whole reason a Turkish-specific encoder exists at
//!    all — a generic Soundex over `to_ascii_uppercase` would
//!    conflate the two.
//! 2. **Accent folding to ASCII equivalents.** `ç → C`, `ğ → G`,
//!    `ı → I`, `ö → O`, `ş → S`, `ü → U`. Turkish spelling is
//!    invertible into ASCII by this table with only minor loss (the
//!    softening semantics of `ğ` are context-dependent, but the
//!    encoder treats it as a plain consonant).
//! 3. **A Soundex-shape 4-character key.** Retain the initial letter;
//!    encode subsequent consonants by class; collapse duplicates;
//!    truncate to length 4, pad with `'0'`.
//!
//! **Classification table.** Adapted from the classic Soundex family:
//!
//! | Code | Turkish letters       |
//! |------|-----------------------|
//! | 1    | B P F V W             |
//! | 2    | C Ç G K Q Ğ J Y       |
//! | 3    | D T                   |
//! | 4    | L                     |
//! | 5    | M N                   |
//! | 6    | R                     |
//! | 7    | S Ş Z X               |
//! | 0    | A E I O U H (vowels + silent-H) |
//!
//! **Grouping rationale.** `B/P/F/V/W` are the labial family. `C/Ç/G/K/Ğ/J/Y`
//! are the velar / palatal cluster (Turkish `ğ` softens the preceding
//! vowel — no independent phonetic value — but the encoder codes it
//! with the velars since surface spelling puts it there). `S/Ş/Z/X`
//! are the sibilants. `R` gets its own class as in every Soundex
//! variant.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone-shaped variable-length encoder.** A future refinement
//!   could produce a stem-like variable-length key; better precision
//!   for record linkage but heavier to reference-test.
//! * **Loanword phonology.** Turkish loanwords from Arabic, Persian,
//!   and French sometimes retain non-native phonotactics; the shipped
//!   encoder treats them as native.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::case_fold::to_turkish_lower_char;

/// The Turkish PHONEX encoder.
///
/// A zero-sized value; construct as [`TurkishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_tr::TurkishPhonex;
///
/// // "İstanbul" — İ → i under Turkish fold → I under ASCII step.
/// let key = TurkishPhonex.encode("İstanbul").unwrap();
/// assert_eq!(key, "I735");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TurkishPhonex;

impl TurkishPhonex {
    /// Encodes `word` per the PHONEX-Turkish algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1: Turkish-aware lowercase, then fold to ASCII
        // uppercase.
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        // Step 2: Soundex-shape encoding.
        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
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

/// Preprocess `word` into uppercase-ASCII letters.
///
/// 1. Turkish-lowercase each scalar (dotted / dotless-I handled).
/// 2. Fold Turkish special letters to their ASCII base
///    (`ç → c`, `ğ → g`, `ı → i` (via the intermediate dotless
///    fold — but at this point we want ASCII, so `ı → I`), `ö → o`,
///    `ş → s`, `ü → u`).
/// 3. Uppercase.
/// 4. Drop non-letter scalars.
fn preprocess(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for c in word.chars() {
        let lowered = to_turkish_lower_char(c);
        let ascii = fold_to_ascii_upper(lowered);
        if let Some(a) = ascii {
            out.push(a);
        }
    }
    out
}

/// Fold `c` (assumed already Turkish-lowercased) to a single ASCII
/// uppercase letter, or return `None` if `c` isn't letter-like.
fn fold_to_ascii_upper(c: char) -> Option<char> {
    // ASCII fast path.
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Turkish-specific single-scalar folds.
    let folded = match c {
        'ç' => 'C',
        'ğ' => 'G',
        'ı' => 'I',
        'ö' => 'O',
        'ş' => 'S',
        'ü' => 'U',
        _ => return None,
    };
    Some(folded)
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the classification table in the [module-level docs](self).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'G' | b'K' | b'Q' | b'J' | b'Y' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' | b'X' => b'7',
        // A E I O U H — dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`TurkishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Turkish::phonetic_encoder`](crate::Turkish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TurkishPhonexAdapter;

impl LanguagePhoneticEncoder for TurkishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        TurkishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-tr"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        TurkishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(TurkishPhonex.encode("").is_none());
        assert!(TurkishPhonex.encode("   ").is_none());
        assert!(TurkishPhonex.encode("---").is_none());
    }

    #[test]
    fn turkish_special_letters_fold_to_ascii() {
        // "çocuk" → lowercase → "çocuk" → ASCII fold: c,o,c,u,k → "COCUK"
        //   Encode: C(seed, last=2), O(reset,last=0), C(2,dup? last is 0
        //     from reset — push, last=2), U(reset), K(2,dup? last is 0
        //     — push, last=2) → C, 2, 2 → wait: after O reset last=0.
        //     C(2) != 0, push → out="CC", last=2. U(reset), last=0.
        //     K(2) != 0, push → "CC2", last=2. Then we're at length 3,
        //     but 4th slot needs pad → "CC20".
        //   Hmm let me re-trace: out=[C]. bytes[1..]=[O,C,U,K].
        //     O: code=0, last=0, continue.
        //     C: code=2, != 0 (last), push. out=[C,2], last=2. len==2.
        //     U: code=0, reset last=0.
        //     K: code=2, != last=0, push. out=[C,2,2], last=2. len==3.
        //   Loop ends. Pad: "C220"
        assert_eq!(p("çocuk"), "C220");
    }

    #[test]
    fn dotted_and_dotless_i_are_handled_distinctly() {
        // "İstanbul" → İ → i → I → "ISTANBUL"
        //   Encode: I(seed,last=0), S(7), T(3), A(reset), N(5),
        //     B(1), U(reset), L(4) → I, 7, 3, 5 (cap at 4) → "I735"
        //   Wait: out=[I]. bytes[1..]=[S,T,A,N,B,U,L].
        //     S: code=7, push. out=[I,7]. last=7.
        //     T: code=3, push. out=[I,7,3]. last=3.
        //     A: code=0, reset last=0.
        //     N: code=5, push. out=[I,7,3,5]. last=5. len==4 → break.
        //   → "I735"
        assert_eq!(p("İstanbul"), "I735");

        // "IŞIK" (light) → I → ı → I; Ş → ş → S; I → ı → I; K → K
        //   → "ISIK"
        //   Encode: I(seed,last=0), S(7), I(reset), K(2) → I, 7, 2 → "I720"
        assert_eq!(p("IŞIK"), "I720");
    }

    #[test]
    fn ascii_and_lowercase_are_equivalent() {
        assert_eq!(p("Ankara"), p("ankara"));
        assert_eq!(p("İZMİR"), p("izmir"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ev"), "E100");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Appa" → A(seed), P(1), P(dup drop), A(0/reset) → "A1" → "A100"
        assert_eq!(p("Appa"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_tr() {
        assert_eq!(TurkishPhonexAdapter.name(), "phonex-tr");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(TurkishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = TurkishPhonexAdapter.encode("İstanbul").unwrap();
        assert_eq!(primary, "I735");
        assert!(alt.is_none());
    }
}
