//! [`UkrainianTokenizer`] — an apostrophe-aware whitespace-and-
//! punctuation word splitter.
//!
//! # Why not the default splitter?
//!
//! Ukrainian orthography uses the ASCII apostrophe **`'` (U+0027)** as
//! a **word-internal** separator marking the boundary between a hard
//! consonant and a following iotated vowel — words like `сім'я`
//! (family), `п'ять` (five), `м'який` (soft), `об'єкт` (object) all
//! carry the apostrophe as part of the surface form of a single
//! lexeme. The workspace's default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) treats every
//! non-alphanumeric character (including the apostrophe) as a
//! separator, so it splits `сім'я` into `["сім", "я"]` — wrong for
//! Ukrainian.
//!
//! This tokenizer promotes the ASCII apostrophe to a **word-internal
//! character** when it sits between two alphanumeric scalars. Every
//! other separator behaves the same way as
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) — whitespace,
//! Unicode punctuation, hyphens, quotes, control characters all end
//! the current token and never appear in output slices.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the Ukrainian block is encoded as **two
//! UTF-8 bytes** (U+0400..=U+04FF and the U+0490/U+0491 `Ґ`/`ґ` pair
//! all fall in the 2-byte range U+0080..=U+07FF). This means the byte
//! length of a Ukrainian word is roughly `2 * char_count`, and any
//! code that mixes byte offsets with character-boundary logic will
//! silently corrupt token boundaries. The iterator below uses
//! [`str::char_indices`] exclusively — every offset it emits is a
//! valid UTF-8 char boundary, and the borrowed slices it hands back
//! are always well-formed `&str` values.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Ukrainian has rich inflectional
//!   morphology; splitting suffixes at the tokenizer would be wrong
//!   for IR indexing (the fused surface form is the token). Suffix
//!   stripping lives in [`crate::snowball`].
//! - **Hyphenated compounds.** Some Ukrainian orthography uses hyphens
//!   as clitics (`будь-який`, `хто-небудь`). The tokenizer treats
//!   ASCII `-` as punctuation and splits on it, matching the shipped
//!   stopword list's treatment. Downstream systems that want to
//!   preserve the hyphenated form should apply a re-glue pass or feed
//!   the pack a pre-normalized input.
//! - **Typographic apostrophe (U+2019).** The Ukrainian keyboard-layer
//!   default types the ASCII `'` (U+0027); some typographic pipelines
//!   substitute the right-single-quotation-mark U+2019. The tokenizer
//!   promotes only the ASCII apostrophe. Callers who need the
//!   typographic variant should pre-normalize with a NFC pass or a
//!   character-map before calling the tokenizer.

/// The Ukrainian tokenizer.
///
/// A zero-sized value. See the [module-level docs](self) for the
/// tokenization rule (apostrophe promoted to word-internal when
/// between alphanumerics; every other non-alphanumeric character is a
/// separator).
///
/// # Example
///
/// ```
/// use stringcheese_uk::UkrainianTokenizer;
///
/// // Apostrophe stays with its word.
/// let toks: Vec<&str> = UkrainianTokenizer::new()
///     .tokenize("Сім'я, п'ять хвилин.")
///     .collect();
/// assert_eq!(toks, ["Сім'я", "п'ять", "хвилин"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UkrainianTokenizer;

impl UkrainianTokenizer {
    /// Constructs a new [`UkrainianTokenizer`]. Zero-sized; free to
    /// call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into apostrophe-aware whitespace-and-punctuation-
    /// delimited tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> UkrainianTokens<'a> {
        UkrainianTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a string, produced by
/// [`UkrainianTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices
/// of it — no allocation is performed. Held state is a single `usize`
/// byte offset.
#[derive(Clone, Debug)]
pub struct UkrainianTokens<'a> {
    text: &'a str,
    offset: usize,
}

impl UkrainianTokens<'_> {
    /// Is the ASCII apostrophe (U+0027) at char position `offset` a
    /// word-internal character?
    ///
    /// Returns `true` iff the previous scalar (walked from
    /// `self.text[..offset]`) is alphanumeric AND the next scalar
    /// (walked from `self.text[offset + 1..]`) is alphanumeric.
    /// Uses [`str::chars`] and [`str::char_indices`] iteration — no
    /// byte offsets are dereferenced past `offset` before checking.
    fn apostrophe_is_word_internal(&self, offset: usize) -> bool {
        // Previous scalar — walk backwards using char_indices.
        let prev_is_alnum = self.text[..offset]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric);
        if !prev_is_alnum {
            return false;
        }
        // Next scalar — skip past the apostrophe (1 byte, ASCII) and
        // peek the first character.
        let next_is_alnum = self.text[offset + 1..]
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric);
        prev_is_alnum && next_is_alnum
    }
}

impl<'a> Iterator for UkrainianTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip any leading separators. An apostrophe leading a
        // potential token is never word-internal (there is no
        // preceding alphanumeric), so it always counts as a
        // separator here.
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if ch.is_alphanumeric() {
                break;
            }
            self.offset += ch.len_utf8();
        }

        if self.offset >= bytes.len() {
            return None;
        }

        // Consume the run — alphanumerics, plus word-internal
        // apostrophes.
        let start = self.offset;
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if ch.is_alphanumeric() {
                self.offset += ch.len_utf8();
                continue;
            }
            if ch == '\'' && self.apostrophe_is_word_internal(self.offset) {
                // ASCII apostrophe is 1 byte; safe to advance by 1
                // (also equals ch.len_utf8()).
                self.offset += ch.len_utf8();
                continue;
            }
            break;
        }

        Some(&self.text[start..self.offset])
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        UkrainianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_ukrainian_sentence() {
        assert_eq!(collect("Кіт спить."), ["Кіт", "спить"]);
    }

    #[test]
    fn cyrillic_letters_stay_inside_tokens() {
        // Every letter in the Ukrainian alphabet is alphabetic under
        // Unicode's classification and stays with its word.
        assert_eq!(collect("український"), ["український"]);
        assert_eq!(collect("Ґудзик"), ["Ґудзик"]);
        assert_eq!(collect("їжак"), ["їжак"]);
        assert_eq!(collect("щастя"), ["щастя"]);
    }

    #[test]
    fn word_internal_apostrophe_is_preserved() {
        // The signature Ukrainian case: apostrophe between a hard
        // consonant and an iotated vowel is part of the word.
        assert_eq!(collect("сім'я"), ["сім'я"]);
        assert_eq!(collect("п'ять"), ["п'ять"]);
        assert_eq!(collect("м'який"), ["м'який"]);
        assert_eq!(collect("об'єкт"), ["об'єкт"]);
    }

    #[test]
    fn leading_or_trailing_apostrophe_is_a_separator() {
        // Apostrophe outside a word (leading or trailing) is not
        // word-internal — it is dropped as a separator.
        assert_eq!(collect("'привіт"), ["привіт"]);
        assert_eq!(collect("привіт'"), ["привіт"]);
        assert_eq!(collect("''"), Vec::<&str>::new());
    }

    #[test]
    fn multiple_apostrophes_word_internally_are_preserved() {
        // Extremely contrived, but the rule is compositional: as long
        // as every apostrophe sits between two alphanumerics, they all
        // stay with the token.
        assert_eq!(collect("a'b'c"), ["a'b'c"]);
    }

    #[test]
    fn dashes_and_em_dashes_are_separators() {
        // ASCII hyphens split (matching the stopword list's treatment
        // of hyphenated compounds).
        assert_eq!(collect("будь-який"), ["будь", "який"]);
        // U+2014 EM DASH splits, matching general Unicode punctuation.
        assert_eq!(collect("Київ — столиця"), ["Київ", "столиця"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("рік 2026 місяць"), ["рік", "2026", "місяць"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Привіт світ";
        let toks: Vec<&str> = UkrainianTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. This is the key safety property of the
        // char_indices-based splitter — no byte offset can slice a
        // multi-byte Cyrillic scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
