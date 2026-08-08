//! Property tests for the Macedonian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::MacedonianPhonex;
use crate::stemmer::MacedonianStemmer;
use crate::tokenizer::MacedonianTokenizer;
use crate::{MACEDONIAN, STOPWORDS};

// The Macedonian alphabet is a *subset* of the Cyrillic block, not a
// contiguous range: the sequence `а..я` in the code-point ordering
// includes Russian-only letters (ъ U+044A, ь U+044C, ы U+044B, ё U+0451,
// э U+044D, й U+0439, щ U+0449, ю U+044E, я U+044F) that Macedonian
// does not use. The regex strategies below spell every Macedonian
// letter out to keep the generator's output strictly inside the
// Macedonian alphabet (31 letters, including ѓ ќ љ њ џ ѕ ј).

/// Strategy for a Macedonian word 1..=15 chars from the lowercase
/// Macedonian alphabet.
fn macedonian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[абвгдѓежзѕијклљмнњопрстќуфхцчџш]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for a mixed-case Macedonian word 1..=15 chars.
fn mixed_case_macedonian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[абвгдѓежзѕијклљмнњопрстќуфхцчџшАБВГДЃЕЖЗЅИЈКЛЉМНЊОПРСТЌУФХЦЧЏШ]{1,15}",
    )
    .expect("static regex is valid")
}

/// Strategy for arbitrary short Macedonian-flavoured text.
fn macedonian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[абвгдѓежзѕијклљмнњопрстќуфхцчџшАБВГДЃЕЖЗЅИЈКЛЉМНЊОПРСТЌУФХЦЧЏШ ,.!?-]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in macedonian_word()) {
        let a = MacedonianStemmer.stem(&w).into_owned();
        let b = MacedonianStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations. Each successful strip shortens the stem, and
    /// the algorithm stops firing when no suffix in the tables
    /// matches.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in macedonian_word()) {
        let mut cur = MacedonianStemmer.stem(&w).into_owned();
        for _ in 0..32 {
            let next = MacedonianStemmer.stem(&cur).into_owned();
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
    /// every rule is a delete-suffix, so the stem monotonically
    /// shrinks (or stays the same).
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in macedonian_word()) {
        let out = MacedonianStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "MacedonianStemmer({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem contains no Russian-only letters — Macedonian's letter
    /// set does not include ъ, ь, ы, ё, э, й, щ, ю, я, and the stemmer
    /// never invents characters.
    #[test]
    fn stemmer_output_has_no_non_macedonian_letters(w in macedonian_word()) {
        let out = MacedonianStemmer.stem(&w).into_owned();
        for c in out.chars() {
            prop_assert!(
                !matches!(c, 'ъ' | 'ь' | 'ы' | 'ё' | 'э' | 'й' | 'щ' | 'ю' | 'я'),
                "stem {:?} contains non-Macedonian letter {:?}",
                out,
                c
            );
        }
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Cyrillic case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(MACEDONIAN.is_stopword(w));
    }

    /// The PHONEX encoder is total on Cyrillic input.
    #[test]
    fn phonex_mk_is_total_on_macedonian(w in macedonian_word()) {
        let out = MacedonianPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "MacedonianPhonex returned None for {:?}",
            w
        );
    }

    /// The PHONEX encoder is case-invariant.
    #[test]
    fn phonex_mk_is_case_invariant(w in mixed_case_macedonian_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = MacedonianPhonex.encode(&lower);
        let b = MacedonianPhonex.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// The PHONEX encoder produces a 4-character-count key on every
    /// Macedonian input.
    #[test]
    fn phonex_mk_output_has_four_char_count(w in macedonian_word()) {
        let key = MacedonianPhonex.encode(&w).unwrap();
        prop_assert_eq!(key.chars().count(), 4);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = MacedonianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in macedonian_text()) {
        let toks: Vec<&str> = MacedonianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in macedonian_text()) {
        for t in MacedonianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
