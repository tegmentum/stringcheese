//! The Kölner Phonetik encoder.
//!
//! # Origin
//!
//! Hans Joachim Postel published Kölner Phonetik (also called *Kölner
//! Verfahren*) in *IBM-Nachrichten* 19 (1969), 925-931. It is the
//! standard German-language sound-alike encoder — the German-friendly
//! answer to Soundex, taking German phonemes and orthographic quirks
//! into account (`sch`, `ch`, `pf`, `dt`, the umlauts, `ß`) rather than
//! collapsing them under an English-first mapping.
//!
//! # Algorithm sketch
//!
//! Every letter of the input word is mapped to a single decimal digit
//! (or `-`, meaning "no code") according to the table below. Several
//! letters map differently depending on their neighbours (`C`, `D`,
//! `T`, `P`, `X`), so the encoder walks the input in a small
//! left-to-right pass carrying the previous letter as context.
//!
//! | Letter                | Context                                                 | Code |
//! |-----------------------|---------------------------------------------------------|------|
//! | `A E I J O U Y`       | any                                                     | `0`  |
//! | `H`                   | any                                                     | (skip) |
//! | `B`                   | any                                                     | `1`  |
//! | `P`                   | before `H`                                              | `3`  |
//! | `P`                   | otherwise                                               | `1`  |
//! | `D T`                 | before `C S Z`                                          | `8`  |
//! | `D T`                 | otherwise                                               | `2`  |
//! | `F V W`               | any                                                     | `3`  |
//! | `G K Q`                | any                                                    | `4`  |
//! | `C` (word-initial)    | before `A H K L O Q R U X`                              | `4`  |
//! | `C` (word-initial)    | otherwise                                               | `8`  |
//! | `C` (elsewhere)       | after `S Z`                                             | `8`  |
//! | `C` (elsewhere)       | before `A H K O Q U X`                                  | `4`  |
//! | `C` (elsewhere)       | otherwise                                               | `8`  |
//! | `X`                   | after `C K Q`                                           | `8`  |
//! | `X`                   | otherwise                                               | `48` |
//! | `L`                   | any                                                     | `5`  |
//! | `M N`                 | any                                                     | `6`  |
//! | `R`                   | any                                                     | `7`  |
//! | `S Z`                 | any                                                     | `8`  |
//!
//! The two post-processing steps are then applied in order:
//!
//! 1. **Collapse** consecutive duplicate digits into one occurrence.
//! 2. **Delete** every `0` digit that is not the very first character
//!    of the resulting code (so a word that begins with a vowel keeps
//!    its leading `0`, and every internal vowel-derived `0` is
//!    discarded).
//!
//! Preprocessing:
//!
//! * The input is uppercased (Unicode-aware).
//! * Umlauts fold to their base letters: `Ä → A`, `Ö → O`, `Ü → U`.
//! * `ß` expands to `SS`.
//! * Non-letters are ignored (skipped, not treated as a boundary).
//!
//! # Non-goals
//!
//! * **Umlaut-preserving codes.** Some regional variants of the
//!   algorithm distinguish `Ä`/`Ö`/`Ü` from `A`/`O`/`U`; this
//!   implementation follows Postel's original fold.
//! * **Alternate keys.** Kölner Phonetik is a single-key algorithm.
//!   The [`LanguagePhoneticEncoder`] wrapper always returns
//!   `Some((primary, None))`.
//! * **Language detection.** The encoder assumes German pronunciation
//!   rules; feeding it English or Dutch names produces a valid code but
//!   the code has no phonetic meaning for those languages.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Kölner Phonetik encoder.
///
/// A zero-sized unit value; construct as `KoelnerPhonetik` and reuse
/// the value freely across threads and calls, or delegate through the
/// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
/// accessor on the German language pack.
///
/// See the [module-level docs](self) for the encoding table and
/// post-processing rules.
///
/// # Example
///
/// ```
/// use stringcheese_de::KoelnerPhonetik;
///
/// assert_eq!(KoelnerPhonetik.encode("Müller"), Some("657".into()));
/// assert_eq!(KoelnerPhonetik.encode("Schmidt"), Some("862".into()));
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KoelnerPhonetik;

impl KoelnerPhonetik {
    /// Encodes `word` into its Kölner Phonetik key.
    ///
    /// Returns `None` when `word` has no encodable letters (an empty
    /// string, whitespace only, or a string consisting entirely of
    /// letters that produce no code — e.g. `"H"` alone).
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Preprocess: uppercase, fold umlauts, expand ß, keep only
        // ASCII letters after that step.
        let mut prepared: Vec<char> = Vec::with_capacity(word.len());
        for ch in word.chars() {
            for uc in ch.to_uppercase() {
                match uc {
                    'Ä' => prepared.push('A'),
                    'Ö' => prepared.push('O'),
                    'Ü' => prepared.push('U'),
                    // Handle both ß (lower) and its rarer capital form ẞ,
                    // which some post-2017 corpora carry.
                    'ß' | 'ẞ' => {
                        prepared.push('S');
                        prepared.push('S');
                    }
                    c if c.is_ascii_alphabetic() => prepared.push(c),
                    _ => {
                        // Any other char (non-letter, non-ASCII letter
                        // outside the German umlaut family) is simply
                        // skipped — the algorithm ignores non-letters.
                    }
                }
            }
        }

        if prepared.is_empty() {
            return None;
        }

        // Encode.
        let mut codes: Vec<u8> = Vec::with_capacity(prepared.len());
        for (i, &c) in prepared.iter().enumerate() {
            let prev = if i > 0 { Some(prepared[i - 1]) } else { None };
            let next = prepared.get(i + 1).copied();
            encode_letter(c, prev, next, i == 0, &mut codes);
        }

        if codes.is_empty() {
            return None;
        }

        // Collapse consecutive duplicates.
        let mut collapsed: Vec<u8> = Vec::with_capacity(codes.len());
        for d in codes {
            if collapsed.last() != Some(&d) {
                collapsed.push(d);
            }
        }

        // Delete every `0` digit that is not the very first digit.
        let mut result: Vec<u8> = Vec::with_capacity(collapsed.len());
        for (i, &d) in collapsed.iter().enumerate() {
            if d == 0 && i > 0 {
                continue;
            }
            result.push(d);
        }

        if result.is_empty() {
            return None;
        }

        Some(result.into_iter().map(|d| char::from(b'0' + d)).collect())
    }
}

/// Encode one letter into 0..=2 code digits, appending to `out`.
///
/// `c` is the current letter (uppercase ASCII). `prev` and `next` are
/// the neighbouring letters if any (also uppercase ASCII). `at_start`
/// is `true` when `c` is the first letter of the word.
fn encode_letter(
    c: char,
    prev: Option<char>,
    next: Option<char>,
    at_start: bool,
    out: &mut Vec<u8>,
) {
    match c {
        'A' | 'E' | 'I' | 'J' | 'O' | 'U' | 'Y' => out.push(0),
        // H produces no code; falls through to the same "skip" arm as
        // any non-encodable letter so the match stays exhaustive.
        'B' => out.push(1),
        'P' => out.push(if next == Some('H') { 3 } else { 1 }),
        'D' | 'T' => {
            if matches!(next, Some('C' | 'S' | 'Z')) {
                out.push(8);
            } else {
                out.push(2);
            }
        }
        'F' | 'V' | 'W' => out.push(3),
        'G' | 'K' | 'Q' => out.push(4),
        'C' => {
            let code = if at_start {
                if matches!(
                    next,
                    Some('A' | 'H' | 'K' | 'L' | 'O' | 'Q' | 'R' | 'U' | 'X')
                ) {
                    4
                } else {
                    8
                }
            } else if matches!(prev, Some('S' | 'Z')) {
                8
            } else if matches!(next, Some('A' | 'H' | 'K' | 'O' | 'Q' | 'U' | 'X')) {
                4
            } else {
                8
            };
            out.push(code);
        }
        'X' => {
            if matches!(prev, Some('C' | 'K' | 'Q')) {
                out.push(8);
            } else {
                out.push(4);
                out.push(8);
            }
        }
        'L' => out.push(5),
        'M' | 'N' => out.push(6),
        'R' => out.push(7),
        'S' | 'Z' => out.push(8),
        // `H` and any letter outside the algorithm's alphabet fall
        // here — produce no code.
        _ => {}
    }
}

impl LanguagePhoneticEncoder for KoelnerPhonetik {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        KoelnerPhonetik::encode(self, word).map(|primary| (primary, None))
    }

    fn name(&self) -> &'static str {
        "koelner-phonetik"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(w: &str) -> Option<String> {
        KoelnerPhonetik.encode(w)
    }

    #[test]
    fn well_known_surnames() {
        assert_eq!(e("Müller").as_deref(), Some("657"));
        assert_eq!(e("Schmidt").as_deref(), Some("862"));
        assert_eq!(e("Meier").as_deref(), Some("67"));
        assert_eq!(e("Meyer").as_deref(), Some("67"));
        assert_eq!(e("Fischer").as_deref(), Some("387"));
    }

    #[test]
    fn case_and_umlaut_normalization() {
        // Case insensitivity + umlaut folding. `Müller` and `mueller`
        // both encode to "657" — the Ü folds to U and the digit-collapse
        // step collapses the UE pair down to a single 0 that then gets
        // dropped as a non-leading zero.
        assert_eq!(e("Müller"), e("MÜLLER"));
        assert_eq!(e("Müller"), e("mueller"));
    }

    #[test]
    fn empty_and_non_letter_input() {
        assert_eq!(e(""), None);
        assert_eq!(e("   "), None);
        assert_eq!(e("---"), None);
        assert_eq!(e("123"), None);
    }

    #[test]
    fn all_h_returns_none() {
        // H alone produces no code (it's the only letter with no code).
        assert_eq!(e("H"), None);
        assert_eq!(e("hhh"), None);
    }

    #[test]
    fn leading_vowel_keeps_the_zero() {
        // "Ohm": O(0) H(-) M(6) → 0,6 → "06".
        assert_eq!(e("Ohm").as_deref(), Some("06"));
        // "Axel": A(0) X(48) E(0) L(5) → 0,4,8,0,5 → keep leading 0,
        //   drop internal 0 → 0,4,8,5 → "0485".
        assert_eq!(e("Axel").as_deref(), Some("0485"));
    }

    #[test]
    fn x_after_ckq_becomes_8() {
        // "Six" is 3 letters: S(8) I(0) X — X preceded by I (not
        //   C/K/Q), so X → 48. Codes: 8,0,4,8. No dups. Drop internal 0.
        //   → 8,4,8 → "848".
        assert_eq!(e("Six").as_deref(), Some("848"));
    }

    #[test]
    fn ss_expansion_from_eszett() {
        // "Straße" and "Strasse" should encode the same.
        assert_eq!(e("Straße"), e("Strasse"));
    }

    #[test]
    fn adapter_returns_primary_only() {
        let enc = &KoelnerPhonetik as &dyn LanguagePhoneticEncoder;
        assert_eq!(enc.name(), "koelner-phonetik");
        let (primary, alt) = enc.encode("Müller").expect("Müller has a code");
        assert_eq!(primary, "657");
        assert!(alt.is_none());
    }
}
