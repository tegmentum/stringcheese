//! Property tests for the German language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::KoelnerPhonetik;
use crate::snowball::SnowballDe;
use crate::{GERMAN, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for German-flavoured mixed-case ASCII word (letters + a
/// small probability of an umlaut / ß).
fn german_word() -> impl Strategy<Value = String> {
    // A word from the letters that our stemmer actually operates on —
    // ASCII letters plus the German-specific scalars.
    prop::string::string_regex("[a-zA-ZäöüÄÖÜß]{1,20}").expect("static regex is valid")
}

/// Strategy for a word containing at least one encodable non-H letter,
/// so the phonetic encoder is guaranteed to produce a code.
fn encodable_word() -> impl Strategy<Value = String> {
    // Force at least one non-H, non-whitespace ASCII letter.
    prop::string::string_regex("[a-gi-zA-GI-Z]{1,20}").expect("static regex is valid")
}

proptest! {
    /// Snowball German is not universally idempotent (some words can
    /// pass through Steps 1, 2, and 3 more than once before reaching a
    /// fixed point — the follow-up cleanup rules can expose newly
    /// stripped suffixes to the outer step). We verify convergence in
    /// at most 5 iterations, which covers the pathological cases with
    /// margin.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in german_word()) {
        let mut cur = SnowballDe.stem(&w).into_owned();
        for _ in 0..5 {
            let next = SnowballDe.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "SnowballDe did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// Snowball stem is never longer (in bytes) than the input. Every
    /// rule is either a truncation or a fold. `ß` (2 UTF-8 bytes)
    /// expands to `ss` (2 UTF-8 bytes) — same byte count. Each
    /// `ä`/`ö`/`ü` (2 bytes) folds to `a`/`o`/`u` (1 byte) — a shrink.
    /// So the output byte length is bounded by the input's.
    #[test]
    fn snowball_stem_is_bounded_by_input(w in german_word()) {
        let out = SnowballDe.stem(&w).into_owned();
        prop_assert!(
            out.len() <= w.len(),
            "SnowballDe({:?}) = {:?} grew from {} to {} bytes",
            w,
            out,
            w.len(),
            out.len()
        );
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped stopword
    /// list.
    #[test]
    fn is_stopword_case_invariant(w in ascii_word()) {
        let hit_lower = GERMAN.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = GERMAN.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every stopword in the list is recognized (and any ASCII-cased
    /// variant thereof — non-ASCII case invariance is documented as
    /// out of scope for the default `is_stopword`).
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(GERMAN.is_stopword(w));
        // ASCII-case-invariance only holds for words whose non-ASCII
        // chars round-trip through `str::eq_ignore_ascii_case` (which
        // treats non-ASCII as-is). So an all-ASCII stopword lifted to
        // uppercase is still recognized; an all-lowercase stopword
        // with an umlaut is recognized as-is (its uppercase form
        // requires Unicode case folding, which the default impl does
        // not do).
        if w.is_ascii() {
            prop_assert!(GERMAN.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The Kölner Phonetik encoder is total: it returns `Some` for any
    /// input containing at least one encodable ASCII letter (i.e. any
    /// ASCII letter other than `H`, which is the only letter that
    /// produces no code).
    #[test]
    fn koelner_phonetik_is_total_on_encodable_input(w in encodable_word()) {
        let out = KoelnerPhonetik.encode(&w);
        prop_assert!(
            out.is_some(),
            "KoelnerPhonetik returned None for {:?}",
            w
        );
    }

    /// Kölner Phonetik output consists only of decimal digits.
    #[test]
    fn koelner_phonetik_output_is_digits(w in mixed_case_word()) {
        if let Some(code) = KoelnerPhonetik.encode(&w) {
            prop_assert!(
                code.chars().all(|c| c.is_ascii_digit()),
                "KoelnerPhonetik({:?}) = {:?} contains a non-digit",
                w,
                code
            );
        }
    }
}
