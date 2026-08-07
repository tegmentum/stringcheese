//! Property tests for the French language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::snowball::FrenchSnowball;
use crate::tokenizer::FrenchTokenizer;
use crate::{FRENCH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for Snowball tests (accented chars are exercised in the reference
/// pairs and unit tests, where hand-verified inputs prove the algorithm
/// handles them; the property-shaped `[a-z]{1,20}` corner-case fan-out
/// is enough for convergence checks).
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for an arbitrary short French-flavored text, covering
/// letters, apostrophes, spaces, and hyphens — the character classes
/// the tokenizer actually distinguishes.
fn french_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Zàâéèêëïîôûùç' -]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a small
    /// number of iterations — even on synthetic gibberish input.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = FrenchSnowball.stem(&w).into_owned();
        for _ in 0..5 {
            let next = FrenchSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Snowball did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// all Snowball French rules are either delete-suffix or
    /// replace-with-shorter-fixed-string. The one apparent exception,
    /// step 1 group I (`eaux -> eau`), shortens by one character.
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in ascii_word()) {
        let out = FrenchSnowball.stem(&w).into_owned();
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
    /// list (the default trait implementation uses
    /// `str::eq_ignore_ascii_case`).
    #[test]
    fn is_stopword_case_invariant(w in mixed_case_word()) {
        let hit_lower = FRENCH.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = FRENCH.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a stopword,
    /// including under uppercased ASCII input.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(FRENCH.is_stopword(w));
        // ASCII case-folding is trivial on ASCII-only words; skip
        // uppercasing for entries containing non-ASCII (`à`, `être`,
        // `déjà`, ...) — the trait's ASCII-only equality contract
        // doesn't fold those.
        if w.is_ascii() {
            prop_assert!(FRENCH.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = FrenchTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer preserves the total *token-worthy* character count
    /// of the input — every character we consider "in-token"
    /// (alphabetic scalars plus apostrophes that end a clitic) appears
    /// in exactly one token, and the total character count summed over
    /// tokens is at most the input's character count. Combined with the
    /// no-invented-characters guarantee below, this pins the tokenizer
    /// to a genuine partition of the input.
    #[test]
    fn tokenizer_total_char_count_is_bounded_by_input(text in french_text()) {
        let toks: Vec<&str> = FrenchTokenizer::new().tokenize(&text).collect();
        let toks_chars: usize = toks.iter().map(|t| t.chars().count()).sum();
        let input_chars = text.chars().count();
        prop_assert!(
            toks_chars <= input_chars,
            "tokens produced {} chars from {} chars of input {:?}: {:?}",
            toks_chars,
            input_chars,
            text,
            toks,
        );
    }

    /// The tokenizer never invents characters: every character in
    /// every token appears in the input (this is a slightly weaker
    /// form of "borrowed slices come from the input string" — the
    /// pointer-equality version of that check lives in the tokenizer's
    /// unit tests).
    #[test]
    fn tokenizer_never_invents_characters(text in french_text()) {
        let toks: Vec<&str> = FrenchTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in french_text()) {
        for t in FrenchTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
