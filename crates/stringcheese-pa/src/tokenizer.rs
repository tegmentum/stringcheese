//! [`PunjabiTokenizer`] — a Gurmukhi-aware whitespace-and-punctuation
//! word splitter.
//!
//! # Why a bespoke tokenizer
//!
//! The default
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) walks on
//! [`char::is_alphanumeric`] and treats every non-alphanumeric scalar
//! as a separator. That works for the base Gurmukhi letters (they
//! satisfy `is_alphanumeric`), but Gurmukhi text carries **Gurmukhi
//! combining marks** — dependent vowel signs (matras like `ਾ` U+0A3E,
//! `ਿ` U+0A3F, `ੀ` U+0A40, …), the virama `੍` U+0A4D
//! (Gurmukhi's halant), tippi `ੰ` U+0A70, bindi `ਂ` U+0A02, addak
//! `ੱ` U+0A71, and nukta `਼` U+0A3C — that are Unicode Mark
//! characters. `char::is_alphanumeric` returns `false` for combining
//! marks, so the default splitter would shatter a word like `ਪੰਜਾਬ`
//! (`ਪ` `ੰ` `ਜ` `ਾ` `ਬ` — 5 scalars) at every tippi and matra.
//!
//! # Rule
//!
//! A [`PunjabiTokenizer`] token is a maximal contiguous run of scalars
//! for which either of the following is `true`:
//!
//! 1. [`char::is_alphanumeric`], **or**
//! 2. the scalar is in the Gurmukhi block U+0A00..=U+0A7F.
//!
//! The Gurmukhi-block clause captures every combining sign inside a
//! Gurmukhi word (matras, virama, tippi, bindi, addak, nukta) without
//! opening the door to combining marks in other scripts. Latin letters
//! and digits still count as word characters under the standard
//! `is_alphanumeric` rule, so mixed-script input tokenizes as expected.
//!
//! Every other scalar (whitespace, ASCII punctuation, and the
//! Devanagari-inherited punctuation `।` U+0964 danda and `॥` U+0965
//! double danda — both used in Gurmukhi typography for sentence
//! termination) is a separator that ends the current token and never
//! appears in the output.
//!
//! # Byte-vs-char safety
//!
//! Every Gurmukhi scalar (U+0A00..=U+0A7F) is encoded as **three
//! UTF-8 bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). This means the byte length of a Gurmukhi word is
//! roughly `3 * char_count`, and any code that mixes byte offsets
//! with character-boundary logic will silently corrupt token
//! boundaries. The iterator below advances by `char::len_utf8` at
//! every step and never subtracts or adds a raw byte constant, so
//! every offset it emits is on a valid UTF-8 boundary and every
//! borrowed slice is a well-formed `&str`.
//!
//! # Danda — inherited from Devanagari
//!
//! Punjabi (Gurmukhi) inherits `।` U+0964 (danda, literally "staff")
//! from Devanagari as its sentence terminator in traditional
//! typography; modern newspaper Punjabi uses ASCII `.` widely too.
//! The double danda `॥` U+0965 marks end of verse or paragraph in
//! Punjabi religious texts (notably in the Guru Granth Sahib). Both
//! are punctuation — `is_alphanumeric` returns `false` for them but
//! they sit *outside* the Gurmukhi block (U+0964/U+0965 are in the
//! Devanagari block, not the Gurmukhi block), so they're already
//! separators under our rule; we don't need to special-case them.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Punjabi postpositions are almost
//!   always written as separate orthographic words (`ਦਾ`, `ਦੀ`,
//!   `ਨੂੰ`); fused case suffixes like `-ੇ` (masc oblique) attach to
//!   the noun. Splitting fused endings at the tokenizer would be
//!   wrong for IR indexing; the light stemmer (see [`crate::stemmer`])
//!   handles the endings that *are* fused.
//! - **Sentence-level segmentation.** The tokenizer emits word
//!   tokens, not sentences. Callers that want sentence segmentation
//!   should split on `।`, `॥`, and ASCII `.`, `?`, `!` themselves.

/// The Punjabi word tokenizer.
///
/// A zero-sized value; construct as [`PunjabiTokenizer`] and reuse
/// across threads and calls. See the [module-level docs](self) for the
/// rule.
///
/// # Example
///
/// ```
/// use stringcheese_pa::PunjabiTokenizer;
///
/// let toks: Vec<&str> = PunjabiTokenizer::new()
///     .tokenize("ਮੈਂ ਪੰਜਾਬੀ ਬੋਲਦਾ ਹਾਂ।")
///     .collect();
/// // The danda (।) is a separator; tippi and matras stay word-internal.
/// assert_eq!(toks, ["ਮੈਂ", "ਪੰਜਾਬੀ", "ਬੋਲਦਾ", "ਹਾਂ"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PunjabiTokenizer;

impl PunjabiTokenizer {
    /// Constructs a new [`PunjabiTokenizer`]. Zero-sized; free to call.
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
    /// a Gurmukhi-block scalar (U+0A00..=U+0A7F, which covers the
    /// combining marks matras / virama / tippi / bindi / addak /
    /// nukta that are not `is_alphanumeric` on their own).
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> PunjabiTokens<'a> {
        PunjabiTokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a Punjabi string, produced by
/// [`PunjabiTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset that only ever advances by whole-scalar
/// [`char::len_utf8`] steps, so it always sits on a valid UTF-8
/// boundary.
#[derive(Clone, Debug)]
pub struct PunjabiTokens<'a> {
    text: &'a str,
    offset: usize,
}

/// Is `c` a token-internal scalar under the Punjabi tokenizer rule?
///
/// Returns `true` for [`char::is_alphanumeric`] scalars and for every
/// scalar in the Gurmukhi block U+0A00..=U+0A7F. The Devanagari-
/// inherited danda scalars `।` U+0964 and `॥` U+0965 sit *outside*
/// the Gurmukhi block and are `false` under `is_alphanumeric`, so
/// they naturally separate tokens without any special case here.
#[inline]
#[must_use]
pub fn is_word_scalar(c: char) -> bool {
    c.is_alphanumeric() || ('\u{0A00}'..='\u{0A7F}').contains(&c)
}

impl<'a> Iterator for PunjabiTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip leading separators. We advance by whole scalars only —
        // `ch.len_utf8()` — so `self.offset` is always on a UTF-8
        // boundary, guaranteed even though Gurmukhi scalars are
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
        PunjabiTokenizer::new().tokenize(input).collect()
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
    fn splits_a_punjabi_sentence_on_ascii_space() {
        assert_eq!(collect("ਮੈਂ ਪੰਜਾਬੀ ਬੋਲਦਾ ਹਾਂ"), ["ਮੈਂ", "ਪੰਜਾਬੀ", "ਬੋਲਦਾ", "ਹਾਂ"]);
    }

    #[test]
    fn danda_is_a_separator() {
        // U+0964 danda — used in Gurmukhi the same way Hindi uses it.
        assert_eq!(collect("ਮੈਂ ਜਾਂਦਾ ਹਾਂ।"), ["ਮੈਂ", "ਜਾਂਦਾ", "ਹਾਂ"]);
    }

    #[test]
    fn double_danda_is_a_separator() {
        // U+0965 double danda — end of verse.
        assert_eq!(collect("ਸਤਿ ਨਾਮੁ॥"), ["ਸਤਿ", "ਨਾਮੁ"]);
    }

    #[test]
    fn matras_stay_word_internal() {
        // ਕਿਤਾਬ = ਕ + ਿ + ਤ + ਾ + ਬ (5 scalars); matras stay inside.
        assert_eq!(collect("ਕਿਤਾਬ"), ["ਕਿਤਾਬ"]);
        // ਪਾਣੀ = ਪ + ਾ + ਣ + ੀ (4 scalars).
        assert_eq!(collect("ਪਾਣੀ"), ["ਪਾਣੀ"]);
    }

    #[test]
    fn virama_stays_word_internal() {
        // Consonant cluster with virama. ਸਤ੍ਯ = ਸ + ਤ + ੍ + ਯ.
        assert_eq!(collect("ਸਤ੍ਯ"), ["ਸਤ੍ਯ"]);
    }

    #[test]
    fn tippi_stays_word_internal() {
        // Tippi (ੰ U+0A70) — nasalization.
        assert_eq!(collect("ਪੰਜਾਬ"), ["ਪੰਜਾਬ"]);
    }

    #[test]
    fn bindi_stays_word_internal() {
        // Bindi (ਂ U+0A02) — vowel nasalization.
        // ਮੈਂ (I) — ਮ + ੈ + ਂ.
        assert_eq!(collect("ਮੈਂ"), ["ਮੈਂ"]);
    }

    #[test]
    fn addak_stays_word_internal() {
        // Addak (ੱ U+0A71) — gemination. ਪੱਕਾ = ਪ + ੱ + ਕ + ਾ.
        assert_eq!(collect("ਪੱਕਾ"), ["ਪੱਕਾ"]);
    }

    #[test]
    fn nukta_stays_word_internal() {
        // Nukta (਼ U+0A3C) modifies a base consonant. ਜ਼ (za)
        // decomposed as ਜ + ਼.
        assert_eq!(collect("ਜ਼ਮੀਨ"), ["ਜ਼ਮੀਨ"]);
    }

    #[test]
    fn ascii_punctuation_is_a_separator() {
        assert_eq!(collect("ਮੈਂ, ਤੂੰ, ਓਹ!"), ["ਮੈਂ", "ਤੂੰ", "ਓਹ"]);
    }

    #[test]
    fn digits_are_tokens() {
        // ASCII digits are `is_alphanumeric`.
        assert_eq!(collect("ਸਾਲ 2026"), ["ਸਾਲ", "2026"]);
        // Gurmukhi digits (U+0A66..=U+0A6F) are in the Gurmukhi block —
        // treated as word scalars.
        assert_eq!(collect("ਸਾਲ ੨੦੨੬"), ["ਸਾਲ", "੨੦੨੬"]);
    }

    #[test]
    fn mixed_script_input_splits_correctly() {
        assert_eq!(collect("hello ਪੰਜਾਬੀ world"), ["hello", "ਪੰਜਾਬੀ", "world"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "ਮੈਂ ਪੰਜਾਬੀ";
        let toks: Vec<&str> = PunjabiTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. The `char::len_utf8`-based iterator guarantees
        // no byte offset can slice a 3-byte Gurmukhi scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
