//! Property tests for the Spanish language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::phonetic::SpanishPhonex;
use crate::snowball::SpanishSnowball;
use crate::tokenizer::SpanishTokenizer;
use crate::{SPANISH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars — the safe subset
/// for Snowball tests (accented chars are exercised in the reference
/// pairs and unit tests, where hand-verified inputs prove the algorithm
/// handles them; the property-shaped `[a-z]{1,20}` corner-case fan-out
/// is enough for convergence checks).
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

/// Strategy for a Spanish-flavoured word (ASCII plus accents / ñ / ü).
fn spanish_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-ZáéíóúñüÁÉÍÓÚÑÜ]{1,20}").expect("static regex is valid")
}

/// Strategy for arbitrary short Spanish-flavoured text — letters,
/// accents, spaces, punctuation.
fn spanish_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[a-zA-Záéíóúñü ,.!?¿¡]{0,40}").expect("static regex is valid")
}

proptest! {
    /// The stemmer must converge to a fixed point within a bounded
    /// number of iterations on ASCII-only input.
    ///
    /// The convergence bound is deliberately generous: Snowball Spanish
    /// can chain step 2b + step 3 into cascades that shrink the stem
    /// by one vowel per iteration on synthetic all-vowel gibberish,
    /// so real English-alphabet corner cases converge in 2–3
    /// iterations but adversarially-generated inputs may take more.
    /// Accent-heavy inputs are exercised by the reference-pair table
    /// (`tests/snowball_reference.rs`), not by this property test,
    /// because the postlude's iterated deacute + step-3 vowel-strip
    /// interaction has no non-trivial fixed-point bound on synthetic
    /// all-accent gibberish.
    #[test]
    fn snowball_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = SpanishSnowball.stem(&w).into_owned();
        for _ in 0..8 {
            let next = SpanishSnowball.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Snowball did not converge in 8 iterations starting from {:?}",
            w
        );
    }

    /// The stem is never longer than the input (in character count) —
    /// every Snowball Spanish rule is either a delete-suffix or a
    /// replace-with-shorter-fixed-string, plus the deacute postlude
    /// which is char-count-preserving.
    ///
    /// Exceptions to strict shrinking:
    ///  - Step 1 group C: `logía → log` (5 → 3, shrinks).
    ///  - Step 1 group D: `ución → u` (5 → 1, shrinks).
    ///  - Step 1 group E: `encia → ente` (5 → 4, shrinks).
    /// Every step's output is bounded above by the input length in
    /// characters.
    #[test]
    fn snowball_stem_char_count_is_no_longer_than_input(w in spanish_word()) {
        let out = SpanishSnowball.stem(&w).into_owned();
        prop_assert!(
            out.chars().count() <= w.chars().count(),
            "Snowball({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.chars().count(),
            out.chars().count()
        );
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped stopword
    /// list (the default trait implementation uses
    /// `str::eq_ignore_ascii_case`). Spanish accented stopwords
    /// (`él`, `más`, `sí`, …) require exact match on the accented form
    /// — the default trait implementation does not fold non-ASCII
    /// scalars — and that's documented in [`crate::stopwords`].
    #[test]
    fn is_stopword_case_invariant_ascii(w in mixed_case_word()) {
        let hit_lower = SPANISH.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = SPANISH.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every entry in the stopword list is recognized as a stopword,
    /// including under uppercased ASCII input.
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(SPANISH.is_stopword(w));
        // ASCII-case-invariance only holds for words whose non-ASCII
        // chars round-trip through `str::eq_ignore_ascii_case` (which
        // treats non-ASCII as-is). So an all-ASCII stopword lifted to
        // uppercase is still recognized; an accented stopword's
        // uppercase form would require Unicode case folding, which the
        // default impl does not do.
        if w.is_ascii() {
            prop_assert!(SPANISH.is_stopword(&w.to_ascii_uppercase()));
        }
    }

    /// The Spanish phonetic encoder is total on ASCII-alphabetic input:
    /// any non-empty input containing at least one letter that isn't
    /// silent-H produces a `Some(_)` key.
    #[test]
    fn phonex_is_total_on_ascii_alphabetic_input(w in ascii_word()) {
        // Include at least one non-H letter to guarantee `.encode()`
        // returns `Some`. The `ascii_word()` strategy can produce
        // all-H strings ("h", "hh", ...), which fold to empty after
        // silent-H stripping — those legitimately return None. Filter
        // them out here since the property under test is totality on
        // "input containing an encodable letter".
        prop_assume!(w.chars().any(|c| c != 'h'));
        let out = SpanishPhonex.encode(&w);
        prop_assert!(
            out.is_some(),
            "SpanishPhonex returned None for {:?}",
            w
        );
    }

    /// The phonex encoder always produces a 4-character key when it
    /// returns Some.
    #[test]
    fn phonex_key_is_always_four_chars(w in spanish_word()) {
        if let Some(k) = SpanishPhonex.encode(&w) {
            prop_assert_eq!(k.chars().count(), 4, "key not 4 chars: {:?}", k);
        }
    }

    /// Case-invariance: uppercasing or lowercasing an ASCII input
    /// doesn't change the phonex key.
    #[test]
    fn phonex_is_case_invariant_ascii(w in mixed_case_word()) {
        let a = SpanishPhonex.encode(&w.to_ascii_lowercase());
        let b = SpanishPhonex.encode(&w.to_ascii_uppercase());
        prop_assert_eq!(a, b);
    }

    /// The tokenizer produces zero tokens for empty input.
    #[test]
    fn tokenizer_empty_input_yields_zero_tokens(_dummy in 0u8..1) {
        let toks: Vec<&str> = SpanishTokenizer::new().tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The tokenizer never invents characters.
    #[test]
    fn tokenizer_never_invents_characters(text in spanish_text()) {
        let toks: Vec<&str> = SpanishTokenizer::new().tokenize(&text).collect();
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
    fn tokenizer_never_yields_empty_tokens(text in spanish_text()) {
        for t in SpanishTokenizer::new().tokenize(&text) {
            prop_assert!(!t.is_empty(), "empty token in output of {:?}", text);
        }
    }
}
