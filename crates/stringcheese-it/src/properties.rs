//! Property tests for the Italian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern
//! as every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::tokenizer::ItalianTokenizer;
use crate::{ITALIAN, STOPWORDS};

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for an Italian-flavoured word (ASCII plus common
/// grave / acute accented vowels).
fn italian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Zàèéìòù]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Italian-flavoured text — letters,
/// accents, spaces, punctuation.
fn italian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Zàèéìòù ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The identity stemmer returns the input verbatim — the MVP
    /// pack ships no inflectional collapse. Documented in the
    /// crate-level docs; this property proves the contract.
    #[test]
    fn stemmer_is_identity(w in italian_word()) {
        let out = ITALIAN.stem(&w).into_owned();
        prop_assert_eq!(out, w);
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped
    /// stopword list (the default trait implementation uses
    /// `str::eq_ignore_ascii_case`).
    #[test]
    fn is_stopword_case_invariant_ascii(w in mixed_case_word()) {
        let hit_lower = ITALIAN.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = ITALIAN.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a
    /// stopword, including under uppercased ASCII input. The MVP
    /// list is ASCII-only, so uppercased forms always resolve.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(ITALIAN.is_stopword(w));
        prop_assert!(ITALIAN.is_stopword(&w.to_ascii_uppercase()));
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = ItalianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in italian_text()) {
        let toks: Vec<&str> = ItalianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in italian_text()) {
        for t in ItalianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
