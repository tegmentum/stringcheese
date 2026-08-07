//! Property tests for the Slovak language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::SlovakPhonex;
use crate::stemmer::SlovakStemmer;
use crate::tokenizer::SlovakTokenizer;
use crate::{SLOVAK, STOPWORDS};

/// Strategy for a Slovak-flavoured word 1..=15 chars from the
/// lowercase Slovak alphabet (ASCII plus the haček + long-vowel +
/// Slovak-specific extended set).
fn slovak_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-záäčďéíĺľňóôŕšťúýž]{1,15}").expect("static regex is valid")
}

/// Strategy for a mixed-case Slovak word 1..=15 chars.
fn mixed_case_slovak_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZáäčďéíĺľňóôŕšťúýžÁÄČĎÉÍĹĽŇÓÔŔŠŤÚÝŽ]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for arbitrary short Slovak-flavoured text.
fn slovak_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-ZáäčďéíĺľňóôŕšťúýžÁÄČĎÉÍĹĽŇÓÔŔŠŤÚÝŽ ,.!?-]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in slovak_word()) {
        let a = SlovakStemmer.stem(&w).into_owned();
        let b = SlovakStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in slovak_word()) {
        let mut cur = SlovakStemmer.stem(&w).into_owned();
        for _ in 0..32 {
            let next = SlovakStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "stemmer did not converge in 32 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically
    /// shrinks (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in slovak_word()) {
        let out = SlovakStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "SlovakStemmer({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Unicode case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(SLOVAK.is_stopword(w));
    }

    /// `is_stopword` is case-invariant under Unicode fold.
    #[test]
    fn is_stopword_case_invariant(w in mixed_case_slovak_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let hit_lower = SLOVAK.is_stopword(&lower);
        let hit_upper = SLOVAK.is_stopword(&upper);
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// The PHONEX-Slovak encoder is total on Slovak-alphabetic input
    /// that contains at least one non-`H` letter (all-H strings fold
    /// to empty after silent-H stripping and legitimately return
    /// None).
    #[test]
    fn phonex_is_total_on_non_h_slovak_input(w in slovak_word()) {
        prop_assume!(w.chars().any(|c| !matches!(c, 'h' | 'H')));
        let out = SlovakPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "SlovakPhonex returned None for {:?}",
            w
        );
    }

    /// The PHONEX-Slovak key is always exactly 4 characters when the
    /// encoder returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in slovak_word()) {
        if let Some(k) = SlovakPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// PHONEX-Slovak is case-invariant.
    #[test]
    fn phonex_is_case_invariant(w in mixed_case_slovak_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = SlovakPhonex.encode(&lower);
        let b = SlovakPhonex.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// PHONEX-Slovak output is ASCII-only.
    #[test]
    fn phonex_output_is_ascii(w in slovak_word()) {
        if let Some(k) = SlovakPhonex.encode(&w) {
            prop_assert!(
                k.is_ascii(),
                "PHONEX-Slovak produced non-ASCII output {:?} for {:?}",
                k, w
            );
        }
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = SlovakTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in slovak_text()) {
        let toks: Vec<&str> = SlovakTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in slovak_text()) {
        for t in SlovakTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
