//! A Romanian-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Romanian lacks a single canonical phonetic encoder the way English
//! has Soundex, German has Kölner Phonetik, and French has PHONEX.
//! Common Romanian record-linkage practice is to apply a Soundex-
//! shaped 4-character key over a diacritic-folded ASCII form with
//! Romance-tuned digraph preprocessing (`ch` → `k` before front
//! vowel, `gh` → `g` before front vowel — Romanian's spelling
//! convention for the hard velar phonemes `/k/` and `/g/` before
//! `e` and `i`). This module ships that PHONEX-Romanian variant.
//!
//! # Implementation
//!
//! The algorithm is a 4-character `<letter><digit><digit><digit>`
//! Soundex key with a Romanian-tuned preprocessing pass and a
//! Romance-tuned classification table:
//!
//! 1. **Fold cedilla to comma-below.** `ş → ș`, `ţ → ț` (both cases).
//!    This lets a corpus authored on older systems collide with
//!    modern-orthography queries under the same key.
//! 2. **Uppercase and un-diacritic.** `Ă → A`, `Â → A`, `Î → I`,
//!    `Ș → S`, `Ț → T`. Non-Romanian accented letters that appear
//!    in imported names (`é`, `ü`, `ç`, `ñ`) also fold to their base
//!    letters for stability.
//! 3. **Romanian digraph and single-letter substitutions.**
//!    * `CH` → `K` when the next letter is `E` or `I` (Romanian
//!      writes `ch` before front vowels to spell hard `/k/`;
//!      `chibrit` "match", `chef` "party").
//!    * `GH` → `G` when the next letter is `E` or `I` (same
//!      pattern, spelling hard `/g/` before front vowel;
//!      `ghid` "guide", `ghereta` "sentry-box").
//!    * `CH` (elsewhere) → `X` (imports; treated as `/tʃ/`).
//!    * `PH` → `F` (imports; e.g., `Philip → Filip`).
//!    * `TZ` → `T` (transliteration of `ț` in older systems).
//!    * `H` (silent after any consonant, kept word-initially) —
//!      Romanian `h` is `/h/` when initial or intervocalic but
//!      silent as a modifier in digraphs.
//!    * `W → V`, `Y → I` (imports).
//! 4. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below;
//!    drop the zero class (vowels); collapse consecutive equal
//!    codes; truncate to three digits and left-pad with `'0'` to
//!    reach length four.
//!
//! **Classification table.**
//!
//! | Code | Letters   |
//! |------|-----------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X |
//! | 3    | D T       |
//! | 4    | L         |
//! | 5    | M N       |
//! | 6    | R         |
//! | 7    | S Z       |
//! | 0    | A E I O U H Y (dropped) |
//!
//! **Grouping rationale.** Romanian `B/P/F/V/W` are labial obstruents
//! (unlike Spanish, Romanian **preserves** the `b`/`v` distinction —
//! `băiat` "boy" vs. `vin` "wine" — but a Soundex-shape encoder
//! collapses them anyway because the two are the most common
//! transcription confusion in written records). `C/K/G/Q/J/X` are
//! velars and the `/tʃ/` cluster (`X` after `CH` preprocessing).
//! `D/T` are dental stops (Romanian `ț` = `/ts/` is close enough
//! to `t` for a Soundex-shape key). `L` and `R` get their own
//! classes. `S/Z` are sibilants (`ș` folds to `S`).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Român.** A parallel variable-length encoder;
//!   better for record-linkage precision, heavier to reference-test.
//! * **`ci` / `ce` / `gi` / `ge` context-sensitive `/tʃ/`, `/dʒ/`
//!   encoding.** Romanian writes `ci` for `/tʃi/` and `ce` for
//!   `/tʃe/` (parallel to Italian). The current encoder treats
//!   `C` before a front vowel as a plain velar class 2 — a
//!   deliberate simplification, tracked for a follow-up.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Romanian PHONEX encoder.
///
/// A zero-sized value; construct as [`RomanianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_ro::RomanianPhonex;
///
/// let key = RomanianPhonex.encode("Popescu").unwrap();
/// assert_eq!(key, "P172");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RomanianPhonex;

impl RomanianPhonex {
    /// Encodes `word` per the PHONEX-Romanian algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1 & 2: fold cedilla, uppercase, un-diacritic, and apply
        // the Romanian digraph substitutions. The result is an
        // ASCII-only working buffer.
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        // Step 3: Soundex-shape encoding.
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
        // Pad with '0' to length 4.
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess `word` into uppercase-ASCII letters after Romanian
/// diacritic folding and digraph substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold cedilla + diacritics to uppercase-ASCII base
    // letters. Non-letter scalars are dropped.
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
                // CH before front vowel → K (hard velar). Elsewhere
                // → X (imports, /tʃ/).
                (b'C', b'H') => {
                    let b3 = bytes.get(i + 2).copied();
                    if matches!(b3, Some(b'E' | b'I')) {
                        out.push('K');
                    } else {
                        out.push('X');
                    }
                    i += 2;
                    continue;
                }
                // GH → G. Romanian writes `gh` before front vowel to
                // spell hard `/g/` (`ghid`, `ghereta`); elsewhere the
                // digraph is rare (loan-words) and the collapse to
                // `G` remains the honest Soundex-shape choice.
                (b'G', b'H') => {
                    out.push('G');
                    i += 2;
                    continue;
                }
                // PH → F (imports).
                (b'P', b'H') => {
                    out.push('F');
                    i += 2;
                    continue;
                }
                // TZ → T (older transliteration of ț).
                (b'T', b'Z') => {
                    out.push('T');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'W' => out.push('V'),
            b'Y' => out.push('I'),
            b'H' => {
                // Silent after any preceding letter; kept word-
                // initially so the seed slot doesn't lose the H.
                if out.is_empty() {
                    out.push('H');
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
    // Fast path: ASCII letters.
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Romanian-specific + Romance-neighbour folds.
    let folded = match c {
        // Romanian modern (comma-below).
        'ă' | 'Ă' | 'â' | 'Â' | 'à' | 'á' | 'ä' | 'À' | 'Á' | 'Ä' => 'A',
        // Romanian sibilants (comma-below and cedilla) plus Portuguese
        // `ç` — all fold to S under the shipped classification.
        'ș' | 'Ș' | 'ş' | 'Ş' | 'ç' | 'Ç' => 'S',
        'ț' | 'Ț' | 'ţ' | 'Ţ' => 'T',
        'î' | 'Î' | 'í' | 'ì' | 'ï' | 'Í' | 'Ì' | 'Ï' => 'I',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ñ' | 'Ñ' => 'N',
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
        b'C' | b'K' | b'G' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        // A E I O U H Y — dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`RomanianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Romanian::phonetic_encoder`](crate::Romanian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RomanianPhonexAdapter;

impl LanguagePhoneticEncoder for RomanianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        RomanianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-ro"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        RomanianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(RomanianPhonex.encode("").is_none());
        assert!(RomanianPhonex.encode("   ").is_none());
        assert!(RomanianPhonex.encode("---").is_none());
    }

    #[test]
    fn diacritics_are_folded() {
        assert_eq!(p("brânză"), p("branza"));
        assert_eq!(p("mămăligă"), p("mamaliga"));
    }

    #[test]
    fn cedilla_and_comma_below_produce_same_key() {
        // ş (U+015F) and ș (U+0219) both fold to S.
        // ţ (U+0163) and ț (U+021B) both fold to T.
        assert_eq!(p("eşti"), p("ești"));
        assert_eq!(p("ţară"), p("țară"));
    }

    #[test]
    fn ch_before_front_vowel_maps_to_k() {
        // "chibrit" — CH before I → K. Then IBRIT.
        //   preprocess: K I B R I T (all-caps ASCII).
        //   encode: K(seed,last=2), I(reset), B(1), R(6), I(reset), T(3)
        //     → K, 1, 6, 3 → "K163"
        assert_eq!(p("chibrit"), "K163");
    }

    #[test]
    fn gh_before_front_vowel_maps_to_g() {
        // "ghid" — GH before I → G. Then ID.
        //   preprocess: G I D.
        //   encode: G(seed,last=2), I(reset), D(3) → "G30" → "G300"
        assert_eq!(p("ghid"), "G300");
    }

    #[test]
    fn ph_maps_to_f() {
        // "Philip" — PH → F. Then ILIP.
        //   preprocess: F I L I P.
        //   encode: F(seed,last=1), I(reset), L(4), I(reset), P(1)
        //     → F, 4, 1 → "F410"
        assert_eq!(p("Philip"), "F410");
    }

    #[test]
    fn silent_h_after_letter() {
        // "Mihai" — the H is intervocalic; preprocessor drops it
        //   (H only kept word-initially). M I A I → encode
        //   M(seed,last=5), I(reset), A(reset), I(reset) → "M000".
        assert_eq!(p("Mihai"), "M000");
    }

    #[test]
    fn word_initial_h_is_kept() {
        // "Horia" — H kept as seed. Then O R I A → encode
        //   H(seed,last=0), O(reset), R(6), I(reset), A(reset)
        //     → "H600"
        assert_eq!(p("Horia"), "H600");
    }

    #[test]
    fn common_romanian_surnames() {
        // "Popescu" — P O P E S K U → encode
        //   P(seed,last=1), O(reset), P(1), E(reset), S(7), K(2), U(reset)
        //     → P, 1, 7, 2 → "P172"
        assert_eq!(p("Popescu"), "P172");
        // "Ionescu" — I O N E S K U → encode
        //   I(seed,last=0), O(reset), N(5), E(reset), S(7), K(2), U(reset)
        //     → I, 5, 7, 2 → "I572"
        assert_eq!(p("Ionescu"), "I572");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("POPESCU"), p("popescu"));
        assert_eq!(p("Popescu"), p("POPESCU"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("De"), "D000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Bunni" → B U N N I → encode
        //   B(seed,last=1), U(reset), N(5), N(dup drop), I(reset)
        //     → B, 5 → "B500"
        assert_eq!(p("Bunni"), "B500");
    }

    #[test]
    fn v_and_b_collapse_in_class_1() {
        // Both `v` and `b` sit in class 1 (labials); a `V`-init and
        // a `B`-init keep their seed letters distinct, but a
        // consonant class 1 in the tail collapses across letters.
        // Same seed → same key.
        assert_eq!(p("Vlad"), "V430");
        assert_eq!(p("Blad"), "B430");
    }

    #[test]
    fn adapter_returns_name_phonex_ro() {
        assert_eq!(RomanianPhonexAdapter.name(), "phonex-ro");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(RomanianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = RomanianPhonexAdapter.encode("Popescu").unwrap();
        assert_eq!(primary, "P172");
        assert!(alt.is_none());
    }
}
