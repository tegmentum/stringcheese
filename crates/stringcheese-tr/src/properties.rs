//! Property tests for the Turkish language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::case_fold::to_turkish_lower;
use crate::phonetic::TurkishPhonex;
use crate::snowball::TurkishSnowball;
use crate::tokenizer::TurkishTokenizer;
use crate::{STOPWORDS, TURKISH};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Turkish-flavoured word — ASCII plus the six Turkish
/// special letters and the dotted-capital-I.
fn turkish_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZçğıİöşüÇĞÖŞÜ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Turkish-flavoured text.
fn turkish_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Zçğıİöşü ,.!?]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    ///
    /// Turkish's agglutinative morphology can chain several stripping
    /// passes on a single input; the bound is generous.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = TurkishSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = TurkishSnowball.stem(&cur).into_owned();
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
    fn snowball_stem_char_count_is_no_longer_than_input(w in turkish_word()) {
        let out = TurkishSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// Turkic case-fold is idempotent: `fold(fold(x)) == fold(x)`.
    ///
    /// This is the property the task spec explicitly names as
    /// "dotless-i idempotence" — after one Turkish-lowercasing pass,
    /// a second pass on the result must produce byte-identical
    /// output.
    #[test]
    fn turkic_case_fold_is_idempotent(w in turkish_word()) {
        let once = to_turkish_lower(&w);
        let twice = to_turkish_lower(&once);
        prop_assert_eq!(once, twice);
    }

    /// `is_stopword` recognizes every entry in the shipped list
    /// under Turkish case-fold rules.
    ///
    /// The Turkish [`Language::is_stopword`] override uses
    /// [`eq_ignore_turkish_case`](crate::case_fold::eq_ignore_turkish_case),
    /// so `İSTANBUL` and `istanbul` are equal, but `Istanbul` and
    /// `istanbul` are NOT (the Latin `I` folds to `ı`, not to `i`).
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(TURKISH.is_stopword(w));
    }

    /// The Turkish phonetic encoder is total on ASCII-alphabetic input:
    /// any non-empty input containing at least one letter that isn't
    /// silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = TurkishPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "TurkishPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in turkish_word()) {
        if let Some(k) = TurkishPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance (Turkic-aware): uppercasing and lowercasing an
    /// input under Turkish rules doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = TurkishPhonex.encode(&w.to_ascii_lowercase());
        let b = TurkishPhonex.encode(&w.to_ascii_uppercase());
        // For ASCII-only strings we still have a subtle wrinkle: the
        // ASCII uppercase of `i` is `I`, which the Turkish encoder
        // folds to `ı` then to `I` — but the ASCII lowercase of `I`
        // is `i`, which the encoder passes through as `i` → `I`. So
        // the encoded ASCII-letter comes out the same, and the keys
        // match.
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = TurkishTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in turkish_text()) {
        let toks: Vec<&str> = TurkishTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in turkish_text()) {
        for t in TurkishTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
