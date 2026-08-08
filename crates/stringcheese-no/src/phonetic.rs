//! A Norwegian-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Norwegian lacks a single canonical published phonetic encoder the
//! way English has Soundex or German has Kölner Phonetik. The closest
//! candidates:
//!
//! * **Language-independent Soundex** applied to ASCII-folded
//!   Norwegian — stable and portable, but linguistically inaccurate
//!   (the `skj` / `sj` clusters, the palatal `kj`, the fronted `k`
//!   before `e`/`i`/`y`/`æ`/`ø`, and the letters `å`/`æ`/`ø` all get
//!   swept under the English mapping).
//! * **PHONEX-family encoders** — the Norwegian equivalent of the
//!   Dutch / French / Spanish / Portuguese PHONEX encoders shipped in
//!   the sibling language packs: apply Norwegian-specific
//!   preprocessing (collapse the `skj` / `sj` / `sk`-before-front-vowel
//!   clusters to the sibilant class, collapse `kj` and `k`-before-
//!   front-vowel to the palatal placeholder, fold the three
//!   Norwegian-specific vowels for phonetic key stability), then run
//!   a Soundex-shape encoder over the classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Norwegian** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with Norwegian-tuned preprocessing and the standard PHONEX
//! classification table:
//!
//! 1. **Uppercase and un-accent.** `Á À Â Ä → A`, `É È Ê Ë → E`, `Í Ì
//!    Î Ï → I`, `Ó Ò Ô Ö → O`, `Ú Ù Û Ü → U`. Norwegian-specific vowel
//!    folds: `Å → O` (open back rounded vowel, phonetically closer to
//!    /o/ or /ɔ/ than to /a/), `Æ → E` (open front vowel, phonetically
//!    closer to /ɛ/ than to /a/), `Ø → E` (rounded mid front vowel;
//!    for encoding purposes it collapses to `E`).
//! 2. **Norwegian digraph and single-letter substitutions.**
//!    Longest-match first:
//!    * `SKJ → S`. The Norwegian cluster `skj` is a single voiceless
//!      palatal-retroflex fricative /ʂ/, uniformly the sibilant class.
//!    * `SK` before a front vowel `E I Y Æ Ø` → `S`. Before back
//!      vowels or consonants, `sk` stays as `s` + `k` (each classified
//!      separately). This captures Norwegian's palatalization rule
//!      where `sk` before front vowels is realized /ʂ/.
//!    * `KJ → C`. The Norwegian palatal `kj` is a voiceless palatal
//!      fricative /ç/; folding to `C` keeps it in the velar/palatal
//!      class alongside `K`.
//!    * `K` before a front vowel `E I Y Æ Ø` → `C`. Norwegian `k`
//!      before front vowels is realized as the same /ç/ palatal
//!      fricative as `kj`; the fold gives them a shared phonetic key.
//!    * `CH → S`. Norwegian doesn't natively use `ch`; loanwords
//!      (`chef`, `sjekk-borrowed spellings`) pronounce it as /ʃ/.
//!      Folding to `S` places it in the sibilant class.
//!    * `H → ` (silent word-initially — dropped). Note: applied
//!      *after* the digraph substitutions so `sh`/`kh`/`ch` clusters
//!      are handled by their digraph rules first.
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
//!
//! **Grouping rationale.** Norwegian `B/P/F/V/W` are labial obstruents
//! and glides (surface `V` is /ʋ/, a labiodental approximant); `C/K/G/
//! Q/J/X` are the velar / palatal family with `C` used as an internal
//! placeholder for the palatal fricative that `KJ` and front-vowel `K`
//! code as; `D/T` are dental stops; `S/Z` are the sibilant family; `L`
//! and `R` get their own classes; `M/N` are nasals. The Norwegian `J`
//! is `/j/` (palatal glide) — placed in the velar/palatal class 2
//! alongside `G` and `K` so `Jansen` and `Gansen` fall into related
//! classes.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Norwegian** — a parallel variable-length encoder with
//!   better discrimination; heavier to reference-test.
//! * **Region-tuned variants** — bokmål / nynorsk / dialect-specific
//!   PHONEX tables (e.g., western Norwegian /r/-drops, trønder tonal
//!   distinctions) would need substantial rule sets out of scope for
//!   a starter pack.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Norwegian PHONEX encoder.
///
/// A zero-sized value; construct as [`NorwegianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_no::NorwegianPhonex;
///
/// // The silent-H preprocessor drops the initial H, so `Hansen`
/// // encodes from the residue `Ansen`: A-seed, N(5), S(7), N(5).
/// let key = NorwegianPhonex.encode("Hansen").unwrap();
/// assert_eq!(key, "A575");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NorwegianPhonex;

impl NorwegianPhonex {
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

/// True if `b` is a Norwegian front vowel (`E I Y Æ Ø`). The
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

/// Adapter that exposes [`NorwegianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Norwegian::phonetic_encoder`](crate::Norwegian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NorwegianPhonexAdapter;

impl LanguagePhoneticEncoder for NorwegianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        NorwegianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        NorwegianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(NorwegianPhonex.encode("").is_none());
        assert!(NorwegianPhonex.encode("   ").is_none());
        assert!(NorwegianPhonex.encode("---").is_none());
    }

    #[test]
    fn norwegian_vowels_fold_to_base_letters() {
        // å → O, æ → E, ø → E.
        assert_eq!(p("så"), p("so"));
        assert_eq!(p("være"), p("vere"));
        assert_eq!(p("øye"), p("eye"));
    }

    #[test]
    fn diaeresis_and_acute_fold_to_base_vowel() {
        assert_eq!(p("café"), p("cafe"));
    }

    #[test]
    fn skj_cluster_encodes_as_sibilant() {
        // `skjære` (magpie) → preprocess SKJ → S, then Æ → E, R, E.
        //   "SERE": S seed last=7. E reset. R code=6, push. E reset.
        //   → "S6" pad → "S600".
        assert_eq!(p("skjære"), "S600");
    }

    #[test]
    fn sk_before_front_vowel_encodes_as_sibilant() {
        // `ski` → preprocess SK before I → S. Then I. "SI".
        //   S seed last=7. I reset. → "S" pad → "S000".
        assert_eq!(p("ski"), "S000");
        // `skøyte` (skate) → preprocess SK before Ø (front vowel!) →
        //   S. Then Ø → E, Y (front-vowel? Y matches front-vowel-list
        //   here too), T, E. "SEYTE".
        //   S seed last=7. E reset. Y is a vowel (in code_of Y → 0).
        //   Reset. T code=3 push. E reset. → "S3" pad → "S300".
        assert_eq!(p("skøyte"), "S300");
    }

    #[test]
    fn sk_before_back_vowel_stays_split() {
        // `skål` (cheers) → SK NOT before front vowel (Å → O is back).
        //   Stays as S + K. Å → O.
        //   Result: "SKOL". S seed last=7. K code=2 push. O reset.
        //   L code=4 push. → "S24" pad → "S240".
        assert_eq!(p("skål"), "S240");
    }

    #[test]
    fn kj_encodes_as_palatal_c() {
        // `kjøre` (drive) → KJ → C, then Ø → E, R, E. "CERE".
        //   C seed last=2. E reset. R code=6 push. E reset. → "C6" pad
        //   → "C600".
        assert_eq!(p("kjøre"), "C600");
    }

    #[test]
    fn k_before_front_vowel_encodes_as_palatal_c() {
        // `kino` (cinema) → K before I → C. Then I, N, O. "CINO".
        //   C seed last=2. I reset. N code=5 push. O reset. → "C5" pad
        //   → "C500".
        assert_eq!(p("kino"), "C500");
    }

    #[test]
    fn k_before_back_vowel_stays_velar() {
        // `kake` (cake) → K before A (back vowel) → stays as K. Then
        //   A, K before E (front vowel! → C). E. "KAKE" preprocessed
        //   as "K" then "A" then "K→C" then "E". So the ASCII stream
        //   becomes K-A-C-E → "KACE".
        //   K seed last=2. A reset last=0. C code=2 — the vowel A
        //   between reset last_code, so C is NOT a dup → push '2'.
        //   E reset. → "K2" pad → "K200".
        //
        //   The velar (K) and palatal (C) share the same PHONEX class
        //   digit 2 — the fold to `C` for front-vowel `K` is a
        //   preprocessing courtesy for readability of the internal
        //   ASCII stream; it doesn't change the coded digit.
        assert_eq!(p("kake"), "K200");
    }

    #[test]
    fn ch_encodes_as_sibilant() {
        // `chef` (loan) → CH → S. Then E, F. "SEF".
        //   S seed last=7. E reset. F code=1 push. → "S1" pad → "S100".
        assert_eq!(p("chef"), "S100");
    }

    #[test]
    fn silent_h_is_stripped() {
        // `Hansen` → H drops → ANSEN.
        //   A seed last=0. N code=5 push. S code=7 push. E reset.
        //   N code=5 push. → "A575" cap → "A575". Wait — the seed
        //   char is 'A' after H stripped. So we get "A575".
        //   But the doc example says H525. Let me reconsider: the
        //   silent-H strip removes the H at position 0, so the new
        //   first letter is A. Actually re-check the doc's example:
        //   I wrote H525 which is wrong. Let me update the doc.
        // Just test the actual behavior here.
        assert_eq!(p("Hansen"), p("Ansen"));
    }

    #[test]
    fn hansen_encodes() {
        // Hansen: H drops → ANSEN. A seed. N(5). S(7). E reset. N(5).
        //   → "A575".
        assert_eq!(p("Hansen"), "A575");
    }

    #[test]
    fn common_norwegian_surnames() {
        // Olsen: O seed last=0. L(4) push. S(7) push. E reset. N(5)
        //   push. → "O475".
        assert_eq!(p("Olsen"), "O475");
        // Berg: B seed last=1. E reset. R(6) push. G(2) push. → "B62"
        //   pad → "B620".
        assert_eq!(p("Berg"), "B620");
        // Larsen: L seed last=4. A reset. R(6) push. S(7) push. E
        //   reset. N(5) push. → "L675".
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
        // "Abba" — A(seed), B(1,push,last=1), B(dup drop), A(reset)
        //   → "A1" → "A100"
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_no() {
        assert_eq!(NorwegianPhonexAdapter.name(), "phonex-no");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(NorwegianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = NorwegianPhonexAdapter.encode("Hansen").unwrap();
        assert_eq!(primary, "A575");
        assert!(alt.is_none());
    }
}
