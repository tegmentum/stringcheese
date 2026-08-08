//! [`BengaliTokenizer`] — a Bengali-aware whitespace-and-punctuation
//! word splitter.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for the base Bengali letters (they
//! satisfy `is_alphanumeric`), but Bengali text carries **Bengali
//! combining marks** — dependent vowel signs (matras / kars like `ি`
//! U+09BF, `ী` U+09C0, `ু` U+09C1, …), the halant/hasant `্`
//! U+09CD (Bengali's virama), anusvara `ং` U+0982, chandrabindu `ঁ`
//! U+0981, visarga `ঃ` U+0983, and nukta `়` U+09BC — that are
//! Unicode Mark characters. `char::is_alphanumeric` returns `false`
//! for combining marks, so the default splitter would shatter a word
//! like `বাংলা` (`ব` `া` `ং` `ল` `া` — 5 scalars) at every matra.
//!
//! # Rule
//!
//! A [`BengaliTokenizer`] token is a maximal contiguous run of scalars
//! for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Bengali block U+0980..=U+09FF.
//!
//! The Bengali-block clause captures every combining sign inside a
//! Bengali word (matras, halant, anusvara, chandrabindu, visarga,
//! nukta) without opening the door to combining marks in other
//! scripts. Latin letters and digits still count as word characters
//! under the standard `is_alphanumeric` rule, so mixed-script input
//! tokenizes as expected.
//!
//! Every other scalar (whitespace, ASCII punctuation, and the
//! Bengali-specific punctuation `।` U+0964 danda and `॥` U+0965
//! double danda — inherited from Devanagari and used in Bengali
//! typography for the same purpose) is a separator that ends the
//! current token and never appears in the output.
//!
//! # Byte-vs-char safety
//!
//! Every Bengali scalar (U+0980..=U+09FF) is encoded as **three UTF-8
//! bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Bengali word is
//! roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.
//!
//! # Danda — Bengali's sentence terminator
//!
//! Bengali inherits `।` U+0964 (danda, literally "staff") from
//! Devanagari as its sentence terminator. The double danda `॥`
//! U+0965 marks end of verse or paragraph in Bengali poetry and
//! religious texts. Both are punctuation — `is_alphanumeric` returns
//! `false` for them but they sit *outside* the Bengali block
//! (U+0964/U+0965 are in the Devanagari block, not the Bengali
//! block), so they're already separators under our rule; we don't
//! need to special-case them.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Bengali postpositions are
//!   sometimes written as separate words, sometimes fused as case
//!   suffixes. Splitting fused endings at the tokenizer would be
//!   wrong for IR indexing; the light stemmer (see [`crate::stemmer`])
//!   handles the endings that *are* fused.
//! - **Sentence-level segmentation.** The tokenizer emits word
//!   tokens, not sentences. Callers that want sentence segmentation
//!   should split on `।` and `॥` themselves.

/// The Bengali word tokenizer.
///
/// A zero-sized value; construct as [`BengaliTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_bn::BengaliTokenizer;
///
/// let toks: Vec<&str> = BengaliTokenizer::new()
///     .tokenize("আমি বাংলা বলি।")
///     .collect();
/// // The danda (।) is a separator; matras stay word-internal.
/// assert_eq!(toks, ["আমি", "বাংলা", "বলি"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BengaliTokenizer;

impl BengaliTokenizer {
    /// Constructs a new [`BengaliTokenizer`]. Zero-sized; free to call.
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
    /// a Bengali-block scalar (U+0980..=U+09FF, which covers the
    /// combining marks matras / halant / anusvara / visarga / nukta
    /// that are not `is_alphanumeric` on their own).
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> BengaliTokens<'a> {
        BengaliTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Bengali string, produced by
/// [`BengaliTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct BengaliTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Bengali tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for every
/// scalar in the Bengali block U+0980..=U+09FF. The Bengali danda
/// scalars `।` U+0964 and `॥` U+0965 sit *outside* the Bengali block
/// (they're inherited from Devanagari) and are `false` under
/// `is_alphanumeric`, so they naturally separate tokens without any
/// special case here.
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    c.is_alphanumeric() || ('\u{0980}'..='\u{09FF}').contains(&c)
}

impl<'a> Iterator for BengaliTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. We advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary, guaranteed even though Bengali scalars are
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
        BengaliTokenizer::new().tokenize(input).collect()
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
    fn splits_a_bengali_sentence_on_ascii_space() {
        assert_eq!(collect("আমি বাংলা বলি"), ["আমি", "বাংলা", "বলি"]);
    }

    #[test]
    fn danda_is_a_separator() {
        // U+0964 danda — used in Bengali the same way Hindi uses it.
        assert_eq!(collect("আমি যাই।"), ["আমি", "যাই"]);
    }

    #[test]
    fn double_danda_is_a_separator() {
        // U+0965 double danda — end of verse.
        assert_eq!(collect("সত্যমেব জয়তে॥"), ["সত্যমেব", "জয়তে"]);
    }

    #[test]
    fn matras_stay_word_internal() {
        // কিতাব = ক + ি + ত + া + ব (5 scalars); matras stay inside.
        assert_eq!(collect("কিতাব"), ["কিতাব"]);
        // পানি = প + া + ন + ি (4 scalars).
        assert_eq!(collect("পানি"), ["পানি"]);
    }

    #[test]
    fn halant_stays_word_internal() {
        // Consonant cluster with halant. সত্য = স + ত + ্ + য.
        assert_eq!(collect("সত্য"), ["সত্য"]);
    }

    #[test]
    fn anusvara_stays_word_internal() {
        // Anusvara (ং U+0982) — nasalization.
        assert_eq!(collect("বাংলা"), ["বাংলা"]);
    }

    #[test]
    fn chandrabindu_stays_word_internal() {
        // Chandrabindu (ঁ U+0981) — vowel nasalization.
        // চাঁদ (moon) — চ + া + ঁ + দ.
        assert_eq!(collect("চাঁদ"), ["চাঁদ"]);
    }

    #[test]
    fn nukta_stays_word_internal() {
        // Nukta (় U+09BC) modifies a base consonant. ড় (rra) can be
        // written as ড + ় in decomposed form.
        assert_eq!(collect("বড়"), ["বড়"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("আমি, তুমি, সে!"), ["আমি", "তুমি", "সে"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("সাল 2026"), ["সাল", "2026"]);
        // Bengali digits (U+09E6..=U+09EF) are in the Bengali block —
        // treated as word scalars.
        assert_eq!(collect("সাল ২০২৬"), ["সাল", "২০২৬"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello বাংলা world"), ["hello", "বাংলা", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "আমি বাংলা";
        let toks: Vec<&str> = BengaliTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Bengali scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
