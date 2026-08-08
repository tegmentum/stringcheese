//! [`RomanianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Romanian orthography is whitespace-and-punctuation delimited with no
//! clitic elision the way French (`l'`, `d'`) or Italian (`dell'`) have
//! it, and no compound-noun agglutination the way German does. The
//! diacritic letters `ă â î ș ț` (plus the legacy cedilla forms `ş ţ`)
//! all satisfy [`char::is_alphanumeric`] and therefore stay word-internal
//! under the default splitter, exactly as needed.
//!
//! # Non-goals
//!
//! * **Postposed article splitting.** Romanian writes the definite
//!   article as a suffix on the noun (`cartea` = "the book"). This is
//!   an orthographic feature — the article is *part of the word* on
//!   the page — so the tokenizer must leave it attached. The Snowball
//!   Romanian stemmer's Step 0 strips the article as part of the
//!   stemming pipeline; see [`crate::snowball`].
//! * **Enclitic pronoun / auxiliary splitting.** Romanian cliticizes
//!   pronouns and the auxiliary `a` onto verbs (`dă-mi` "give me",
//!   `l-am văzut` "I saw him"). The hyphen is a **separator** under
//!   [`char::is_alphanumeric`], so `dă-mi` splits into `dă` and `mi`;
//!   `l-am` splits into `l` and `am`. That's the intended behavior
//!   for IR — the clitics are stopwords anyway.
//! * **Diacritic normalization.** Cedilla → comma-below folding is
//!   the stemmer's / phonex's / stopword-lookup's job, not the
//!   tokenizer's. A caller who indexes tokens with cedilla forms and
//!   queries with comma-below forms will get zero overlap on the
//!   surface tokens — that's expected. Downstream processing (stem,
//!   phonex, stopword) handles the merge.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Romanian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Romanian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_ro::RomanianTokenizer;
///
/// let toks: Vec<&str> = RomanianTokenizer::new()
///     .tokenize("Bună ziua, prietene!")
///     .collect();
/// assert_eq!(toks, ["Bună", "ziua", "prietene"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RomanianTokenizer;

impl RomanianTokenizer {
    /// Constructs a new [`RomanianTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> Tokens<'a> {
        SimpleTokenizer::new().tokenize(text)
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        RomanianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_romanian_sentence() {
        assert_eq!(
            collect("Pisica doarme pe canapea."),
            ["Pisica", "doarme", "pe", "canapea"]
        );
    }

    #[test]
    fn comma_below_diacritics_stay_word_internal() {
        // ș (U+0219) and ț (U+021B) satisfy char::is_alphanumeric.
        assert_eq!(collect("și țară ești"), ["și", "țară", "ești"]);
    }

    #[test]
    fn cedilla_forms_also_stay_word_internal() {
        // ş (U+015F) and ţ (U+0163) also satisfy char::is_alphanumeric.
        // The tokenizer does NOT normalize — downstream stemmer /
        // phonex / stopword handle the fold.
        assert_eq!(collect("şi ţară eşti"), ["şi", "ţară", "eşti"]);
    }

    #[test]
    fn caret_and_breve_diacritics_stay_word_internal() {
        // â (U+00E2), î (U+00EE), ă (U+0103).
        assert_eq!(
            collect("România înaltă brânză"),
            ["România", "înaltă", "brânză"]
        );
    }

    #[test]
    fn hyphenated_clitic_splits_at_the_hyphen() {
        // Enclitic pronoun forms: `dă-mi` splits (hyphen is a separator
        // under is_alphanumeric).
        assert_eq!(collect("dă-mi cartea"), ["dă", "mi", "cartea"]);
        // `l-am văzut` splits at both hyphens and spaces.
        assert_eq!(collect("l-am văzut"), ["l", "am", "văzut"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("anul 2026 luna"), ["anul", "2026", "luna"]);
    }
}
