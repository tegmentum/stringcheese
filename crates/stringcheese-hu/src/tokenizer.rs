//! [`HungarianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Hungarian orthography is whitespace-and-punctuation delimited. It
//! uses ASCII letters plus long-vowel and umlaut-vowel scalars:
//! `á` (U+00E1), `é` (U+00E9), `í` (U+00ED), `ó` (U+00F3),
//! `ö` (U+00F6), `ő` (U+0151), `ú` (U+00FA), `ü` (U+00FC),
//! `ű` (U+0171). All of these are alphabetic under
//! `char::is_alphanumeric` and therefore stay inside tokens naturally
//! with the default splitter — the tokenizer has no special knowledge
//! of Hungarian orthography beyond what Unicode's own letter
//! classification provides.
//!
//! Hungarian's **digraphs** `cs`, `dz`, `gy`, `ly`, `ny`, `sz`, `ty`,
//! `zs` (and the trigraph `dzs`) are single letters for collation but
//! two/three ASCII scalars in UTF-8; because both scalars are
//! alphabetic, they stay inside tokens without special handling.
//!
//! There is therefore no reason for the Hungarian pack to ship its own
//! tokenizer implementation — this module exposes
//! [`HungarianTokenizer`] as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Hungarian-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Hungarian is agglutinative — a
//!   single orthographic word can carry several morphemes. Splitting
//!   these at the tokenizer would be wrong for IR indexing; the fused
//!   surface word is the token, and the Snowball stemmer handles
//!   suffix stripping. See [`crate::snowball`].
//! - **Compound-word segmentation.** Hungarian writes noun compounds
//!   as a single word (`asztalitenisz` "table tennis"); segmenting
//!   these needs a lexicon and is out of scope.
//! - **Case folding.** The tokenizer preserves the surface case; case
//!   folding is a separate pipeline concern.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Hungarian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Hungarian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_hu::HungarianTokenizer;
///
/// let toks: Vec<&str> = HungarianTokenizer::new()
///     .tokenize("Szia, világ! Budapest szép város.")
///     .collect();
/// assert_eq!(toks, ["Szia", "világ", "Budapest", "szép", "város"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HungarianTokenizer;

impl HungarianTokenizer {
    /// Constructs a new [`HungarianTokenizer`]. Zero-sized; free to call.
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
        HungarianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_hungarian_sentence() {
        assert_eq!(collect("A kutya alszik."), ["A", "kutya", "alszik"]);
    }

    #[test]
    fn hungarian_special_vowels_stay_inside_tokens() {
        // á, é, í, ó, ö, ő, ú, ü, ű are all alphabetic under Unicode.
        assert_eq!(collect("álom"), ["álom"]);
        assert_eq!(collect("éjszaka"), ["éjszaka"]);
        assert_eq!(collect("Ígéret"), ["Ígéret"]);
        assert_eq!(collect("óra"), ["óra"]);
        assert_eq!(collect("öröm"), ["öröm"]);
        assert_eq!(collect("őz"), ["őz"]);
        assert_eq!(collect("úszik"), ["úszik"]);
        assert_eq!(collect("üveg"), ["üveg"]);
        assert_eq!(collect("űr"), ["űr"]);
    }

    #[test]
    fn hungarian_digraphs_stay_inside_tokens() {
        // cs, sz, zs, gy, ny, ty, ly — every ASCII scalar in a Hungarian
        // digraph is alphabetic, so the digraph naturally stays one
        // token.
        assert_eq!(collect("csirke"), ["csirke"]);
        assert_eq!(collect("szó"), ["szó"]);
        assert_eq!(collect("zsák"), ["zsák"]);
        assert_eq!(collect("gyerek"), ["gyerek"]);
        assert_eq!(collect("nyár"), ["nyár"]);
        assert_eq!(collect("tyúk"), ["tyúk"]);
        assert_eq!(collect("lyuk"), ["lyuk"]);
        assert_eq!(collect("dzseki"), ["dzseki"]);
    }

    #[test]
    fn punctuation_splits_tokens() {
        assert_eq!(
            collect("Igen, persze; miért ne?"),
            ["Igen", "persze", "miért", "ne"]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("év 2026 nap"), ["év", "2026", "nap"]);
    }
}
