//! [`ArmenianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Armenian orthography is whitespace-and-punctuation delimited. Every
//! Armenian letter in the modern Armenian inventory (U+0530..=U+058F)
//! is alphabetic under Unicode's `char::is_alphanumeric` classification
//! and therefore stays inside tokens naturally with the default
//! splitter. Armenian-script punctuation — `։` (U+0589 full stop), `՝`
//! (U+055D comma), `՞` (U+055E question mark), `՜` (U+055C exclamation
//! mark), and `֊` (U+058A hyphen) — is classified as Unicode
//! punctuation (`Po` / `Pd`) and therefore splits under the default
//! tokenizer. There is no reason for the Armenian pack to ship its own
//! tokenizer implementation — this module exposes [`ArmenianTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names an Armenian-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Armenian scalar in the modern Armenian block (U+0530..=U+058F)
//! is encoded as **two UTF-8 bytes** (this range falls entirely inside
//! U+0080..=U+07FF, UTF-8's 2-byte window). The byte length of an
//! Armenian word is roughly `2 * char_count`, and any code that mixes
//! byte offsets with character-boundary logic will silently corrupt
//! token boundaries. [`SimpleTokenizer`] itself uses [`str::chars`]
//! internally, so the boundaries it emits are always valid UTF-8 char
//! boundaries and the borrowed token slices are always well-formed
//! `&str` values. This wrapper adds no arithmetic of its own, so the
//! pack does not introduce a new opportunity to get byte / char math
//! wrong.
//!
//! # The ech-yiwn ligature
//!
//! Armenian has a **single-scalar ligature** `և` (U+0587, small
//! ligature ech-yiwn) that spells the conjunction "and" as a single
//! character. It is classified as a lowercase letter (`Ll`) and
//! therefore stays inside tokens. The stopword list carries both `և`
//! and its two-letter spelling `եւ` — either form matches.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Armenian has rich inflectional
//!   morphology — 7 cases, plural markers, and a rich verb
//!   inflection paradigm all show up as suffixes on the surface form.
//!   Splitting these at the tokenizer would be wrong for IR indexing;
//!   the fused surface word is the token, and the Armenian stemmer
//!   handles suffix stripping. See [`crate::stemmer`].
//! - **Compound splitting.** Armenian forms compounds (both hyphenated
//!   with `֊` and continuous). The Armenian hyphen `֊` splits under
//!   the default rule; continuous compounds require a lexicon and are
//!   deferred.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Armenian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Armenian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_hy::ArmenianTokenizer;
///
/// let toks: Vec<&str> = ArmenianTokenizer::new()
///     .tokenize("Բարև, աշխարհ։ Երևանը՝ մայրաքաղաքն է։")
///     .collect();
/// assert_eq!(
///     toks,
///     ["Բարև", "աշխարհ", "Երևանը", "մայրաքաղաքն", "է"]
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArmenianTokenizer;

impl ArmenianTokenizer {
    /// Constructs a new [`ArmenianTokenizer`]. Zero-sized; free to
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
        ArmenianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_an_armenian_sentence() {
        assert_eq!(collect("Կատուն քնում է։"), ["Կատուն", "քնում", "է"]);
    }

    #[test]
    fn armenian_letters_stay_inside_tokens() {
        // Every letter in the modern Armenian alphabet is alphabetic
        // under Unicode's classification and stays with its word.
        assert_eq!(collect("հայերեն"), ["հայերեն"]);
        assert_eq!(collect("Երևան"), ["Երևան"]);
        assert_eq!(collect("Հայաստան"), ["Հայաստան"]);
    }

    #[test]
    fn ech_yiwn_ligature_stays_inside_tokens() {
        // `և` (U+0587) is a lowercase letter under Unicode's
        // classification and stays with its word.
        assert_eq!(collect("Երևան"), ["Երևան"]);
        // Bare `և` is itself a single-letter token.
        assert_eq!(collect("սեր և կյանք"), ["սեր", "և", "կյանք"]);
    }

    #[test]
    fn armenian_full_stop_is_a_separator() {
        // `։` (U+0589 ARMENIAN FULL STOP) is `Po` and splits.
        assert_eq!(collect("այո։ ոչ։"), ["այո", "ոչ"]);
    }

    #[test]
    fn armenian_comma_is_a_separator() {
        // `՝` (U+055D ARMENIAN COMMA) is `Po` and splits.
        assert_eq!(collect("մեկ՝ երկու՝ երեք"), ["մեկ", "երկու", "երեք"]);
    }

    #[test]
    fn armenian_question_mark_is_a_separator() {
        // `՞` (U+055E ARMENIAN QUESTION MARK) is `Po` and splits.
        assert_eq!(collect("Ինչ՞ ես անում"), ["Ինչ", "ես", "անում"]);
    }

    #[test]
    fn armenian_exclamation_is_a_separator() {
        // `՜` (U+055C ARMENIAN EXCLAMATION MARK) is `Po` and splits.
        assert_eq!(collect("Բարի՜ եկար"), ["Բարի", "եկար"]);
    }

    #[test]
    fn armenian_hyphen_is_a_separator() {
        // `֊` (U+058A ARMENIAN HYPHEN) is `Pd` and splits.
        assert_eq!(collect("տուն֊տեղ"), ["տուն", "տեղ"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("տարի 2026 ամիս"), ["տարի", "2026", "ամիս"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Բարև աշխարհ";
        let toks: Vec<&str> = ArmenianTokenizer::new().tokenize(text).collect();
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
