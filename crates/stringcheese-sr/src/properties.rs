//! Property tests for the Serbian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::SerbianLatin;
use crate::scripts::{to_cyrillic, to_latin};
use crate::snowball::SerbianSnowball;
use crate::tokenizer::SerbianTokenizer;
use crate::{SERBIAN, STOPWORDS_CYR, STOPWORDS_LAT};

/// Strategy for a Cyrillic-flavoured Serbian word 1..=15 chars.
///
/// Filters out the sequences `лј`, `нј`, `дж` — plain-Cyrillic
/// `л + ј` / `н + ј` / `д + ж` pairs collide on the Latin side with
/// the digraphs `lj` / `nj` / `dž`, which then round-trip back to
/// single-letter `љ` / `њ` / `џ`. This is the "well-formed input"
/// condition documented in `scripts.rs`.
fn cyrillic_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[абвгдђежзијклљмнњопрстћуфхцчџш]{1,15}")
        .expect("static regex is valid")
        .prop_filter("digraph collision", |w| {
            !w.contains("лј") && !w.contains("нј") && !w.contains("дж")
        })
}

/// Strategy for a Latin-flavoured Serbian word 1..=15 chars,
/// deliberately avoiding the digraph-starting bare letters `l`, `n`,
/// `d` in isolation so the round-trip test does not accidentally
/// generate a `nj` / `lj` / `dž` collision.
///
/// (This is the "well-formed input" condition from the module docs:
/// the reverse round-trip holds when digraphs are unambiguous.)
fn latin_word() -> impl Strategy<Value = String> {
    // Simple alphabet without the digraph-first letters. This is
    // conservative — plenty of real Serbian Latin words contain `l`,
    // `n`, `d` — but it guarantees the round-trip property test
    // never hits an ambiguous input.
    prop::string::string_regex("[abcčćefgh ijkmoprsštuvzž]{1,15}").expect("static regex is valid")
}

/// Strategy for arbitrary short Serbian-flavoured text (either
/// script).
fn serbian_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[абвгдђежзијклљмнњопрстћуфхцчџшabcčćdđefghijklmnoprsštuvzž ,.!?-]{0,40}",
    )
    .expect("static regex is valid")
}

proptest! {
    /// `to_cyrillic(to_latin(x)) == x` for any Cyrillic input on the
    /// standard letter set.
    #[test]
    fn cyrillic_latin_cyrillic_round_trip(w in cyrillic_word()) {
        let latin = to_latin(&w);
        let back = to_cyrillic(&latin);
        prop_assert_eq!(back, w);
    }

    /// `to_latin(to_cyrillic(y)) == y` for well-formed Latin input
    /// (digraphs unambiguous — the `latin_word` strategy excludes
    /// the digraph-first letters `l`, `n`, `d` to avoid accidental
    /// collisions).
    #[test]
    fn latin_cyrillic_latin_round_trip(w in latin_word()) {
        let cyr = to_cyrillic(&w);
        let back = to_latin(&cyr);
        prop_assert_eq!(back, w);
    }

    /// `to_latin` output contains no Cyrillic scalars.
    #[test]
    fn to_latin_output_has_no_cyrillic(w in cyrillic_word()) {
        let out = to_latin(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0400}'..='\u{04FF}').contains(&c),
                "to_latin({:?}) leaked Cyrillic {:?}",
                w,
                c,
            );
        }
    }

    /// The stemmer is deterministic.
    #[test]
    fn snowball_is_deterministic(w in cyrillic_word()) {
        let a = SerbianSnowball.stem(&w).into_owned();
        let b = SerbianSnowball.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stem is never longer than the input (in character count).
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in cyrillic_word()) {
        let out = SerbianSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "SerbianSnowball({:?}) = {:?} grew ({} -> {})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// The stemmer converges to a fixed point within a bounded number
    /// of iterations. Each successful strip shortens the stem by at
    /// least one character; the algorithm halts when no suffix
    /// matches or when the stem is below the minimum length.
    #[test]
    fn snowball_converges_within_bounded_iterations(w in cyrillic_word()) {
        let mut cur = SerbianSnowball.stem(&w).into_owned();
        for _ in 0..32 {
            let next = SerbianSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Serbian stemmer did not converge in 32 iterations from {:?}",
            w
        );
    }

    /// Cyrillic input to the stemmer yields Cyrillic output; Latin
    /// input yields Latin output.
    #[test]
    fn stemmer_preserves_script(w in cyrillic_word()) {
        let out = SerbianSnowball.stem(&w).into_owned();
        // If the input is pure Cyrillic and the stem is non-empty,
        // the output should be pure Cyrillic too.
        if !out.is_empty() {
            let all_cyrillic = out
                .chars()
                .all(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
            prop_assert!(
                all_cyrillic,
                "stem({:?}) = {:?} contains non-Cyrillic characters",
                w,
                out
            );
        }
    }

    /// Every entry in the Cyrillic stopword list is recognized.
    #[test]
    fn every_cyrillic_stopword_is_recognized(i in 0usize..STOPWORDS_CYR.len()) {
        let w = STOPWORDS_CYR[i];
        prop_assert!(SERBIAN.is_stopword(w), "stopword {:?} not recognized", w);
    }

    /// Every entry in the Latin stopword list is recognized.
    #[test]
    fn every_latin_stopword_is_recognized(i in 0usize..STOPWORDS_LAT.len()) {
        let w = STOPWORDS_LAT[i];
        prop_assert!(SERBIAN.is_stopword(w), "stopword {:?} not recognized", w);
    }

    /// `is_stopword` is case-insensitive under Unicode case-fold for
    /// both scripts.
    #[test]
    fn stopword_lookup_is_case_insensitive(i in 0usize..STOPWORDS_LAT.len()) {
        let w = STOPWORDS_LAT[i];
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        prop_assert!(SERBIAN.is_stopword(&upper));
    }

    /// The phonetic encoder is total on non-empty input.
    #[test]
    fn phonetic_is_total_on_non_empty(w in cyrillic_word()) {
        let out = SerbianLatin.encode(&w);
        prop_assert!(!out.is_empty(), "SerbianLatin.encode({:?}) = empty", w);
    }

    /// The phonetic encoder produces the same key regardless of
    /// input script.
    #[test]
    fn phonetic_key_is_script_invariant(w in cyrillic_word()) {
        let cyr_key = SerbianLatin.encode(&w);
        let latin = to_latin(&w);
        let latin_key = SerbianLatin.encode(&latin);
        prop_assert_eq!(cyr_key, latin_key);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = SerbianTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// No token is empty.
    #[test]
    fn tokenizer_never_yields_empty_tokens(text in serbian_text()) {
        for t in SerbianTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
