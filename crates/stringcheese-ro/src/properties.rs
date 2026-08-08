//! Property tests for the Romanian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::RomanianPhonex;
use crate::snowball::RomanianSnowball;
use crate::tokenizer::RomanianTokenizer;
use crate::{ROMANIAN, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for Snowball tests.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Romanian-flavoured word (ASCII plus Romanian
/// diacritics — both comma-below and cedilla forms).
fn romanian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZăâîșțĂÂÎȘȚşţŞŢ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Romanian-flavoured text.
fn romanian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Zăâîșț ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = RomanianSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = RomanianSnowball.stem(&cur).into_owned();
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

    /// The stem is never longer than the input in character count —
    /// every step is either a delete-suffix or a replace-with-shorter-
    /// or-equal fixed string.
    ///
    /// Exceptions to strict shrinking:
    ///  - Step 0 group Atie: `atei` (4) → `ație` (4) — same length.
    ///  - Step 1 group Abil: `abilitate` (9) → `abil` (4) — shrinks.
    /// Every step's output is bounded above (weakly) by the input
    /// length in characters.
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in romanian_word()) {
        let out = RomanianSnowball.stem(&w).into_owned();
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
    /// list (the override folds cedilla and falls through to
    /// `str::eq_ignore_ascii_case`).
    #[test]
    fn is_stopword_case_invariant_ascii(w in mixed_case_word()) {
        let hit_lower = ROMANIAN.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = ROMANIAN.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a stopword.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(ROMANIAN.is_stopword(w));
        if w.is_ascii() {
            prop_assert!(ROMANIAN.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The Romanian phonetic encoder is total on ASCII-alphabetic input
    /// that contains at least one non-silent-H letter.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        let out = RomanianPhonex.encode(&w);
        prop_assert!(out.is_some(), "RomanianPhonex returned None for {:?}", w);
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in romanian_word()) {
        if let Some(k) = RomanianPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = RomanianPhonex.encode(&w.to_ascii_lowercase());
        let b = RomanianPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// Cedilla → comma-below fold: encoding a cedilla-form word
    /// yields the same key as encoding its comma-below-form twin.
    #[test]
    fn phonex_cedilla_and_comma_below_agree(w in prop::string::string_regex("[a-zșț]{1,15}").unwrap()) {
        // Fabricate a matching cedilla form by unfolding.
        let cedilla: String = w.chars().map(|c| match c {
            'ș' => 'ş',
            'ț' => 'ţ',
            other => other,
        }).collect();
        let a = RomanianPhonex.encode(&w);
        let b = RomanianPhonex.encode(&cedilla);
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = RomanianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in romanian_text()) {
        let toks: Vec<&str> = RomanianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in romanian_text()) {
        for t in RomanianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
