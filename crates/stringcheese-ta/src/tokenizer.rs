//! [`TamilTokenizer`] — a Tamil-aware whitespace-and-punctuation word
//! splitter.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for the base Tamil letters (they
//! satisfy `is_alphanumeric`), but Tamil text carries **Tamil
//! combining marks** — dependent vowel signs (matras like `ா`
//! U+0BBE, `ி` U+0BBF, `ு` U+0BC1, …) and the **pulli** `்`
//! U+0BCD (Tamil's virama, which suppresses the inherent vowel and
//! is used to build every consonant cluster) — that are Unicode
//! Mark characters. `char::is_alphanumeric` returns `false` for
//! combining marks, so the default splitter would shatter a word
//! like `நான்` (`ந` `ா` `ன` `்` — 4 scalars) at every matra and
//! at the pulli.
//!
//! # Rule
//!
//! A [`TamilTokenizer`] token is a maximal contiguous run of scalars
//! for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Tamil block U+0B80..=U+0BFF.
//!
//! The Tamil-block clause captures every combining sign inside a
//! Tamil word (matras and pulli) without opening the door to
//! combining marks in other scripts. Latin letters and digits still
//! count as word characters under the standard `is_alphanumeric`
//! rule, so mixed-script input tokenizes as expected.
//!
//! Every other scalar (whitespace, ASCII punctuation) is a separator
//! that ends the current token and never appears in the output.
//!
//! # Byte-vs-char safety
//!
//! Every Tamil scalar (U+0B80..=U+0BFF) is encoded as **three UTF-8
//! bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Tamil word is
//! roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.
//!
//! # No conjunct consonants — every cluster uses an explicit pulli
//!
//! Unlike Devanagari (which forms conjunct ligatures via a virama
//! that then visually disappears), Tamil writes **every** consonant
//! cluster with a *visible* pulli `்` U+0BCD. So `நன்றி` "thanks"
//! is `ந + ன + ் + ற + ி` (5 scalars) and the pulli is a genuine
//! word-internal mark. The tokenizer must keep it inside the word.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Tamil is agglutinative — nouns
//!   and verbs stack case, tense, person, and honorific suffixes onto
//!   a single orthographic word. Splitting fused endings at the
//!   tokenizer would be wrong for IR indexing; the light stemmer
//!   (see [`crate::stemmer`]) handles the endings that *are* fused.
//! - **Sentence-level segmentation.** The tokenizer emits word
//!   tokens, not sentences. Tamil uses ASCII `.`, `?`, `!` as
//!   sentence terminators; callers that want sentence segmentation
//!   should split on those themselves.

/// The Tamil word tokenizer.
///
/// A zero-sized value; construct as [`TamilTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_ta::TamilTokenizer;
///
/// let toks: Vec<&str> = TamilTokenizer::new()
///     .tokenize("நான் தமிழ் பேசுகிறேன்.")
///     .collect();
/// // The pulli and matras stay word-internal; `.` is a separator.
/// assert_eq!(toks, ["நான்", "தமிழ்", "பேசுகிறேன்"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TamilTokenizer;

impl TamilTokenizer {
    /// Constructs a new [`TamilTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed. Every emitted token is a
    /// maximal run of "word" scalars — [`char::is_alphanumeric`] *or*
    /// a Tamil-block scalar (U+0B80..=U+0BFF, which covers the
    /// combining matras and the pulli that are not `is_alphanumeric`
    /// on their own).
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> TamilTokens<'a> {
        TamilTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Tamil string, produced by
/// [`TamilTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct TamilTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Tamil tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for every
/// scalar in the Tamil block U+0B80..=U+0BFF (the combining matras
/// and the pulli `்` are `false` under `is_alphanumeric` on their own
/// but must stay word-internal in Tamil).
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    c.is_alphanumeric() || ('\u{0B80}'..='\u{0BFF}').contains(&c)
}

impl<'a> Iterator for TamilTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. We advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary, guaranteed even though Tamil scalars are
        // 3 bytes each.
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
        TamilTokenizer::new().tokenize(input).collect()
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
    fn splits_a_tamil_sentence_on_ascii_space() {
        assert_eq!(collect("நான் தமிழ் பேசுகிறேன்"), ["நான்", "தமிழ்", "பேசுகிறேன்"]);
    }

    #[test]
    fn matras_stay_word_internal() {
        // தமிழ் = த + ம + ி + ழ + ் (5 scalars); matras and pulli stay.
        assert_eq!(collect("தமிழ்"), ["தமிழ்"]);
        // நான் = ந + ா + ன + ் (4 scalars).
        assert_eq!(collect("நான்"), ["நான்"]);
    }

    #[test]
    fn pulli_stays_word_internal() {
        // Every Tamil consonant cluster uses a visible pulli.
        // நன்றி = ந + ன + ் + ற + ி (5 scalars).
        assert_eq!(collect("நன்றி"), ["நன்றி"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("நான், நீ, அவன்!"), ["நான்", "நீ", "அவன்"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("ஆண்டு 2026"), ["ஆண்டு", "2026"]);
        // Tamil digits (U+0BE6..=U+0BEF) are in the Tamil block —
        // treated as word scalars.
        assert_eq!(collect("ஆண்டு ௨௦௨௬"), ["ஆண்டு", "௨௦௨௬"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello தமிழ் world"), ["hello", "தமிழ்", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "நான் தமிழ்";
        let toks: Vec<&str> = TamilTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Tamil scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
