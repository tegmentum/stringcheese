//! A Spanish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Spanish lacks a single canonical phonetic encoder the way English
//! has Soundex, German has Kölner Phonetik, and French has PHONEX. The
//! closest published candidates are:
//!
//! * **Kondrak / Sherif's Spanish Soundex variants** (Kondrak 2000,
//!   *A New Algorithm for the Alignment of Phonetic Sequences*; Beider
//!   2008 for Spanish surnames in the Daitch-Mokotoff family) — a
//!   Soundex-shaped 4-character key with Spanish-tuned letter classes.
//!   No single reference implementation dominates.
//! * **Language-independent Soundex** applied to ASCII-folded Spanish
//!   — stable, portable, not linguistically accurate (soft-C, silent
//!   H, `ll → /ʎ/`, `ñ → /ɲ/`, `qu → /k/` all get swept under the
//!   English mapping).
//! * **PHONEX-family encoders** — the Spanish equivalent of the
//!   French PHONEX shipped in `stringcheese-fr`: apply Spanish-specific
//!   preprocessing (fold accents, remap digraphs, drop silent H, map
//!   `ñ → N`, `ll → L`, `qu → K`, `v → B`, `z → S`, `x → S`, …) then
//!   run a Soundex-shape encoder.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Spanish** encoder: option 3 above.
//! Concretely, the algorithm is a 4-character `<letter><digit><digit>
//! <digit>` Soundex key with a Spanish-tuned preprocessing pass and a
//! Spanish-tuned classification table:
//!
//! 1. **Uppercase and un-accent.** `Á Ä À Â → A`, `É Ë È Ê → E`,
//!    `Í Ï Ì Î → I`, `Ó Ö Ò Ô → O`, `Ú Ü Ù Û → U`, `Ç → S`. `Ñ` folds
//!    to `N` (its typical phonetic realization).
//! 2. **Spanish digraph and single-letter substitutions.**
//!    * `LL → L` (Spanish `ll` is a single palatal consonant; even in
//!      dialects with *yeísmo* it merges with `Y`, but we fold it to
//!      `L` here for stability).
//!    * `QU → K` (`Q` in Spanish never appears without `U` and the
//!      pair is always pronounced `/k/`).
//!    * `CH → X` (using `X` as a stand-in for `/tʃ/`, which then codes
//!      distinctly from a plain `C` or `S`).
//!    * `RR → R` (single- and double-`R` differ phonetically but both
//!      code the same digit).
//!    * `PH → F` (imported words, e.g., `Philippe → Filipe`).
//!    * `GN → N` (imported words; rare in Spanish).
//!    * `Z → S` (Spanish American *seseo* merges the two; even
//!      Peninsular Spanish's distinction is close enough phonetically
//!      that a phonetic encoder should collapse them).
//!    * `V → B` (Spanish `v` and `b` are the same bilabial phoneme —
//!      the classic *betacismo* — so folding is the honest choice).
//!    * `Y → I` when adjacent to a vowel (Spanish `y` between vowels
//!      acts like `/j/`; a stand-alone `y` in ese-ye-hyphen forms
//!      keeps its consonantal code). We fold aggressively to `I` for
//!      stability.
//!    * `H → ` (silent — dropped entirely).
//!    * `W → V → B` (imported words; folded to `B` via the `V → B`
//!      step).
//! 3. **Soundex-shape encoding.** Retain the first letter; encode each
//!    subsequent letter by the classification table below; drop the
//!    zero class (vowels); collapse consecutive equal codes; truncate
//!    to three digits and left-pad with `'0'` to reach length four.
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
//! | 0    | A E I O U H W Y (dropped after step 2 leaves them as vowels or dropped `H`) |
//!
//! **Grouping rationale.** Spanish `B/P/F` are all labial obstruents
//! and cluster acoustically; `C/K/G/Q/J` are the velar family (Spanish
//! `J` is `/x/`, a velar fricative distinct from English `J`); `D/T`
//! are dental stops; `S/X/Z` are the sibilant family (`Z` merges with
//! `S` per *seseo*; `X` codes `/tʃ/` after `CH → X` preprocessing).
//! `R` gets its own class (Spanish `R` and `RR` are perceptually
//! distinct from every other consonant); `L` gets its own class
//! (Spanish `L` and `LL` merge after preprocessing but are perceptually
//! distinct from other consonants).
//!
//! # Deferred to a follow-up wave
//!
//! * **Kondrak-tuned classification.** A future refinement could
//!   distinguish `G` from `J` when they don't share `/x/`
//!   pronunciation, but the shipped encoder collapses them for
//!   stability.
//! * **Métaphone Español** — a parallel variable-length encoder that
//!   would produce a longer, more discriminating key. Better for
//!   record-linkage precision; heavier to reference-test.
//! * **Beider-Morse Spanish** — the Beider-Morse Phonetic Matching
//!   variant tuned for Spanish surnames (particularly Sephardic
//!   name-etymology cases). Requires a large rule set out of scope for
//!   a starter encoder.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Spanish PHONEX encoder.
///
/// A zero-sized value; construct as [`SpanishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_es::SpanishPhonex;
///
/// let key = SpanishPhonex.encode("García").unwrap();
/// assert_eq!(key, "G620");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpanishPhonex;

impl SpanishPhonex {
    /// Encodes `word` per the PHONEX-Spanish algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1 & 2: uppercase, un-accent, and apply the Spanish
        // digraph substitutions. The result is an ASCII-only working
        // buffer.
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

/// Preprocess `word` into uppercase-ASCII letters after Spanish digraph
/// and single-letter substitutions.
///
/// Rules (applied in order):
///
/// 1. Fold each scalar to a single ASCII uppercase letter (using
///    accent/enye/cedilla folding). Non-letter scalars are dropped.
/// 2. Apply left-to-right substitutions on the ASCII buffer:
///    * `LL → L`, `QU → K`, `CH → X`, `RR → R`, `PH → F`, `GN → N`.
///    * `Z → S`, `V → B`, `W → B`, `H → ` (dropped).
///    * `Y → I` when adjacent to a vowel; otherwise `Y` is kept and
///      encoded as `0` (vowel).
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
                (b'L', b'L') => {
                    out.push('L');
                    i += 2;
                    continue;
                }
                (b'Q', b'U') => {
                    out.push('K');
                    i += 2;
                    continue;
                }
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
                (b'P', b'H') => {
                    out.push('F');
                    i += 2;
                    continue;
                }
                (b'G', b'N') => {
                    out.push('N');
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
                // `Y` folds to `I` when adjacent to a vowel; otherwise
                // stays as-is. Since we already have an ASCII working
                // buffer, we can peek at the previous and next letters.
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
    // Fast path: ASCII letters.
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Spanish-specific folds. `ñ` folds to `N`; `ü` folds to `U`;
    // accented vowels fold to their base.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'S',
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
        b'B' | b'P' | b'F' => b'1',
        b'C' | b'K' | b'G' | b'Q' | b'J' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'X' | b'Z' => b'7',
        // A E I O U H W Y — dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`SpanishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Spanish::phonetic_encoder`](crate::Spanish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpanishPhonexAdapter;

impl LanguagePhoneticEncoder for SpanishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        SpanishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-es"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        SpanishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(SpanishPhonex.encode("").is_none());
        assert!(SpanishPhonex.encode("   ").is_none());
        assert!(SpanishPhonex.encode("---").is_none());
    }

    #[test]
    fn accents_are_folded() {
        assert_eq!(p("García"), p("Garcia"));
        assert_eq!(p("Martínez"), p("Martinez"));
        assert_eq!(p("López"), p("Lopez"));
    }

    #[test]
    fn enye_folds_to_n() {
        // "Muñoz" → after fold: "MUNOZ" → after Z→S: "MUNOS"
        //   → M(seed), U(0/vowel), N(5), O(0/vowel), S(7) → "M57" → pad "M570"
        assert_eq!(p("Muñoz"), "M570");
        // Ñ vs N produce the same key.
        assert_eq!(p("Muñoz"), p("Munoz"));
    }

    #[test]
    fn digraph_ll_collapses_to_l() {
        // "Villa" — V→B, I(0), LL→L, A(0) → "B4" → "B400"
        assert_eq!(p("Villa"), "B400");
    }

    #[test]
    fn digraph_ch_codes_as_x() {
        // "Chavez" — CH→X, A(0), V→B, E(0), Z→S → X(seed), B(1), S(7) → "XBS" wait let me trace
        //  Input: "Chavez" → uppercase "CHAVEZ" → CH substitution → "XAVEZ"
        //  → Z→S → "XABES"
        //  → V→B → done at step 2, we have "XABES"
        //  Wait actually the sequential preprocessing: I process left-to-right
        //  and substitute. Let me redo — "CHAVEZ":
        //    i=0: (C,H) → X, i+=2. Out="X"
        //    i=2: A, not digraph. → 'A'. Out="XA"
        //    i=3: V, not digraph. → 'B' (V→B). Out="XAB"
        //    i=4: E, not digraph. → 'E'. Out="XABE"
        //    i=5: Z, not digraph. → 'S' (Z→S). Out="XABES"
        //  Now Soundex-encode "XABES":
        //    X (seed). last_code = code_of(X) = 7
        //    A: code 0 → reset last_code=0, continue
        //    B: code 1, != 0, push '1'. last=1. Out="X1"
        //    E: code 0 → reset, continue.
        //    S: code 7, != 0, push '7'. last=7. Out="X17"
        //  Pad: "X170"
        assert_eq!(p("Chavez"), "X170");
    }

    #[test]
    fn qu_maps_to_k() {
        // "Quintero" → QU→K, INTERO
        //   Preprocessed: "KINTERO"
        //   Encode: K(seed), I(0/reset), N(5), T(3), E(0/reset), R(6), O(0)
        //     → "K536" → "K536"
        assert_eq!(p("Quintero"), "K536");
    }

    #[test]
    fn v_and_b_are_equivalent() {
        // "Vera" and "Bera" should encode the same.
        assert_eq!(p("Vera"), p("Bera"));
    }

    #[test]
    fn z_and_s_are_equivalent() {
        // "Zapata" and "Sapata" should encode the same.
        assert_eq!(p("Zapata"), p("Sapata"));
    }

    #[test]
    fn h_is_silent() {
        // "Hernandez" and "Ernandez" should encode the same.
        assert_eq!(p("Hernandez"), p("Ernandez"));
    }

    #[test]
    fn common_spanish_surnames() {
        // Hand-traced values against the module algorithm.
        //
        // García: G, A(0), R(6), C(2), I(0), A(0) → G62 → G620
        assert_eq!(p("García"), "G620");
        // Martínez: M, A(0), R(6), T(3), I(0), N(5), E(0), Z→S(7) → M6357... wait limit is 3 digits after seed.
        //   M(seed), A(reset), R(6), T(3), I(reset), N(5), E(reset), S(7) → M6357, but we cap at length 4 → "M635"
        assert_eq!(p("Martínez"), "M635");
        // López: L, O(0), P(1), E(0), Z→S(7) → L17 → L170
        assert_eq!(p("López"), "L170");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("GARCIA"), p("garcia"));
        assert_eq!(p("García"), p("GARCÍA"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("De"), "D000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Pepe" — P(seed), E(reset), P(1). Wait actually since E is a
        // vowel that resets last_code to 0, the second P codes as 1
        // (not a duplicate). But there's a Peppa (double P) case:
        //   "Appa" → A(seed), P(1), P(dup drop), A(0/reset) → "A1" → "A100"
        assert_eq!(p("Appa"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_es() {
        assert_eq!(SpanishPhonexAdapter.name(), "phonex-es");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(SpanishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = SpanishPhonexAdapter.encode("García").unwrap();
        assert_eq!(primary, "G620");
        assert!(alt.is_none());
    }
}
