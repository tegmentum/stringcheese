//! [`PersianTokenizer`] — a ZWNJ-aware whitespace-and-punctuation word
//! splitter for Persian text.
//!
//! # Why not the default tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for Modern Standard Arabic (see
//! `stringcheese-ar`), but Persian carries a *word-internal* character
//! the default rule mishandles: the **zero-width non-joiner** (ZWNJ,
//! U+200C).
//!
//! ZWNJ is a Unicode Format (`Cf`) character — `is_alphanumeric`
//! returns `false` for it — but Persian orthography uses it *inside*
//! compound words to prevent adjacent morphemes from ligating while
//! keeping them a single orthographic unit. The classical example is
//! the imperfective marker `می`, which attaches to a verb stem with a
//! ZWNJ in between: `می‌روم` "I go" is one word, spelled
//! `م ی <ZWNJ> ر و م`. The default tokenizer would break that on the
//! ZWNJ and emit `["می", "روم"]`, which is wrong for Persian IR.
//!
//! # Rule
//!
//! A [`PersianTokenizer`] token is a maximal contiguous run of scalars
//! for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is the zero-width non-joiner (U+200C).
//!
//! Every other scalar is a separator that ends the current token and
//! never appears in the output.
//!
//! This gives `می‌روم` back as a single token (the ZWNJ counts as
//! word-internal), while whitespace, punctuation, and other separators
//! still break tokens the way they should.
//!
//! # Interaction with the normalizer
//!
//! If the caller has run [`crate::normalize::PersianNormalizer::with_strip_zwnj`]
//! before tokenizing, the tokenizer sees no ZWNJs at all — every
//! compound word has been fused into a single ligated run — and the
//! `is_alphanumeric` path alone produces the same tokens. The ZWNJ
//! rule below is the *conservative* baseline: preserve ZWNJs and treat
//! them as word-internal, so a caller who never touches the normalizer
//! still gets one-word-per-compound tokens.
//!
//! # RTL note
//!
//! The tokenizer operates on **logical** (UTF-8 byte) order. Because
//! whitespace and ZWNJ are byte-neutral (both are single-scalar
//! sequences regardless of surrounding script), the split points are
//! the same in logical and display order. Emitted tokens are borrowed
//! slices of the input and preserve the input's byte order.

/// The Persian word tokenizer.
///
/// A zero-sized value; construct as [`PersianTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_fa::PersianTokenizer;
///
/// let toks: Vec<&str> = PersianTokenizer::new()
///     .tokenize("محمد کتاب می\u{200C}خواند.")
///     .collect();
/// // `می‌خواند` stays one token — the ZWNJ is word-internal.
/// assert_eq!(toks, ["محمد", "کتاب", "می\u{200C}خواند"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PersianTokenizer;

impl PersianTokenizer {
    /// Constructs a new [`PersianTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into whitespace-and-punctuation-delimited tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> PersianTokens<'a> {
        PersianTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Persian string, produced by
/// [`PersianTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset.
#[derive(Clone, Debug)]
pub struct PersianTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Persian tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for the
/// zero-width non-joiner (U+200C).
#[inline]
#[must_use]
fn is_word_scalar(c: char) -> bool {
    c.is_alphanumeric() || c == '\u{200C}'
}

impl<'a> Iterator for PersianTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip any leading separators.
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

        // Consume the run of word scalars.
        let start = self.offset;
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if !is_word_scalar(ch) {
                break;
            }
            self.offset += ch.len_utf8();
        }

        // Trim trailing ZWNJs — a run that ended on ZWNJ (because the
        // next scalar was a separator) should not carry the dangling
        // joiner into the emitted token. This matches the behaviour of
        // every Persian NLP toolkit that treats ZWNJ as word-internal.
        let mut end = self.offset;
        while end > start {
            let slice = &self.text[start..end];
            if let Some(c) = slice.chars().next_back()
                && c == '\u{200C}'
            {
                end -= c.len_utf8();
                continue;
            }
            break;
        }

        // Also trim leading ZWNJs.
        let mut real_start = start;
        while real_start < end {
            let slice = &self.text[real_start..end];
            if let Some(c) = slice.chars().next()
                && c == '\u{200C}'
            {
                real_start += c.len_utf8();
                continue;
            }
            break;
        }

        if real_start >= end {
            // The run was only ZWNJs — try again from the current
            // offset for the next real token.
            return self.next();
        }

        Some(&self.text[real_start..end])
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        PersianTokenizer::new().tokenize(input).collect()
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
    fn zwnj_only_yields_no_tokens() {
        // A bare ZWNJ is trimmed off — no real token remains.
        assert!(collect("\u{200C}").is_empty());
        assert!(collect("\u{200C}\u{200C}").is_empty());
    }

    #[test]
    fn splits_persian_sentence_on_ascii_space() {
        assert_eq!(collect("محمد کتاب خواند"), ["محمد", "کتاب", "خواند"]);
    }

    #[test]
    fn zwnj_is_word_internal() {
        // می‌روم — the ZWNJ holds the compound together as one token.
        assert_eq!(collect("می\u{200C}روم"), ["می\u{200C}روم"]);
        assert_eq!(collect("محمد می\u{200C}رود"), ["محمد", "می\u{200C}رود"]);
    }

    #[test]
    fn splits_on_arabic_and_persian_punctuation() {
        // Arabic comma (U+060C), Arabic full stop rendered as U+002E.
        assert_eq!(collect("محمد، کتاب خواند."), ["محمد", "کتاب", "خواند"]);
        // Arabic question mark (U+061F).
        assert_eq!(collect("چطوری؟"), ["چطوری"]);
    }

    #[test]
    fn trailing_zwnj_stripped_from_emitted_token() {
        // A word that ends on a ZWNJ (because the next scalar is a
        // separator) should not carry the ZWNJ into the emitted token.
        let input = "کتاب\u{200C} خواند";
        assert_eq!(collect(input), ["کتاب", "خواند"]);
    }

    #[test]
    fn leading_zwnj_stripped_from_emitted_token() {
        // A word that starts with a ZWNJ (because the previous scalar
        // was a separator) should not carry the ZWNJ.
        let input = " \u{200C}کتاب";
        assert_eq!(collect(input), ["کتاب"]);
    }

    #[test]
    fn attached_particles_stay_with_their_word() {
        // Persian object marker `را` attached as a separate word after a
        // space — comes out as a separate token, which is correct.
        assert_eq!(collect("کتاب را خواند"), ["کتاب", "را", "خواند"]);
    }

    #[test]
    fn digits_are_tokens() {
        // Both Western and Persian digits are `is_alphanumeric` under
        // Unicode.
        assert_eq!(collect("سال ۲۰۲۴"), ["سال", "۲۰۲۴"]);
        assert_eq!(collect("سال 2024"), ["سال", "2024"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "محمد کتاب";
        let toks: Vec<&str> = PersianTokenizer::new().tokenize(text).collect();
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }

    #[test]
    fn zwnj_between_words_treated_as_word_internal_but_trimmed() {
        // A ZWNJ sandwiched between letters and a separator is
        // word-internal on the letter side and stripped from the emitted
        // slice on the separator side.
        let input = "کتاب\u{200C}ها";
        assert_eq!(collect(input), ["کتاب\u{200C}ها"]);
    }
}
