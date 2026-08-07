//! Property tests for the Bulgarian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::BulgarianGost779B;
use crate::snowball::BulgarianSnowball;
use crate::tokenizer::BulgarianTokenizer;
use crate::{BULGARIAN, STOPWORDS};

// The Bulgarian alphabet is a *subset* of the Cyrillic block, not a
// contiguous range: the sequence `а..я` in the code-point ordering
// includes Russian-only letters (ы U+044B, ё U+0451, э U+044D) that
// Bulgarian does not use. The regex strategies below spell every
// Bulgarian letter out to keep the generator's output strictly inside
// the Bulgarian alphabet (30 letters, keeping `ъ` as a vowel).

/// Strategy for a Bulgarian word 1..=15 chars from the lowercase
/// Bulgarian alphabet (excluding Russian-only letters `ё`, `ы`, `э`).
fn bulgarian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[абвгдежзийклмнопрстуфхцчшщъьюя]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for a mixed-case Bulgarian word 1..=15 chars.
fn mixed_case_bulgarian_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[абвгдежзийклмнопрстуфхцчшщъьюяАБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЬЮЯ]{1,15}",
    )
    .expect("static regex is valid")
}

/// Strategy for arbitrary short Bulgarian-flavoured text.
fn bulgarian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[абвгдежзийклмнопрстуфхцчшщъьюяАБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЬЮЯ ,.!?-]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in bulgarian_word()) {
        let a = BulgarianSnowball.stem(&w).into_owned();
        let b = BulgarianSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations. Each successful strip shortens the stem, and
    /// the algorithm stops firing when no suffix in the tables
    /// matches.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in bulgarian_word()) {
        let mut cur = BulgarianSnowball.stem(&w).into_owned();
        for _ in 0..32 {
            let next = BulgarianSnowball.stem(&cur).into_owned();
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
    fn stemmer_stem_char_count_is_no_longer_than_input(w in bulgarian_word()) {
        let out = BulgarianSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "BulgarianSnowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stem contains no `ё` — Bulgarian's letter set does not
    /// include `ё`, and the stemmer never invents characters.
    #[test]
    fn stemmer_output_has_no_yo(w in bulgarian_word()) {
        let out = BulgarianSnowball.stem(&w).into_owned();
        prop_assert!(
            !out.contains('ё'),
            "stem {:?} contains Russian-only ё",
            out
        );
    }

    /// `is_stopword` recognizes every entry in the shipped list under
    /// Cyrillic case-fold rules.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(BULGARIAN.is_stopword(w));
    }

    /// The transliteration encoder is total on Cyrillic input.
    #[test]
    fn gost_779_b_bg_is_total_on_cyrillic(w in bulgarian_word()) {
        let out = BulgarianGost779B.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "BulgarianGost779B returned empty for {:?}",
            w
        );
    }

    /// The transliteration encoder is idempotent under case-fold:
    /// encoding an uppercased word matches encoding the lowercased
    /// word.
    #[test]
    fn gost_779_b_bg_is_case_invariant(w in mixed_case_bulgarian_word()) {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        let a = BulgarianGost779B.encode(&lower);
        let b = BulgarianGost779B.encode(&upper);
        prop_assert_eq!(a, b);
    }

    /// The transliteration is ASCII-total on pure-Bulgarian input.
    #[test]
    fn gost_779_b_bg_output_is_ascii(w in bulgarian_word()) {
        let out = BulgarianGost779B.encode(&w);
        prop_assert!(
            out.is_ascii(),
            "BulgarianGost779B produced non-ASCII output {:?} for {:?}",
            out,
            w
        );
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = BulgarianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in bulgarian_text()) {
        let toks: Vec<&str> = BulgarianTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in bulgarian_text()) {
        for t in BulgarianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
