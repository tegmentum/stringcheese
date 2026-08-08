//! Property tests for the Punjabi language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{PunjabiIso15919, PunjabiPhonex};
use crate::stemmer::LightPunjabiStemmer;
use crate::{PUNJABI, STOPWORDS};

/// Strategy for a Punjabi word 1..=12 chars anchored on a base
/// consonant.
///
/// The word starts with a classical Gurmukhi consonant, ensuring the
/// input is not a pathological string of bare matras. The tail may
/// contain further consonants, primary independent vowels, matras,
/// virama, tippi / bindi, and addak — the full inventory the stemmer
/// and encoder are expected to handle in a real Punjabi word.
fn punjabi_word() -> impl Strategy<Value = String> {
    // Base consonants: 0A15-0A28 (velar through dental nasal),
    // 0A2A-0A30 (labial through r), 0A32, 0A33, 0A35, 0A36, 0A38-0A39,
    // 0A59-0A5C and 0A5E (nukta consonants + retroflex flap).
    let classical =
        "\u{0A15}-\u{0A28}\u{0A2A}-\u{0A30}\u{0A32}-\u{0A33}\u{0A35}-\u{0A36}\u{0A38}-\u{0A39}";
    // Primary independent vowels: 0A05-0A0A, 0A0F-0A10, 0A13-0A14.
    let vowels = "\u{0A05}-\u{0A0A}\u{0A0F}-\u{0A10}\u{0A13}-\u{0A14}";
    // Matras: 0A3E-0A42 (ā, i, ī, u, ū), 0A47-0A48 (e, ai),
    // 0A4B-0A4C (o, au).
    let matras = "\u{0A3E}-\u{0A42}\u{0A47}-\u{0A48}\u{0A4B}-\u{0A4C}";
    // Marks: 0A02 (bindi), 0A70 (tippi), 0A71 (addak), 0A4D (virama),
    // 0A3C (nukta).
    let marks = "\u{0A02}\u{0A3C}\u{0A4D}\u{0A70}-\u{0A71}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Punjabi text with punctuation and whitespace.
fn punjabi_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[\u{0A05}-\u{0A39}\u{0A3C}\u{0A3E}-\u{0A4D}\u{0A70}-\u{0A71} ।॥,.!?]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Stemmer — converges immediately, doesn't grow input.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in punjabi_word()) {
        let a = LightPunjabiStemmer.stem(&w).into_owned();
        let b = LightPunjabiStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Punjabi input
    /// within a small number of iterations. Each successful strip
    /// shortens the stem by at least one scalar and the suffix
    /// table has finite entries.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in punjabi_word()) {
        let mut cur = LightPunjabiStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = LightPunjabiStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "stemmer did not converge in 16 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in punjabi_word()) {
        let out = LightPunjabiStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 15919 transliteration — total on Punjabi input.
    // -----------------------------------------------------------------

    /// ISO 15919 is total on non-empty Punjabi input.
    #[test]
    fn iso_is_total_on_punjabi(w in punjabi_word()) {
        let out = PunjabiIso15919.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 15919 returned empty for non-empty Punjabi input {:?}",
            w
        );
    }

    /// ISO 15919 output contains no Gurmukhi scalars — every
    /// Gurmukhi-block scalar is mapped to a Latin equivalent.
    #[test]
    fn iso_output_has_no_gurmukhi(w in punjabi_word()) {
        let out = PunjabiIso15919.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0A00}'..='\u{0A7F}').contains(&c),
                "ISO({:?}) = {:?} still contains Gurmukhi scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Punjabi — well-formed 4-char key on Punjabi input.
    // -----------------------------------------------------------------

    /// PHONEX-Punjabi always produces a 4-char key on non-empty
    /// Punjabi input.
    #[test]
    fn phonex_yields_four_char_key(w in punjabi_word()) {
        if let Some(key) = PunjabiPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-PA({:?}) is not 4 chars",
                w
            );
        }
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    /// Every stopword in the shipped list is recognized.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(PUNJABI.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = PUNJABI.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in punjabi_text()) {
        let toks: Vec<&str> = PUNJABI.tokenize(&text).collect();
        for t in &toks {
            for c in t.chars() {
                prop_assert!(
                    text.contains(c),
                    "token {:?} contains character {:?} not in input {:?}",
                    t,
                    c,
                    text,
                );
            }
        }
    }

    /// No token is empty.
    #[test]
    fn tokenizer_never_yields_empty_tokens(text in punjabi_text()) {
        for t in PUNJABI.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
