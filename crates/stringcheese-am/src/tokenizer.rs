//! [`AmharicTokenizer`] — a Ge'ez-aware whitespace-and-punctuation
//! word splitter.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for most Ge'ez syllables (they satisfy
//! `is_alphanumeric` under Unicode's `Lo` "Letter, other" category),
//! but Ge'ez text carries **two Ge'ez-specific punctuation scalars**
//! that need explicit handling:
//!
//! * **`፡` (U+1361 ETHIOPIC WORDSPACE).** The traditional Ge'ez
//!   word separator — used the same way Latin-script languages use
//!   ASCII space. Any tokenizer that expects ASCII space to
//!   separate words will merge two Amharic words joined by wordspace
//!   into a single token.
//! * **`።` (U+1362 ETHIOPIC FULL STOP).** The Ge'ez sentence
//!   terminator. Under Unicode both scalars are `Po` "Punctuation,
//!   other" (they satisfy `!is_alphanumeric`), so the default
//!   splitter would in principle *already* treat them as separators
//!   — but historically the U+1361 wordspace has been miscoded as
//!   a letter in some fonts / tools, and callers appreciate the
//!   explicit contract.
//!
//! Additional Ge'ez punctuation (U+1363..=U+1368) — comma,
//! semicolon, colon, preface colon, question mark, paragraph
//! separator — is also outside the alphanumeric range and separates
//! tokens under the default rule; the tokenizer treats every scalar
//! in U+1361..=U+1368 as a separator explicitly.
//!
//! # Rule
//!
//! An [`AmharicTokenizer`] token is a maximal contiguous run of
//! scalars for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Ge'ez main block U+1200..=U+137F but
//!    *not* in the punctuation range U+1361..=U+1368, **or**
//! 3. the scalar is in the Ge'ez supplement U+1380..=U+139F, **or**
//! 4. the scalar is in the Ge'ez extended block U+2D80..=U+2DDF.
//!
//! Every other scalar (whitespace, ASCII punctuation, and the
//! Ge'ez-specific punctuation U+1361..=U+1368) is a separator that
//! ends the current token and never appears in the output.
//!
//! # Byte-vs-char safety
//!
//! Every Ge'ez main-block scalar (U+1200..=U+137F) is encoded as
//! **three UTF-8 bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Ge'ez word is
//! roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.

use crate::geez::{is_geez_extended, is_geez_main, is_geez_supplement};

/// The Amharic word tokenizer.
///
/// A zero-sized value; construct as [`AmharicTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for
/// the rule.
///
/// # Example
///
/// ```
/// use stringcheese_am::AmharicTokenizer;
///
/// // The Ge'ez wordspace ፡ and full stop ። are separators; Ge'ez
/// // letters stay word-internal.
/// let toks: Vec<&str> = AmharicTokenizer::new()
///     .tokenize("እኔ፡አማርኛ፡እወዳለሁ።")
///     .collect();
/// assert_eq!(toks, ["እኔ", "አማርኛ", "እወዳለሁ"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AmharicTokenizer;

impl AmharicTokenizer {
    /// Constructs a new [`AmharicTokenizer`]. Zero-sized; free to
    /// call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed. Every emitted token is a
    /// maximal run of "word" scalars under the classifier
    /// [`is_word_scalar`].
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> AmharicTokens<'a> {
        AmharicTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of an Amharic string, produced by
/// [`AmharicTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices
/// of it — no allocation is performed. Held state is a single
/// `usize` byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct AmharicTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Amharic tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars, for every
/// Ge'ez main-block scalar (U+1200..=U+137F) *except* the Ge'ez
/// punctuation range U+1361..=U+1368, for every Ge'ez supplement
/// scalar (U+1380..=U+139F), and for every Ge'ez extended scalar
/// (U+2D80..=U+2DDF).
///
/// The Ge'ez wordspace `፡` (U+1361), full stop `።` (U+1362), comma
/// `፣` (U+1363), semicolon `፤` (U+1364), colon `፥` (U+1365), preface
/// colon `፦` (U+1366), question mark `፧` (U+1367), and paragraph
/// separator `፨` (U+1368) are all explicitly *excluded* — they are
/// separators.
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    let cp = c as u32;
    if (0x1361..=0x1368).contains(&cp) {
        // Ge'ez punctuation — explicit separators.
        return false;
    }
    if is_geez_main(c) || is_geez_supplement(c) || is_geez_extended(c) {
        return true;
    }
    c.is_alphanumeric()
}

impl<'a> Iterator for AmharicTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. Advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary even though Ge'ez scalars are 3 bytes each.
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if is_word_scalar(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }

        if self.offset >= bytes.len() {
            return None;
        }

        // Consume the maximal run of word scalars.
        let start = self.offset;
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if !is_word_scalar(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }

        Some(&self.text[start..self.offset])
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        AmharicTokenizer::new().tokenize(input).collect()
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
    fn splits_amharic_sentence_on_ascii_space() {
        assert_eq!(collect("እኔ አማርኛ እወዳለሁ"), ["እኔ", "አማርኛ", "እወዳለሁ"]);
    }

    #[test]
    fn splits_on_ethiopic_wordspace() {
        // ፡ U+1361 — traditional Ge'ez word separator.
        assert_eq!(collect("እኔ፡አማርኛ"), ["እኔ", "አማርኛ"]);
    }

    #[test]
    fn splits_on_ethiopic_full_stop() {
        // ። U+1362 — sentence terminator.
        assert_eq!(collect("እኔ አማርኛ እወዳለሁ።"), ["እኔ", "አማርኛ", "እወዳለሁ"]);
    }

    #[test]
    fn splits_on_mixed_wordspace_and_full_stop() {
        assert_eq!(
            collect("እኔ፡አማርኛ፡እወዳለሁ።ግን፡ትንሽ፡እችላለሁ።"),
            ["እኔ", "አማርኛ", "እወዳለሁ", "ግን", "ትንሽ", "እችላለሁ"]
        );
    }

    #[test]
    fn geez_scalars_stay_word_internal() {
        // A multi-syllable Amharic word. Every scalar is a Ge'ez
        // syllable and must stay together.
        assert_eq!(collect("አማርኛ"), ["አማርኛ"]);
        assert_eq!(collect("ኢትዮጵያ"), ["ኢትዮጵያ"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("እኔ, አንተ, እሱ!"), ["እኔ", "አንተ", "እሱ"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("ዓመት 2026"), ["ዓመት", "2026"]);
        // Ethiopic digits (U+1369..=U+137C) fall inside the main
        // Ge'ez block, so they count as word scalars.
        assert_eq!(collect("ዓመት ፲፱፻"), ["ዓመት", "፲፱፻"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello አማርኛ world"), ["hello", "አማርኛ", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "እኔ አማርኛ";
        let toks: Vec<&str> = AmharicTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Ge'ez scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }

    #[test]
    fn geez_comma_and_semicolon_are_separators() {
        // ፣ U+1363 comma, ፤ U+1364 semicolon.
        assert_eq!(collect("እኔ፣አንተ፤እሱ"), ["እኔ", "አንተ", "እሱ"]);
    }

    #[test]
    fn is_word_scalar_covers_geez_letters_and_excludes_punctuation() {
        // Ge'ez letters.
        assert!(is_word_scalar('ሀ'));
        assert!(is_word_scalar('አ'));
        assert!(is_word_scalar('ዘ'));
        // Ge'ez punctuation.
        assert!(!is_word_scalar('\u{1361}'));
        assert!(!is_word_scalar('\u{1362}'));
        assert!(!is_word_scalar('\u{1368}'));
        // ASCII.
        assert!(is_word_scalar('a'));
        assert!(is_word_scalar('7'));
        assert!(!is_word_scalar(' '));
        assert!(!is_word_scalar(','));
    }
}
