//! [`MalayalamTokenizer`] — a Malayalam-aware whitespace-and-
//! punctuation word splitter.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for the base Malayalam letters (they
//! satisfy `is_alphanumeric`), but Malayalam text carries **Malayalam
//! combining marks** — dependent vowel signs (matras like `ാ`
//! U+0D3E, `ി` U+0D3F, `ു` U+0D41, …), the **virama** `്` U+0D4D
//! (which suppresses the inherent vowel and is used to build
//! consonant clusters and conjuncts), the anusvara `ം` U+0D02,
//! the visarga `ഃ` U+0D03, and the six **chillu** letters
//! (U+0D7A..=U+0D7F: `ൺ ൻ ർ ൽ ൾ ൿ`) that carry pure consonants
//! at word-final position — that either are Unicode Mark characters
//! or are Letter characters that we want to keep inside their word.
//! `char::is_alphanumeric` returns `false` for combining marks, so
//! the default splitter would shatter a word like `ഞാൻ` (`ഞ` `ാ`
//! `ൻ` — 3 scalars) at the matra `ാ`.
//!
//! # Rule
//!
//! A [`MalayalamTokenizer`] token is a maximal contiguous run of
//! scalars for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Malayalam block U+0D00..=U+0D7F.
//!
//! The Malayalam-block clause captures every combining sign inside a
//! Malayalam word (matras, virama, anusvara, visarga) *and* every
//! chillu letter, without opening the door to combining marks in
//! other scripts. Latin letters and digits still count as word
//! characters under the standard `is_alphanumeric` rule, so
//! mixed-script input tokenizes as expected. The zero-width joiner
//! `U+200D` and zero-width non-joiner `U+200C` (used in Malayalam to
//! control conjunct formation) sit outside the Malayalam block and
//! are not `is_alphanumeric`; they act as separators.
//!
//! # Byte-vs-char safety
//!
//! Every Malayalam scalar (U+0D00..=U+0D7F) is encoded as **three
//! UTF-8 bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Malayalam word
//! is roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.
//!
//! # Conjunct consonants — the virama is genuinely word-internal
//!
//! Unlike Tamil, which lacks conjunct consonants, Malayalam **does**
//! form conjunct consonants via the virama `്` U+0D4D — some are
//! rendered as ligatures, some as visible virama sequences, and some
//! Malayalam consonant clusters use the atomic chillu forms at
//! word-final. All of these live inside the Malayalam block, so the
//! tokenizer keeps the entire cluster inside its word by construction.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Malayalam is agglutinative —
//!   nouns and verbs stack case, tense, aspect, and honorific
//!   suffixes onto a single orthographic word. Splitting fused
//!   endings at the tokenizer would be wrong for IR indexing; the
//!   light stemmer (see [`crate::stemmer`]) handles the endings
//!   that *are* fused.
//! - **Sentence-level segmentation.** The tokenizer emits word
//!   tokens, not sentences. Malayalam uses ASCII `.`, `?`, `!` as
//!   sentence terminators; callers that want sentence segmentation
//!   should split on those themselves.

/// The Malayalam word tokenizer.
///
/// A zero-sized value; construct as [`MalayalamTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_ml::MalayalamTokenizer;
///
/// let toks: Vec<&str> = MalayalamTokenizer::new()
///     .tokenize("ഞാൻ മലയാളം സംസാരിക്കുന്നു.")
///     .collect();
/// // Matras, virama and chillu stay word-internal; `.` is a separator.
/// assert_eq!(toks, ["ഞാൻ", "മലയാളം", "സംസാരിക്കുന്നു"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MalayalamTokenizer;

impl MalayalamTokenizer {
    /// Constructs a new [`MalayalamTokenizer`]. Zero-sized; free to
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
    /// maximal run of "word" scalars — [`char::is_alphanumeric`] *or*
    /// a Malayalam-block scalar (U+0D00..=U+0D7F, which covers the
    /// combining matras, the virama, anusvara / visarga, and the six
    /// chillu letters that are not `is_alphanumeric` on their own or
    /// that we want kept inside their word).
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> MalayalamTokens<'a> {
        MalayalamTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Malayalam string, produced by
/// [`MalayalamTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct MalayalamTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Malayalam tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for every
/// scalar in the Malayalam block U+0D00..=U+0D7F (the combining
/// matras, the virama, anusvara / visarga, and the six chillu
/// letters — some of these are `false` under `is_alphanumeric` on
/// their own but must stay word-internal in Malayalam).
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    c.is_alphanumeric() || ('\u{0D00}'..='\u{0D7F}').contains(&c)
}

impl<'a> Iterator for MalayalamTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. We advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary, guaranteed even though Malayalam scalars are
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
        MalayalamTokenizer::new().tokenize(input).collect()
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
    fn splits_a_malayalam_sentence_on_ascii_space() {
        assert_eq!(collect("ഞാൻ മലയാളം സംസാരിക്കുന്നു"), ["ഞാൻ", "മലയാളം", "സംസാരിക്കുന്നു"]);
    }

    #[test]
    fn matras_stay_word_internal() {
        // ഞാൻ = ഞ + ാ + ൻ (3 scalars); matra stays.
        assert_eq!(collect("ഞാൻ"), ["ഞാൻ"]);
    }

    #[test]
    fn virama_stays_word_internal() {
        // Consonant cluster with virama. സത്യം = സ + ത + ് + യ + ം.
        assert_eq!(collect("സത്യം"), ["സത്യം"]);
    }

    #[test]
    fn chillu_stays_word_internal() {
        // ൻ chillu (U+0D7B) — word-final atomic "pure consonant".
        assert_eq!(collect("അവൻ"), ["അവൻ"]);
        // ർ chillu (U+0D7C).
        assert_eq!(collect("അവർ"), ["അവർ"]);
        // ൽ chillu (U+0D7D) — often the locative case marker.
        assert_eq!(collect("വീട്ടിൽ"), ["വീട്ടിൽ"]);
    }

    #[test]
    fn anusvara_stays_word_internal() {
        // Anusvara (ം U+0D02) — nasalization at word-end is common.
        assert_eq!(collect("മലയാളം"), ["മലയാളം"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("ഞാൻ, നീ, അവൻ!"), ["ഞാൻ", "നീ", "അവൻ"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("വർഷം 2026"), ["വർഷം", "2026"]);
        // Malayalam digits (U+0D66..=U+0D6F) are in the Malayalam
        // block — treated as word scalars.
        assert_eq!(collect("വർഷം ൨൦൨൬"), ["വർഷം", "൨൦൨൬"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello മലയാളം world"), ["hello", "മലയാളം", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "ഞാൻ മലയാളം";
        let toks: Vec<&str> = MalayalamTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Malayalam scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
