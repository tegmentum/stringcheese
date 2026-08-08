//! Property tests for the Hungarian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::HungarianPhonex;
use crate::snowball::HungarianSnowball;
use crate::tokenizer::HungarianTokenizer;
use crate::{HUNGARIAN, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Hungarian-flavoured word — ASCII plus the nine
/// Hungarian long / umlaut vowels in both cases.
fn hungarian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZáéíóöőúüűÁÉÍÓÖŐÚÜŰ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Hungarian-flavoured text.
fn hungarian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-ZáéíóöőúüűÁÉÍÓÖŐÚÜŰ ,.!?]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    ///
    /// Hungarian's agglutinative morphology can chain several
    /// stripping passes on a single input; the bound is generous.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = HungarianSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = HungarianSnowball.stem(&cur).into_owned();
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
    fn snowball_stem_char_count_is_no_longer_than_input(w in hungarian_word()) {
        let out = HungarianSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stemmer never leaves a stem shorter than the configured
    /// minimum unless the input was itself shorter — the `MIN_STEM_LEN`
    /// floor is enforced on every strip.
    #[test]
    fn snowball_never_over_strips_below_min(w in hungarian_word()) {
        let out = HungarianSnowball.stem(&w).into_owned();
        let n = w.chars().count();
        let out_n = out.chars().count();
        // MIN_STEM_LEN = 2; if input has >2 chars, output must have
        // at least 2.
        if n > 2 {
            prop_assert!(
                out_n >= 2,
                "Snowball({:?}) = {:?} shorter than MIN_STEM_LEN=2 ({}→{})",
                w, out, n, out_n
            );
        }
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Unicode case-fold.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(HUNGARIAN.is_stopword(w));
    }

    /// The Hungarian phonetic encoder is total on ASCII-alphabetic
    /// input: any non-empty input containing at least one non-`h`
    /// letter produces a `Some(_)` key (silent-`h` inputs may
    /// preprocess to the empty string and return None).
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = HungarianPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "HungarianPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in hungarian_word()) {
        if let Some(k) = HungarianPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing and lowercasing a mixed-case
    /// ASCII input doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = HungarianPhonex.encode(&w.to_ascii_lowercase());
        let b = HungarianPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = HungarianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in hungarian_text()) {
        let toks: Vec<&str> = HungarianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in hungarian_text()) {
        for t in HungarianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
