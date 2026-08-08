//! Property tests for the Icelandic language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::IcelandicPhonex;
use crate::stemmer::IcelandicStemmer;
use crate::tokenizer::IcelandicTokenizer;
use crate::{ICELANDIC, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for stemmer property tests. Accented chars are exercised in the
/// reference pairs and unit tests, where hand-verified inputs prove
/// the algorithm handles them; the property-shaped `[a-z]{1,20}`
/// corner-case fan-out is enough for convergence checks.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for an Icelandic-flavoured word (ASCII plus Icelandic
/// letters `þ ð æ ö` and the long-vowel accented scalars).
fn icelandic_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZþðæöÞÐÆÖáéíóúýÁÉÍÓÚÝ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Icelandic-flavoured text — letters,
/// Icelandic-specific letters, spaces, punctuation.
fn icelandic_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-ZþðæöÞÐÆÖáéíóúýÁÉÍÓÚÝ ,.!?]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    ///
    /// The stemmer's internal loop already runs to convergence, but
    /// this property test verifies that an external
    /// `stem(stem(w)) == stem(w)` idempotence holds after one
    /// external call — every internal iteration further shortens
    /// the input, so an outer call can never see additional
    /// stripping.
    #[test]
    fn stemmer_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = IcelandicStemmer.stem(&w).into_owned();
        for _ in 0..8 {
            let next = IcelandicStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Icelandic stemmer did not converge in 8 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count)
    /// — every rule is a suffix-delete, which shortens by at least
    /// one char.
    #[test]
    fn stemmer_stem_char_count_is_no_longer_than_input(w in icelandic_word()) {
        let out = IcelandicStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Icelandic stemmer grew {:?} → {:?} ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem always has at least three characters when the input
    /// has at least three characters. Guarded by `MIN_STEM_CHARS`.
    #[test]
    fn stemmer_respects_minimum_stem_length(w in icelandic_word()) {
        let out = IcelandicStemmer.stem(&w).into_owned();
        if w.chars().count() >= 3 {
            prop_assert!(
                out.chars().count() >= 3,
                "Icelandic stemmer under-stemmed {:?} → {:?}",
                w,
                out
            );
        }
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped stopword
    /// list (the default trait implementation uses
    /// `str::eq_ignore_ascii_case`).
    #[test]
    fn is_stopword_case_invariant_ascii(w in mixed_case_word()) {
        let hit_lower = ICELANDIC.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = ICELANDIC.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a
    /// stopword, including under uppercased ASCII input.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(ICELANDIC.is_stopword(w));
        if w.is_ascii() {
            prop_assert!(ICELANDIC.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The Icelandic phonetic encoder is total on ASCII-alphabetic
    /// input: any non-empty input containing at least one letter
    /// that isn't silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        // Include at least one non-H letter to guarantee `.encode()`
        // returns `Some`. All-H strings fold to empty after silent-H
        // stripping — those legitimately return None.
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = IcelandicPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "IcelandicPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in icelandic_word()) {
        if let Some(k) = IcelandicPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = IcelandicPhonex.encode(&w.to_ascii_lowercase());
        let b = IcelandicPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = IcelandicTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in icelandic_text()) {
        let toks: Vec<&str> = IcelandicTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in icelandic_text()) {
        for t in IcelandicTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
