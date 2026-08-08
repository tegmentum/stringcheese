//! Property tests for the Finnish language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::FinnishPhonex;
use crate::snowball::FinnishSnowball;
use crate::tokenizer::FinnishTokenizer;
use crate::{FINNISH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Finnish-flavoured word — ASCII plus the three
/// Finnish special letters `ä`, `ö`, `å` and their uppercase forms.
fn finnish_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZäöåÄÖÅ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Finnish-flavoured text.
fn finnish_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Zäöå ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations. Finnish's agglutinative morphology can
    /// chain several stripping passes on a single input; the bound is
    /// generous.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in finnish_word()) {
        let mut cur = FinnishSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = FinnishSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Snowball did not converge in 8 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically
    /// shrinks (or stays the same).
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in finnish_word()) {
        let out = FinnishSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
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
        prop_assert!(FINNISH.is_stopword(w));
    }

    /// The Finnish phonetic encoder is total on ASCII-alphabetic input:
    /// any non-empty input containing at least one letter that isn't
    /// silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = FinnishPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "FinnishPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in finnish_word()) {
        if let Some(k) = FinnishPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing and lowercasing an input doesn't
    /// change the phonex key. Finnish's default Unicode fold covers
    /// every letter without special handling.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = FinnishPhonex.encode(&w.to_lowercase());
        let b = FinnishPhonex.encode(&w.to_uppercase());
        prop_assert_eq!(a, b);
    }

    /// Long-vowel and long-consonant collapse: doubling a letter in
    /// the input must not change the phonex key.
    #[test]
    fn phonex_double_letter_is_idempotent(w in ascii_word()) {
        prop_assume!(!w.is_empty());
        let doubled: String = w.chars().flat_map(|c| [c, c]).collect();
        let a = FinnishPhonex.encode(&w);
        let b = FinnishPhonex.encode(&doubled);
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = FinnishTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in finnish_text()) {
        let toks: Vec<&str> = FinnishTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in finnish_text()) {
        for t in FinnishTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
