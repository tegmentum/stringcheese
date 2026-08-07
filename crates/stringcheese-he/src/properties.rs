//! Property tests for the Hebrew language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::normalize::{HebrewNormalizer, normalize};
use crate::phonetic::{Iso259, hebrew_to_iso259, iso259_to_hebrew};
use crate::stemmer::LightHebrewStemmer;
use crate::{HEBREW, STOPWORDS};

/// Strategy for arbitrary short Hebrew text made from letters (base
/// plus final forms), niqqud, and a handful of separators.
#[allow(clippy::unicode_not_nfc)] // the character class deliberately holds standalone niqqud
fn he_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[אבגדהוזחטיכלמנסעפצקרשתךםןףץְֱֲֳִֵֶַָׇֹֻּֿׁׂ ־]{0,40}")
        .expect("static regex is valid")
}

/// Strategy for arbitrary short Hebrew *words* (letters only, no
/// separators, no combining marks — the shape a stemmer sees
/// post-normalize).
fn he_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[אבגדהוזחטיכלמנסעפצקרשתךםןףץ]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for ISO 259 ASCII strings — the alphabet the forward
/// mapping produces.
fn iso259_text() -> impl Strategy<Value = String> {
    // Every ASCII character in the forward table.
    prop::string::string_regex(r"['bgdhwzHTyklmnspcqr\$t`]{0,20}").expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Normalizer — idempotent, doesn't grow input.
    // -----------------------------------------------------------------

    /// The default normalizer is idempotent.
    #[test]
    fn normalizer_default_is_idempotent(t in he_text()) {
        let once = normalize(&t);
        let twice = normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The final-form-folding normalizer is idempotent.
    #[test]
    fn normalizer_with_final_form_folding_is_idempotent(t in he_text()) {
        let n = HebrewNormalizer::builder().with_final_form_folding(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The hebrew-punctuation-stripping normalizer is idempotent.
    #[test]
    fn normalizer_with_hebrew_punct_is_idempotent(t in he_text()) {
        let n = HebrewNormalizer::builder().with_strip_hebrew_punctuation(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The normalizer never grows the input (niqqud / cantillation /
    /// punctuation stripping delete bytes; final-form folding is a
    /// same-length UTF-8 substitution — both variants sit in the
    /// U+05Dx range and take 2 bytes each).
    #[test]
    fn normalizer_never_grows_input(t in he_text()) {
        let out = normalize(&t);
        prop_assert!(
            out.len() <= t.len(),
            "normalize({:?}) grew from {} to {} bytes",
            t,
            t.len(),
            out.len()
        );
    }

    /// After default normalization, no niqqud or cantillation scalars
    /// remain (both are stripped by default).
    #[test]
    fn default_normalizer_leaves_no_niqqud_or_cantillation(t in he_text()) {
        let out = normalize(&t);
        for c in out.chars() {
            prop_assert!(
                !crate::normalize::is_niqqud(c),
                "niqqud {:?} survived default normalization of {:?}",
                c,
                t
            );
            prop_assert!(
                !crate::normalize::is_cantillation(c),
                "cantillation {:?} survived default normalization of {:?}",
                c,
                t
            );
        }
    }

    /// After final-form folding, no final-form letters remain.
    #[test]
    fn final_form_folding_leaves_no_final_forms(t in he_text()) {
        let n = HebrewNormalizer::builder().with_final_form_folding(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                !matches!(
                    c,
                    '\u{05DA}' | '\u{05DD}' | '\u{05DF}' | '\u{05E3}' | '\u{05E5}'
                ),
                "final form {:?} survived folding of {:?}",
                c,
                t
            );
        }
    }

    // -----------------------------------------------------------------
    // Stemmer — converges, doesn't grow input.
    // -----------------------------------------------------------------

    /// LightHebrewStemmer converges to a fixed point within a bounded
    /// number of iterations. Real Hebrew input converges in one call;
    /// adversarial nested-prefix input (e.g. `כככבככאא` — a synthetic
    /// string of stacked prefix letters) can peel one single-letter
    /// prefix per iteration and needs roughly one iteration per
    /// input character. The bound below covers the strategy's max
    /// word length (15 chars) with room to spare. Same single-pass
    /// semantics as `stringcheese_ar::Light10`; the higher bound
    /// reflects Hebrew's larger seven-letter single-prefix inventory
    /// (Arabic light10 has only two: `و` and `ال`).
    #[test]
    fn stemmer_converges_within_twenty_iterations(w in he_word()) {
        let mut cur = LightHebrewStemmer.stem(&w).into_owned();
        for _ in 0..20 {
            let next = LightHebrewStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "LightHebrewStemmer did not converge in 20 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in he_word()) {
        let out = LightHebrewStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // ISO 259 — mapping is bijective on the base-letter subset.
    // -----------------------------------------------------------------

    /// Round-trip: for input made of ISO 259 ASCII, `encode(inverse(x)) == x`
    /// — the inverse turns each ASCII scalar back to its Hebrew base-form
    /// counterpart, and forward encoding of that Hebrew scalar returns
    /// the original ASCII.
    #[test]
    fn iso259_round_trip_ascii_to_hebrew_to_ascii(s in iso259_text()) {
        let hebrew = Iso259.inverse(&s);
        let back = Iso259.encode(&hebrew);
        prop_assert_eq!(&back, &s, "round-trip failed on {:?}", s);
    }

    /// Every ASCII scalar in the inverse mapping round-trips back to
    /// itself via inverse → encode.
    #[test]
    fn iso259_forward_and_reverse_agree_on_every_ascii(c in any::<char>()) {
        if let Some(hebrew) = iso259_to_hebrew(c) {
            let back = hebrew_to_iso259(hebrew);
            prop_assert_eq!(
                back,
                Some(c),
                "iso259 {:?} → hebrew {:?} → {:?} did not round-trip",
                c,
                hebrew,
                back
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
        prop_assert!(HEBREW.is_stopword(w));
    }

    /// Hebrew has no case, so `is_stopword` on an ASCII-uppercased form
    /// of a Hebrew-only stopword is the same as on the original form
    /// (the default `str::eq_ignore_ascii_case` is a no-op on Hebrew
    /// scalars).
    #[test]
    fn is_stopword_hebrew_only_ascii_case_is_noop(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        let upper = w.to_ascii_uppercase();
        prop_assert_eq!(w, upper.as_str());
    }
}
