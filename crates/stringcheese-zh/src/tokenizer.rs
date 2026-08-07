//! [`ChineseTokenizer`] — a character-level (dictionary-free) tokenizer
//! for Chinese.
//!
//! # Why no dictionary?
//!
//! The gold standard for Chinese word segmentation is dictionary-driven:
//! jieba, thulac, pkuseg, `HanLP`, LTP all ship 100K-400K-entry lexica
//! (megabytes on disk) and run Viterbi / conditional-random-field
//! searches over word-lattice hypotheses. Shipping any of those in a
//! wasm-first / offline-first language pack defeats the whole point of
//! the crate's size budget — jieba's default dictionary alone is
//! ~10 MB, and no smaller drop-in replacement gets remotely close to
//! its accuracy.
//!
//! Instead, this module ships a **character-level** tokenizer:
//!
//! - Every Han (CJK Unified Ideographs) scalar becomes its own token.
//! - Runs of Latin letters stay together (`"hello"` = one token, not
//!   five).
//! - Runs of digits stay together (`"2026"` = one token, not four).
//! - Whitespace and punctuation are separators — they are not emitted
//!   as tokens.
//!
//! This mirrors **BERT's Chinese preprocessing** ([BERT Multilingual
//! README][bert-mbert]): the multilingual BERT tokenizer splits every
//! CJK character into a standalone `WordPiece` token, and downstream
//! attention layers learn the character-composition patterns from
//! data. The trade-off is well-understood: character-level segmentation
//! is coarser than word-level for Chinese IR (a query for `中国`
//! matches `中` and `国` independently, and `中国人` "Chinese person"
//! is now three tokens: `中`, `国`, `人`), but it is deterministic,
//! dictionary-free, and near-zero-cost at both compile and runtime.
//!
//! Callers that need jieba-quality word segmentation should compose a
//! purpose-built segmenter (a hypothetical `stringcheese-zh-jieba`
//! crate, deferred to a follow-up) on top of the
//! [`Language`](stringcheese_lang::Language) trait's tokenize
//! accessor — the base pack stays wasm-friendly.
//!
//! [bert-mbert]: https://github.com/google-research/bert/blob/master/multilingual.md
//!
//! # Character types
//!
//! Every scalar in the input classifies into exactly one of these
//! types (see [`CharType`]):
//!
//! - [`Han`](CharType::Han) — CJK Unified Ideographs
//!   (U+4E00..=U+9FFF), CJK Unified Ideographs Extension A
//!   (U+3400..=U+4DBF), CJK Compatibility Ideographs
//!   (U+F900..=U+FAFF), plus U+3005 `々` (ideographic iteration mark)
//!   and U+3007 `〇` (ideographic number zero). **Every Han scalar
//!   emits as its own token.**
//! - [`Alphabet`](CharType::Alphabet) — ASCII `[A-Za-z]` and full-width
//!   Latin letters U+FF21..=U+FF3A / U+FF41..=U+FF5A. Runs stay
//!   together.
//! - [`Digit`](CharType::Digit) — ASCII `[0-9]` and full-width digits
//!   U+FF10..=U+FF19. Runs stay together.
//! - [`Punctuation`](CharType::Punctuation) — CJK Symbols and
//!   Punctuation (U+3000..=U+303F, minus U+3005 / U+3007 which are
//!   classified as Han above), ASCII punctuation, General Punctuation
//!   (U+2000..=U+206F), and the Halfwidth and Fullwidth Forms block
//!   (U+FF00..=U+FFEF) for anything not classified as a letter or a
//!   digit. Punctuation is a **separator** — dropped from the token
//!   stream.
//! - [`Whitespace`](CharType::Whitespace) — every scalar for which
//!   [`char::is_whitespace`] returns `true` (this includes U+3000
//!   IDEOGRAPHIC SPACE). Separator.
//! - [`Other`](CharType::Other) — anything the ranges above don't
//!   cover (emoji, dingbats, other-script letters). Treated as a
//!   single token-worthy run: an `Other` scalar between two Han
//!   characters does not join them (Han is always character-split).
//!
//! Classification is **total** — every `char` receives exactly one
//! type — and matches the ranges published in the Unicode Standard
//! (Unicode 15.1). No per-character property lookup tables are
//! shipped; the classification is a small cascade of contiguous
//! range checks.
//!
//! # Tokenization rule
//!
//! Skip leading separators (`Whitespace` / `Punctuation`). Then:
//!
//! - If the current scalar is `Han`: emit it as a single-character
//!   token; do not extend.
//! - Otherwise: extend as a maximal run of same-type scalars, then
//!   emit that run as one token.
//!
//! Examples (see the reference tests for a full table):
//!
//! - `你好` (Han-Han) → `["你", "好"]` — every Han character is its
//!   own token.
//! - `hello` (Alphabet run) → `["hello"]` — Latin runs stay together.
//! - `2026年` (Digit-Digit-Digit-Digit-Han) → `["2026", "年"]` —
//!   digit run stays together; Han is on its own.
//! - `中文hello123` → `["中", "文", "hello", "123"]`.
//!
//! # Non-goals
//!
//! - **Dictionary-driven word segmentation.** The whole point of this
//!   tokenizer is to *not* need a dictionary. A downstream project
//!   that needs jieba/pkuseg-scale segmentation should compose a
//!   purpose-built segmenter on top of the
//!   [`Language`](stringcheese_lang::Language) trait's tokenize
//!   accessor.
//! - **Sentence segmentation.** The tokenizer emits *characters and
//!   Latin-runs*, not *sentences*. Sentence boundaries in Chinese are
//!   marked by `。` (U+3002), `？` (U+FF1F), `！` (U+FF01) and are
//!   classified as punctuation; a caller who wants sentence chunks
//!   should scan for those separators independently.
//! - **Simplified↔Traditional conversion.** Traditional Chinese
//!   (Taiwan / Hong Kong) uses different characters for many words
//!   (`簡` vs. `简`, `國` vs. `国`, `們` vs. `们`). Both variants
//!   classify as `Han` and tokenize the same way (character-level),
//!   but stopword and pinyin lookups target the Simplified surface
//!   form. Callers with Traditional text should either compose an
//!   S↔T converter or use a `stringcheese-zh-hant` sibling pack
//!   (deferred).
//! - **Bopomofo / Zhuyin.** The Taiwan-facing phonetic script
//!   (U+3100..=U+312F) is not classified specially — a stretch of
//!   Bopomofo falls into `Other`.

use core::str::CharIndices;

/// Character-type classification.
///
/// See the [module-level docs](self#character-types) for the range
/// each variant covers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CharType {
    /// Han (CJK Unified Ideographs) — main block, Extension A,
    /// Compatibility Ideographs, plus U+3005 `々` and U+3007 `〇`.
    /// Every Han scalar emits as its own token.
    Han,
    /// Latin letters — ASCII `[A-Za-z]` and full-width Latin letters.
    Alphabet,
    /// Decimal digits — ASCII `[0-9]` and full-width digits.
    Digit,
    /// Punctuation — CJK symbols and punctuation, ASCII punctuation,
    /// and the general punctuation blocks. Treated as a separator.
    Punctuation,
    /// Whitespace — every scalar for which [`char::is_whitespace`]
    /// returns `true`, including U+3000 IDEOGRAPHIC SPACE. Treated as
    /// a separator.
    Whitespace,
    /// Anything the classification cascade above does not cover
    /// (emoji, dingbats, Bopomofo, Hangul, hiragana/katakana in
    /// Chinese-tagged text, ...). Treated as a token-worthy run.
    Other,
}

/// Classify `c` into its [`CharType`].
///
/// Classification is total — every `char` receives exactly one type —
/// and matches the ranges published in the Unicode Standard (Unicode
/// 15.1). See the [module-level docs](self#character-types).
#[must_use]
pub fn classify(c: char) -> CharType {
    // Whitespace is checked first so U+3000 IDEOGRAPHIC SPACE (which
    // also falls inside the CJK Symbols and Punctuation block) is
    // classified as Whitespace, not Punctuation.
    if c.is_whitespace() {
        return CharType::Whitespace;
    }

    // Digits before Alphabet.
    if c.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(&c) {
        return CharType::Digit;
    }

    if c.is_ascii_alphabetic()
        || ('\u{FF21}'..='\u{FF3A}').contains(&c)
        || ('\u{FF41}'..='\u{FF5A}').contains(&c)
    {
        return CharType::Alphabet;
    }

    // Han: main CJK block, Extension A, Compatibility Ideographs.
    // Plus U+3005 IDEOGRAPHIC ITERATION MARK 々 and U+3007 IDEOGRAPHIC
    // NUMBER ZERO 〇 — both are conventionally treated as Han-adjacent
    // (the iteration mark 々 follows a Han character it repeats; 〇 is
    // used as a numeric zero in Han-script writing).
    if ('\u{4E00}'..='\u{9FFF}').contains(&c)
        || ('\u{3400}'..='\u{4DBF}').contains(&c)
        || ('\u{F900}'..='\u{FAFF}').contains(&c)
        || c == '\u{3005}'
        || c == '\u{3007}'
    {
        return CharType::Han;
    }

    // CJK Symbols and Punctuation (U+3000..=U+303F). We already
    // caught U+3000 (whitespace), U+3005 and U+3007 (Han-adjacent)
    // above; the rest are punctuation.
    if ('\u{3000}'..='\u{303F}').contains(&c) {
        return CharType::Punctuation;
    }

    // ASCII punctuation and the two big Unicode punctuation blocks.
    if c.is_ascii_punctuation() {
        return CharType::Punctuation;
    }
    if ('\u{2000}'..='\u{206F}').contains(&c) {
        return CharType::Punctuation;
    }
    // Halfwidth and Fullwidth Forms block, everything not already
    // classified above (full-width ASCII punctuation such as `！`,
    // `？`, `，`, `。` etc., minus what we picked off as digits and
    // letters).
    if ('\u{FF00}'..='\u{FFEF}').contains(&c) {
        return CharType::Punctuation;
    }

    CharType::Other
}

/// The Chinese character-level tokenizer.
///
/// A zero-sized value; construct as [`ChineseTokenizer`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the classification and
/// emission rules.
///
/// # Example
///
/// ```
/// use stringcheese_zh::ChineseTokenizer;
///
/// let toks: Vec<&str> = ChineseTokenizer::new()
///     .tokenize("中文hello123")
///     .collect();
/// assert_eq!(toks, ["中", "文", "hello", "123"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChineseTokenizer;

impl ChineseTokenizer {
    /// Constructs a new [`ChineseTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into character-level tokens (for Han characters)
    /// and same-type runs (for Latin / digit / other).
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> ChineseTokens<'a> {
        ChineseTokens {
            text,
            chars: text.char_indices(),
            peeked: None,
        }
    }
}

/// Iterator over the character-level / same-type-run tokens of a
/// string, produced by [`ChineseTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices
/// of it — no allocation is performed. Held state is a
/// [`CharIndices`] cursor over the input plus a single-slot
/// look-ahead buffer for the character that ended the previous scan.
#[derive(Clone, Debug)]
pub struct ChineseTokens<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    /// Single-slot look-ahead: `Some((i, ch))` when the previous call
    /// to `next_char` peeked past the end of a token and we need to
    /// feed that character back on the next call.
    peeked: Option<(usize, char)>,
}

impl ChineseTokens<'_> {
    /// Byte index one past the end of the input.
    #[inline]
    fn end(&self) -> usize {
        self.text.len()
    }

    /// Consume the next `(byte_index, char)` pair, honoring the
    /// look-ahead buffer.
    #[inline]
    fn next_char(&mut self) -> Option<(usize, char)> {
        if let Some(x) = self.peeked.take() {
            return Some(x);
        }
        self.chars.next()
    }

    /// Push a `(byte_index, char)` pair back onto the input.
    #[inline]
    fn push_back(&mut self, x: (usize, char)) {
        debug_assert!(self.peeked.is_none(), "look-ahead slot is single-shot");
        self.peeked = Some(x);
    }
}

/// A character-type value counts as *token-worthy* if it is not a
/// separator (whitespace or punctuation).
#[inline]
fn is_token_worthy(ct: CharType) -> bool {
    !matches!(ct, CharType::Whitespace | CharType::Punctuation)
}

impl<'a> Iterator for ChineseTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        // Skip leading separators (whitespace + punctuation).
        let (start, first, primary) = loop {
            let (i, c) = self.next_char()?;
            let ct = classify(c);
            if is_token_worthy(ct) {
                break (i, c, ct);
            }
        };

        // The initial `end` must cover the first scalar we already
        // consumed to find `start` — otherwise a one-character token
        // followed immediately by a separator would return the empty
        // slice `&text[start..start]`.
        let mut end = start + first.len_utf8();

        // Han characters are always one-scalar tokens: emit the single
        // Han scalar and stop. This is the deliberate character-level
        // rule that mirrors BERT's Chinese preprocessing — the base
        // pack ships no dictionary, so it makes no attempt to merge
        // adjacent Han scalars into words.
        if primary == CharType::Han {
            return Some(&self.text[start..end]);
        }

        // Non-Han runs extend as maximal same-type sequences.
        loop {
            let Some((i, c)) = self.next_char() else {
                end = self.end();
                break;
            };
            let ct = classify(c);

            if ct == primary {
                end = i + c.len_utf8();
                continue;
            }
            // Terminator — push it back so the outer loop can see it
            // as the start of the next token (or as a separator to
            // skip).
            self.push_back((i, c));
            break;
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
        ChineseTokenizer::new().tokenize(input).collect()
    }

    // ---------------------------------------------------------------
    // classify() smoke tests — one representative per character type.
    // ---------------------------------------------------------------

    #[test]
    fn classify_ascii_letters() {
        assert_eq!(classify('a'), CharType::Alphabet);
        assert_eq!(classify('Z'), CharType::Alphabet);
    }

    #[test]
    fn classify_ascii_digits() {
        assert_eq!(classify('0'), CharType::Digit);
        assert_eq!(classify('9'), CharType::Digit);
    }

    #[test]
    fn classify_han() {
        assert_eq!(classify('中'), CharType::Han);
        assert_eq!(classify('国'), CharType::Han);
        assert_eq!(classify('人'), CharType::Han);
        assert_eq!(classify('々'), CharType::Han); // iteration mark
        assert_eq!(classify('〇'), CharType::Han); // ideographic zero
    }

    #[test]
    fn classify_traditional_han() {
        // Traditional Han characters live in the same CJK Unified
        // Ideographs block; they classify as Han just like Simplified.
        assert_eq!(classify('國'), CharType::Han); // 国 (Trad)
        assert_eq!(classify('們'), CharType::Han); // 们 (Trad)
        assert_eq!(classify('簡'), CharType::Han); // 简 (Trad)
    }

    #[test]
    fn classify_fullwidth_letters_and_digits() {
        assert_eq!(classify('Ａ'), CharType::Alphabet);
        assert_eq!(classify('ｚ'), CharType::Alphabet);
        assert_eq!(classify('０'), CharType::Digit);
        assert_eq!(classify('９'), CharType::Digit);
    }

    #[test]
    fn classify_whitespace_including_ideographic_space() {
        assert_eq!(classify(' '), CharType::Whitespace);
        assert_eq!(classify('\t'), CharType::Whitespace);
        assert_eq!(classify('\n'), CharType::Whitespace);
        assert_eq!(classify('\u{3000}'), CharType::Whitespace); // IDEOGRAPHIC SPACE
    }

    #[test]
    fn classify_punctuation() {
        assert_eq!(classify('。'), CharType::Punctuation); // 全角句号
        assert_eq!(classify('，'), CharType::Punctuation); // 全角逗号
        assert_eq!(classify('、'), CharType::Punctuation); // 顿号
        assert_eq!(classify('！'), CharType::Punctuation); // fullwidth ！
        assert_eq!(classify('？'), CharType::Punctuation); // fullwidth ？
        assert_eq!(classify('!'), CharType::Punctuation); // ASCII !
        assert_eq!(classify('.'), CharType::Punctuation);
        assert_eq!(classify(','), CharType::Punctuation);
    }

    #[test]
    fn classify_other() {
        // Emoji falls outside every named script/type range.
        assert_eq!(classify('🎉'), CharType::Other);
        // Hiragana is Other in a Chinese-tagged tokenizer.
        assert_eq!(classify('あ'), CharType::Other);
    }

    // ---------------------------------------------------------------
    // tokenizer behavior.
    // ---------------------------------------------------------------

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
        assert!(collect("。，！？").is_empty());
        assert!(collect("...").is_empty());
    }

    #[test]
    fn every_han_char_is_its_own_token() {
        assert_eq!(collect("你好"), ["你", "好"]);
        assert_eq!(collect("中国人"), ["中", "国", "人"]);
        assert_eq!(collect("我是学生"), ["我", "是", "学", "生"]);
    }

    #[test]
    fn latin_run_stays_together() {
        assert_eq!(collect("hello"), ["hello"]);
        assert_eq!(collect("JavaScript"), ["JavaScript"]);
    }

    #[test]
    fn digit_run_stays_together() {
        assert_eq!(collect("2026"), ["2026"]);
        assert_eq!(collect("123abc"), ["123", "abc"]);
    }

    #[test]
    fn mixed_han_latin_digit() {
        assert_eq!(collect("中文hello123"), ["中", "文", "hello", "123"]);
    }

    #[test]
    fn punctuation_separates_tokens() {
        assert_eq!(collect("你好，世界！"), ["你", "好", "世", "界"]);
        assert_eq!(collect("北京。上海"), ["北", "京", "上", "海"]);
    }

    #[test]
    fn whitespace_separates_tokens() {
        assert_eq!(collect("你 好"), ["你", "好"]);
        assert_eq!(collect("北京 hello"), ["北", "京", "hello"]);
    }

    #[test]
    fn ideographic_iteration_mark_is_own_token() {
        // 人々 tokenizes as two separate Han tokens under the
        // character-level rule — no okurigana-style merging like
        // Japanese.
        assert_eq!(collect("人々"), ["人", "々"]);
    }

    #[test]
    fn traditional_han_tokenizes_the_same_way() {
        // Traditional glyphs are still Han — character-level split.
        assert_eq!(collect("國家"), ["國", "家"]);
    }

    #[test]
    fn digit_alphabet_transition_splits() {
        assert_eq!(collect("abc123def"), ["abc", "123", "def"]);
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "中国";
        let toks: Vec<&str> = ChineseTokenizer::new().tokenize(text).collect();
        assert_eq!(toks.len(), 2);
        // The token slices are borrowed from the input string,
        // verified by pointer arithmetic (no unsafe).
        let base = text.as_ptr() as usize;
        assert_eq!(toks[0].as_ptr() as usize - base, 0);
        assert_eq!(toks[0], "中");
        assert_eq!(toks[1], "国");
    }
}
