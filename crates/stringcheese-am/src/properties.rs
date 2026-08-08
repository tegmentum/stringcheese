//! Property tests for the Amharic language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::geez::{compose, decompose};
use crate::phonetic::{AmharicBgnPcgn, AmharicPhonex};
use crate::stemmer::LightAmharicStemmer;
use crate::{AMHARIC, STOPWORDS};

/// Strategy for an Amharic word: a run of 1..=10 Ge'ez main-block
/// scalars (U+1200..=U+135F — avoiding the punctuation range
/// U+1361..=U+1368 by cutting off before it).
fn amharic_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\u{1200}-\u{135F}]{1,10}").expect("static regex is valid")
}

/// Strategy for Amharic text with wordspace, full stop, and ASCII
/// punctuation.
fn amharic_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\u{1200}-\u{135F} ፡።፣!?]{0,40}").expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Ge'ez decompose / compose — round-trip for every main-block scalar.
    // -----------------------------------------------------------------

    /// For every main-block scalar, `compose(base, order)` where
    /// `(base, order) = decompose(c)` returns `c` itself.
    #[test]
    fn geez_decompose_compose_round_trip(cp in 0x1200u32..=0x137Fu32) {
        let c = char::from_u32(cp).unwrap();
        let (base, order) = decompose(c).unwrap();
        let back = compose(base, order).unwrap();
        prop_assert_eq!(back, c);
    }

    /// Every family head (offset % 8 == 0) decomposes to (itself, 0).
    #[test]
    fn geez_family_heads_decompose_to_self_order_zero(family in 0u32..48) {
        let cp = 0x1200 + family * 8;
        let c = char::from_u32(cp).unwrap();
        let (base, order) = decompose(c).unwrap();
        prop_assert_eq!(base, c);
        prop_assert_eq!(order, 0);
    }

    // -----------------------------------------------------------------
    // Stemmer — converges, doesn't grow input, deterministic.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic.
    #[test]
    fn stemmer_is_deterministic(w in amharic_word()) {
        let a = LightAmharicStemmer.stem(&w).into_owned();
        let b = LightAmharicStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Amharic input
    /// within a small number of iterations. `stem` itself iterates
    /// to convergence, so a *second* call must be a no-op.
    #[test]
    fn stemmer_is_idempotent(w in amharic_word()) {
        let once = LightAmharicStemmer.stem(&w).into_owned();
        let twice = LightAmharicStemmer.stem(&once).into_owned();
        prop_assert_eq!(once, twice);
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in amharic_word()) {
        let out = LightAmharicStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // BGN/PCGN transliteration — output is ASCII plus a couple of
    // stand-in punctuation marks for glottal / ayn.
    // -----------------------------------------------------------------

    /// BGN/PCGN is total on non-empty Amharic input.
    #[test]
    fn bgn_pcgn_is_total_on_amharic(w in amharic_word()) {
        let out = AmharicBgnPcgn.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "BGN/PCGN returned empty for non-empty Amharic input {:?}",
            w
        );
    }

    /// BGN/PCGN output contains no Ge'ez scalars — every main-block
    /// scalar is mapped to Latin.
    #[test]
    fn bgn_pcgn_output_has_no_geez(w in amharic_word()) {
        let out = AmharicBgnPcgn.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{1200}'..='\u{137F}').contains(&c),
                "BGN/PCGN({:?}) = {:?} still contains Ge'ez scalar {:?}",
                w,
                out,
                c
            );
        }
    }

    // -----------------------------------------------------------------
    // PHONEX-Amharic — well-formed 4-char key on Amharic input.
    // -----------------------------------------------------------------

    /// PHONEX-Amharic always produces a 4-char key on non-empty
    /// Amharic input.
    #[test]
    fn phonex_yields_four_char_key(w in amharic_word()) {
        if let Some(key) = AmharicPhonex.encode(&w) {
            prop_assert_eq!(
                key.chars().count(),
                4,
                "PHONEX-AM({:?}) is not 4 chars",
                w
            );
        }
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    /// Every stopword in the shipped list is recognized.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(AMHARIC.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = AMHARIC.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in amharic_text()) {
        let toks: Vec<&str> = AMHARIC.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in amharic_text()) {
        for t in AMHARIC.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
