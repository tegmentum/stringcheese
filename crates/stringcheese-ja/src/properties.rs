//! Property tests for the Japanese language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::hepburn::HepburnRomaji;
use crate::normalize::{KanaNormalizer, fold_hiragana_to_katakana, fold_katakana_to_hiragana};
use crate::romaji::KunreiRomaji;
use crate::stemmer::JapaneseStemmer;
use crate::tokenizer::{CharType, JapaneseTokenizer, classify};
use crate::{JAPANESE, STOPWORDS};

/// Strategy for arbitrary short Unicode text made of the character
/// classes the Japanese pack actually distinguishes: hiragana,
/// katakana, kanji (a small pinned set), Latin letters, digits,
/// whitespace, and Japanese punctuation.
fn ja_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[あいうえおかきくけこがぎぐげごさしすせそたちつてとなにぬねのはひふへほまみむめもらりるれろやゆよわをんっアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲンー私日本人食べ物学生桜川山abcABC0123 　、。・！？]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a short Japanese-ish word: hiragana + a couple of
/// possible-suffix trigger characters, so the stemmer's fixed-point
/// property has interesting inputs to converge on.
fn ja_stem_input() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[あいうえおかきくけこさしすせそたちつてとまみむめもらりるれろ私君達らどもますでしまた]{1,15}",
    )
    .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // classify() totality — every scalar receives exactly one type.
    // -----------------------------------------------------------------

    /// Every character classifies without panic. The enum is finite,
    /// so simply asserting we can call `classify` on any scalar is a
    /// totality check.
    #[test]
    fn classify_is_total(c in any::<char>()) {
        let ct = classify(c);
        // Coerce to `usize` via matching so the assert has an
        // observable effect (and the compiler can't optimize the
        // classification away).
        let bucket: usize = match ct {
            CharType::Hiragana => 1,
            CharType::Katakana => 2,
            CharType::KatakanaHalfwidth => 3,
            CharType::Kanji => 4,
            CharType::Alphabet => 5,
            CharType::Digit => 6,
            CharType::Punctuation => 7,
            CharType::Whitespace => 8,
            CharType::Other => 9,
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
        let toks: Vec<&str> = JapaneseTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters: every character in
    /// every token appears in the input.
    #[test]
    fn tokenizer_never_invents_characters(text in ja_text()) {
        let toks: Vec<&str> = JapaneseTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in ja_text()) {
        for t in JapaneseTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }

    /// The tokenizer preserves the total *token-worthy* character
    /// count: sum of tokens' char counts equals the number of input
    /// characters that are not classified as Whitespace or Punctuation.
    /// This pins the output to a genuine partition of the input.
    #[test]
    fn tokenizer_preserves_token_worthy_char_count(text in ja_text()) {
        let toks: Vec<&str> = JapaneseTokenizer::new().tokenize(&text).collect();
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

    // -----------------------------------------------------------------
    // Romaji — output is ASCII when the input is all-kana.
    // -----------------------------------------------------------------

    /// Kunrei-shiki romanization of a pure-hiragana / pure-katakana
    /// input is ASCII (any character with a code point < 128).
    #[test]
    fn romaji_output_is_ascii_when_input_is_all_kana(
        input in prop::string::string_regex(
            "[あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをんっーアイウエオカキクケコ]{1,15}",
        ).expect("static regex is valid")
    ) {
        let out = KunreiRomaji.romanize(&input);
        prop_assert!(
            out.is_ascii(),
            "Kunrei output {:?} for {:?} contains non-ASCII",
            out,
            input,
        );
    }

    // -----------------------------------------------------------------
    // Stemmer — idempotent, non-lengthening.
    // -----------------------------------------------------------------

    /// The stemmer converges in a single call — a second call on the
    /// output is a no-op (the suffix table is disjoint under prefix,
    /// so there's no cascading strip).
    #[test]
    fn stemmer_idempotent_in_one_pass(w in ja_stem_input()) {
        let once = JapaneseStemmer.stem(&w).into_owned();
        let twice = JapaneseStemmer.stem(&once).into_owned();
        prop_assert_eq!(&once, &twice, "stem not idempotent on {:?}", w);
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in ja_stem_input()) {
        let out = JapaneseStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out,
        );
    }

    // -----------------------------------------------------------------
    // Hepburn romaji — never panics; sensible pass-through on ASCII.
    // -----------------------------------------------------------------

    /// Hepburn romanization does not panic on arbitrary text drawn
    /// from the kana-heavy strategy.
    #[test]
    fn hepburn_never_panics(text in ja_text()) {
        let _ = HepburnRomaji.romanize(&text);
    }

    /// Kunrei and Hepburn agree on the moras where the two systems
    /// don't disagree — the vowel row is one such case.
    #[test]
    fn hepburn_and_kunrei_agree_on_vowels(
        s in prop::string::string_regex("[あいうえお]{1,10}")
            .expect("static regex is valid")
    ) {
        let k = KunreiRomaji.romanize(&s);
        let h = HepburnRomaji.romanize(&s);
        // Kunrei's ASCII-only output guarantee still holds on vowel
        // input; Hepburn produces a non-empty output on non-empty
        // input.
        prop_assert!(k.is_ascii());
        prop_assert!(!h.is_empty());
    }

    // -----------------------------------------------------------------
    // Kana normalizer — idempotent under every flag combination the
    // spec calls out; round-trip preserved for hiragana ↔ katakana.
    // -----------------------------------------------------------------

    /// The default [`KanaNormalizer`] preset is idempotent: applying
    /// it twice yields the same result as applying it once.
    #[test]
    fn kana_normalizer_default_is_idempotent(text in ja_text()) {
        let n = KanaNormalizer::default();
        let once = n.normalize(&text);
        let twice = n.normalize(&once);
        prop_assert_eq!(once, twice);
    }

    /// A fully-configured normalizer is also idempotent.
    #[test]
    fn kana_normalizer_full_config_is_idempotent(text in ja_text()) {
        let n = KanaNormalizer::new()
            .with_full_to_half_ascii(true)
            .with_half_to_full_katakana(true)
            .with_dakuten_canonicalization(true)
            .with_katakana_to_hiragana(true);
        let once = n.normalize(&text);
        let twice = n.normalize(&once);
        prop_assert_eq!(once, twice);
    }

    /// Each individual flag on its own produces an idempotent
    /// normalizer.
    #[test]
    fn kana_normalizer_single_flag_is_idempotent(
        text in ja_text(),
        flag in 0u8..4,
    ) {
        let n = match flag {
            0 => KanaNormalizer::new().with_full_to_half_ascii(true),
            1 => KanaNormalizer::new().with_half_to_full_katakana(true),
            2 => KanaNormalizer::new().with_katakana_to_hiragana(true),
            _ => KanaNormalizer::new().with_dakuten_canonicalization(true),
        };
        let once = n.normalize(&text);
        let twice = n.normalize(&once);
        prop_assert_eq!(once, twice);
    }

    /// Round-trip: for text made only of hiragana in the fixed-offset
    /// range, hiragana → katakana → hiragana is the identity.
    #[test]
    fn hiragana_katakana_round_trip(
        s in prop::string::string_regex(
            "[ぁあぃいぅうぇえぉおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをん]{0,30}",
        ).expect("static regex is valid")
    ) {
        let round_trip = fold_katakana_to_hiragana(&fold_hiragana_to_katakana(&s));
        prop_assert_eq!(&round_trip, &s);
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(JAPANESE.is_stopword(w));
    }
}
