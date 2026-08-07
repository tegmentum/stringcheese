//! Property tests for the Russian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::RussianGost779B;
use crate::snowball::RussianSnowball;
use crate::tokenizer::RussianTokenizer;
use crate::{RUSSIAN, STOPWORDS};

/// Strategy for a Cyrillic-flavoured word 1..=15 chars from the modern
/// Russian alphabet (lowercase, plus `ё`).
fn cyrillic_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[а-яё]{1,15}").expect("static regex is valid")
}

/// Strategy for a mixed-case Cyrillic word 1..=15 chars.
fn mixed_case_cyrillic_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[а-яА-ЯёЁ]{1,15}").expect("static regex is valid")
}

/// Strategy for arbitrary short Russian-flavoured text.
fn russian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[а-яА-ЯёЁ ,.!?-]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    ///
    /// (Note: Snowball's Russian algorithm is not guaranteed to be a
    /// fixed point of itself in general — the algorithm is a single
    /// pass over the input's suffix table, and bare-vowel noun endings
    /// like `-е` / `-о` can keep matching on the shortened stem
    /// through repeated iteration. Callers are expected to stem once
    /// per token and index the result.)
    #[test]
    fn snowball_is_deterministic(w in cyrillic_word()) {
        let a = RussianSnowball.stem(&w).into_owned();
        let b = RussianSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on ASCII-free Cyrillic
    /// input within a bounded number of iterations.
    ///
    /// The bound is generous — Russian's fusional morphology can
    /// leave residues that cascade under repeated stemming, but the
    /// process always terminates because each successful strip
    /// shortens the stem by at least one character and the algorithm
    /// stops firing when no suffix in the table matches.
    #[test]
    fn snowball_converges_within_bounded_iterations(w in cyrillic_word()) {
        let mut cur = RussianSnowball.stem(&w).into_owned();
        for _ in 0..32 {
            let next = RussianSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Snowball did not converge in 32 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix, so the stem monotonically
    /// shrinks (or stays the same, except for the `ё → е` fold which
    /// is length-preserving).
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in cyrillic_word()) {
        let out = RussianSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem contains no `ё` — the preprocessing step folds them
    /// away.
    #[test]
    fn snowball_output_has_no_yo(w in cyrillic_word()) {
        let out = RussianSnowball.stem(&w).into_owned();
        prop_assert!(
            !out.contains('ё'),
            "stem {:?} still contains ё (should be folded to е)",
            out
        );
    }

    /// `ё` and `е` variants of the same word stem identically.
    #[test]
    fn stemming_is_yo_invariant(w in cyrillic_word()) {
        let with_yo = w.clone();
        let without_yo: String = w.chars()
            .map(|c| if c == 'ё' { 'е' } else { c })
            .collect();
        let a = RussianSnowball.stem(&with_yo).into_owned();
        let b = RussianSnowball.stem(&without_yo).into_owned();
        prop_assert_eq!(a, b);
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Cyrillic case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(RUSSIAN.is_stopword(w));
    }

    /// The transliteration encoder is total on Cyrillic input.
    #[test]
    fn gost_779_b_is_total_on_cyrillic(w in cyrillic_word()) {
        let out = RussianGost779B.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "GOST 7.79-B returned empty for {:?}",
            w
        );
    }

    /// The GOST 7.79-B encoder is idempotent under case-fold: encoding
    /// an uppercased word matches encoding the lowercased word.
    #[test]
    fn gost_779_b_is_case_invariant(w in mixed_case_cyrillic_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = RussianGost779B.encode(&lower);
        let b = RussianGost779B.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// GOST 7.79-B is ASCII-total on pure-Cyrillic input.
    #[test]
    fn gost_779_b_output_is_ascii(w in cyrillic_word()) {
        let out = RussianGost779B.encode(&w);
        prop_assert!(
            out.is_ascii(),
            "GOST 7.79-B produced non-ASCII output {:?} for {:?}",
            out,
            w
        );
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = RussianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in russian_text()) {
        let toks: Vec<&str> = RussianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in russian_text()) {
        for t in RussianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
