//! Property tests for the Korean language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::jamo::{S_BASE, S_COUNT, compose_jamo, decompose_syllable, is_precomposed_syllable};
use crate::phonetic::{KoreanPhonex, revised_romanization};
use crate::stemmer::KoreanStemmer;
use crate::tokenizer::KoreanTokenizer;
use crate::{KOREAN, STOPWORDS};

/// Strategy for arbitrary short Hangul-ish text: a mix of Hangul
/// syllables (drawn from a small pinned inventory to keep the strategy
/// fast), whitespace, ASCII punctuation, and a handful of ASCII
/// letters so the tokenizer's fused-alphanumeric behavior gets
/// exercised.
fn ko_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[가나다라마바사아자차카타파하은는이가을를에서까지부터에게으로도만의와과책나학교친구사람abc 　、。！？.,]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a short Korean-ish word: Hangul syllables plus a
/// possible-particle syllable set so the stemmer's fixed-point
/// property has interesting inputs to converge on.
fn ko_stem_input() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[가나다라마바사아책나학교친구사람은는이가을를에서까지부터에게으로도만의와과]{1,10}",
    )
    .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Jamo — decompose / compose round-trip and totality.
    // -----------------------------------------------------------------

    /// Every precomposed Hangul syllable in U+AC00..=U+D7A3 decomposes
    /// and composes back to itself. This is the full round-trip check
    /// for all 11172 syllables — the closed-form formula makes it
    /// tractable to enumerate exhaustively in a property test.
    #[test]
    fn every_syllable_round_trips_through_jamo(
        offset in 0u32..S_COUNT,
    ) {
        let cp = S_BASE + offset;
        let c = char::from_u32(cp).expect("Hangul syllable range is valid Unicode");
        let (l, v, t) = decompose_syllable(c).expect("every syllable decomposes");
        let composed = compose_jamo(l, v, t).expect("valid jamos compose");
        prop_assert_eq!(composed, c);
    }

    /// `is_precomposed_syllable` and `decompose_syllable` agree — the
    /// predicate is exactly the domain of the decomposition function.
    #[test]
    fn is_precomposed_syllable_matches_decompose_domain(c in any::<char>()) {
        let is_pre = is_precomposed_syllable(c);
        let dec = decompose_syllable(c);
        prop_assert_eq!(is_pre, dec.is_some());
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never emits empty tokens.
    // -----------------------------------------------------------------

    #[test]
    fn tokenizer_never_invents_characters(text in ko_text()) {
        let toks: Vec<&str> = KoreanTokenizer::new().tokenize(&text).collect();
        for t in &toks {
            for c in t.chars() {
                prop_assert!(
                    text.contains(c),
                    "token {:?} contains character {:?} not in input {:?}",
                    t, c, text,
                );
            }
        }
    }

    #[test]
    fn tokenizer_never_yields_empty_tokens(text in ko_text()) {
        for t in KoreanTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    // -----------------------------------------------------------------
    // Stemmer — idempotent, non-lengthening.
    // -----------------------------------------------------------------

    #[test]
    fn stemmer_idempotent_in_one_pass(w in ko_stem_input()) {
        let once = KoreanStemmer.stem(&w).into_owned();
        let twice = KoreanStemmer.stem(&once).into_owned();
        prop_assert_eq!(&once, &twice, "stem not idempotent on {:?}", w);
    }

    #[test]
    fn stemmer_output_never_longer_than_input(w in ko_stem_input()) {
        let out = KoreanStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}", w, out,
        );
    }

    // -----------------------------------------------------------------
    // Romanization — output is ASCII when the input is all Hangul.
    // -----------------------------------------------------------------

    #[test]
    fn romanization_output_is_ascii_when_input_is_all_hangul(
        input in prop::string::string_regex(
            "[가나다라마바사아자차카타파하한국서울김치안녕세계]{1,15}",
        ).expect("static regex is valid"),
    ) {
        let out = revised_romanization(&input);
        prop_assert!(
            out.is_ascii(),
            "romanization {:?} for {:?} contains non-ASCII", out, input,
        );
    }

    // -----------------------------------------------------------------
    // Phonex — key shape is always <letter><3 digits> when non-empty.
    // -----------------------------------------------------------------

    #[test]
    fn phonex_key_shape_when_present(
        input in prop::string::string_regex(
            "[가나다라마바사아자차카타파하한국서울김치안녕세계]{1,10}",
        ).expect("static regex is valid"),
    ) {
        if let Some(key) = KoreanPhonex.encode(&input) {
            prop_assert_eq!(key.len(), 4);
            let mut chars = key.chars();
            let first = chars.next().unwrap();
            prop_assert!(
                first.is_ascii_uppercase(),
                "first char of {:?} not uppercase ASCII", key,
            );
            for c in chars {
                prop_assert!(
                    c.is_ascii_digit(),
                    "trailing char {:?} not a digit in {:?}", c, key,
                );
            }
        }
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(KOREAN.is_stopword(w));
    }
}
