//! Property tests for the Polish language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::PolishPhonex;
use crate::snowball::PolishSnowball;
use crate::tokenizer::PolishTokenizer;
use crate::{POLISH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for Snowball tests (diacritic-carrying chars are exercised in the
/// reference pairs and unit tests, where hand-verified inputs prove
/// the algorithm handles them; the property-shaped `[a-z]{1,20}`
/// corner-case fan-out is enough for convergence checks).
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Polish-flavoured word (ASCII plus the nine Polish
/// diacritic-carrying letters).
fn polish_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZąćęłńóśźżĄĆĘŁŃÓŚŹŻ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Polish-flavoured text — letters,
/// diacritics, spaces, punctuation.
fn polish_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-ZąćęłńóśźżĄĆĘŁŃÓŚŹŻ ,.!?-]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in polish_word()) {
        let a = PolishSnowball.stem(&w).into_owned();
        let b = PolishSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in ascii_word()) {
        let mut cur = PolishSnowball.stem(&w).into_owned();
        for _ in 0..16 {
            let next = PolishSnowball.stem(&cur).into_owned();
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

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically shrinks
    /// (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in polish_word()) {
        let out = PolishSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "PolishSnowball({:?}) = {:?} grew ({}→{})",
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
        prop_assert!(POLISH.is_stopword(w));
    }

    /// `is_stopword` is Unicode-case-invariant on the shipped stopword
    /// list — the pack overrides the default trait implementation to
    /// apply Unicode lowercase before comparison.
    #[test]
    fn is_stopword_case_invariant_unicode(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        prop_assert!(POLISH.is_stopword(&upper), "uppercase {upper:?} not recognized");
    }

    /// The Polish phonetic encoder is total on ASCII-alphabetic input
    /// containing at least one non-H letter.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        // Include at least one non-H letter to guarantee `.encode()`
        // returns `Some`. All-H strings fold to empty after silent-H
        // stripping — those legitimately return None.
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = PolishPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "PolishPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in polish_word()) {
        if let Some(k) = PolishPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = PolishPhonex.encode(&w.to_ascii_lowercase());
        let b = PolishPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = PolishTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in polish_text()) {
        let toks: Vec<&str> = PolishTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in polish_text()) {
        for t in PolishTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
