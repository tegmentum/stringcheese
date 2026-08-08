//! Property tests for the Indonesian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::IndonesianPhonex;
use crate::stemmer::IndonesianStemmer;
use crate::tokenizer::IndonesianTokenizer;
use crate::{INDONESIAN, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — Indonesian's
/// alphabet is a strict subset of ASCII, so no non-ASCII strategy
/// is needed.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Indonesian-flavoured text (ASCII
/// letters, digits, common punctuation, hyphen for reduplication).
fn indonesian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Z0-9 \-,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    #[test]
    fn stemmer_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = IndonesianStemmer.stem(&w).into_owned();
        for _ in 0..8 {
            let next = IndonesianStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "IndonesianStemmer did not converge in 8 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count).
    /// Every rule is either a delete-suffix, a delete-prefix, or a
    /// delete-prefix-plus-restore-one-char — the last of which cannot
    /// grow the stem net (at least one prefix char stripped, at most
    /// one restored, so length is monotonically non-increasing OR
    /// decreases by ≥ 1).
    #[test]
    fn stemmer_char_count_never_exceeds_input(w in ascii_word()) {
        let out = IndonesianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "stem({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` recognizes every entry in the shipped list
    /// under ASCII case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(INDONESIAN.is_stopword(w));
    }

    /// The Indonesian phonetic encoder is total on ASCII-alphabetic
    /// input: any non-empty input containing at least one letter
    /// that isn't silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = IndonesianPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "IndonesianPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in ascii_word()) {
        if let Some(k) = IndonesianPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing and lowercasing the input
    /// (ASCII-only) doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = IndonesianPhonex.encode(&w.to_ascii_lowercase());
        let b = IndonesianPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = IndonesianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in indonesian_text()) {
        let toks: Vec<&str> = IndonesianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in indonesian_text()) {
        for t in IndonesianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
