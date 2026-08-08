//! Property tests for the Georgian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::GeorgianPhonex;
use crate::stemmer::GeorgianStemmer;
use crate::tokenizer::GeorgianTokenizer;
use crate::{GEORGIAN, STOPWORDS};

/// Strategy for a Mkhedruli-only word 1..=15 chars.
fn georgian_word() -> impl Strategy<Value = String> {
    // The modern 33-letter Mkhedruli range: U+10D0..=U+10F0. Excluding
    // the archaic and extension slots keeps the generated corpus clean
    // and predictable.
    prop::string::string_regex("[ა-ჰ]{1,15}").expect("static regex is valid")
}

/// Strategy for arbitrary short Georgian-flavoured text.
fn georgian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[ა-ჰ ,.!?-]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in georgian_word()) {
        let a = GeorgianStemmer.stem(&w).into_owned();
        let b = GeorgianStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Georgian input within
    /// a bounded number of iterations. Because bare 1-char suffixes
    /// (`-ი`, `-ს`) can chain-strip stem tails that happen to end in
    /// those letters, allow up to 32 iterations to reach the fixed
    /// point — the same bound the sibling packs use.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in georgian_word()) {
        let mut cur = GeorgianStemmer.stem(&w).into_owned();
        for _ in 0..32 {
            let next = GeorgianStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Georgian stemmer did not converge in 32 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically shrinks
    /// (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in georgian_word()) {
        let out = GeorgianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Georgian stemmer({:?}) = {:?} grew ({}->{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` recognizes every entry in the shipped list.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(GEORGIAN.is_stopword(w));
    }

    /// The PHONEX-Georgian encoder produces a 4-char key on Georgian
    /// input.
    #[test]
    fn phonex_key_is_four_chars_on_georgian(w in georgian_word()) {
        let out = GeorgianPhonex.encode(&w);
        prop_assert!(out.is_some(), "phonex returned None for {:?}", w);
        let key = out.unwrap();
        prop_assert_eq!(key.chars().count(), 4, "phonex key {:?} is not 4 chars", key);
    }

    /// PHONEX-Georgian output is ASCII (uppercase letter + digits).
    #[test]
    fn phonex_output_is_ascii(w in georgian_word()) {
        if let Some(key) = GeorgianPhonex.encode(&w) {
            prop_assert!(key.is_ascii(), "phonex produced non-ASCII {:?}", key);
        }
    }

    /// Ejective and aspirated counterparts produce the same phonex
    /// key when substituted into the same word — the phonex collapses
    /// the two by design.
    #[test]
    fn phonex_folds_ejective_pairs(w in georgian_word()) {
        // Substitute ejective ტ (t') for aspirate თ (t) and check
        // that the phonex agrees.
        let aspirate: String = w.chars().map(|c| if c == 'თ' { 'ტ' } else { c }).collect();
        let ejective: String = w.chars().map(|c| if c == 'ტ' { 'თ' } else { c }).collect();
        let a = GeorgianPhonex.encode(&aspirate);
        let b = GeorgianPhonex.encode(&ejective);
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = GeorgianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in georgian_text()) {
        let toks: Vec<&str> = GeorgianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in georgian_text()) {
        for t in GeorgianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
