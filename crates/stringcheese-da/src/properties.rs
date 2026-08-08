//! Property tests for the Danish language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::DanishPhonex;
use crate::snowball::DanishSnowball;
use crate::tokenizer::DanishTokenizer;
use crate::{DANISH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for Snowball tests (accented chars are exercised in the reference
/// pairs and unit tests, where hand-verified inputs prove the algorithm
/// handles them; the property-shaped `[a-z]{1,20}` corner-case fan-out
/// is enough for convergence checks).
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Danish-flavoured word (ASCII plus Danish letters
/// `æ ø å` and common diacritics on borrowings).
fn danish_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZæøåÆØÅéèêë]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Danish-flavoured text — letters,
/// Danish-specific letters, spaces, punctuation.
fn danish_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-ZæøåÆØÅéèêë ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    ///
    /// Diacritic-carrying inputs are exercised by the reference-pair
    /// table (`tests/snowball_reference.rs`), not by this property
    /// test.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = DanishSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = DanishSnowball.stem(&cur).into_owned();
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
    /// every Snowball Danish rule is a suffix-delete (which shortens by
    /// at least one char) or the `løst → løs` replacement (which
    /// shortens by exactly one char).
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in danish_word()) {
        let out = DanishSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped stopword
    /// list (the default trait implementation uses
    /// `str::eq_ignore_ascii_case`).
    #[test]
    fn is_stopword_case_invariant_ascii(w in mixed_case_word()) {
        let hit_lower = DANISH.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = DANISH.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a stopword,
    /// including under uppercased ASCII input.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(DANISH.is_stopword(w));
        if w.is_ascii() {
            prop_assert!(DANISH.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The Danish phonetic encoder is total on ASCII-alphabetic input:
    /// any non-empty input containing at least one letter that isn't
    /// silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        // Include at least one non-H letter to guarantee `.encode()`
        // returns `Some`. All-H strings fold to empty after silent-H
        // stripping — those legitimately return None.
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = DanishPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "DanishPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in danish_word()) {
        if let Some(k) = DanishPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = DanishPhonex.encode(&w.to_ascii_lowercase());
        let b = DanishPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = DanishTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in danish_text()) {
        let toks: Vec<&str> = DanishTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in danish_text()) {
        for t in DanishTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
