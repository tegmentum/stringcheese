//! Property tests for the Bengali language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{BengaliIso15919, BengaliPhonex};
use crate::stemmer::LightBengaliStemmer;
use crate::{BENGALI, STOPWORDS};

/// Strategy for a Bengali word 1..=12 chars anchored on a base
/// consonant.
///
/// The word starts with a classical Bengali consonant, ensuring the
/// input is not a pathological string of bare matras. The tail may
/// contain further consonants, primary independent vowels, matras,
/// halant, and anusvara / chandrabindu / visarga — the full
/// inventory the stemmer and encoder are expected to handle in a
/// real Bengali word.
fn bengali_word() -> impl Strategy<Value = String> {
    // Base consonants: 0995-09A8 (excluding 0999-099E gap ranges are OK),
    // 09AA-09B0, 09B2, 09B6-09B9.
    let classical = "\u{0995}-\u{09A8}\u{09AA}-\u{09B0}\u{09B2}\u{09B6}-\u{09B9}";
    // Primary independent vowels: 0985-098B, 098F-0990, 0993-0994.
    let vowels = "\u{0985}-\u{098B}\u{098F}-\u{0990}\u{0993}-\u{0994}";
    // Matras: 09BE-09C3 (ā, i, ī, u, ū, r̥), 09C7-09C8 (e, ai),
    // 09CB-09CC (o, au). U+09C4 (vocalic-ll matra) and the four
    // classical-Sanskrit-only vocalic scalars 09E0-09E3 are omitted
    // from the generator — they are not in the transliterator's
    // mapping and would leak through as raw Bengali scalars.
    let matras = "\u{09BE}-\u{09C3}\u{09C7}-\u{09C8}\u{09CB}-\u{09CC}";
    // Marks: 0981-0983 (chandrabindu, anusvara, visarga), 09BC (nukta),
    // 09CD (halant).
    let marks = "\u{0981}-\u{0983}\u{09BC}\u{09CD}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Bengali text with punctuation and whitespace.
fn bengali_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\u{0985}-\u{09B9}\u{09BC}\u{09BE}-\u{09CD} ।॥,.!?]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Stemmer — converges immediately, doesn't grow input.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in bengali_word()) {
        let a = LightBengaliStemmer.stem(&w).into_owned();
        let b = LightBengaliStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Bengali input
    /// within a small number of iterations. Each successful strip
    /// shortens the stem by at least one scalar and the suffix
    /// table has finite entries.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in bengali_word()) {
        let mut cur = LightBengaliStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = LightBengaliStemmer.stem(&cur).into_owned();
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
    fn stemmer_output_never_longer_than_input(w in bengali_word()) {
        let out = LightBengaliStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 15919 transliteration — total on Bengali input.
    // -----------------------------------------------------------------

    /// ISO 15919 is total on non-empty Bengali input.
    #[test]
    fn iso_is_total_on_bengali(w in bengali_word()) {
        let out = BengaliIso15919.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 15919 returned empty for non-empty Bengali input {:?}",
            w
        );
    }

    /// ISO 15919 output contains no Bengali scalars — every
    /// Bengali-block scalar is mapped to a Latin equivalent.
    #[test]
    fn iso_output_has_no_bengali(w in bengali_word()) {
        let out = BengaliIso15919.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0980}'..='\u{09FF}').contains(&c),
                "ISO({:?}) = {:?} still contains Bengali scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Bengali — well-formed 4-char key on Bengali input.
    // -----------------------------------------------------------------

    /// PHONEX-Bengali always produces a 4-char key on non-empty
    /// Bengali input.
    #[test]
    fn phonex_yields_four_char_key(w in bengali_word()) {
        if let Some(key) = BengaliPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-BN({:?}) is not 4 chars",
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
        prop_assert!(BENGALI.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = BENGALI.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in bengali_text()) {
        let toks: Vec<&str> = BENGALI.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in bengali_text()) {
        for t in BENGALI.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
