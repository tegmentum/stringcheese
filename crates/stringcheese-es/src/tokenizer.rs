//! [`SpanishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Spanish does not have French-style clitic elision (`l'`/`d'`/`qu'`)
//! nor German-style compound-noun agglutination. Its orthography is
//! whitespace-and-punctuation delimited with no attached-word contract:
//! `¿Cómo estás?` splits on the inverted question mark and the space,
//! exactly the way [`SimpleTokenizer`] already handles them, and
//! contractions like `del` (= `de` + `el`) or `al` (= `a` + `el`) are
//! written as single orthographic words and belong intact in a single
//! token.
//!
//! There is therefore no reason for the Spanish pack to ship its own
//! tokenizer implementation — this module exposes [`SpanishTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the
//! pack's public surface still names a Spanish-specific tokenizer type
//! (a courtesy for callers who match the language-pack pattern) without
//! duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Enclitic pronoun splitting.** Spanish does *cliticize* object
//!   pronouns onto the ends of infinitives, gerunds, and affirmative
//!   imperatives (`darme`, `dárselo`, `cómpramelo`). Splitting these at
//!   the tokenizer would be wrong for IR indexing — the enclitic-fused
//!   form *is* the surface word. The Snowball stemmer's Step 0 strips
//!   the pronouns as part of the stemming pipeline; see
//!   [`crate::snowball`].
//! - **Inverted punctuation.** Spanish writes `¿…?` and `¡…!` on
//!   sentence-level questions and exclamations. Both are punctuation
//!   and both fall out naturally as separators — no special handling
//!   needed at the token level.
//! - **Ordinal indicators.** `1.º`, `2.ª`, etc. The `.` and the
//!   masculine/feminine ordinal indicators are separators under
//!   [`char::is_alphanumeric`]; this pack does not attempt to preserve
//!   the ordinal as a single token.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Spanish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Spanish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_es::SpanishTokenizer;
///
/// let toks: Vec<&str> = SpanishTokenizer::new()
///     .tokenize("¿Cómo estás? Bien, gracias.")
///     .collect();
/// assert_eq!(toks, ["Cómo", "estás", "Bien", "gracias"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpanishTokenizer;

impl SpanishTokenizer {
    /// Constructs a new [`SpanishTokenizer`]. Zero-sized; free to call.
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
        SpanishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_spanish_sentence() {
        assert_eq!(collect("El gato duerme."), ["El", "gato", "duerme"]);
    }

    #[test]
    fn accented_characters_stay_inside_tokens() {
        assert_eq!(collect("¿Cómo estás?"), ["Cómo", "estás"]);
        assert_eq!(collect("día año niño"), ["día", "año", "niño"]);
    }

    #[test]
    fn inverted_punctuation_is_a_separator() {
        // ¿ (U+00BF) and ¡ (U+00A1) are punctuation — SimpleTokenizer
        // classifies them via `char::is_alphanumeric` (false → separator).
        assert_eq!(collect("¿Qué?"), ["Qué"]);
        assert_eq!(collect("¡Hola!"), ["Hola"]);
    }

    #[test]
    fn contractions_stay_intact() {
        // `del` and `al` are written as single orthographic words.
        assert_eq!(collect("del río al mar"), ["del", "río", "al", "mar"]);
    }

    #[test]
    fn enclitic_pronoun_verb_stays_one_token() {
        // Enclitic pronouns are attached to the verb; the tokenizer
        // must not split them. Stemming Step 0 handles the split.
        assert_eq!(
            collect("darme dárselo cómpramelo"),
            ["darme", "dárselo", "cómpramelo",]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("año 2026 mes"), ["año", "2026", "mes"]);
    }
}
