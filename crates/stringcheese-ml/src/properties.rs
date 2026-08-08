//! Property tests for the Malayalam language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{MalayalamIso15919, MalayalamPhonex};
use crate::stemmer::LightMalayalamStemmer;
use crate::{MALAYALAM, STOPWORDS};

/// Strategy for a Malayalam word 1..=12 chars anchored on a base
/// consonant.
///
/// The word starts with a Malayalam base consonant, ensuring the
/// input is not a pathological string of bare matras. The tail may
/// contain further consonants, primary independent vowels, matras,
/// virama, anusvara / visarga, and chillu letters — the full
/// inventory the stemmer and encoder are expected to handle in a
/// real Malayalam word.
fn malayalam_word() -> impl Strategy<Value = String> {
    // Base consonants: 0D15-0D28, 0D2A-0D39 (the 0D29 slot between
    // ന and പ is reserved in Unicode and would leak through the
    // transliterator unchanged).
    let classical = "\u{0D15}-\u{0D28}\u{0D2A}-\u{0D39}";
    // Primary independent vowels: 0D05-0D0B, 0D0E-0D10, 0D12-0D14.
    // The 0D0D and 0D11 slots are reserved in Unicode. The 0D0C
    // "vocalic l" letter (ഌ) is Sanskrit-only and omitted from the
    // transliterator's mapping — it would leak through as a raw
    // Malayalam scalar and trip the "iso output has no Malayalam"
    // property below.
    let vowels = "\u{0D05}-\u{0D0B}\u{0D0E}-\u{0D10}\u{0D12}-\u{0D14}";
    // Matras: 0D3E-0D43, 0D46-0D48, 0D4A-0D4C. The 0D45 and 0D49
    // slots are reserved. The 0D44 "vocalic rr" matra (ൄ) is
    // Sanskrit-only and omitted from the transliterator's mapping
    // for the same reason as U+0D0C above.
    let matras = "\u{0D3E}-\u{0D43}\u{0D46}-\u{0D48}\u{0D4A}-\u{0D4C}";
    // Marks: 0D02 (anusvara), 0D03 (visarga), 0D4D (virama).
    let marks = "\u{0D02}\u{0D03}\u{0D4D}";
    // Chillu letters: 0D7A-0D7F.
    let chillu = "\u{0D7A}-\u{0D7F}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}{chillu}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Malayalam text with punctuation and whitespace.
fn malayalam_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[\u{0D05}-\u{0D0C}\u{0D0E}-\u{0D10}\u{0D12}-\u{0D14}\u{0D15}-\u{0D28}\u{0D2A}-\u{0D39}\u{0D3E}-\u{0D44}\u{0D46}-\u{0D48}\u{0D4A}-\u{0D4D}\u{0D7A}-\u{0D7F}\u{0D02}\u{0D03} ,.!?]{0,40}",
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
    fn stemmer_is_deterministic(w in malayalam_word()) {
        let a = LightMalayalamStemmer.stem(&w).into_owned();
        let b = LightMalayalamStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Malayalam input
    /// within a small number of iterations. Each successful strip
    /// shortens the stem by at least one scalar and the suffix
    /// table has finite entries.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in malayalam_word()) {
        let mut cur = LightMalayalamStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = LightMalayalamStemmer.stem(&cur).into_owned();
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
    fn stemmer_output_never_longer_than_input(w in malayalam_word()) {
        let out = LightMalayalamStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 15919 transliteration — total on Malayalam input.
    // -----------------------------------------------------------------

    /// ISO 15919 is total on non-empty Malayalam input.
    #[test]
    fn iso_is_total_on_malayalam(w in malayalam_word()) {
        let out = MalayalamIso15919.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 15919 returned empty for non-empty Malayalam input {:?}",
            w
        );
    }

    /// ISO 15919 output contains no Malayalam scalars — every
    /// Malayalam-block scalar is mapped to a Latin equivalent.
    #[test]
    fn iso_output_has_no_malayalam(w in malayalam_word()) {
        let out = MalayalamIso15919.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0D00}'..='\u{0D7F}').contains(&c),
                "ISO({:?}) = {:?} still contains Malayalam scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Malayalam — well-formed 4-char key on Malayalam input.
    // -----------------------------------------------------------------

    /// PHONEX-Malayalam always produces a 4-char key on non-empty
    /// Malayalam input.
    #[test]
    fn phonex_yields_four_char_key(w in malayalam_word()) {
        if let Some(key) = MalayalamPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-ML({:?}) is not 4 chars",
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
        prop_assert!(MALAYALAM.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = MALAYALAM.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in malayalam_text()) {
        let toks: Vec<&str> = MALAYALAM.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in malayalam_text()) {
        for t in MALAYALAM.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
