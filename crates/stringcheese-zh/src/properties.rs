//! Property tests for the Chinese language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::Pinyin;
use crate::stemmer::ChineseStemmer;
use crate::tokenizer::{CharType, ChineseTokenizer, classify};
use crate::{CHINESE, STOPWORDS};

/// Strategy for arbitrary short text made of the character classes
/// the Chinese pack actually distinguishes: a small pinned set of
/// high-frequency Han characters, Latin letters, digits, whitespace,
/// and Chinese punctuation.
fn zh_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[的一是不了在人有我他这中大来上国个到说们为子和你地出道也时年得就那要下以生会自着去之过家学对可她里后小么心多天而能好都然没日于起还发成事只作当想看用长因主从现力abcABC0123 　，。！？、]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a short Han-heavy word, used to exercise the pinyin
/// encoder's determinism.
fn zh_word() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[的一是不了在人有我他这中大来上国个到说们为子和你地出道也时年得就那要下以]{1,10}",
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
            CharType::Han => 1,
            CharType::Alphabet => 2,
            CharType::Digit => 3,
            CharType::Punctuation => 4,
            CharType::Whitespace => 5,
            CharType::Other => 6,
        };
        prop_assert!(bucket > 0);
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never emits empty tokens,
    // preserves total token-worthy character count.
    // -----------------------------------------------------------------

    /// The tokenizer emits zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = ChineseTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters: every character in
    /// every token appears in the input.
    #[test]
    fn tokenizer_never_invents_characters(text in zh_text()) {
        let toks: Vec<&str> = ChineseTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in zh_text()) {
        for t in ChineseTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    /// The tokenizer preserves the total *token-worthy* character
    /// count: sum of tokens' char counts equals the number of input
    /// characters that are not classified as Whitespace or
    /// Punctuation. This pins the output to a genuine partition of
    /// the input.
    #[test]
    fn tokenizer_preserves_token_worthy_char_count(text in zh_text()) {
        let toks: Vec<&str> = ChineseTokenizer::new().tokenize(&text).collect();
        let tokens_chars: usize = toks.iter().map(|t| t.chars().count()).sum();
        let worthy_chars: usize = text
            .chars()
            .filter(|c| {
                !matches!(classify(*c), CharType::Whitespace | CharType::Punctuation)
            })
            .count();
        prop_assert_eq!(
            tokens_chars,
            worthy_chars,
            "tokens produced {} chars from {} token-worthy chars in {:?}: {:?}",
            tokens_chars,
            worthy_chars,
            text,
            toks,
        );
    }

    /// Every Han character in the input becomes its own single-char
    /// token — no merging.
    #[test]
    fn every_han_char_becomes_own_token(text in zh_text()) {
        let toks: Vec<&str> = ChineseTokenizer::new().tokenize(&text).collect();
        for t in &toks {
            if t.chars().next().is_some_and(|c| classify(c) == CharType::Han) {
                prop_assert_eq!(
                    t.chars().count(),
                    1,
                    "Han-starting token {:?} has more than one char in {:?}",
                    t,
                    text,
                );
            }
        }
    }

    /// Tokenization is idempotent: re-tokenizing each emitted token
    /// yields that token unchanged (a single token has no separators
    /// inside it).
    #[test]
    fn tokenizer_idempotent(text in zh_text()) {
        for t in ChineseTokenizer::new().tokenize(&text) {
            let inner: Vec<&str> = ChineseTokenizer::new().tokenize(t).collect();
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
    // Pinyin — deterministic and ASCII-only.
    // -----------------------------------------------------------------

    /// Pinyin encoding is deterministic: two calls on the same input
    /// yield the same result.
    #[test]
    fn pinyin_is_deterministic(w in zh_word()) {
        prop_assert_eq!(Pinyin.encode(&w), Pinyin.encode(&w));
    }

    /// Pinyin output is ASCII (letters, digits, spaces, `?` for
    /// unknown Han).
    #[test]
    fn pinyin_output_is_ascii(w in zh_word()) {
        let out = Pinyin.encode(&w);
        prop_assert!(
            out.is_ascii(),
            "Pinyin output {:?} for {:?} contains non-ASCII",
            out,
            w,
        );
    }

    // -----------------------------------------------------------------
    // Stemmer — identity (trivially idempotent, non-lengthening).
    // -----------------------------------------------------------------

    /// Identity stemmer: `stem(w) == w` for every input.
    #[test]
    fn stemmer_is_identity(w in zh_word()) {
        prop_assert_eq!(&*ChineseStemmer.stem(&w), w.as_str());
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(CHINESE.is_stopword(w));
    }
}
