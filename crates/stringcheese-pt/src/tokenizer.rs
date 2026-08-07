//! [`PortugueseTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Portuguese, like Spanish, does not have French-style clitic elision
//! (`l'`/`d'`/`qu'`) nor German-style compound-noun agglutination. Its
//! orthography is whitespace-and-punctuation delimited with no
//! attached-word contract, and contractions like `do` (= `de` + `o`),
//! `da`, `no`, `na`, `pelo`, `pela`, `dum`, `duma` are written as
//! single orthographic words and belong intact in a single token.
//!
//! There is therefore no reason for the Portuguese pack to ship its own
//! tokenizer implementation — this module exposes [`PortugueseTokenizer`]
//! as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names a
//! Portuguese-specific tokenizer type (a courtesy for callers who
//! match the language-pack pattern) without duplicating any splitting
//! logic.
//!
//! # Non-goals
//!
//! - **Mesoclisis / clitic pronoun splitting.** Portuguese (particularly
//!   Peninsular) uses the mesoclitic form `dar-lhe-ei` where the object
//!   pronoun sits between the verb stem and the future marker. Both
//!   parts are hyphen-joined at the surface; the tokenizer treats the
//!   hyphen as a separator per
//!   [`SimpleTokenizer`], so `dar-lhe-ei` splits into `dar`, `lhe`,
//!   `ei` — which is the honest
//!   choice for IR indexing (the enclitic pronoun is a stopword; the
//!   future marker `ei` can be re-attached at the stem step if the
//!   pipeline needs it).
//! - **Ordinal indicators.** `1.º`, `2.ª`, etc. The `.` and the
//!   masculine/feminine ordinal indicators are separators under
//!   [`char::is_alphanumeric`]; this pack does not attempt to preserve
//!   the ordinal as a single token.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Portuguese tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Portuguese does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_pt::PortugueseTokenizer;
///
/// let toks: Vec<&str> = PortugueseTokenizer::new()
///     .tokenize("Como está você? Bem, obrigado.")
///     .collect();
/// assert_eq!(toks, ["Como", "está", "você", "Bem", "obrigado"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PortugueseTokenizer;

impl PortugueseTokenizer {
    /// Constructs a new [`PortugueseTokenizer`]. Zero-sized; free to
    /// call.
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
        PortugueseTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_portuguese_sentence() {
        assert_eq!(collect("O gato dorme."), ["O", "gato", "dorme"]);
    }

    #[test]
    fn accented_characters_stay_inside_tokens() {
        assert_eq!(collect("Como está você?"), ["Como", "está", "você"]);
        assert_eq!(collect("dia ano coração"), ["dia", "ano", "coração"]);
    }

    #[test]
    fn contractions_stay_intact() {
        // `do`, `da`, `no`, `na`, `pelo`, `pela` are written as single
        // orthographic words in Portuguese.
        assert_eq!(
            collect("do rio à praia pelo caminho"),
            ["do", "rio", "à", "praia", "pelo", "caminho"]
        );
    }

    #[test]
    fn tilde_endings_stay_inside_tokens() {
        assert_eq!(collect("mão coração pão"), ["mão", "coração", "pão"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("ano 2026 mês"), ["ano", "2026", "mês"]);
    }
}
