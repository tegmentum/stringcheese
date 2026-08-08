//! A Norwegian-tuned Soundex-family phonetic encoder, packaged for
//! Nynorsk.
//!
//! # Origin
//!
//! Nynorsk shares its phonological cluster set with Bokmål (both
//! written standards describe the same set of spoken Norwegian
//! phonemes; the differences are lexical and morphological, not
//! phonetic). This module therefore ports the same PHONEX-Norwegian
//! encoder documented in the sibling
//! [`stringcheese-no`](https://docs.rs/stringcheese-no) crate —
//! Soundex-shaped 4-character key, Norwegian-tuned preprocessing
//! (`skj → S`, `sk` before front vowels → `S`, `kj → C`, `k` before
//! front vowels → `C`, `ch → S`, silent word-initial `h`, and the
//! `å → O` / `æ → E` / `ø → E` vowel folds), and the standard PHONEX
//! classification table.
//!
//! The adapter name is `"phonex-nn"` rather than `"phonex-no"` so a
//! caller picking the encoder by name can distinguish which language
//! pack it came from.
//!
//! # Implementation choice
//!
//! Concretely, the algorithm is a 4-character
//! `<letter><digit><digit><digit>` Soundex key with Norwegian-tuned
//! preprocessing and the standard PHONEX classification table:
//!
//! 1. **Uppercase and un-accent.** `Á À Â Ä → A`, `É È Ê Ë → E`, `Í Ì
//!    Î Ï → I`, `Ó Ò Ô Ö → O`, `Ú Ù Û Ü → U`. Norwegian-specific vowel
//!    folds: `Å → O`, `Æ → E`, `Ø → E`.
//! 2. **Norwegian digraph and single-letter substitutions.**
//!    Longest-match first:
//!    * `SKJ → S`; `SK` before a front vowel `E I Y` → `S`; `KJ → C`;
//!      `K` before a front vowel `E I Y` → `C`; `CH → S`; word-initial
//!      `H` dropped.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below; drop
//!    the zero class (vowels); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach length
//!    four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X |
//! | 3    | D T     |
//! | 4    | L       |
//! | 5    | M N     |
//! | 6    | R       |
//! | 7    | S Z     |
//! | 0    | A E I O U Y (dropped as vowels); H is stripped in preprocessing |

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Nynorsk PHONEX encoder (same algorithm as the Bokmål sibling's
/// `NorwegianPhonex`).
///
/// A zero-sized value; construct as [`NynorskPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_nn::NynorskPhonex;
///
/// // The silent-H preprocessor drops the initial H, so `Hansen`
/// // encodes from the residue `Ansen`: A-seed, N(5), S(7), N(5).
/// let key = NynorskPhonex.encode("Hansen").unwrap();
/// assert_eq!(key, "A575");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NynorskPhonex;

impl NynorskPhonex {
    /// Encodes `word` per the PHONEX-Norwegian algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to nothing
    /// after silent-`H` stripping). Otherwise returns a 4-character
    /// key of the form `<uppercase letter><three ASCII digits>`.
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

/// Preprocess `word` into uppercase-ASCII letters after Norwegian
/// digraph and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII (drops non-letter code points).
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions. Longest-match
    // first: SKJ before SK before S, and KJ before K.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Three-byte digraphs first.
        if i + 2 < bytes.len() && b == b'S' && bytes[i + 1] == b'K' && bytes[i + 2] == b'J' {
            out.push('S');
            i += 3;
            continue;
        }
        // Two-byte digraphs and front-vowel-context single-letter
        // substitutions.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'S', b'K') if is_front_vowel(bytes.get(i + 2).copied()) => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                (b'K', b'J') => {
                    out.push('C');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                (b'K', _) if is_front_vowel(Some(b2)) => {
                    out.push('C');
                    i += 1;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* drop silent H */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// True if `b` is a Norwegian front vowel (`E I Y`). The
/// Norwegian-specific vowels `Æ` and `Ø` have already been folded to
/// `E` in [`fold_letter`], so after preprocessing they present as `E`
/// here — that's fine: the palatalization rule fires on the same
/// segments regardless of whether they were spelled `æ`/`ø` or `e`.
#[inline]
fn is_front_vowel(b: Option<u8>) -> bool {
    matches!(b, Some(b'E' | b'I' | b'Y'))
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Norwegian-specific folds. `å → O` (open back rounded /ɔ/), `æ →
    // E` (open front /ɛ/), `ø → E` (rounded mid front). Diaeresis /
    // acute-carrying loans fold to base vowels. The Norwegian-specific
    // scalars share arms with their base-vowel target so
    // clippy::match_same_arms doesn't flag the collapse.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' | 'æ' | 'Æ' | 'ø' | 'Ø' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'å' | 'Å' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
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

/// Adapter that exposes [`NynorskPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Nynorsk::phonetic_encoder`](crate::Nynorsk) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NynorskPhonexAdapter;

impl LanguagePhoneticEncoder for NynorskPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        NynorskPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-nn"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        NynorskPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(NynorskPhonex.encode("").is_none());
        assert!(NynorskPhonex.encode("   ").is_none());
        assert!(NynorskPhonex.encode("---").is_none());
    }

    #[test]
    fn norwegian_vowels_fold_to_base_letters() {
        // å → O, æ → E, ø → E.
        assert_eq!(p("så"), p("so"));
        assert_eq!(p("vera"), p("vera"));
        assert_eq!(p("øye"), p("eye"));
    }

    #[test]
    fn diaeresis_and_acute_fold_to_base_vowel() {
        assert_eq!(p("café"), p("cafe"));
    }

    #[test]
    fn skj_cluster_encodes_as_sibilant() {
        // `skjære` (magpie) → SKJ → S, ÆRE → SERE → "S600".
        assert_eq!(p("skjære"), "S600");
    }

    #[test]
    fn sk_before_front_vowel_encodes_as_sibilant() {
        // `ski` → SK before I → S. Then I. → "S000".
        assert_eq!(p("ski"), "S000");
        // `skøyte` — SK before Ø(→E, front vowel) → S. → "S300".
        assert_eq!(p("skøyte"), "S300");
    }

    #[test]
    fn sk_before_back_vowel_stays_split() {
        // `skål` — Å→O (back). SK stays split. → "S240".
        assert_eq!(p("skål"), "S240");
    }

    #[test]
    fn kj_encodes_as_palatal_c() {
        // `kjøre` (drive) → KJ → C, ØRE → CERE → "C600".
        assert_eq!(p("kjøre"), "C600");
    }

    #[test]
    fn k_before_front_vowel_encodes_as_palatal_c() {
        // `kino` (cinema) → K before I → C, INO → CINO → "C500".
        assert_eq!(p("kino"), "C500");
    }

    #[test]
    fn k_before_back_vowel_stays_velar() {
        // `kake` (cake) → K, A, K before E → C, E → "K200".
        assert_eq!(p("kake"), "K200");
    }

    #[test]
    fn ch_encodes_as_sibilant() {
        // `chef` → CH → S, EF → SEF → "S100".
        assert_eq!(p("chef"), "S100");
    }

    #[test]
    fn silent_h_is_stripped() {
        assert_eq!(p("Hansen"), p("Ansen"));
    }

    #[test]
    fn hansen_encodes() {
        assert_eq!(p("Hansen"), "A575");
    }

    #[test]
    fn common_norwegian_surnames() {
        assert_eq!(p("Olsen"), "O475");
        assert_eq!(p("Berg"), "B620");
        assert_eq!(p("Larsen"), "L675");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("HANSEN"), p("hansen"));
        assert_eq!(p("SKJÆRE"), p("skjære"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("På"), "P000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_nn() {
        assert_eq!(NynorskPhonexAdapter.name(), "phonex-nn");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(NynorskPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = NynorskPhonexAdapter.encode("Hansen").unwrap();
        assert_eq!(primary, "A575");
        assert!(alt.is_none());
    }
}
