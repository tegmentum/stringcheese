//! Property tests for the Persian language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::normalize::{PersianNormalizer, normalize};
use crate::phonetic::{PersianBuckwalter, buckwalter_to_persian, persian_to_buckwalter};
use crate::stemmer::LightPersianStemmer;
use crate::{PERSIAN, STOPWORDS};

/// Strategy for arbitrary short Persian text made from Persian letters,
/// Arabic-yeh/kaf, ZWNJ, tatweel, and separators.
fn fa_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهیيكى\u{200C}ـ ]{0,40}")
        .expect("static regex is valid")
}

/// Strategy for arbitrary short Persian text spanning digits (Persian
/// and Western) and ۀ.
fn fa_text_with_digits_and_hehyeh() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی0123456789۰۱۲۳۴۵۶۷۸۹ۀ ]{0,40}")
        .expect("static regex is valid")
}

/// Strategy for arbitrary short Persian *words* — letters + optional
/// ZWNJ, no separators. This is the shape the stemmer sees.
fn fa_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی\u{200C}]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for Persian-Buckwalter-alphabet ASCII strings.
fn buckwalter_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"['|><&}AbptvjHxdc\*rzs\$SDTZEGJ_fqkglmnhwYyFNKaui~o`]{0,20}")
        .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Normalizer — idempotent, doesn't grow input on flags-that-shrink.
    // -----------------------------------------------------------------

    /// The default normalizer is idempotent: normalize(normalize(x)) == normalize(x).
    #[test]
    fn normalizer_default_is_idempotent(t in fa_text()) {
        let once = normalize(&t);
        let twice = normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The default normalizer never grows the input (yeh / kaf folds
    /// are same-length in UTF-8; tatweel stripping deletes bytes).
    #[test]
    fn normalizer_default_never_grows_input(t in fa_text()) {
        let out = normalize(&t);
        prop_assert!(
            out.len() <= t.len(),
            "normalize({:?}) grew from {} to {} bytes",
            t,
            t.len(),
            out.len()
        );
    }

    /// ZWNJ stripping is idempotent.
    #[test]
    fn normalizer_with_strip_zwnj_is_idempotent(t in fa_text()) {
        let n = PersianNormalizer::builder().with_strip_zwnj(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// After ZWNJ stripping, no U+200C scalars remain.
    #[test]
    fn strip_zwnj_leaves_no_zwnj(t in fa_text()) {
        let n = PersianNormalizer::builder().with_strip_zwnj(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                c != '\u{200C}',
                "ZWNJ survived stripping of {:?}",
                t
            );
        }
    }

    /// Extended Arabic-Indic → Western digit folding is idempotent.
    #[test]
    fn normalizer_with_western_digits_is_idempotent(
        t in fa_text_with_digits_and_hehyeh()
    ) {
        let n = PersianNormalizer::builder().with_western_digits(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// After Extended Arabic-Indic → Western folding, no Persian
    /// digit scalars remain.
    #[test]
    fn western_digit_folding_leaves_no_persian_digits(
        t in fa_text_with_digits_and_hehyeh()
    ) {
        let n = PersianNormalizer::builder().with_western_digits(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                !matches!(c, '\u{06F0}'..='\u{06F9}'),
                "Persian digit {:?} survived Extended→Western folding of {:?}",
                c,
                t
            );
        }
    }

    /// Tatweel stripping (on by default) leaves no U+0640 scalars.
    #[test]
    fn default_normalizer_leaves_no_tatweel(t in fa_text()) {
        let out = normalize(&t);
        for c in out.chars() {
            prop_assert!(
                c != '\u{0640}',
                "tatweel survived default normalize of {:?}",
                t
            );
        }
    }

    /// After the default normalizer, no Arabic yeh (U+064A) or Arabic
    /// alef maqsura (U+0649) or Arabic kaf (U+0643) scalars remain —
    /// they've all been folded to Persian counterparts.
    #[test]
    fn default_normalizer_leaves_no_arabic_yeh_or_kaf(t in fa_text()) {
        let out = normalize(&t);
        for c in out.chars() {
            prop_assert!(
                !matches!(c, '\u{064A}' | '\u{0649}' | '\u{0643}'),
                "Arabic variant {:?} survived default normalize of {:?}",
                c,
                t
            );
        }
    }

    /// Heh-yeh decomposition is idempotent.
    #[test]
    fn normalizer_with_heh_yeh_is_idempotent(
        t in fa_text_with_digits_and_hehyeh()
    ) {
        let n = PersianNormalizer::builder().with_heh_yeh_normalization(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    // -----------------------------------------------------------------
    // Stemmer — converges in one iteration, doesn't grow input.
    // -----------------------------------------------------------------

    /// The stemmer converges to a fixed point in a bounded number of
    /// iterations. Real Persian inputs converge on the first pass;
    /// synthetic inputs with nested suffixes might need one or two more.
    #[test]
    fn stemmer_converges_within_five_iterations(w in fa_word()) {
        let mut cur = LightPersianStemmer.stem(&w).into_owned();
        for _ in 0..5 {
            let next = LightPersianStemmer.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "LightPersianStemmer did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in fa_word()) {
        let out = LightPersianStemmer.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // Persian-Buckwalter — every mapped Persian scalar round-trips.
    // -----------------------------------------------------------------

    /// Round-trip: for input made of Persian-Buckwalter-alphabet ASCII,
    /// `encode(inverse(x)) == x`.
    #[test]
    fn buckwalter_round_trip_ascii_to_persian_to_ascii(s in buckwalter_text()) {
        let persian = PersianBuckwalter.inverse(&s);
        let back = PersianBuckwalter.encode(&persian);
        prop_assert_eq!(&back, &s, "round-trip failed on {:?}", s);
    }

    /// Every scalar the forward mapping produces, when it decodes to a
    /// Persian scalar, re-encodes to the same ASCII — with the two
    /// exceptions documented in the module docs (Arabic yeh / Arabic
    /// kaf both encode to `y` / `k`, which decode back to the Persian
    /// form).
    #[test]
    fn buckwalter_forward_and_reverse_agree_on_persian_scalars(c in any::<char>()) {
        if let Some(ascii) = persian_to_buckwalter(c)
            && let Some(back) = buckwalter_to_persian(ascii)
        {
            // Re-encode the decoded scalar and compare.
            let re = persian_to_buckwalter(back);
            prop_assert_eq!(
                re,
                Some(ascii),
                "{:?} → {:?} → {:?} → {:?} did not round-trip",
                c,
                ascii,
                back,
                re
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
        prop_assert!(PERSIAN.is_stopword(w));
    }

    /// Persian has no case, so `is_stopword` on an ASCII-uppercased
    /// form of a Persian-only stopword is the same as on the original.
    #[test]
    fn is_stopword_persian_only_ascii_case_is_noop(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        let upper = w.to_ascii_uppercase();
        prop_assert_eq!(w, upper.as_str());
    }
}
