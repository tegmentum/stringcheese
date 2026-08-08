//! [`KoreanTokenizer`] — a whitespace-and-punctuation splitter for
//! Korean text.
//!
//! # Why not a character-type tokenizer?
//!
//! Unlike Japanese or Chinese, **Korean text uses spaces between
//! words**. The Korean orthographic convention (띄어쓰기 "spacing")
//! places a space before every noun-phrase-with-particle and before
//! every verb / adjective, so a whitespace-and-punctuation splitter
//! produces the same tokens a native reader would identify — with two
//! caveats that this pack accepts:
//!
//! - **Particles are fused to their noun.** Korean writes the topic
//!   marker `은/는`, the subject marker `이/가`, the object marker
//!   `을/를`, and every other case particle *attached* to the preceding
//!   noun with no space (`책은` "book-TOP", not "책 은"). The tokenizer
//!   emits the fused form as one token; the [`stemmer`](crate::stemmer)
//!   strips the particle from the syllable end after tokenization.
//! - **Verb / adjective endings are fused to their stem.** Same shape:
//!   Korean is agglutinative, so `가다` "to go" appears in text as
//!   `갔습니다` "went (formal)" with the stem `가` glued to a stack of
//!   suffixes. Again, the fused surface form is the token; suffix
//!   stripping is the stemmer's job.
//!
//! # Splitter rule
//!
//! Tokens are maximal runs of characters that are neither whitespace
//! nor a Korean-relevant punctuation scalar. Explicitly:
//!
//! - **Whitespace** — every scalar for which [`char::is_whitespace`]
//!   returns `true`, including U+3000 IDEOGRAPHIC SPACE (rare but
//!   sometimes seen in Korean typography borrowed from Japanese).
//! - **ASCII punctuation** — `!` `?` `.` `,` `;` `:` `(` `)` `[` `]`
//!   etc. per [`char::is_ascii_punctuation`].
//! - **Korean-facing punctuation** — the full-width variants `！` `？`
//!   `。` `、` U+3001..=U+301F (mostly borrowed from Japanese but also
//!   used in Korean, especially in vertical typesetting and older
//!   texts), plus the general-punctuation block U+2000..=U+206F.
//!
//! # Alphanumeric fusion
//!
//! ASCII letters, digits, and other-script letters that appear inside
//! Korean text stay glued to any adjacent Hangul without a script
//! transition, matching how Korean readers group borrowed English
//! terms: `2025년` is one token (year 2025), `iOS앱` is one token (iOS
//! app), just as they would be in the source text without whitespace.
//! Unlike the Japanese pack — which splits at every script transition
//! because Japanese has no inter-word spaces to rely on — Korean's
//! space-based word boundaries make script-transition splitting
//! unnecessary and, in fact, wrong for common Korean-English mixed
//! phrases.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** A production Korean IR pipeline
//!   typically uses `mecab-ko` or `khaiii` to split `갔습니다` into the
//!   morphemes `가` + `았` + `습니다`. That level of quality needs a
//!   dictionary-scale morphological analyzer and is out of scope; see
//!   the [`crate::stemmer`] module docs for the coarse suffix-stripping
//!   approximation this pack ships.
//! - **Sentence segmentation.** The tokenizer emits *words*, not
//!   *sentences*. Korean sentence-ending punctuation (`.` `?` `!` `。`
//!   `！` `？`) is treated as a separator; callers who want sentence
//!   chunks should scan for those separators independently.
//! - **Hanja / Chinese-character joining.** Korean occasionally mixes
//!   Chinese characters (Hanja, U+4E00..=U+9FFF) with Hangul; they are
//!   letters under [`char::is_alphanumeric`] and stay inside their
//!   space-delimited word naturally.

use core::str::CharIndices;

/// The Korean tokenizer.
///
/// A zero-sized value; construct as [`KoreanTokenizer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the split rule.
///
/// # Example
///
/// ```
/// use stringcheese_ko::KoreanTokenizer;
///
/// let toks: Vec<&str> = KoreanTokenizer::new()
///     .tokenize("나는 학교에 갑니다.")
///     .collect();
/// assert_eq!(toks, ["나는", "학교에", "갑니다"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KoreanTokenizer;

impl KoreanTokenizer {
    /// Constructs a new [`KoreanTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into whitespace- and punctuation-delimited tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> KoreanTokens<'a> {
        KoreanTokens {
            text,
            chars: text.char_indices(),
            peeked: None,
        }
    }
}

/// Iterator over the tokens of a string, produced by
/// [`KoreanTokenizer::tokenize`].
///
/// Borrows the input string and yields borrowed slices of it — no
/// allocation is performed.
#[derive(Clone, Debug)]
pub struct KoreanTokens<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    /// Single-slot look-ahead: `Some((i, ch))` when the previous scan
    /// consumed a character past the end of a token and we need to feed
    /// it back on the next call.
    peeked: Option<(usize, char)>,
}

impl KoreanTokens<'_> {
    #[inline]
    fn next_char(&mut self) -> Option<(usize, char)> {
        if let Some(x) = self.peeked.take() {
            return Some(x);
        }
        self.chars.next()
    }

    #[inline]
    fn push_back(&mut self, x: (usize, char)) {
        debug_assert!(self.peeked.is_none(), "look-ahead slot is single-shot");
        self.peeked = Some(x);
    }
}

/// Is `c` a Korean-relevant separator — whitespace or one of the
/// punctuation ranges the module docs enumerate?
#[inline]
fn is_separator(c: char) -> bool {
    if c.is_whitespace() {
        return true;
    }
    if c.is_ascii_punctuation() {
        return true;
    }
    // CJK Symbols and Punctuation (borrowed by Korean typography).
    // U+3000 IDEOGRAPHIC SPACE is already caught by `is_whitespace`.
    if ('\u{3001}'..='\u{303F}').contains(&c) {
        return true;
    }
    // General Punctuation.
    if ('\u{2000}'..='\u{206F}').contains(&c) {
        return true;
    }
    // Halfwidth and Fullwidth Forms — punctuation range only.
    // Full-width ASCII punctuation `！？。、` etc. lives here.
    if ('\u{FF01}'..='\u{FF0F}').contains(&c)
        || ('\u{FF1A}'..='\u{FF20}').contains(&c)
        || ('\u{FF3B}'..='\u{FF40}').contains(&c)
        || ('\u{FF5B}'..='\u{FF65}').contains(&c)
    {
        return true;
    }
    false
}

impl<'a> Iterator for KoreanTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        // Skip leading separators (whitespace + punctuation).
        let (start, first) = loop {
            let (i, c) = self.next_char()?;
            if !is_separator(c) {
                break (i, c);
            }
        };

        let mut end = start + first.len_utf8();

        loop {
            let Some((i, c)) = self.next_char() else {
                end = self.text.len();
                break;
            };
            if is_separator(c) {
                self.push_back((i, c));
                break;
            }
            end = i + c.len_utf8();
        }

        Some(&self.text[start..end])
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        KoreanTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
        assert!(collect("\t\n\u{3000}").is_empty());
    }

    #[test]
    fn punctuation_only_yields_no_tokens() {
        assert!(collect("。、！？").is_empty());
        assert!(collect("...").is_empty());
    }

    #[test]
    fn splits_on_whitespace() {
        assert_eq!(collect("나는 학교에 갑니다"), ["나는", "학교에", "갑니다"]);
    }

    #[test]
    fn trailing_period_is_a_separator() {
        assert_eq!(collect("나는 학교에 갑니다."), ["나는", "학교에", "갑니다"]);
    }

    #[test]
    fn korean_full_width_punctuation_separates() {
        assert_eq!(collect("네、아니오。"), ["네", "아니오"]);
    }

    #[test]
    fn ascii_alphanumeric_fuses_with_hangul() {
        // Common Korean-English mixed phrasing — spaces are the only
        // separators.
        assert_eq!(collect("iOS앱"), ["iOS앱"]);
        assert_eq!(collect("2025년"), ["2025년"]);
    }

    #[test]
    fn hangul_run_stays_together() {
        assert_eq!(collect("안녕하세요"), ["안녕하세요"]);
    }

    #[test]
    fn hangul_with_particle_is_one_token() {
        // The tokenizer emits fused surface forms; particle stripping
        // is the stemmer's job.
        assert_eq!(collect("책은"), ["책은"]);
        assert_eq!(collect("학교에서"), ["학교에서"]);
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "안녕 세상";
        let toks: Vec<&str> = KoreanTokenizer::new().tokenize(text).collect();
        assert_eq!(toks.len(), 2);
        let base = text.as_ptr() as usize;
        for t in &toks {
            let off = t.as_ptr() as usize - base;
            assert!(off < text.len(), "token pointer outside input");
        }
    }
}
