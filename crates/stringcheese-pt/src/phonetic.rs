//! A Portuguese-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Portuguese, like Spanish, lacks a single canonical phonetic encoder
//! the way English has Soundex and German has Kölner Phonetik. The
//! closest published candidates are:
//!
//! * **Language-independent Soundex** applied to ASCII-folded
//!   Portuguese — stable, portable, not linguistically accurate
//!   (nasal `ão`, palatal `lh` / `nh`, silent `H`, `ç` all get swept
//!   under the English mapping).
//! * **PHONEX-family encoders** — the Portuguese equivalent of the
//!   French PHONEX shipped in `stringcheese-fr` and the Spanish
//!   PHONEX shipped in `stringcheese-es`: apply Portuguese-specific
//!   preprocessing (fold accents, remap digraphs `LH → L` / `NH → N`
//!   / `CH → X`, map `QU → K`, `Ç → S`, collapse the nasal `ão`,
//!   drop silent `H`) then run a Soundex-shape encoder.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Portuguese** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with a Portuguese-tuned preprocessing pass and a
//! Portuguese-tuned classification table:
//!
//! 1. **Uppercase and un-accent.** `Á Ä À Â → A`, `É Ë È Ê → E`,
//!    `Í Ï Ì Î → I`, `Ó Ö Ò Ô → O`, `Ú Ü Ù Û → U`, `Ç → S`. Nasal
//!    vowels `Ã → A`, `Õ → O` (their consonantal nasal quality is
//!    absorbed by any following `N` / `M`; when the nasal is
//!    word-final we fold the vowel and let the `O` after `Ã` — as
//!    in `ão` — code together with the vowel-run collapse).
//! 2. **Portuguese digraph and single-letter substitutions.**
//!    * `LH → L` (Portuguese `lh` is a palatal lateral, phonetically
//!      close to `L`).
//!    * `NH → N` (palatal nasal, phonetically close to `N`).
//!    * `CH → X` (using `X` as a stand-in for `/ʃ/`, which codes as
//!      the sibilant class).
//!    * `QU → K` (`Q` in Portuguese never appears without `U` and the
//!      pair is always pronounced `/k/` or `/kw/`).
//!    * `RR → R` (single and double R differ phonetically but both
//!      code the same digit).
//!    * `PH → F` (imported words).
//!    * `GN → N` (imported words; rare in Portuguese).
//!    * `ÃO / AO → 0` (the nasal-final diphthong collapses to just
//!      the `A` vowel — silent for coding purposes).
//!    * `Ç → S` (already folded in step 1; kept here as belt-and-
//!      braces for `ç` in mixed-case input).
//!    * `H → ` (silent — dropped entirely).
//!    * `W → V → B` (imported words; folded to `B` via the `V → B`
//!      step below).
//!    * `V → B` (unlike Spanish, Portuguese distinguishes `v` and `b`,
//!      but the acoustic space is close enough for a phonetic
//!      encoder to fold — this choice is conservative and matches
//!      the -es pack's design).
//!    * `Z → S` (Portuguese `z` between vowels voices to `/z/`, but
//!      for encoding purposes we fold it into the sibilant class
//!      with `S`).
//!    * `Y → I` when adjacent to a vowel; kept otherwise.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below; drop
//!    the zero class (vowels); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach
//!    length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F   |
//! | 2    | C K G Q J |
//! | 3    | D T     |
//! | 4    | L       |
//! | 5    | M N     |
//! | 6    | R       |
//! | 7    | S X Z   |
//! | 0    | A E I O U H W Y (dropped as vowels or silent H) |
//!
//! **Grouping rationale.** Portuguese `B/P/F` are all labial
//! obstruents; `C/K/G/Q/J` are the velar family (Portuguese `J` is
//! `/ʒ/`, distinct in place but close enough to the velar family for
//! a phonetic encoder); `D/T` are dental stops; `S/X/Z` are the
//! sibilant family (`X` codes `/ʃ/` after `CH → X` preprocessing;
//! `Z` codes `/z/` or `/s/`); `R` gets its own class; `L` gets its
//! own class (post-`LH → L` fold).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Português** — a parallel variable-length encoder
//!   with better discrimination; heavier to reference-test.
//! * **Beider-Morse Portuguese** — a Sephardic-name-aware phonetic
//!   encoder; requires a substantial rule set out of scope for a
//!   starter pack.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Portuguese PHONEX encoder.
///
/// A zero-sized value; construct as [`PortuguesePhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_pt::PortuguesePhonex;
///
/// let key = PortuguesePhonex.encode("Silva").unwrap();
/// assert_eq!(key, "S410");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PortuguesePhonex;

impl PortuguesePhonex {
    /// Encodes `word` per the PHONEX-Portuguese algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
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

/// Preprocess `word` into uppercase-ASCII letters after Portuguese
/// digraph and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII.
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph substitutions first (longest match).
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'L', b'H') => {
                    out.push('L');
                    i += 2;
                    continue;
                }
                // `NH → N` and `GN → N` share an action; collapsed
                // into a single arm to satisfy `clippy::match_same_arms`.
                (b'N', b'H') | (b'G', b'N') => {
                    out.push('N');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    out.push('X');
                    i += 2;
                    continue;
                }
                (b'Q', b'U') => {
                    out.push('K');
                    i += 2;
                    continue;
                }
                (b'R', b'R') => {
                    out.push('R');
                    i += 2;
                    continue;
                }
                (b'P', b'H') => {
                    out.push('F');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'Z' => out.push('S'),
            b'V' | b'W' => out.push('B'),
            b'H' => { /* drop */ }
            b'Y' => {
                let prev = if i == 0 { None } else { Some(bytes[i - 1]) };
                let next = if i + 1 < bytes.len() {
                    Some(bytes[i + 1])
                } else {
                    None
                };
                let is_vowel = |b: u8| matches!(b, b'A' | b'E' | b'I' | b'O' | b'U');
                let adjacent_vowel = prev.is_some_and(is_vowel) || next.is_some_and(is_vowel);
                if adjacent_vowel {
                    out.push('I');
                } else {
                    out.push('Y');
                }
            }
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
    // Portuguese-specific folds. Nasal vowels fold to their base.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'S',
        'ñ' | 'Ñ' => 'N',
        _ => return None,
    };
    Some(folded)
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' => b'1',
        b'C' | b'K' | b'G' | b'Q' | b'J' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'X' | b'Z' => b'7',
        _ => b'0',
    }
}

/// Adapter that exposes [`PortuguesePhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Portuguese::phonetic_encoder`](crate::Portuguese) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PortuguesePhonexAdapter;

impl LanguagePhoneticEncoder for PortuguesePhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        PortuguesePhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-pt"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        PortuguesePhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(PortuguesePhonex.encode("").is_none());
        assert!(PortuguesePhonex.encode("   ").is_none());
        assert!(PortuguesePhonex.encode("---").is_none());
    }

    #[test]
    fn accents_are_folded() {
        // Nasal accents fold to the base vowel — `ã` and `õ` are
        // equivalent to their unadorned forms for coding purposes.
        assert_eq!(p("São"), p("Sao"));
        assert_eq!(p("João"), p("Joao"));
        // The cedilla in `Coração` marks the /s/ pronunciation and
        // encodes with the sibilant class, distinct from a plain `c`.
        // The equivalence to test is against `Corasao` (post-fold),
        // not the un-cedilla'd `Coracao`.
        assert_eq!(p("Coração"), p("Corasao"));
    }

    #[test]
    fn cedilla_folds_to_s() {
        // "Cação" → C(seed,last=2), A(reset), Ç→S(7), A(reset),
        //          Ã→A(reset), O(reset) → C, 7 → "C700"
        assert_eq!(p("Cação"), "C700");
        assert_eq!(p("Cação"), p("Casao"));
    }

    #[test]
    fn digraph_lh_collapses_to_l() {
        // "Filho" — F(seed,last=1), I(reset), LH→L(4), O(reset) → F, 4 → "F400"
        assert_eq!(p("Filho"), "F400");
    }

    #[test]
    fn digraph_nh_collapses_to_n() {
        // "Ninho" — N(seed,last=5), I(reset), NH→N(5) — dup drop after
        //   reset? Let me trace:
        //   After preprocess: "NINO" (NH→N).
        //   Encode: N(seed,last=5), I(reset,last=0), N(5,push,last=5),
        //   O(reset,last=0) → "N5" → "N500"
        assert_eq!(p("Ninho"), "N500");
    }

    #[test]
    fn digraph_ch_codes_as_x() {
        // "Chaves" — CH→X, A, V→B, E, S → "XABES"
        //   Encode: X(seed,last=7), A(reset), B(1,push,last=1), E(reset),
        //   S(7,push,last=7) → "X17" → "X170"
        assert_eq!(p("Chaves"), "X170");
    }

    #[test]
    fn qu_maps_to_k() {
        // "Queiroz" — QU→K, EIROZ. Preprocess: "KEIROS" (Z→S)
        //   Encode: K(seed,last=2), E(reset), I(reset), R(6,push,last=6),
        //   O(reset), S(7,push,last=7) → "K67" → "K670"
        assert_eq!(p("Queiroz"), "K670");
    }

    #[test]
    fn h_is_silent() {
        assert_eq!(p("Henrique"), p("Enrique"));
    }

    #[test]
    fn v_and_b_are_equivalent() {
        assert_eq!(p("Vieira"), p("Bieira"));
    }

    #[test]
    fn z_and_s_are_equivalent() {
        assert_eq!(p("Souza"), p("Sousa"));
    }

    #[test]
    fn common_portuguese_surnames() {
        // Silva: S(seed,last=7), I(reset), L(4), B(1), A(reset)
        //   → S, 4, 1 → "S410"
        assert_eq!(p("Silva"), "S410");
        // Santos: S(seed,last=7), A(reset), N(5), T(3), O(reset), S(7)
        //   → S, 5, 3, 7 → "S537"
        assert_eq!(p("Santos"), "S537");
        // Oliveira: O(seed,last=0), L(4), I(reset), B(1) (V→B), E(reset), I(reset), R(6), A(reset)
        //   → O, 4, 1, 6 → "O416"
        assert_eq!(p("Oliveira"), "O416");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("SILVA"), p("silva"));
        assert_eq!(p("São"), p("SÃO"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("De"), "D000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Abba" — A(seed), B(1,push,last=1), B(dup drop), A(reset)
        //   → "A1" → "A100"
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_pt() {
        assert_eq!(PortuguesePhonexAdapter.name(), "phonex-pt");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(PortuguesePhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = PortuguesePhonexAdapter.encode("Silva").unwrap();
        assert_eq!(primary, "S410");
        assert!(alt.is_none());
    }
}
