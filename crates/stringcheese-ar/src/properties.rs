//! Property tests for the Arabic language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::normalize::{ArabicNormalizer, normalize};
use crate::phonetic::{Buckwalter, arabic_to_buckwalter, buckwalter_to_arabic};
use crate::stemmer::Light10;
use crate::{ARABIC, STOPWORDS};

/// Strategy for arbitrary short Arabic text made from letters, harakat,
/// and a handful of separators.
fn ar_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ابتثجحخدذرزسشصضطظعغفقكلمنهويىءأإآؤئةًٌٍَُِّْ 	]{0,40}")
        .expect("static regex is valid")
}

/// Strategy for arbitrary short Arabic *words* (letters only, no
/// separators, no diacritics — the shape a stemmer sees post-normalize).
fn ar_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ابتثجحخدذرزسشصضطظعغفقكلمنهويىة]{1,15}")
        .expect("static regex is valid")
}

/// Strategy for Buckwalter-alphabet ASCII strings.
fn buckwalter_text() -> impl Strategy<Value = String> {
    // The full Buckwalter alphabet — every ASCII character in the
    // forward mapping.
    prop::string::string_regex(r"['|><&}AbptvjHxd\*rzs\$SDTZEg_fqklmnhwYyFNKaui~o`]{0,20}")
        .expect("static regex is valid")
}

proptest! {
    // -----------------------------------------------------------------
    // Normalizer — idempotent, doesn't grow input.
    // -----------------------------------------------------------------

    /// The default normalizer is idempotent: normalize(normalize(x)) == normalize(x).
    #[test]
    fn normalizer_default_is_idempotent(t in ar_text()) {
        let once = normalize(&t);
        let twice = normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The teh-marbuta-folding normalizer is idempotent as well.
    #[test]
    fn normalizer_with_teh_marbuta_is_idempotent(t in ar_text()) {
        let n = ArabicNormalizer::builder().with_teh_marbuta_folding(true);
        let once = n.normalize(&t);
        let twice = n.normalize(&once);
        prop_assert_eq!(&once, &twice, "normalizer not idempotent on {:?}", t);
    }

    /// The normalizer never grows the input (harakat stripping deletes
    /// bytes; alef/yeh/teh-marbuta rewrites are all same-length
    /// substitutions in UTF-8).
    #[test]
    fn normalizer_never_grows_input(t in ar_text()) {
        let out = normalize(&t);
        prop_assert!(
            out.len() <= t.len(),
            "normalize({:?}) grew from {} to {} bytes",
            t,
            t.len(),
            out.len()
        );
    }

    // -----------------------------------------------------------------
    // Stemmer — converges in one iteration, doesn't grow input.
    // -----------------------------------------------------------------

    /// Light10 converges to a fixed point in a bounded number of
    /// iterations. In real Arabic input the first call converges
    /// immediately (one pass suffices — the tables are curated so
    /// no valid stem re-matches a table entry). Adversarial input
    /// with nested prefixes — e.g. `الواف`, `الو…` — can require a
    /// second or third pass because the residue of one prefix strip
    /// may itself start with another prefix; the classical
    /// single-pass semantics is deliberately preserved (see the
    /// [`stemmer` module docs](crate::stemmer#contract) for the
    /// `الوقت` counter-example that justifies keeping the algorithm
    /// non-iterating). The bound below covers every case proptest
    /// has surfaced with room to spare.
    #[test]
    fn stemmer_converges_within_five_iterations(w in ar_word()) {
        let mut cur = Light10.stem(&w).into_owned();
        for _ in 0..5 {
            let next = Light10.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Light10 did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input.
    #[test]
    fn stemmer_output_never_longer_than_input(w in ar_word()) {
        let out = Light10.stem(&w);
        prop_assert!(
            out.len() <= w.len(),
            "stem grew on {:?}: {:?}",
            w,
            out.as_ref()
        );
    }

    // -----------------------------------------------------------------
    // Buckwalter — every mapped Arabic scalar round-trips.
    // -----------------------------------------------------------------

    /// Round-trip: for input made of Buckwalter-alphabet ASCII,
    /// `encode(inverse(x)) == x` — the inverse turns each ASCII scalar
    /// back to its Arabic counterpart, and forward encoding of that
    /// Arabic scalar returns the original ASCII.
    #[test]
    fn buckwalter_round_trip_ascii_to_arabic_to_ascii(s in buckwalter_text()) {
        let arabic = Buckwalter.inverse(&s);
        let back = Buckwalter.encode(&arabic);
        prop_assert_eq!(&back, &s, "round-trip failed on {:?}", s);
    }

    /// Every scalar the forward mapping produces is inversible.
    #[test]
    fn buckwalter_forward_and_reverse_agree_on_every_scalar(c in any::<char>()) {
        if let Some(ascii) = arabic_to_buckwalter(c) {
            let back = buckwalter_to_arabic(ascii);
            prop_assert_eq!(
                back,
                Some(c),
                "arabic {:?} → buckwalter {:?} → {:?} did not round-trip",
                c,
                ascii,
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
        prop_assert!(ARABIC.is_stopword(w));
    }

    /// Arabic has no case, so `is_stopword` on an ASCII-uppercased form
    /// of an Arabic-only stopword is the same as on the original form
    /// (the default `str::eq_ignore_ascii_case` is a no-op on Arabic
    /// scalars).
    #[test]
    fn is_stopword_arabic_only_ascii_case_is_noop(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        // A pure-Arabic stopword's uppercased form (under ASCII case
        // folding) is byte-identical to the original.
        let upper = w.to_ascii_uppercase();
        prop_assert_eq!(w, upper.as_str());
    }
}
