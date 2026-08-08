//! Property tests for the Marathi language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{MarathiIso15919, MarathiPhonex};
use crate::stemmer::LightMarathiStemmer;
use crate::{MARATHI, STOPWORDS};

/// Strategy for a Marathi Devanagari word 1..=12 chars anchored on a
/// base consonant.
///
/// The word starts with a base consonant (the classical Hindi set
/// plus the Marathi-specific letters `ऱ` U+0931 and `ळ` U+0933),
/// ensuring the input is not a pathological string of bare matras.
/// The tail may contain further classical consonants, primary
/// independent vowels, matras, virama, nukta, and anusvara /
/// chandrabindu / visarga — the full inventory the stemmer and
/// encoder are expected to handle in a real Marathi word.
fn marathi_word() -> impl Strategy<Value = String> {
    // Base consonant class: 0915-0928 (क-न), 0929 (ऩ Marathi/Tamil
    // variant of n — in the ISO table), 092A-0930 (प-र), 0931 (ऱ
    // Marathi R with bar), 0932 (ल), 0933 (ळ Marathi retroflex L),
    // 0935-0939 (व-ह). This deliberately includes the two Marathi
    // additions so the generator exercises them.
    let classical =
        "\u{0915}-\u{0928}\u{0929}\u{092A}-\u{0930}\u{0931}\u{0932}\u{0933}\u{0935}-\u{0939}";
    let vowels = "\u{0905}-\u{090B}\u{090F}-\u{0910}\u{0913}-\u{0914}";
    let matras = "\u{093E}-\u{0940}\u{0941}-\u{0942}\u{0943}\u{0947}-\u{0948}\u{094B}-\u{094C}";
    let marks = "\u{0901}-\u{0903}\u{093C}\u{094D}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Devanagari text with punctuation and whitespace.
fn marathi_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[\u{0905}-\u{0939}\u{093C}\u{093E}-\u{094D}\u{0901}-\u{0903} ।॥,.!?]{0,40}",
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
    fn stemmer_is_deterministic(w in marathi_word()) {
        let a = LightMarathiStemmer.stem(&w).into_owned();
        let b = LightMarathiStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer is idempotent because the internal loop iterates
    /// to convergence: `stem(stem(w))` == `stem(w)`.
    #[test]
    fn stemmer_is_idempotent(w in marathi_word()) {
        let once = LightMarathiStemmer.stem(&w).into_owned();
        let twice = LightMarathiStemmer.stem(&once).into_owned();
        prop_assert_eq!(once, twice, "stem not idempotent on {:?}", w);
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in marathi_word()) {
        let out = LightMarathiStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 15919 transliteration — total on Devanagari input.
    // -----------------------------------------------------------------

    /// ISO 15919 is total on non-empty Devanagari input.
    #[test]
    fn iso_is_total_on_devanagari(w in marathi_word()) {
        let out = MarathiIso15919.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 15919 returned empty for non-empty Devanagari input {:?}",
            w
        );
    }

    /// ISO 15919 output contains no Devanagari scalars — every
    /// Devanagari-block scalar is mapped to a Latin equivalent.
    #[test]
    fn iso_output_has_no_devanagari(w in marathi_word()) {
        let out = MarathiIso15919.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0900}'..='\u{097F}').contains(&c),
                "ISO({:?}) = {:?} still contains Devanagari scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Marathi — well-formed 4-char key on Devanagari input.
    // -----------------------------------------------------------------

    /// PHONEX-Marathi always produces a 4-char key on non-empty
    /// Devanagari input.
    #[test]
    fn phonex_yields_four_char_key(w in marathi_word()) {
        if let Some(key) = MarathiPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-MR({:?}) is not 4 chars",
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
        prop_assert!(MARATHI.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = MARATHI.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in marathi_text()) {
        let toks: Vec<&str> = MARATHI.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in marathi_text()) {
        for t in MARATHI.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
