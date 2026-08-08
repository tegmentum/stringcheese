//! Property tests for the Estonian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::EstonianPhonex;
use crate::stemmer::EstonianStemmer;
use crate::tokenizer::EstonianTokenizer;
use crate::{ESTONIAN, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for an Estonian-flavoured word — ASCII plus the six
/// Estonian special letters `ä`, `ö`, `ü`, `õ`, `š`, `ž` and their
/// uppercase forms.
fn estonian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZäöüõšžÄÖÜÕŠŽ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Estonian-flavoured text.
fn estonian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Zäöüõšž ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations. The Estonian stemmer strips at most one
    /// suffix per call, so convergence is quick; the bound is
    /// generous.
    #[test]
    fn stemmer_converges_to_a_fixed_point(w in estonian_word()) {
        let mut cur = EstonianStemmer.stem(&w).into_owned();
        for _ in 0..8 {
            let next = EstonianStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "stemmer did not converge in 8 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically
    /// shrinks (or stays the same).
    #[test]
    fn stemmer_char_count_is_no_longer_than_input(w in estonian_word()) {
        let out = EstonianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "stem({:?}) = {:?} grew ({}→{})",
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
        prop_assert!(ESTONIAN.is_stopword(w));
    }

    /// The Estonian phonetic encoder is total on ASCII-alphabetic
    /// input: any non-empty input containing at least one letter that
    /// isn't silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = EstonianPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "EstonianPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in estonian_word()) {
        if let Some(k) = EstonianPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing and lowercasing an input doesn't
    /// change the phonex key. Estonian's default Unicode fold covers
    /// every letter without special handling.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = EstonianPhonex.encode(&w.to_lowercase());
        let b = EstonianPhonex.encode(&w.to_uppercase());
        prop_assert_eq!(a, b);
    }

    /// Long-vowel and long-consonant collapse: doubling a letter in
    /// the input must not change the phonex key.
    #[test]
    fn phonex_double_letter_is_idempotent(w in ascii_word()) {
        prop_assume!(!w.is_empty());
        let doubled: String = w.chars().flat_map(|c| [c, c]).collect();
        let a = EstonianPhonex.encode(&w);
        let b = EstonianPhonex.encode(&doubled);
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = EstonianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in estonian_text()) {
        let toks: Vec<&str> = EstonianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in estonian_text()) {
        for t in EstonianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
