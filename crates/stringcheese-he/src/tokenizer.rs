//! [`HebrewTokenizer`] — a maqaf-aware whitespace/punctuation splitter
//! for Hebrew text.
//!
//! # Why not the default tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) treats any
//! character where [`char::is_alphanumeric`] returns `false` as a
//! separator. That works for most of Hebrew — every Hebrew letter
//! (U+05D0..=U+05EA, including the five final forms) is
//! `is_alphanumeric` under Unicode — but it *breaks compound words*
//! joined by the maqaf.
//!
//! The maqaf (`־` U+05BE) is a Hebrew hyphen used to join words that
//! function as a single lexical unit — the canonical example is
//! `בית־ספר` (literally "house-of-book", i.e. "school"). Under Unicode
//! the maqaf is classified as `Pd` (Punctuation, Dash), which means
//! `is_alphanumeric` returns `false` for it and the default tokenizer
//! would split `בית־ספר` into `["בית", "ספר"]` — losing the compound.
//!
//! [`HebrewTokenizer`] extends the classifier: a character is a "word
//! character" if [`char::is_alphanumeric`] returns `true` *or* the
//! character is the maqaf. Every other separator (whitespace, Hebrew
//! punctuation `׳ ״ ׃ ׀`, ASCII punctuation) still splits.
//!
//! # When is *this* not enough?
//!
//! - **Niqqud carried through.** Combining niqqud marks
//!   (U+05B0..=U+05BD, etc.) carry the Unicode `Other_Alphabetic`
//!   property (see `DerivedCoreProperties.txt`), so
//!   [`char::is_alphanumeric`] returns `true` for them and they stay
//!   attached to their host letter inside a token. That's the right
//!   behavior for pointed text — the vowel points remain in the
//!   token. Callers who want the *unpointed* surface form should run
//!   [`crate::normalize::normalize`] before *or* after tokenizing;
//!   both orderings produce the same token count.
//! - **Attached-particle segmentation.** Hebrew fuses the definite
//!   article (`ה-`) and single-letter coordinating particles (`ו-`
//!   "and", `ב-` "in", `כ-` "like", `ל-` "to", `מ-` "from", `ש-`
//!   "that") to the following word: `והספר` "and-the-book" is one
//!   orthographic word but three morphemes. The
//!   [`crate::stemmer::LightHebrewStemmer`] strips these prefixes off
//!   surface forms; this tokenizer does not split them into separate
//!   tokens. If per-morpheme tokens are needed, apply the stemmer to
//!   each emitted token or wire in a dedicated segmenter (`HebMorph`,
//!   `MILA`, `YAP`).
//! - **Geresh / gershayim inside acronyms.** `ד״ר` "Dr." carries a
//!   gershayim (`״` U+05F4) as a word-internal character; this
//!   tokenizer will split on it because U+05F4 is Unicode punctuation
//!   (`Po`, not `Pd`, so the maqaf carve-out doesn't apply). A
//!   downstream pipeline that wants the acronym intact should
//!   pre-strip gershayim via
//!   [`HebrewNormalizer::with_strip_hebrew_punctuation`](crate::normalize::HebrewNormalizer::with_strip_hebrew_punctuation).
//!
//! # RTL note
//!
//! The tokenizer operates on **logical** (UTF-8 byte) order. Because
//! spaces and maqafs are byte-neutral (the UTF-8 encoding is
//! independent of surrounding script), the split points are the same
//! in logical and display order: what you see between two visible
//! Hebrew words is the same space the tokenizer splits on. Emitted
//! tokens are borrowed slices of the input and preserve the input's
//! byte order, which is first-consonant-first (the RTL renderer
//! displays them right-to-left; the tokenizer neither knows nor cares).

/// The Hebrew word tokenizer.
///
/// A maqaf-aware whitespace/punctuation splitter — compound words like
/// `בית־ספר` "school" stay as a single token. See the
/// [module-level docs](self) for the classifier and the discussion of
/// when a caller might want something smarter.
///
/// A zero-sized value; construct as [`HebrewTokenizer`] and reuse
/// across threads and calls.
///
/// # Example
///
/// ```
/// use stringcheese_he::HebrewTokenizer;
///
/// // Compound word `בית־ספר` stays as one token.
/// let toks: Vec<&str> = HebrewTokenizer::new()
///     .tokenize("אני לומד בבית־ספר")
///     .collect();
/// assert_eq!(toks, ["אני", "לומד", "בבית־ספר"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HebrewTokenizer;

impl HebrewTokenizer {
    /// Constructs a new [`HebrewTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into words on whitespace and punctuation, treating
    /// maqaf (U+05BE) as a word-internal joiner so compound words
    /// stay as a single token.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> HebrewTokens<'a> {
        HebrewTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a string, produced by
/// [`HebrewTokenizer::tokenize`].
///
/// Yields borrowed `&'a str` slices of the input; no allocation is
/// performed.
#[derive(Clone, Debug)]
pub struct HebrewTokens<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Iterator for HebrewTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip any leading separators.
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if is_hebrew_word_char(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }

        if self.offset >= bytes.len() {
            return None;
        }

        // Consume the run of word characters.
        let start = self.offset;
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if !is_hebrew_word_char(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }

        Some(&self.text[start..self.offset])
    }
}

/// Is `c` a "word character" under the Hebrew tokenizer's classifier?
///
/// Returns `true` for any character where [`char::is_alphanumeric`] is
/// true (every Hebrew letter, every ASCII letter, every digit, every
/// Latin letter …) *or* for the maqaf (U+05BE) — the Hebrew compound-
/// word joiner.
#[inline]
#[must_use]
pub fn is_hebrew_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\u{05BE}'
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        HebrewTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
    }

    #[test]
    fn splits_hebrew_sentence_on_ascii_space() {
        assert_eq!(collect("אני אוהב ספרים"), ["אני", "אוהב", "ספרים"]);
    }

    #[test]
    fn splits_on_ascii_punctuation() {
        assert_eq!(collect("שלום, עולם!"), ["שלום", "עולם"]);
    }

    #[test]
    fn maqaf_stays_word_internal() {
        // The canonical compound: בית־ספר "school".
        assert_eq!(collect("בית־ספר"), ["בית־ספר"]);
    }

    #[test]
    fn maqaf_inside_prefixed_compound() {
        // A prefix particle before a compound word: בבית־ספר "in a school".
        assert_eq!(collect("אני לומד בבית־ספר"), ["אני", "לומד", "בבית־ספר"]);
    }

    #[test]
    fn multiple_maqafs_in_one_compound() {
        // Three-morpheme compound.
        assert_eq!(collect("בן־אדם־אחד"), ["בן־אדם־אחד"]);
    }

    #[test]
    fn attached_particles_stay_with_their_word() {
        // והספר is one orthographic word ("and-the-book") and stays
        // as one token; morpheme splitting is the stemmer's job.
        assert_eq!(collect("והספר טוב"), ["והספר", "טוב"]);
    }

    #[test]
    fn gershayim_splits_the_token() {
        // U+05F4 GERSHAYIM is not the maqaf, so it splits.
        // A caller who wants ד״ר as one token should strip gershayim
        // before tokenizing.
        assert_eq!(collect("ד״ר"), ["ד", "ר"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "אני ואתה";
        let toks: Vec<&str> = HebrewTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }

    #[test]
    fn is_word_char_covers_hebrew_letters_and_maqaf() {
        // Sample of Hebrew letters, including final forms.
        for c in ['א', 'ב', 'י', 'ת', 'ך', 'ם', 'ן', 'ף', 'ץ'] {
            assert!(is_hebrew_word_char(c), "{c:?} should be a word char");
        }
        // Maqaf.
        assert!(is_hebrew_word_char('\u{05BE}'));
        // ASCII letters and digits too.
        assert!(is_hebrew_word_char('a'));
        assert!(is_hebrew_word_char('7'));
        // Space, geresh, gershayim all fail.
        assert!(!is_hebrew_word_char(' '));
        assert!(!is_hebrew_word_char('\u{05F3}'));
        assert!(!is_hebrew_word_char('\u{05F4}'));
        // Niqqud combining marks (patah, dagesh) carry
        // `Other_Alphabetic` per Unicode's `DerivedCoreProperties`,
        // so `char::is_alphanumeric` returns `true` for them and they
        // stay attached to their host letter inside a token. See the
        // module-level docs.
        assert!(is_hebrew_word_char('\u{05B7}'));
    }
}
