//! Property tests for the Greek language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::{GreekIso843, fold_accents};
use crate::snowball::GreekSnowball;
use crate::tokenizer::GreekTokenizer;
use crate::{GREEK, STOPWORDS};

/// Strategy for a Greek-flavoured word 1..=15 chars from the modern
/// Greek alphabet (lowercase base letters only).
fn greek_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[α-ω]{1,15}").expect("static regex is valid")
}

/// Strategy for a Greek word that may contain accented / diaeresis
/// vowels and the final-sigma variant.
fn greek_word_with_accents() -> impl Strategy<Value = String> {
    prop::string::string_regex("[α-ωάέήίόύώϊϋΐΰς]{1,15}").expect("static regex is valid")
}

/// Strategy for a mixed-case Greek word 1..=15 chars.
fn mixed_case_greek_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[α-ωΑ-Ωάέήίόύώ]{1,15}").expect("static regex is valid")
}

/// Strategy for arbitrary short Greek-flavoured text.
fn greek_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[α-ωΑ-Ωάέήίόύώϊϋ ,.!?-]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn snowball_is_deterministic(w in greek_word()) {
        let a = GreekSnowball.stem(&w).into_owned();
        let b = GreekSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on ASCII-free Greek
    /// input within a bounded number of iterations.
    #[test]
    fn snowball_converges_within_bounded_iterations(w in greek_word()) {
        let mut cur = GreekSnowball.stem(&w).into_owned();
        for _ in 0..32 {
            let next = GreekSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Greek stemmer did not converge in 32 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every rule is a delete-suffix or a length-preserving fold, so
    /// the stem monotonically shrinks (or stays the same).
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in greek_word_with_accents()) {
        let out = GreekSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Greek stemmer({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem contains no final sigma — the `ς → σ` fold is applied
    /// as part of preprocessing.
    #[test]
    fn snowball_output_has_no_final_sigma(w in greek_word_with_accents()) {
        let out = GreekSnowball.stem(&w).into_owned();
        prop_assert!(
            !out.contains('ς'),
            "stem {:?} still contains final sigma ς (should be folded to σ)",
            out
        );
    }

    /// The stem contains no accented vowels — the accent-fold is
    /// applied as part of preprocessing.
    #[test]
    fn snowball_output_has_no_accented_vowels(w in greek_word_with_accents()) {
        let out = GreekSnowball.stem(&w).into_owned();
        for c in out.chars() {
            prop_assert!(
                !matches!(c, 'ά' | 'έ' | 'ή' | 'ί' | 'ό' | 'ύ' | 'ώ' | 'ϊ' | 'ϋ' | 'ΐ' | 'ΰ'),
                "stem {:?} still contains accented / dialytika character {:?}",
                out,
                c
            );
        }
    }

    /// Accented and unaccented variants of the same word stem
    /// identically.
    #[test]
    fn stemming_is_accent_invariant(w in greek_word_with_accents()) {
        let unaccented: String = w.chars().map(fold_accents).collect();
        let a = GreekSnowball.stem(&w).into_owned();
        let b = GreekSnowball.stem(&unaccented).into_owned();
        prop_assert_eq!(a, b);
    }

    /// Final-sigma and non-final sigma variants of the same word stem
    /// identically.
    #[test]
    fn stemming_is_final_sigma_invariant(w in greek_word_with_accents()) {
        let with_final: String = w.chars()
            .map(|c| if c == 'σ' { 'ς' } else { c })
            .collect();
        let with_nonfinal: String = w.chars()
            .map(|c| if c == 'ς' { 'σ' } else { c })
            .collect();
        let a = GreekSnowball.stem(&with_final).into_owned();
        let b = GreekSnowball.stem(&with_nonfinal).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The final-sigma fold is idempotent: folding twice matches
    /// folding once.
    #[test]
    fn final_sigma_fold_is_idempotent(w in greek_word_with_accents()) {
        let once: String = w.chars()
            .map(|c| if c == 'ς' { 'σ' } else { c })
            .collect();
        let twice: String = once.chars()
            .map(|c| if c == 'ς' { 'σ' } else { c })
            .collect();
        prop_assert_eq!(once, twice);
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Greek case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(GREEK.is_stopword(w));
    }

    /// The transliteration encoder is total on Greek input.
    #[test]
    fn iso_843_is_total_on_greek(w in greek_word()) {
        let out = GreekIso843.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "ISO 843 returned empty for {:?}",
            w
        );
    }

    /// The ISO 843 encoder is idempotent under case-fold: encoding an
    /// uppercased word matches encoding the lowercased word.
    #[test]
    fn iso_843_is_case_invariant(w in mixed_case_greek_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = GreekIso843.encode(&lower);
        let b = GreekIso843.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// ISO 843 is ASCII-total on pure-Greek input.
    #[test]
    fn iso_843_output_is_ascii(w in greek_word_with_accents()) {
        let out = GreekIso843.encode(&w);
        prop_assert!(
            out.is_ascii(),
            "ISO 843 produced non-ASCII output {:?} for {:?}",
            out,
            w
        );
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = GreekTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in greek_text()) {
        let toks: Vec<&str> = GreekTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in greek_text()) {
        for t in GreekTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
