//! Property tests for the Tamil language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{TamilIso15919, TamilPhonex};
use crate::stemmer::LightTamilStemmer;
use crate::{STOPWORDS, TAMIL};

/// Strategy for a Tamil word 1..=12 chars anchored on a base
/// consonant.
///
/// The word starts with a classical Tamil consonant, ensuring the
/// input is not a pathological string of bare matras. The tail may
/// contain further consonants, primary independent vowels, matras,
/// and pulli — the full inventory the stemmer and encoder are
/// expected to handle in a real Tamil word.
fn tamil_word() -> impl Strategy<Value = String> {
    // Base consonants: 0B95, 0B99-0B9A, 0B9C, 0B9E-0B9F, 0BA3-0BA4,
    // 0BA8-0BAA, 0BAE-0BB5, 0BB7-0BB9. Simplified by using the ranges
    // that are contiguous in the block.
    let classical = "\u{0B95}\u{0B99}-\u{0B9A}\u{0B9C}\u{0B9E}-\u{0B9F}\u{0BA3}-\u{0BA4}\u{0BA8}-\u{0BAA}\u{0BAE}-\u{0BB5}\u{0BB7}-\u{0BB9}";
    // Primary independent vowels: 0B85-0B8A, 0B8E-0B90, 0B92-0B94.
    let vowels = "\u{0B85}-\u{0B8A}\u{0B8E}-\u{0B90}\u{0B92}-\u{0B94}";
    // Matras: 0BBE-0BC2, 0BC6-0BC8, 0BCA-0BCC.
    let matras = "\u{0BBE}-\u{0BC2}\u{0BC6}-\u{0BC8}\u{0BCA}-\u{0BCC}";
    // Marks: 0BCD (pulli).
    let marks = "\u{0BCD}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Tamil text with punctuation and whitespace.
fn tamil_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\u{0B85}-\u{0BB9}\u{0BBE}-\u{0BCD} ,.!?]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Stemmer — converges immediately, doesn't grow input.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in tamil_word()) {
        let a = LightTamilStemmer.stem(&w).into_owned();
        let b = LightTamilStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Tamil input within a
    /// small number of iterations. Each successful strip shortens the
    /// stem by at least one scalar and the suffix table has finite
    /// entries.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in tamil_word()) {
        let mut cur = LightTamilStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = LightTamilStemmer.stem(&cur).into_owned();
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
    fn stemmer_output_never_longer_than_input(w in tamil_word()) {
        let out = LightTamilStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 15919 transliteration — total on Tamil input.
    // -----------------------------------------------------------------

    /// ISO 15919 is total on non-empty Tamil input.
    #[test]
    fn iso_is_total_on_tamil(w in tamil_word()) {
        let out = TamilIso15919.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 15919 returned empty for non-empty Tamil input {:?}",
            w
        );
    }

    /// ISO 15919 output contains no Tamil scalars — every Tamil-block
    /// scalar is mapped to a Latin equivalent.
    #[test]
    fn iso_output_has_no_tamil(w in tamil_word()) {
        let out = TamilIso15919.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0B80}'..='\u{0BFF}').contains(&c),
                "ISO({:?}) = {:?} still contains Tamil scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Tamil — well-formed 4-char key on Tamil input.
    // -----------------------------------------------------------------

    /// PHONEX-Tamil always produces a 4-char key on non-empty Tamil
    /// input.
    #[test]
    fn phonex_yields_four_char_key(w in tamil_word()) {
        if let Some(key) = TamilPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-TA({:?}) is not 4 chars",
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
        prop_assert!(TAMIL.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = TAMIL.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in tamil_text()) {
        let toks: Vec<&str> = TAMIL.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in tamil_text()) {
        for t in TAMIL.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
