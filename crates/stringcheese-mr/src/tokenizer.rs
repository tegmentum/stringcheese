//! [`MarathiTokenizer`] — a Devanagari-aware whitespace-and-punctuation
//! word splitter, tuned for Marathi.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for the base Devanagari letters (they
//! satisfy `is_alphanumeric`), but Marathi text carries **Devanagari
//! combining marks** — dependent vowel signs (matras like `ि` U+093F,
//! `ी` U+0940, `ु` U+0941, …), the virama `्` U+094D, anusvara `ं`
//! U+0902, chandrabindu `ँ` U+0901, visarga `ः` U+0903, and nukta `़`
//! U+093C — that are Unicode Mark characters. `char::is_alphanumeric`
//! returns `false` for combining marks, so the default splitter would
//! shatter a word like `पुस्तक` (`प` `ु` `स` `्` `त` `क` — 6 scalars)
//! at every matra and virama.
//!
//! Marathi additionally uses the Marathi-specific letters `ळ` (U+0933
//! retroflex L) and `ऱ` (U+0931 R with lower diagonal). Both sit
//! inside the Devanagari block (U+0900..=U+097F) so they are already
//! captured by the block clause below — no special-casing needed at
//! the tokenizer.
//!
//! # Rule
//!
//! A [`MarathiTokenizer`] token is a maximal contiguous run of scalars
//! for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Devanagari block U+0900..=U+097F.
//!
//! The Devanagari-block clause captures every combining sign inside a
//! Marathi word (matras, virama, anusvara, chandrabindu, visarga,
//! nukta) *and* the Marathi-specific letters `ळ` / `ऱ`, without
//! opening the door to combining marks in other scripts. Latin
//! letters and digits still count as word characters under the
//! standard `is_alphanumeric` rule, so mixed-script input tokenizes
//! as expected.
//!
//! Every other scalar (whitespace, ASCII punctuation, and the
//! Devanagari-specific punctuation `।` U+0964 danda, `॥` U+0965
//! double danda, `॰` U+0970 abbreviation sign) is a separator that
//! ends the current token and never appears in the output.
//!
//! # Byte-vs-char safety
//!
//! Every Devanagari scalar (U+0900..=U+097F) is encoded as **three
//! UTF-8 bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Marathi word is
//! roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.
//!
//! # Danda — the Devanagari "full stop" as used in Marathi
//!
//! Marathi text ends sentences with `।` U+0964 (danda), inherited from
//! Devanagari — the same convention as Hindi. The double danda `॥`
//! U+0965 marks end of verse or paragraph in classical texts.
//! Modern Marathi also uses the ASCII period `.` alongside the danda;
//! both are separators under the rule above (the danda because it
//! sits at U+0964, inside the Devanagari block, and gets special-cased
//! as an explicit separator; ASCII `.` because it fails
//! `is_alphanumeric`).
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Marathi's case markers (`-ला`,
//!   `-चा`, `-ने`) are agglutinative — they attach to the noun stem
//!   as one orthographic word (`घराला` "to the house"). Splitting
//!   fused endings at the tokenizer would be wrong; the light stemmer
//!   (see [`crate::stemmer`]) handles the case markers.
//! - **Sentence-level segmentation.** The tokenizer emits word
//!   tokens, not sentences. Callers that want sentence segmentation
//!   should split on `।` and `॥` themselves.

/// The Marathi word tokenizer.
///
/// A zero-sized value; construct as [`MarathiTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_mr::MarathiTokenizer;
///
/// let toks: Vec<&str> = MarathiTokenizer::new()
///     .tokenize("मी मराठी बोलतो।")
///     .collect();
/// // The danda (।) is a separator; matras stay word-internal.
/// assert_eq!(toks, ["मी", "मराठी", "बोलतो"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MarathiTokenizer;

impl MarathiTokenizer {
    /// Constructs a new [`MarathiTokenizer`]. Zero-sized; free to call.
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
    /// a Devanagari-block scalar (U+0900..=U+097F, which covers the
    /// combining marks matras / virama / anusvara / visarga / nukta
    /// that are not `is_alphanumeric` on their own, plus the
    /// Marathi-specific letters `ळ` U+0933 and `ऱ` U+0931).
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> MarathiTokens<'a> {
        MarathiTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Marathi string, produced by
/// [`MarathiTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct MarathiTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Marathi tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for every
/// scalar in the Devanagari block U+0900..=U+097F **except** the three
/// Devanagari punctuation scalars `।` U+0964 (danda), `॥` U+0965
/// (double danda), and `॰` U+0970 (abbreviation sign), which are
/// separators.
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    if matches!(c, '\u{0964}' | '\u{0965}' | '\u{0970}') {
        return false;
    }
    c.is_alphanumeric() || ('\u{0900}'..='\u{097F}').contains(&c)
}

impl<'a> Iterator for MarathiTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. We advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary, guaranteed even though Devanagari scalars are
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
        MarathiTokenizer::new().tokenize(input).collect()
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
    fn splits_a_marathi_sentence_on_ascii_space() {
        assert_eq!(collect("मी मराठी बोलतो"), ["मी", "मराठी", "बोलतो"]);
    }

    #[test]
    fn danda_is_a_separator() {
        // U+0964 Devanagari danda — the primary Marathi sentence
        // terminator.
        assert_eq!(collect("मी जातो।"), ["मी", "जातो"]);
    }

    #[test]
    fn double_danda_is_a_separator() {
        // U+0965 Devanagari double danda — end of verse.
        assert_eq!(collect("सत्यमेव जयते॥"), ["सत्यमेव", "जयते"]);
    }

    #[test]
    fn matras_stay_word_internal() {
        // पुस्तक = प + ु + स + ् + त + क (6 scalars); matras and virama
        // stay word-internal.
        assert_eq!(collect("पुस्तक"), ["पुस्तक"]);
    }

    #[test]
    fn marathi_specific_retroflex_l_stays_word_internal() {
        // The Marathi-specific letter ळ (U+0933) sits inside the
        // Devanagari block and is captured by the block clause.
        // शाळा "school" = श + ा + ळ + ा.
        assert_eq!(collect("शाळा"), ["शाळा"]);
    }

    #[test]
    fn marathi_specific_r_with_diagonal_stays_word_internal() {
        // ऱ (U+0931) also sits inside the Devanagari block.
        assert_eq!(collect("ऱ्हास"), ["ऱ्हास"]);
    }

    #[test]
    fn anusvara_stays_word_internal() {
        // Anusvara (ं U+0902) — nasalization. आम्ही has anusvara nowhere;
        // instead use आणि.
        assert_eq!(collect("आणि"), ["आणि"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("मी, तू, तो!"), ["मी", "तू", "तो"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("वर्ष 2026"), ["वर्ष", "2026"]);
        // Devanagari digits (U+0966..=U+096F) are in the Devanagari
        // block — treated as word scalars.
        assert_eq!(collect("वर्ष २०२६"), ["वर्ष", "२०२६"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello मराठी world"), ["hello", "मराठी", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "मी मराठी";
        let toks: Vec<&str> = MarathiTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Devanagari scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
