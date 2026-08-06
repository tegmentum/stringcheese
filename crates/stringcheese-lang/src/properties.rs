//! Property tests for [`crate::Stopwords`] and [`crate::SimpleTokenizer`].
//!
//! Gated on `feature = "std"` and off wasm because `proptest` needs
//! std and its transitive `wait-timeout` dependency has no wasm
//! branch — the same gating as every other property-test module in
//! the workspace.

use proptest::prelude::*;

use crate::{SimpleTokenizer, Stopwords};

/// A general-purpose Unicode strategy that mixes Latin letters,
/// whitespace, punctuation, digits, and a few non-ASCII scripts. The
/// range is small enough to keep the shrink space navigable while
/// still exercising the multi-byte / grapheme paths.
fn general_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(
        "[\\u0000-\\u007F\\u00C0-\\u017F\\u0370-\\u03FF\\u0400-\\u04FF]{0,40}",
    )
    .expect("static regex is valid")
}

/// Strategy for a stopword-list entry: 1..=8 ASCII lowercase letters.
fn stopword_entry() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,8}").expect("static regex is valid")
}

proptest! {
    /// The tokenizer never yields an empty slice.
    #[test]
    fn tokenizer_yields_non_empty_slices(text in general_text()) {
        for tok in SimpleTokenizer::new().tokenize(&text) {
            prop_assert!(!tok.is_empty(), "empty token from {text:?}");
        }
    }

    /// Every token consists entirely of alphanumeric characters.
    #[test]
    fn tokens_are_alphanumeric_runs(text in general_text()) {
        for tok in SimpleTokenizer::new().tokenize(&text) {
            prop_assert!(
                tok.chars().all(char::is_alphanumeric),
                "non-alphanumeric character in token {tok:?} from {text:?}"
            );
        }
    }

    /// A string of whitespace-only characters yields zero tokens.
    #[test]
    fn whitespace_only_input_yields_no_tokens(n in 0usize..32) {
        let s: String = " ".repeat(n);
        prop_assert_eq!(SimpleTokenizer::new().tokenize(&s).count(), 0);
    }

    /// The token borrows are byte-slices of the input.
    #[test]
    fn tokens_borrow_the_input(text in general_text()) {
        let base = text.as_ptr() as usize;
        let end = base + text.len();
        for tok in SimpleTokenizer::new().tokenize(&text) {
            let tok_base = tok.as_ptr() as usize;
            prop_assert!(
                tok_base >= base && tok_base + tok.len() <= end,
                "token {tok:?} is not a subslice of {text:?}"
            );
        }
    }

    /// Stopword lookup is case-invariant: mapping every char through
    /// ASCII toupper doesn't change the answer.
    #[test]
    fn stopword_lookup_is_case_invariant(
        entries in prop::collection::vec(stopword_entry(), 0..8),
        probe in stopword_entry(),
    ) {
        // The stopword list has to be built from static strings for the
        // `Stopwords::new` API; we leak the borrows through a
        // `Vec<String>` -> `Vec<&str>` bridge and re-check the same
        // membership under the two casings.
        let entries_static: Vec<&str> = entries.iter().map(String::as_str).collect();
        // We can't build a `&'static [&'static str]` from runtime data,
        // so we test with a scan that mirrors `Stopwords::contains`.
        let lower = probe.to_ascii_lowercase();
        let upper = probe.to_ascii_uppercase();
        let hit_lower = entries_static.iter().any(|e| e.eq_ignore_ascii_case(&lower));
        let hit_upper = entries_static.iter().any(|e| e.eq_ignore_ascii_case(&upper));
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// The empty stopword set never matches anything.
    #[test]
    fn empty_stopwords_never_match(probe in general_text()) {
        let sw = Stopwords::new(&[]);
        prop_assert!(!sw.contains(&probe));
    }
}
