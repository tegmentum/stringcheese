//! Property tests for the Belarusian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::BelarusianPhonex;
use crate::stemmer::BelarusianStemmer;
use crate::tokenizer::BelarusianTokenizer;
use crate::{BELARUSIAN, STOPWORDS};

// The Belarusian alphabet is a *subset* of the Cyrillic block, not a
// contiguous range: the sequence `а..я` in the code-point ordering
// includes Russian-only letters (и U+0438, щ U+0449, ъ U+044A) that
// Belarusian does not use. The regex strategies below spell every
// Belarusian letter out to keep the generator's output strictly
// inside the Belarusian alphabet.

/// Strategy for a Belarusian word 1..=15 chars from the lowercase
/// Belarusian alphabet (including the Belarusian-specific letters
/// `ў`, `і`; excluding Russian-only letters `и`, `щ`, `ъ`).
fn belarusian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[абвгдеёжзійклмнопрстуўфхцчшыьэюя]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for a mixed-case Belarusian word 1..=15 chars.
fn mixed_case_belarusian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[абвгдеёжзійклмнопрстуўфхцчшыьэюяАБВГДЕЁЖЗІЙКЛМНОПРСТУЎФХЦЧШЫЬЭЮЯ]{1,15}",
    )
    .expect("static regex is valid")
}

/// Strategy for arbitrary short Belarusian-flavoured text.
fn belarusian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[абвгдеёжзійклмнопрстуўфхцчшыьэюяАБВГДЕЁЖЗІЙКЛМНОПРСТУЎФХЦЧШЫЬЭЮЯ ,.!?'-]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in belarusian_word()) {
        let a = BelarusianStemmer.stem(&w).into_owned();
        let b = BelarusianStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in belarusian_word()) {
        let mut cur = BelarusianStemmer.stem(&w).into_owned();
        for _ in 0..32 {
            let next = BelarusianStemmer.stem(&cur).into_owned();
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
    fn stemmer_stem_char_count_is_no_longer_than_input(w in belarusian_word()) {
        let out = BelarusianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "BelarusianStemmer({:?}) = {:?} grew ({}→{})",
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
        prop_assert!(BELARUSIAN.is_stopword(w));
    }

    /// When the PHONEX encoder returns a key, it is always exactly
    /// 4 characters. Inputs that consist entirely of scalars the
    /// preprocess step drops (e.g. a lone soft sign `ь`) legitimately
    /// return `None` — the assertion is on shape, not on totality.
    #[test]
    fn phonex_key_shape(w in belarusian_word()) {
        if let Some(key) = BelarusianPhonex.encode(&w) {
            prop_assert_eq!(key.chars().count(), 4);
        }
    }

    /// The PHONEX encoder is idempotent under case-fold: encoding an
    /// uppercased word matches encoding the lowercased word.
    #[test]
    fn phonex_is_case_invariant(w in mixed_case_belarusian_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = BelarusianPhonex.encode(&lower);
        let b = BelarusianPhonex.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// PHONEX output is always uppercase-ASCII.
    #[test]
    fn phonex_output_is_ascii(w in belarusian_word()) {
        if let Some(key) = BelarusianPhonex.encode(&w) {
            prop_assert!(
                key.is_ascii(),
                "BelarusianPhonex produced non-ASCII output {:?} for {:?}",
                key,
                w
            );
        }
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = BelarusianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in belarusian_text()) {
        let toks: Vec<&str> = BelarusianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in belarusian_text()) {
        for t in BelarusianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    /// Every token, if it contains an ASCII apostrophe, has that
    /// apostrophe surrounded by non-apostrophe characters (the
    /// apostrophe never appears at the beginning or end of a token).
    #[test]
    fn tokenizer_apostrophe_is_word_internal(text in belarusian_text()) {
        for t in BelarusianTokenizer::new().tokenize(&text) {
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
