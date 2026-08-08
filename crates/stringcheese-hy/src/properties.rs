//! Property tests for the Armenian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::ArmenianPhonex;
use crate::stemmer::ArmenianStemmer;
use crate::tokenizer::ArmenianTokenizer;
use crate::{ARMENIAN, STOPWORDS};

/// Strategy for an Armenian-flavoured word 1..=15 chars from the
/// lowercase Armenian alphabet (base letters only).
fn armenian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ա-ֆ]{1,15}").expect("static regex is valid")
}

/// Strategy for a mixed-case Armenian word 1..=15 chars.
fn mixed_case_armenian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ա-ֆԱ-Ֆ]{1,15}").expect("static regex is valid")
}

/// Strategy for arbitrary short Armenian-flavoured text.
fn armenian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[ա-ֆԱ-Ֆ ,.!?։՝՞՜-]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in armenian_word()) {
        let a = ArmenianStemmer.stem(&w).into_owned();
        let b = ArmenianStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded
    /// number of iterations.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in armenian_word()) {
        let mut cur = ArmenianStemmer.stem(&w).into_owned();
        for _ in 0..32 {
            let next = ArmenianStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Armenian stemmer did not converge in 32 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix or a length-preserving fold, so
    /// the stem monotonically shrinks (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in armenian_word()) {
        let out = ArmenianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Armenian stemmer({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem never contains uppercase — the case-fold is applied
    /// as part of preprocessing.
    #[test]
    fn stemmer_output_is_lowercase(w in mixed_case_armenian_word()) {
        let out = ArmenianStemmer.stem(&w).into_owned();
        for c in out.chars() {
            prop_assert!(
                !c.is_uppercase(),
                "stem {:?} contains uppercase char {:?}",
                out,
                c
            );
        }
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// the pack's case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(ARMENIAN.is_stopword(w));
    }

    /// The PHONEX encoder is total on Armenian input (non-empty
    /// input always encodes).
    #[test]
    fn phonex_is_total_on_armenian(w in armenian_word()) {
        let out = ArmenianPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "PHONEX returned None for {:?}",
            w
        );
    }

    /// The PHONEX encoder is case-invariant.
    #[test]
    fn phonex_is_case_invariant(w in mixed_case_armenian_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = ArmenianPhonex.encode(&lower);
        let b = ArmenianPhonex.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// PHONEX outputs a 4-character key with a Latin-letter seed and
    /// three ASCII digits.
    #[test]
    fn phonex_output_shape_is_letter_plus_three_digits(w in armenian_word()) {
        if let Some(key) = ArmenianPhonex.encode(&w) {
            prop_assert_eq!(key.chars().count(), 4);
            let mut it = key.chars();
            let seed = it.next().unwrap();
            prop_assert!(
                seed.is_ascii_alphabetic(),
                "seed {:?} of key {:?} is not ASCII alphabetic",
                seed,
                key
            );
            for c in it {
                prop_assert!(
                    c.is_ascii_digit(),
                    "non-digit {:?} in tail of key {:?}",
                    c,
                    key
                );
            }
        }
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = ArmenianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in armenian_text()) {
        let toks: Vec<&str> = ArmenianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in armenian_text()) {
        for t in ArmenianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
