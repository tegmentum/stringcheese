//! Property tests for the Vietnamese language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::is_nfc;

use crate::normalize::{VietnameseNormalizer, is_tone_mark, normalize};
use crate::phonetic::VietnamesePhonex;
use crate::stemmer::VietnameseStemmer;
use crate::tokenizer::VietnameseTokenizer;
use crate::{STOPWORDS, VIETNAMESE};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for correctness tests.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Vietnamese-flavoured word — ASCII plus the seven
/// letter-modifier characters and a broad selection of precomposed
/// tone-marked vowels. Emitted in NFC form.
fn vi_word() -> impl Strategy<Value = String> {
    // The regex covers plain ASCII + letter-modifiers + a broad set
    // of NFC precomposed tone-marked vowels (à á ả ã ạ è é ẻ ẽ ẹ …).
    prop::string::string_regex(
        "[a-zA-Zăâđêôơư\
         àáảãạằắẳẵặầấẩẫậ\
         èéẻẽẹềếểễệ\
         ìíỉĩị\
         òóỏõọồốổỗộờớởỡợ\
         ùúủũụừứửữự\
         ỳýỷỹỵ\
         ÀÁẢÃẠĂẰẮẲẴẶÂẦẤẨẪẬ\
         ÈÉẺẼẸÊỀẾỂỄỆ\
         ÌÍỈĨỊ\
         ÒÓỎÕỌÔỒỐỔỖỘƠỜỚỞỠỢ\
         ÙÚỦŨỤƯỪỨỬỮỰ\
         ỲÝỶỸỴ\
         Đ]{1,20}",
    )
    .expect("static regex is valid")
}

/// Strategy for arbitrary short Vietnamese-flavoured text — letters,
/// diacritics, spaces, punctuation.
fn vi_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        r"[a-zA-ZăâđêôơưàáảãạằắẳẵặầấẩẫậèéẻẽẹềếểễệìíỉĩịòóỏõọồốổỗộờớởỡợùúủũụừứửữựỳýỷỹỵĐ ,.!?-]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a Vietnamese-flavoured NFD string — deliberately
/// decomposed input so we can test NFC canonicalization.
fn vi_nfd_word() -> impl Strategy<Value = String> {
    vi_word().prop_map(|w| w.nfd().collect())
}

proptest! {
    // -----------------------------------------------------------------
    // Normalizer — idempotence per flag configuration.
    // -----------------------------------------------------------------

    /// The default normalizer is idempotent: normalize(normalize(x)) == normalize(x).
    #[test]
    fn normalizer_default_is_idempotent(t in vi_text()) {
        let once = normalize(&t);
        let twice = normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The default normalizer always produces NFC output.
    #[test]
    fn normalizer_default_produces_nfc(t in vi_text()) {
        let out = normalize(&t);
        prop_assert!(is_nfc(&out), "normalize({:?}) = {:?} is not NFC", t, out);
    }

    /// Tone-mark stripping is idempotent.
    #[test]
    fn normalizer_with_strip_tone_marks_is_idempotent(t in vi_text()) {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "tone-strip not idempotent on {:?}", t);
    }

    /// After tone-mark stripping, no tone-mark combining scalar remains
    /// in the output.
    #[test]
    fn strip_tone_marks_leaves_no_tone_marks(t in vi_text()) {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        let out = n.normalize(&t);
        // The output should be decomposable to a form that contains
        // no tone-mark combining scalars. NFD the output and inspect.
        for c in out.nfd() {
            prop_assert!(
                !is_tone_mark(c),
                "tone mark {:?} survived stripping of {:?}",
                c,
                t
            );
        }
    }

    /// Full-diacritic stripping is idempotent.
    #[test]
    fn normalizer_with_strip_all_is_idempotent(t in vi_text()) {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "strip-all not idempotent on {:?}", t);
    }

    /// After full-diacritic stripping, every character is ASCII.
    #[test]
    fn strip_all_leaves_only_ascii(t in vi_text()) {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        let out = n.normalize(&t);
        for c in out.chars() {
            prop_assert!(
                c.is_ascii(),
                "non-ASCII {:?} survived full-diacritic strip of {:?}",
                c,
                t
            );
        }
    }

    /// NFC canonicalization on NFD input round-trips through the
    /// original NFC form.
    #[test]
    fn nfc_round_trip_matches_direct_nfc(w in vi_nfd_word()) {
        let via_normalizer = normalize(&w);
        let direct_nfc: String = w.nfc().collect();
        prop_assert_eq!(via_normalizer, direct_nfc);
    }

    // -----------------------------------------------------------------
    // Stemmer — the NFC canonicalizer is idempotent and identity on NFC.
    // -----------------------------------------------------------------

    /// The stemmer is idempotent.
    #[test]
    fn stemmer_is_idempotent(w in vi_word()) {
        let once = VietnameseStemmer.stem(&w).into_owned();
        let twice = VietnameseStemmer.stem(&once).into_owned();
        prop_assert_eq!(once, twice);
    }

    /// The stemmer is the identity on already-NFC input.
    #[test]
    fn stemmer_is_identity_on_nfc_input(w in vi_word()) {
        // Round-trip through NFC first to guarantee NFC-ness.
        let nfc: String = w.nfc().collect();
        let out = VietnameseStemmer.stem(&nfc);
        prop_assert_eq!(out.as_ref(), nfc.as_str());
    }

    /// The stem's char count is never longer than the input.
    #[test]
    fn stemmer_char_count_no_longer_than_input(w in vi_word()) {
        let out = VietnameseStemmer.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "stem grew on {:?}: {}→{}",
            w,
            w.chars().count(),
            out.chars().count()
        );
    }

    // -----------------------------------------------------------------
    // PHONEX — fixed-width output, total on ASCII-alphabetic input.
    // -----------------------------------------------------------------

    /// The Vietnamese phonetic encoder is total on ASCII-alphabetic
    /// input containing at least one non-H letter.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = VietnamesePhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "VietnamesePhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in vi_word()) {
        if let Some(k) = VietnamesePhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = VietnamesePhonex.encode(&w.to_ascii_lowercase());
        let b = VietnamesePhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tail three characters of a phonex key are always ASCII
    /// digits.
    #[test]
    fn phonex_key_tail_is_digits(w in vi_word()) {
        if let Some(k) = VietnamesePhonex.encode(&w) {
            let bytes = k.as_bytes();
            for &b in &bytes[1..] {
                prop_assert!(
                    b.is_ascii_digit(),
                    "phonex key {:?} has non-digit at position after seed",
                    k
                );
            }
        }
    }

    // -----------------------------------------------------------------
    // Stopword lookup — every entry recognized.
    // -----------------------------------------------------------------

    /// `is_stopword` recognizes every entry in the shipped list.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(VIETNAMESE.is_stopword(w));
    }

    /// `is_stopword` is Unicode-case-invariant on the shipped stopword
    /// list — the pack overrides the default trait implementation to
    /// apply Unicode lowercase before comparison.
    #[test]
    fn is_stopword_case_invariant_unicode(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        prop_assert!(VIETNAMESE.is_stopword(&upper), "uppercase {upper:?} not recognized");
    }

    // -----------------------------------------------------------------
    // Tokenizer.
    // -----------------------------------------------------------------

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = VietnameseTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// No token is empty.
    #[test]
    fn tokenizer_never_yields_empty_tokens(text in vi_text()) {
        for t in VietnameseTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
