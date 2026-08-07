//! Property tests for the Ukrainian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::UkrainianGost779B;
use crate::snowball::UkrainianSnowball;
use crate::tokenizer::UkrainianTokenizer;
use crate::{STOPWORDS, UKRAINIAN};

// The Ukrainian alphabet is a *subset* of the Cyrillic block, not a
// contiguous range: the sequence `а..я` in the code-point ordering
// includes Russian-only letters (ъ U+044A, ы U+044B, ё U+0451,
// э U+044D) that Ukrainian does not use. The regex strategies below
// spell every Ukrainian letter out to keep the generator's output
// strictly inside the Ukrainian alphabet.

/// Strategy for a Ukrainian word 1..=15 chars from the lowercase
/// Ukrainian alphabet (including the Ukrainian-specific letters
/// `ґ`, `є`, `і`, `ї`; excluding Russian-only letters).
fn ukrainian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[абвгґдеєжзиіїйклмнопрстуфхцчшщьюя]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for a mixed-case Ukrainian word 1..=15 chars.
fn mixed_case_ukrainian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[абвгґдеєжзиіїйклмнопрстуфхцчшщьюяАБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ]{1,15}",
    )
    .expect("static regex is valid")
}

/// Strategy for arbitrary short Ukrainian-flavoured text.
fn ukrainian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[абвгґдеєжзиіїйклмнопрстуфхцчшщьюяАБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ ,.!?'-]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in ukrainian_word()) {
        let a = UkrainianSnowball.stem(&w).into_owned();
        let b = UkrainianSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in ukrainian_word()) {
        let mut cur = UkrainianSnowball.stem(&w).into_owned();
        for _ in 0..32 {
            let next = UkrainianSnowball.stem(&cur).into_owned();
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
    /// every rule is a delete-suffix or delete-soft-sign, so the stem
    /// monotonically shrinks (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in ukrainian_word()) {
        let out = UkrainianSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "UkrainianSnowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Cyrillic case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(UKRAINIAN.is_stopword(w));
    }

    /// The transliteration encoder is total on Cyrillic input.
    #[test]
    fn gost_779_b_uk_is_total_on_cyrillic(w in ukrainian_word()) {
        let out = UkrainianGost779B.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "UkrainianGost779B returned empty for {:?}",
            w
        );
    }

    /// The transliteration encoder is idempotent under case-fold:
    /// encoding an uppercased word matches encoding the lowercased
    /// word.
    #[test]
    fn gost_779_b_uk_is_case_invariant(w in mixed_case_ukrainian_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = UkrainianGost779B.encode(&lower);
        let b = UkrainianGost779B.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// The transliteration is ASCII-total on pure-Ukrainian input.
    #[test]
    fn gost_779_b_uk_output_is_ascii(w in ukrainian_word()) {
        let out = UkrainianGost779B.encode(&w);
        prop_assert!(
            out.is_ascii(),
            "UkrainianGost779B produced non-ASCII output {:?} for {:?}",
            out,
            w
        );
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = UkrainianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in ukrainian_text()) {
        let toks: Vec<&str> = UkrainianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in ukrainian_text()) {
        for t in UkrainianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    /// Every token, if it contains an ASCII apostrophe, has that
    /// apostrophe surrounded by non-apostrophe characters (the
    /// apostrophe never appears at the beginning or end of a token).
    #[test]
    fn tokenizer_apostrophe_is_word_internal(text in ukrainian_text()) {
        for t in UkrainianTokenizer::new().tokenize(&text) {
            if t.starts_with('\'') || t.ends_with('\'') {
                prop_assert!(
                    false,
                    "token {:?} has boundary apostrophe (input {:?})",
                    t, text
                );
            }
        }
    }
}
