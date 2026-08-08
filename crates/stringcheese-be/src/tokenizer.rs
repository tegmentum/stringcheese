//! [`BelarusianTokenizer`] — an apostrophe-aware whitespace-and-
//! punctuation word splitter.
//!
//! # Why not the default splitter?
//!
//! Belarusian orthography uses the ASCII apostrophe **`'` (U+0027)** as
//! a **word-internal** separator marking the boundary between a hard
//! consonant and a following iotated vowel — words like `сям'я`
//! (family), `аб'ект` (object), `пад'езд` (entrance) all carry the
//! apostrophe as part of the surface form of a single lexeme. The
//! workspace's default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) treats every
//! non-alphanumeric character (including the apostrophe) as a
//! separator, so it splits `сям'я` into `["сям", "я"]` — wrong for
//! Belarusian.
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
//! Every Cyrillic scalar in the Belarusian block is encoded as **two
//! UTF-8 bytes** (U+0400..=U+04FF and the U+045E/U+040E `ў`/`Ў` pair
//! all fall in the 2-byte range U+0080..=U+07FF). This means the byte
//! length of a Belarusian word is roughly `2 * char_count`, and any
//! code that mixes byte offsets with character-boundary logic will
//! silently corrupt token boundaries. The iterator below uses
//! [`str::char_indices`] exclusively — every offset it emits is a
//! valid UTF-8 char boundary, and the borrowed slices it hands back
//! are always well-formed `&str` values.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Belarusian has rich inflectional
//!   morphology; splitting suffixes at the tokenizer would be wrong
//!   for IR indexing (the fused surface form is the token). Suffix
//!   stripping lives in [`crate::stemmer`].
//! - **Hyphenated compounds.** Some Belarusian orthography uses hyphens
//!   as clitics (`штосьці`, `хтосьці`, `будзь-які`). The tokenizer
//!   treats ASCII `-` as punctuation and splits on it, matching the
//!   shipped stopword list's treatment. Downstream systems that want
//!   to preserve the hyphenated form should apply a re-glue pass or
//!   feed the pack a pre-normalized input.
//! - **Typographic apostrophe (U+2019).** The Belarusian keyboard-layer
//!   default types the ASCII `'` (U+0027); some typographic pipelines
//!   substitute the right-single-quotation-mark U+2019. The tokenizer
//!   promotes only the ASCII apostrophe. Callers who need the
//!   typographic variant should pre-normalize with a NFC pass or a
//!   character-map before calling the tokenizer.

/// The Belarusian tokenizer.
///
/// A zero-sized value. See the [module-level docs](self) for the
/// tokenization rule (apostrophe promoted to word-internal when
/// between alphanumerics; every other non-alphanumeric character is a
/// separator).
///
/// # Example
///
/// ```
/// use stringcheese_be::BelarusianTokenizer;
///
/// // Apostrophe stays with its word.
/// let toks: Vec<&str> = BelarusianTokenizer::new()
///     .tokenize("Сям'я, аб'ект.")
///     .collect();
/// assert_eq!(toks, ["Сям'я", "аб'ект"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BelarusianTokenizer;

impl BelarusianTokenizer {
    /// Constructs a new [`BelarusianTokenizer`]. Zero-sized; free to
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
    pub fn tokenize<'a>(&self, text: &'a str) -> BelarusianTokens<'a> {
        BelarusianTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a string, produced by
/// [`BelarusianTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices
/// of it — no allocation is performed. Held state is a single `usize`
/// byte offset.
#[derive(Clone, Debug)]
pub struct BelarusianTokens<'a> {
    text: &'a str,
    offset: usize,
}

impl BelarusianTokens<'_> {
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

impl<'a> Iterator for BelarusianTokens<'a> {
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
        BelarusianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_belarusian_sentence() {
        assert_eq!(collect("Кот спіць."), ["Кот", "спіць"]);
    }

    #[test]
    fn cyrillic_letters_stay_inside_tokens() {
        // Every letter in the Belarusian alphabet is alphabetic under
        // Unicode's classification and stays with its word.
        assert_eq!(collect("беларускі"), ["беларускі"]);
        assert_eq!(collect("Мінск"), ["Мінск"]);
        assert_eq!(collect("праўда"), ["праўда"]);
        assert_eq!(collect("яшчэ"), ["яшчэ"]);
    }

    #[test]
    fn word_internal_apostrophe_is_preserved() {
        // The signature Belarusian case: apostrophe between a hard
        // consonant and an iotated vowel is part of the word.
        assert_eq!(collect("сям'я"), ["сям'я"]);
        assert_eq!(collect("аб'ект"), ["аб'ект"]);
        assert_eq!(collect("пад'езд"), ["пад'езд"]);
    }

    #[test]
    fn leading_or_trailing_apostrophe_is_a_separator() {
        // Apostrophe outside a word (leading or trailing) is not
        // word-internal — it is dropped as a separator.
        assert_eq!(collect("'прывітанне"), ["прывітанне"]);
        assert_eq!(collect("прывітанне'"), ["прывітанне"]);
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
    fn short_u_stays_inside_tokens() {
        // Belarusian-specific: ў (U+045E) is alphabetic under Unicode
        // and stays inside its word.
        assert_eq!(collect("праўда"), ["праўда"]);
        assert_eq!(collect("Аўтар"), ["Аўтар"]);
    }

    #[test]
    fn dashes_and_em_dashes_are_separators() {
        // ASCII hyphens split (matching the stopword list's treatment
        // of hyphenated compounds).
        assert_eq!(collect("будзь-які"), ["будзь", "які"]);
        // U+2014 EM DASH splits, matching general Unicode punctuation.
        assert_eq!(collect("Мінск — сталіца"), ["Мінск", "сталіца"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("год 2026 месяц"), ["год", "2026", "месяц"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Прывітанне свет";
        let toks: Vec<&str> = BelarusianTokenizer::new().tokenize(text).collect();
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
