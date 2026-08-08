//! Property tests for the Hindi language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::normalize::{HindiNormalizer, normalize};
use crate::phonetic::HindiIast;
use crate::stemmer::LightHindiStemmer;
use crate::{HINDI, STOPWORDS};

/// Strategy for a Devanagari word 1..=12 chars anchored on a base
/// consonant.
///
/// The word starts with a base consonant (`क`..`ह`, excluding the
/// three Marathi/Tamil-only letters `ऩ` U+0929, `ऱ` U+0931, `ऴ`
/// U+0934 that are not in the classical Hindi consonant inventory
/// and therefore not in the IAST encoder's mapping), ensuring the
/// input is not a pathological string of bare matras. The tail may
/// contain further classical consonants, primary independent vowels,
/// matras, virama, and anusvara / chandrabindu / visarga — the full
/// inventory the stemmer and encoder are expected to handle in a
/// real Hindi word.
fn devanagari_word() -> impl Strategy<Value = String> {
    // Classical consonant class (base — excludes the Marathi/Tamil
    // letters 0929, 0931, 0934): 0915-0928, 092A-0930, 0932, 0935-0939.
    // Primary independent vowels: 0905-090B (a ā i ī u ū ṛ), 090F-0910
    // (e ai), 0913-0914 (o au).
    // Matras: 093E-0940 (ā i ī), 0941-0942 (u ū), 0943 (ṛ), 0947-0948
    // (e ai), 094B-094C (o au). Virama: 094D. Nukta: 093C. Combining
    // marks: 0901-0903.
    let classical = "\u{0915}-\u{0928}\u{092A}-\u{0930}\u{0932}\u{0935}-\u{0939}";
    let vowels = "\u{0905}-\u{090B}\u{090F}-\u{0910}\u{0913}-\u{0914}";
    let matras = "\u{093E}-\u{0940}\u{0941}-\u{0942}\u{0943}\u{0947}-\u{0948}\u{094B}-\u{094C}";
    let marks = "\u{0901}-\u{0903}\u{093C}\u{094D}";
    let pattern = format!("[{classical}][{classical}{vowels}{matras}{marks}]{{0,11}}");
    prop::string::string_regex(&pattern).expect("static regex is valid")
}

/// Strategy for Devanagari text with punctuation and whitespace.
fn devanagari_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[\u{0905}-\u{0939}\u{093C}\u{093E}-\u{094D}\u{0901}-\u{0903} ।॥,.!?]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for Devanagari text plus digits (Devanagari and ASCII).
fn devanagari_text_with_digits() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\u{0905}-\u{0939}\u{093E}-\u{094D}0-9\u{0966}-\u{096F} ]{0,40}")
        .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Normalizer — idempotent, doesn't grow input on shrinking flags.
    // -----------------------------------------------------------------

    /// The default normalizer is idempotent: normalize(normalize(x)) == normalize(x).
    #[test]
    fn normalizer_default_is_idempotent(t in devanagari_text()) {
        let once = normalize(&t);
        let twice = normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The default normalizer is the identity (no folds enabled).
    #[test]
    fn normalizer_default_is_identity(t in devanagari_text()) {
        let out = normalize(&t);
        prop_assert_eq!(&out, &t, "default normalizer changed input {:?}", t);
    }

    /// Devanagari digit → ASCII digit folding is idempotent.
    #[test]
    fn normalizer_with_digit_folding_is_idempotent(
        t in devanagari_text_with_digits()
    ) {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// After Devanagari → ASCII digit folding, no Devanagari digit
    /// scalars remain.
    #[test]
    fn digit_folding_leaves_no_devanagari_digits(
        t in devanagari_text_with_digits()
    ) {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                !matches!(c, '\u{0966}'..='\u{096F}'),
                "Devanagari digit {:?} survived fold of {:?}",
                c,
                t
            );
        }
    }

    /// Nukta stripping is idempotent.
    #[test]
    fn normalizer_with_nukta_stripping_is_idempotent(t in devanagari_text()) {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// After nukta stripping, no standalone nukta scalars remain.
    /// (Precomposed nukta letters are unaffected.)
    #[test]
    fn nukta_stripping_leaves_no_standalone_nukta(t in devanagari_text()) {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                c != '\u{093C}',
                "standalone nukta survived stripping of {:?}",
                t
            );
        }
    }

    /// The digit-folding normalizer never grows the input (3-byte
    /// Devanagari digits become 1-byte ASCII).
    #[test]
    fn digit_folding_never_grows_input(t in devanagari_text_with_digits()) {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        let out = n.normalize(&t);
        prop_assert!(
            out.len() <= t.len(),
            "digit-folding grew {:?} from {} to {} bytes",
            t,
            t.len(),
            out.len()
        );
    }

    /// The nukta-stripping normalizer never grows the input.
    #[test]
    fn nukta_stripping_never_grows_input(t in devanagari_text()) {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        let out = n.normalize(&t);
        prop_assert!(
            out.len() <= t.len(),
            "nukta-stripping grew {:?} from {} to {} bytes",
            t,
            t.len(),
            out.len()
        );
    }

    // -----------------------------------------------------------------
    // Stemmer — converges immediately, doesn't grow input.
    // -----------------------------------------------------------------

    /// The stemmer is deterministic: two calls on the same input yield
    /// the same output.
    #[test]
    fn stemmer_is_deterministic(w in devanagari_word()) {
        let a = LightHindiStemmer.stem(&w).into_owned();
        let b = LightHindiStemmer.stem(&w).into_owned();
        prop_assert_eq!(a, b);
    }

    /// The stemmer converges to a fixed point on Devanagari input
    /// within a small number of iterations. The single-pass API is
    /// not strictly idempotent in every case — stripping a plural
    /// matra can leave a bare single-char matra that the next pass
    /// strips again — but the process always terminates because
    /// each successful strip shortens the stem by at least one
    /// scalar and the suffix table has finite entries.
    #[test]
    fn stemmer_converges_within_bounded_iterations(w in devanagari_word()) {
        let mut cur = LightHindiStemmer.stem(&w).into_owned();
        for _ in 0..16 {
            let next = LightHindiStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "stemmer did not converge in 16 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in devanagari_word()) {
        let out = LightHindiStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // IAST transliteration — total on Devanagari input.
    // -----------------------------------------------------------------

    /// IAST is total on non-empty Devanagari input.
    #[test]
    fn iast_is_total_on_devanagari(w in devanagari_word()) {
        let out = HindiIast.encode(&w);
        prop_assert!(
            !out.is_empty(),
            "IAST returned empty for non-empty Devanagari input {:?}",
            w
        );
    }

    /// IAST output contains no Devanagari scalars — every
    /// Devanagari-block scalar is mapped to a Latin equivalent (or,
    /// for the digit scalars, to ASCII).
    #[test]
    fn iast_output_has_no_devanagari(w in devanagari_word()) {
        let out = HindiIast.encode(&w);
        for c in out.chars() {
            prop_assert!(
                !('\u{0900}'..='\u{097F}').contains(&c),
                "IAST({:?}) = {:?} still contains Devanagari scalar {:?}",
                w,
                out,
                c
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
        prop_assert!(HINDI.is_stopword(w));
    }

    // -----------------------------------------------------------------
    // Tokenizer — never invents characters, never yields empty tokens.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = HINDI.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in devanagari_text()) {
        let toks: Vec<&str> = HINDI.tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in devanagari_text()) {
        for t in HINDI.tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
