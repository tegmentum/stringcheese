//! [`JapaneseTokenizer`] — a character-type-based tokenizer for Japanese.
//!
//! # Why not morphological analysis?
//!
//! The gold standard for Japanese tokenization is dictionary-driven
//! morphological analysis: `MeCab`, kuromoji, sudachi, and vibrato all
//! ship `IPADic`-scale lexica (~100 MB on disk) and Viterbi search over
//! word-lattice hypotheses. That level of quality is out of scope for a
//! wasm-first / offline-first language pack — shipping a 100 MB
//! dictionary as a static asset defeats the whole point of the crate's
//! size budget.
//!
//! Instead, this module ships a **character-type-based** tokenizer:
//! classify every scalar by script / character-type and split at every
//! type transition. The result is coarser than kuromoji but
//! deterministic, dictionary-free, and near-zero-cost at both compile
//! and runtime.
//!
//! # Character types
//!
//! Every scalar in the input classifies into exactly one of these types
//! (see [`CharType`]):
//!
//! - [`Hiragana`](CharType::Hiragana) — U+3041..=U+309F, the hiragana block.
//! - [`Katakana`](CharType::Katakana) — U+30A0..=U+30FF, the katakana block
//!   (excluding U+30FB `・` which is classified as punctuation).
//!   Includes U+30FC `ー` (prolonged-sound mark), which is the reason
//!   a katakana run doesn't need special-case handling for long-vowel
//!   marks.
//! - [`KatakanaHalfwidth`](CharType::KatakanaHalfwidth) — U+FF66..=U+FF9F,
//!   the half-width katakana range. Treated as its own type because
//!   half-width and full-width katakana carry different semantic
//!   registers (half-width is typically legacy or ASCII-mode input).
//! - [`Kanji`](CharType::Kanji) — U+4E00..=U+9FFF (CJK Unified),
//!   U+3400..=U+4DBF (Extension A), U+F900..=U+FAFF (Compatibility
//!   Ideographs), plus U+3005 `々` (ideographic iteration mark) and
//!   U+3007 `〇` (ideographic number zero).
//! - [`Alphabet`](CharType::Alphabet) — ASCII `[A-Za-z]` and the
//!   full-width Latin letters U+FF21..=U+FF3A, U+FF41..=U+FF5A.
//! - [`Digit`](CharType::Digit) — ASCII `[0-9]` and full-width digits
//!   U+FF10..=U+FF19.
//! - [`Punctuation`](CharType::Punctuation) — CJK Symbols and
//!   Punctuation (U+3000..=U+303F, minus what's classified above),
//!   the katakana middle dot U+30FB `・`, ASCII punctuation, the
//!   General Punctuation block U+2000..=U+206F, and the Halfwidth and
//!   Fullwidth Forms block U+FF00..=U+FFEF for anything not already
//!   classified as a letter or a digit.
//! - [`Whitespace`](CharType::Whitespace) — every scalar for which
//!   [`char::is_whitespace`] returns `true` (this includes U+3000
//!   IDEOGRAPHIC SPACE).
//! - [`Other`](CharType::Other) — anything the ranges above don't cover
//!   (emoji, dingbats, other-script letters). Treated as a single
//!   token-worthy run: an `Other` character between two Kanji does not
//!   join them, matching the behavior for any other type transition.
//!
//! Classification is **total** — every `char` receives exactly one
//! type — and matches the ranges published in the Unicode Standard
//! (Unicode 15.1). No per-character property lookup tables are shipped;
//! the classification is a small cascade of contiguous range checks.
//!
//! # Tokenization rule
//!
//! Skip leading `Whitespace` / `Punctuation`. Emit tokens as
//! maximal runs of same-type characters, with **one deliberate
//! exception**: a `Kanji` run absorbs a following `Hiragana` run
//! (okurigana). Once the Kanji run has swallowed one or more hiragana
//! scalars, the run terminates at the next non-Hiragana boundary
//! (including a return to Kanji). Concretely:
//!
//! - `食べる` (Kanji-Hiragana-Hiragana) → one token `食べる`. This is
//!   the standard okurigana pattern for a verb: kanji stem plus
//!   hiragana conjugation ending. Splitting them at the type transition
//!   would separate the verb from its own inflection.
//! - `食べ物` (Kanji-Hiragana-Kanji) → two tokens `食べ` and `物`. The
//!   compound noun `食べ物` (*food*) is genuinely two morphemes; a
//!   dictionary-free tokenizer can't know they belong together, and
//!   splitting them at the Hiragana→Kanji boundary is the honest
//!   coarse-grained answer.
//! - `私は日本人です` (K-H-K-K-K-H-H) → two tokens `私は` and
//!   `日本人です`. Neither is a single "word" morphologically, but each
//!   is a semantically coherent noun-phrase-plus-particle chunk. This
//!   is a documented coarse-grained artifact.
//! - `JavaScriptを勉強しています` — the Alphabet run yields
//!   `JavaScript`; the Hiragana `を` is its own token (particle,
//!   because it's Hiragana following Alphabet, not Kanji); the Kanji
//!   `勉強` absorbs the trailing `しています` hiragana as okurigana;
//!   result: `[JavaScript, を, 勉強しています]`.
//!
//! # Non-goals
//!
//! - **Kuromoji-scale morphology.** The whole point of this tokenizer
//!   is to *not* need a dictionary. A downstream project that needs
//!   word-lattice segmentation (Viterbi over `IPADic` hypotheses)
//!   should compose a purpose-built morphological analyzer on top of
//!   the [`Language`](stringcheese_lang::Language) trait's tokenize
//!   accessor.
//! - **Sentence segmentation.** The tokenizer emits *words*, not
//!   *sentences*. Sentence boundaries in Japanese are marked by `。`
//!   (U+3002) and are classified as punctuation; a caller who wants
//!   sentence chunks should scan for those separators independently.
//! - **Furigana / ruby text.** Callers that ingest HTML with `<ruby>`
//!   markup should strip the ruby annotations before feeding the text
//!   to this tokenizer.

use core::str::CharIndices;

/// Character-type classification.
///
/// See the [module-level docs](self#character-types) for the range each
/// variant covers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CharType {
    /// Hiragana (U+3041..=U+309F).
    Hiragana,
    /// Full-width katakana (U+30A0..=U+30FF, minus the middle dot
    /// U+30FB which is punctuation). Includes the prolonged-sound mark
    /// U+30FC `ー`.
    Katakana,
    /// Half-width katakana (U+FF66..=U+FF9F).
    KatakanaHalfwidth,
    /// Kanji: CJK Unified Ideographs, Extension A, Compatibility
    /// Ideographs, plus the ideographic iteration and zero marks.
    Kanji,
    /// Latin letters — ASCII `[A-Za-z]` and full-width Latin letters.
    Alphabet,
    /// Decimal digits — ASCII `[0-9]` and full-width digits.
    Digit,
    /// Punctuation — CJK symbols and punctuation, katakana middle dot,
    /// ASCII punctuation, and the general punctuation blocks.
    Punctuation,
    /// Whitespace — every scalar for which [`char::is_whitespace`]
    /// returns `true`, including U+3000 IDEOGRAPHIC SPACE.
    Whitespace,
    /// Anything the classification cascade above does not cover
    /// (emoji, dingbats, other-script letters, ...).
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

    // Digits before Alphabet (both are checked against contiguous
    // ranges; either order works).
    if c.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(&c) {
        return CharType::Digit;
    }

    if c.is_ascii_alphabetic()
        || ('\u{FF21}'..='\u{FF3A}').contains(&c)
        || ('\u{FF41}'..='\u{FF5A}').contains(&c)
    {
        return CharType::Alphabet;
    }

    // Hiragana block. U+3040 and U+3097..=U+3098 are unassigned but the
    // range is small and unlikely to appear; classifying the whole
    // range as Hiragana is fine and keeps the check a single compare.
    if ('\u{3041}'..='\u{309F}').contains(&c) {
        return CharType::Hiragana;
    }

    // Katakana middle dot U+30FB is a separator, not a katakana letter.
    if c == '\u{30FB}' {
        return CharType::Punctuation;
    }

    // Katakana block. Includes U+30FC `ー` (prolonged-sound mark), which
    // is exactly right — the mark attaches to the preceding kana and
    // should stay inside the katakana run.
    if ('\u{30A0}'..='\u{30FF}').contains(&c) {
        return CharType::Katakana;
    }

    // Half-width katakana.
    if ('\u{FF66}'..='\u{FF9F}').contains(&c) {
        return CharType::KatakanaHalfwidth;
    }

    // Kanji: main CJK block, Extension A, Compatibility Ideographs.
    // Plus U+3005 IDEOGRAPHIC ITERATION MARK 々 and U+3007 IDEOGRAPHIC
    // NUMBER ZERO 〇 — both are conventionally treated as kanji-adjacent
    // (the iteration mark 々 always follows a kanji it repeats, e.g.
    // 人々).
    if ('\u{4E00}'..='\u{9FFF}').contains(&c)
        || ('\u{3400}'..='\u{4DBF}').contains(&c)
        || ('\u{F900}'..='\u{FAFF}').contains(&c)
        || c == '\u{3005}'
        || c == '\u{3007}'
    {
        return CharType::Kanji;
    }

    // CJK Symbols and Punctuation (U+3000..=U+303F). We already
    // caught U+3000 (whitespace), U+3005 and U+3007 (kanji-adjacent)
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
    // classified above (full-width ASCII punctuation such as `！`, `？`,
    // `，`, `。` etc., minus what we picked off as digits, letters, and
    // half-width katakana).
    if ('\u{FF00}'..='\u{FFEF}').contains(&c) {
        return CharType::Punctuation;
    }

    CharType::Other
}

/// The Japanese character-type-based tokenizer.
///
/// A zero-sized value; construct as [`JapaneseTokenizer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the classification and merge
/// rules.
///
/// # Example
///
/// ```
/// use stringcheese_ja::JapaneseTokenizer;
///
/// let toks: Vec<&str> = JapaneseTokenizer::new()
///     .tokenize("彼はJavaScriptを勉強しています")
///     .collect();
/// assert_eq!(toks, ["彼は", "JavaScript", "を", "勉強しています"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JapaneseTokenizer;

impl JapaneseTokenizer {
    /// Constructs a new [`JapaneseTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into character-type-based tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> JapaneseTokens<'a> {
        JapaneseTokens {
            text,
            chars: text.char_indices(),
            peeked: None,
        }
    }
}

/// Iterator over the character-type-based tokens of a string, produced
/// by [`JapaneseTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a
/// [`CharIndices`] cursor over the input plus a
/// single-slot look-ahead buffer for the character that ended the
/// previous scan.
#[derive(Clone, Debug)]
pub struct JapaneseTokens<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    /// Single-slot look-ahead: `Some((i, ch))` when the previous call
    /// to `next_char` peeked past the end of a token and we need to
    /// feed that character back on the next call.
    peeked: Option<(usize, char)>,
}

impl JapaneseTokens<'_> {
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

impl<'a> Iterator for JapaneseTokens<'a> {
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

        // Kanji runs absorb a trailing hiragana suffix (okurigana) but
        // stop as soon as we're back in hiragana continuation and see
        // any non-hiragana. This mirrors the standard Japanese
        // orthographic convention: kanji stem + hiragana conjugation
        // is one word.
        let mut saw_okurigana = false;

        loop {
            let Some((i, c)) = self.next_char() else {
                end = self.end();
                break;
            };
            let ct = classify(c);

            let extend = match primary {
                CharType::Kanji => {
                    match ct {
                        CharType::Kanji => {
                            // Extending back to Kanji after we've
                            // already consumed hiragana okurigana means
                            // we've hit the *next* word — stop.
                            !saw_okurigana
                        }
                        CharType::Hiragana => {
                            saw_okurigana = true;
                            true
                        }
                        _ => false,
                    }
                }
                _ => ct == primary,
            };

            if extend {
                end = i + c.len_utf8();
                continue;
            }
            // Terminator — push it back so the outer loop can see it as
            // the start of the next token (or as a separator to skip).
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
        JapaneseTokenizer::new().tokenize(input).collect()
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
    fn classify_hiragana() {
        assert_eq!(classify('あ'), CharType::Hiragana);
        assert_eq!(classify('ん'), CharType::Hiragana);
        assert_eq!(classify('っ'), CharType::Hiragana); // small tsu
    }

    #[test]
    fn classify_katakana() {
        assert_eq!(classify('ア'), CharType::Katakana);
        assert_eq!(classify('ン'), CharType::Katakana);
        assert_eq!(classify('ー'), CharType::Katakana); // prolonged sound
    }

    #[test]
    fn classify_katakana_halfwidth() {
        assert_eq!(classify('\u{FF71}'), CharType::KatakanaHalfwidth); // ｱ
        assert_eq!(classify('\u{FF9D}'), CharType::KatakanaHalfwidth); // ﾝ
    }

    #[test]
    fn classify_kanji() {
        assert_eq!(classify('人'), CharType::Kanji);
        assert_eq!(classify('日'), CharType::Kanji);
        assert_eq!(classify('々'), CharType::Kanji); // iteration mark
        assert_eq!(classify('〇'), CharType::Kanji); // ideographic zero
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
        assert_eq!(classify('。'), CharType::Punctuation); // 全角句点
        assert_eq!(classify('、'), CharType::Punctuation); // 全角読点
        assert_eq!(classify('・'), CharType::Punctuation); // katakana middle dot
        assert_eq!(classify('！'), CharType::Punctuation); // fullwidth ！ (U+FF01)
        assert_eq!(classify('!'), CharType::Punctuation); // ASCII !
        assert_eq!(classify('.'), CharType::Punctuation);
        assert_eq!(classify(','), CharType::Punctuation);
    }

    #[test]
    fn classify_other() {
        // Emoji falls outside every named script/type range.
        assert_eq!(classify('🎉'), CharType::Other);
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
        assert!(collect("。、！？").is_empty());
        assert!(collect("...").is_empty());
    }

    #[test]
    fn splits_at_hiragana_katakana_transition() {
        assert_eq!(collect("あいカタ"), ["あい", "カタ"]);
    }

    #[test]
    fn splits_at_alphabet_kanji_transition() {
        assert_eq!(collect("abc人"), ["abc", "人"]);
    }

    #[test]
    fn splits_at_digit_alphabet_transition() {
        assert_eq!(collect("123abc"), ["123", "abc"]);
    }

    #[test]
    fn splits_at_alphabet_digit_transition() {
        assert_eq!(collect("abc123"), ["abc", "123"]);
    }

    #[test]
    fn splits_at_katakana_halfwidth_boundary() {
        // Full-width katakana ア vs half-width ｱ are distinct types
        // and split.
        assert_eq!(collect("アｱ"), ["ア", "ｱ"]);
    }

    #[test]
    fn kanji_absorbs_okurigana_hiragana() {
        // 食べる: 食(K) + べ(H) + る(H) — one token thanks to the
        // Kanji+Hiragana merge rule.
        assert_eq!(collect("食べる"), ["食べる"]);
    }

    #[test]
    fn kanji_hiragana_kanji_splits_at_second_kanji() {
        // 食べ物: 食(K) + べ(H) + 物(K) — split at the H→K return.
        assert_eq!(collect("食べ物"), ["食べ", "物"]);
    }

    #[test]
    fn hiragana_run_extends_naturally() {
        assert_eq!(collect("あいうえお"), ["あいうえお"]);
    }

    #[test]
    fn katakana_run_includes_prolonged_sound_mark() {
        // コーヒー = コ + ー + ヒ + ー — all in the katakana block.
        assert_eq!(collect("コーヒー"), ["コーヒー"]);
    }

    #[test]
    fn kanji_run_extends_across_multiple_kanji() {
        assert_eq!(collect("日本語"), ["日本語"]);
    }

    #[test]
    fn whitespace_and_punctuation_separate_tokens() {
        assert_eq!(collect("これ は 本"), ["これ", "は", "本"]);
        assert_eq!(collect("これ、それ。"), ["これ", "それ"]);
    }

    #[test]
    fn mixed_script_sentence_full_example() {
        // The task-spec example: mixed-script sentence with Alphabet,
        // Kanji, Hiragana, particle, and verb-with-okurigana runs.
        let toks = collect("彼はJavaScriptを勉強しています");
        assert_eq!(toks, ["彼は", "JavaScript", "を", "勉強しています"]);
    }

    #[test]
    fn ideographic_iteration_mark_stays_with_kanji_run() {
        // 人々 = 人 + 々 — both classified as Kanji, so one token.
        assert_eq!(collect("人々"), ["人々"]);
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "食べる";
        let toks: Vec<&str> = JapaneseTokenizer::new().tokenize(text).collect();
        assert_eq!(toks.len(), 1);
        // The token slice is borrowed from the input string, verified
        // by pointer arithmetic (no unsafe).
        let base = text.as_ptr() as usize;
        assert_eq!(toks[0].as_ptr() as usize - base, 0);
        assert_eq!(toks[0], "食べる");
    }
}
