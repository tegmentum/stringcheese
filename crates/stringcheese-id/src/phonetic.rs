//! An Indonesian-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Indonesian has no widely-adopted canonical phonetic encoder the
//! way English has Soundex or German has Kölner Phonetik. Two
//! considerations make Indonesian a light lift:
//!
//! 1. **Highly phonetic orthography.** The modern Ejaan Yang
//!    Disempurnakan (EYD, "Perfected Spelling System") is close to a
//!    1:1 mapping between grapheme and phoneme. Consonant clusters
//!    are rare; letter-to-sound rules have few exceptions.
//! 2. **A short list of native digraphs.** Only four digraphs
//!    contribute distinct phonemes: `ny` /ɲ/, `ng` /ŋ/, `sy` /ʃ/,
//!    `kh` /x/. Everything else is a single grapheme.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Indonesian** encoder: a 4-character
//! Soundex-shape key preceded by an Indonesian-tuned preprocessing
//! pass. Concretely:
//!
//! 1. **Digraph rewrites** (applied left-to-right, longest-match
//!    first):
//!    * `ny → N` — palatal nasal /ɲ/ collapses to the nasal slot.
//!    * `ng → G` — velar nasal /ŋ/ folds to `G` (which encodes as
//!      class 2 alongside `k`/`c`/`q`/`x`/`j`; distinguishes it from
//!      the plain `n` that would code as class 5). Using `G` rather
//!      than a bespoke placeholder keeps the encoder's alphabet
//!      inside ASCII uppercase.
//!    * `sy → S` — /ʃ/ collapses to the sibilant `S`.
//!    * `kh → K` — /x/ folds to `K` (velar).
//! 2. **Uppercase ASCII fold** — Indonesian's alphabet is exactly
//!    the 26 English letters, so `char::to_ascii_uppercase` is
//!    complete. Non-letters are dropped.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below;
//!    drop the zero class (vowels); collapse consecutive equal
//!    codes; truncate to three digits and left-pad with `'0'` to
//!    reach length four.
//!
//! **Classification table.**
//!
//! | Code | Letters             |
//! |------|---------------------|
//! | 1    | B P F V W           |
//! | 2    | C G K Q X J         |
//! | 3    | D T                 |
//! | 4    | L                   |
//! | 5    | M N                 |
//! | 6    | R                   |
//! | 7    | S Z                 |
//! | 0    | A E I O U Y H (vowels/glide + silent-H) |
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone-shaped variable-length encoder.** A future refinement
//!   could produce a stem-like variable-length key; better precision
//!   for record linkage but heavier to reference-test.
//! * **Loanword phonology.** Indonesian carries many Dutch, Arabic,
//!   Sanskrit, and English loanwords; the shipped encoder treats them
//!   as native and does not special-case (e.g.) `q` in Arabic-origin
//!   `qur'an` differently from `k` in native `kaki`.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Indonesian PHONEX encoder.
///
/// A zero-sized value; construct as [`IndonesianPhonex`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_id::IndonesianPhonex;
///
/// // "menyapu" — starts with the `ny` digraph after `me` fold
/// // wait — the encoder does NOT strip prefixes, it just rewrites
/// // digraphs and encodes. So `menyapu` → digraph pass:
/// // `m`+`e`+`ny(→N)`+`a`+`p`+`u` → "MENAPU".
/// let key = IndonesianPhonex.encode("menyapu").unwrap();
/// assert_eq!(key, "M510");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndonesianPhonex;

impl IndonesianPhonex {
    /// Encodes `word` per the PHONEX-Indonesian algorithm.
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

/// Preprocess `word` into uppercase-ASCII letters after Indonesian
/// digraph and silent-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: uppercase-ASCII fold — drop anything that isn't an
    // ASCII letter. Indonesian orthography is ASCII-only.
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if c.is_ascii_alphabetic() {
            ascii.push(c.to_ascii_uppercase());
        }
    }

    // Step 2: digraph substitutions on the ASCII uppercase buffer.
    // Silent `H` is dropped after any `KH` digraph fires so bare `H`
    // (as in loanword `hotel`) doesn't contribute.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph substitutions (longest match).
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'N', b'Y') => {
                    // /ɲ/ — collapse to the nasal slot.
                    out.push('N');
                    i += 2;
                    continue;
                }
                (b'N', b'G') => {
                    // /ŋ/ — fold to G (class 2), distinguishes from
                    // plain N (class 5).
                    out.push('G');
                    i += 2;
                    continue;
                }
                (b'S', b'Y') => {
                    // /ʃ/ — sibilant.
                    out.push('S');
                    i += 2;
                    continue;
                }
                (b'K', b'H') => {
                    // /x/ — velar fricative folds to K (class 2).
                    out.push('K');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* drop as silent — after KH already fired */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'G' | b'K' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        // A E I O U Y — dropped (vowels + glide).
        _ => b'0',
    }
}

/// Adapter that exposes [`IndonesianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Indonesian::phonetic_encoder`](crate::Indonesian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndonesianPhonexAdapter;

impl LanguagePhoneticEncoder for IndonesianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        IndonesianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-id"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        IndonesianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(IndonesianPhonex.encode("").is_none());
        assert!(IndonesianPhonex.encode("   ").is_none());
        assert!(IndonesianPhonex.encode("---").is_none());
    }

    #[test]
    fn ascii_case_insensitive() {
        assert_eq!(p("Jakarta"), p("jakarta"));
        assert_eq!(p("BALI"), p("bali"));
    }

    #[test]
    fn short_input_pads_to_four() {
        // "a" → A(seed, last=0) → "A" → "A000".
        assert_eq!(p("a"), "A000");
        // "ba" → B(seed last=1). A reset last=0 → "B" → "B000".
        assert_eq!(p("ba"), "B000");
    }

    #[test]
    fn ny_digraph_collapses_to_nasal() {
        // "nyanyi" → NYANYI → NANI (both `ny`s fold to `N`).
        //   N seed last=5. A reset. N code=5 (dup with last=0 — no,
        //   after A reset last=0, so 5 != 0, push). Wait: after seed,
        //   last=5. Then A: reset, last=0. Then N: code=5 != 0, push.
        //   out="N5" last=5. I reset. → "N5" pad → "N500".
        assert_eq!(p("nyanyi"), "N500");
    }

    #[test]
    fn ng_digraph_folds_to_g_class_2() {
        // "bunga" → BUNGA → BUGA (ng→G).
        //   B seed last=1. U reset. G code=2 push → "B2" last=2.
        //   A reset last=0. → "B2" pad → "B200".
        assert_eq!(p("bunga"), "B200");
        // Contrast with plain `n`: "bunda" → BUNDA.
        //   B seed last=1. U reset. N code=5 push → "B5" last=5.
        //   D code=3 push → "B53" last=3. A reset. → "B530".
        assert_eq!(p("bunda"), "B530");
        // So `bunga` and `bunda` produce different keys — the `ng`
        // digraph's fold-to-G rather than fold-to-N is what preserves
        // the distinction.
        assert_ne!(p("bunga"), p("bunda"));
    }

    #[test]
    fn sy_digraph_folds_to_sibilant() {
        // "syarat" (condition) → SYARAT → SARAT (sy→S).
        //   S seed last=7. A reset. R code=6 push → "S6". A reset.
        //   T code=3 push → "S63". → "S630".
        assert_eq!(p("syarat"), "S630");
    }

    #[test]
    fn kh_digraph_folds_to_k() {
        // "khusus" (special) → KHUSUS → KUSUS (kh→K).
        //   K seed last=2. U reset. S code=7 push → "K7". U reset.
        //   S code=7, but after U reset last=0 so 7 != 0, push
        //   → "K77" wait actually that duplicates. Let me retrace.
        //   After K seed, last=2. U: reset last=0. S: code=7, 7 != 0,
        //   push. out="K7", last=7. U: reset last=0. S: code=7, 7 !=
        //   0 (last is 0), push. out="K77", last=7. Hmm — two S's on
        //   either side of a vowel produce two 7 codes because the
        //   vowel-reset clears the dup-collapse tracker between them.
        //   → "K77" pad → "K770".
        assert_eq!(p("khusus"), "K770");
    }

    #[test]
    fn silent_h_drops() {
        // "hotel" → HOTEL → OTEL (H dropped).
        //   O seed last=0. T code=3 push → "O3" last=3. E reset. L
        //   code=4 push → "O34". → "O340".
        assert_eq!(p("hotel"), "O340");
    }

    #[test]
    fn kh_h_is_not_dropped_before_processing() {
        // The KH digraph fires first (i+=2), so the H inside is
        // consumed by the rewrite — not dropped as silent-H.
        // "akhir" (end) → AKHIR → AKIR (kh→K).
        //   A seed last=0. K code=2 push → "A2" last=2. I reset. R
        //   code=6 push → "A26". → "A260".
        assert_eq!(p("akhir"), "A260");
    }

    #[test]
    fn common_indonesian_words() {
        // "makan" (eat) → MAKAN.
        //   M seed last=5. A reset. K code=2 push → "M2" last=2.
        //   A reset. N code=5 push → "M25" last=5. → "M250".
        assert_eq!(p("makan"), "M250");
        // "buku" (book) → BUKU.
        //   B seed last=1. U reset. K code=2 push → "B2" last=2.
        //   U reset. → "B2" pad → "B200".
        assert_eq!(p("buku"), "B200");
        // "rumah" (house) → RUMAH → RUMA (H dropped).
        //   R seed last=6. U reset. M code=5 push → "R5" last=5.
        //   A reset. → "R5" pad → "R500".
        assert_eq!(p("rumah"), "R500");
        // "jalan" (road) → JALAN.
        //   J seed last=2. A reset. L code=4 push → "J4" last=4.
        //   A reset. N code=5 push → "J45" last=5. → "J450".
        assert_eq!(p("jalan"), "J450");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "appa" → A(seed), P(1,push), P(dup drop), A(reset)
        //   → "AP" → "AP" pad → wait, out is 'A','1' → "A100"?
        //   Trace: A seed last=0. P code=1, 1 != 0, push out="A1"
        //   last=1. P code=1, 1 == last=1, skip. A reset last=0.
        //   → "A1" → "A100".
        assert_eq!(p("appa"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_id() {
        assert_eq!(IndonesianPhonexAdapter.name(), "phonex-id");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(IndonesianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = IndonesianPhonexAdapter.encode("Jakarta").unwrap();
        // "Jakarta" → JAKARTA.
        //   J seed last=2. A reset. K code=2 push (2 != 0), out="J2"
        //   last=2. A reset. R code=6 push out="J26" last=6. T code=3
        //   push out="J263" last=3. len==4 break. → "J263".
        assert_eq!(primary, "J263");
        assert!(alt.is_none());
    }
}
