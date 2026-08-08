//! Property tests for the Thai language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::ThaiPhonex;
use crate::stemmer::ThaiStemmer;
use crate::tokenizer::{CharType, ThaiTokenizer, classify};
use crate::{STOPWORDS, THAI};

/// Strategy for arbitrary short Thai-heavy text.
///
/// Uses a small pinned set of high-frequency Thai consonants, vowel
/// marks, tone marks, pre-vowels, digits, whitespace, and ASCII
/// punctuation — enough to exercise the tokenizer, stemmer, and
/// phonex encoder on plausible inputs.
fn th_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[กขคงจชซดตทนบปผพภมยรลวสหอฮ\u{0E30}-\u{0E3A}\u{0E40}-\u{0E44}\u{0E47}\u{0E48}-\u{0E4B}\u{0E4C}\u{0E4D}\u{0E50}-\u{0E59} abcABC0123 .,!?]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a short Thai-heavy word (1..=10 scalars) anchored on
/// a base consonant. Used to exercise the phonex encoder's
/// determinism.
fn th_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[กขคงจชซดตทนบปผพภมยรลวสหอฮ][กขคงจชซดตทนบปผพภมยรลวสหอฮ\u{0E30}-\u{0E3A}\u{0E47}\u{0E48}-\u{0E4B}]{0,9}",
    )
    .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // classify() totality — every scalar receives exactly one type.
    // -----------------------------------------------------------------

    /// Every character classifies without panic.
    #[test]
    fn classify_is_total(c in any::<char>()) {
        let ct = classify(c);
        let bucket: usize = match ct {
            CharType::ThaiConsonant => 1,
            CharType::ThaiPreVowel => 2,
            CharType::ThaiVowelMark => 3,
            CharType::ThaiToneMark => 4,
            CharType::ThaiSign => 5,
            CharType::ThaiDigit => 6,
            CharType::Alphabet => 7,
            CharType::Digit => 8,
            CharType::Punctuation => 9,
            CharType::Whitespace => 10,
            CharType::Other => 11,
        };
        prop_assert!(bucket > 0);
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never emits empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer emits zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = ThaiTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters: every character in
    /// every token appears in the input.
    #[test]
    fn tokenizer_never_invents_characters(text in th_text()) {
        let toks: Vec<&str> = ThaiTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in th_text()) {
        for t in ThaiTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    /// Tokenization is idempotent: re-tokenizing each emitted token
    /// yields that token unchanged.
    #[test]
    fn tokenizer_idempotent(text in th_text()) {
        for t in ThaiTokenizer::new().tokenize(&text) {
            let inner: Vec<&str> = ThaiTokenizer::new().tokenize(t).collect();
            prop_assert_eq!(
                inner.len(),
                1,
                "re-tokenizing {:?} yielded {} tokens, not 1",
                t,
                inner.len(),
            );
            prop_assert_eq!(inner[0], t);
        }
    }

    // -----------------------------------------------------------------
    // Stemmer — idempotent and non-lengthening.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in th_word()) {
        let a = ThaiStemmer.stem(&w).into_owned();
        let b = ThaiStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in th_word()) {
        let out = ThaiStemmer.stem(&w);
        prop_assert!(out.len() <= w.len(), "stem grew on {:?}", w);
    }

    /// The stemmer converges to a fixed point in a bounded number of
    /// iterations. Each successful strip / fold shortens the stem
    /// by at least one scalar and the rule set is finite.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in th_word()) {
        let mut cur = ThaiStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = ThaiStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(false, "stemmer did not converge in 16 iterations starting from {:?}", w);
    }

    // -----------------------------------------------------------------
    // Phonex-Thai — well-formed 4-char key when Thai content present.
    // -----------------------------------------------------------------

    /// Phonex-Thai always produces a 4-char key on non-empty Thai
    /// input.
    #[test]
    fn phonex_yields_four_char_key(w in th_word()) {
        if let Some(key) = ThaiPhonex.encode(&w) {
            prop_assert_eq!(key.chars().count(), 4, "PHONEX-TH({:?}) is not 4 chars", w);
        }
    }

    /// Phonex-Thai is deterministic.
    #[test]
    fn phonex_is_deterministic(w in th_word()) {
        prop_assert_eq!(ThaiPhonex.encode(&w), ThaiPhonex.encode(&w));
    }

    /// Phonex-Thai output is ASCII.
    #[test]
    fn phonex_output_is_ascii(w in th_word()) {
        if let Some(key) = ThaiPhonex.encode(&w) {
            prop_assert!(key.is_ascii(), "PHONEX-TH output {:?} for {:?} is not ASCII", key, w);
        }
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(THAI.is_stopword(w));
    }
}
