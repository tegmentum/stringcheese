//! Kana normalization utilities.
//!
//! Japanese text arrives in many equivalent forms — half-width vs.
//! full-width katakana, hiragana vs. katakana, decomposed dakuten vs.
//! precomposed voiced kana, full-width vs. half-width Latin letters and
//! digits. Search, deduplication, and phonetic-key lookup all want a
//! canonical form before comparing. This module ships the pieces needed
//! to build that canonical form, with a [`KanaNormalizer`] builder that
//! composes them into one pass.
//!
//! # Transforms
//!
//! * **Half-width katakana widening.** `ｶﾀｶﾅ → カタカナ`. Half-width
//!   dakuten (U+FF9E) and handakuten (U+FF9F) become the combining
//!   marks (U+3099 / U+309A), which the dakuten canonicalization pass
//!   then folds into the precomposed voiced form (`ｶﾞ → ガ`).
//! * **Dakuten canonicalization.** `か` + U+3099 → `が`. Same for
//!   handakuten: `は` + U+309A → `ぱ`. The pass leaves any combining
//!   mark that does not attach to a voice-able base kana in place.
//! * **Katakana ↔ hiragana.** `カタカナ ↔ かたかな`. A fixed-offset fold
//!   over U+30A1..=U+30F6 ↔ U+3041..=U+3096. Scalars outside that
//!   range (the prolonged sound mark ー, the iteration marks, the
//!   ヴ-family, half-width forms) pass through — half-width katakana is
//!   only folded after the widening pass has run.
//! * **Full-width ASCII → half-width.** `０-９ → 0-9`, `Ａ-Ｚ → A-Z`,
//!   `ａ-ｚ → a-z`. A single subtraction of `0xFEE0` from the code
//!   point; nothing else is touched.
//!
//! # Non-goals
//!
//! * **NFKC / full Unicode normalization.** The transforms here are a
//!   Japanese-facing subset. Callers who want the full Unicode
//!   normalization pipeline should compose this module with an
//!   NFKC implementation from elsewhere in the workspace.
//! * **Half-width ← full-width katakana.** The reverse direction has
//!   no single-character mapping for many full-width kana (dakuten
//!   forms decompose into two half-width scalars) and is rarely useful
//!   in canonicalization.
//! * **Full-width punctuation folding.** `。、！？` all stay as-is —
//!   the wider ASCII fold is limited to letters and digits so a caller
//!   using this module for search keys doesn't accidentally lose
//!   Japanese punctuation.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ja::KanaNormalizer;
//!
//! // Default preset: dakuten canonicalization + half-width katakana
//! // widening. Both are lossless canonicalization passes.
//! let n = KanaNormalizer::default();
//! assert_eq!(n.normalize("ｶﾞｯｺｳ"), "ガッコウ");
//! // か + combining voiced sound mark → が
//! assert_eq!(n.normalize("か\u{3099}"), "が");
//! ```

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

/// A composable kana-normalization pipeline.
///
/// Enable the transforms you want with `with_*` builder methods, then
/// call [`normalize`](Self::normalize) on your input. The builder is
/// [`Copy`], zero-sized aside from four `bool`s, and can be reused
/// across calls and threads.
///
/// # Default preset
///
/// [`KanaNormalizer::default`] enables the two "safe" transforms —
/// half-width katakana widening and dakuten canonicalization — because
/// those two are strictly lossless: no scalar is dropped, and re-running
/// them on already-normalized text is a no-op. The other two flags
/// (`full_to_half_ascii`, `katakana_to_hiragana`) fold characters into
/// visually distinct forms and are opt-in.
///
/// # Idempotence
///
/// Every combination of flags is idempotent — `normalize(normalize(x))
/// == normalize(x)`. This is the property tests hold the builder to.
///
/// # Example
///
/// ```
/// use stringcheese_ja::KanaNormalizer;
///
/// let strong = KanaNormalizer::new()
///     .with_half_to_full_katakana(true)
///     .with_dakuten_canonicalization(true)
///     .with_katakana_to_hiragana(true)
///     .with_full_to_half_ascii(true);
/// assert_eq!(strong.normalize("ｶﾞｯｺｳ"), "がっこう");
/// assert_eq!(strong.normalize("Ａ１"), "A1");
/// ```
// Four independent transforms → four independent `bool`s. Clippy's
// "more than 3 bools" heuristic wants a state machine, but every
// combination of these flags is meaningful (all 16 subsets are valid
// and semantically distinct), so the honest representation is a bit
// field of four `bool`s, not a folded enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct KanaNormalizer {
    full_to_half_ascii: bool,
    half_to_full_katakana: bool,
    katakana_to_hiragana: bool,
    dakuten_canonicalization: bool,
}

impl Default for KanaNormalizer {
    /// The "safe" default: half-width katakana widening plus dakuten
    /// canonicalization. Both are lossless canonicalization passes;
    /// callers who want the more opinionated folds
    /// (`katakana → hiragana`, full-width ASCII → half-width) enable
    /// them explicitly with the corresponding `with_*` methods.
    fn default() -> Self {
        Self {
            full_to_half_ascii: false,
            half_to_full_katakana: true,
            katakana_to_hiragana: false,
            dakuten_canonicalization: true,
        }
    }
}

impl KanaNormalizer {
    /// An empty normalizer with every transform turned off.
    ///
    /// Use this when you want to enable transforms explicitly and not
    /// inherit the "safe" default preset.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            full_to_half_ascii: false,
            half_to_full_katakana: false,
            katakana_to_hiragana: false,
            dakuten_canonicalization: false,
        }
    }

    /// Toggle the full-width ASCII → half-width fold.
    ///
    /// Maps `Ａ-Ｚ → A-Z`, `ａ-ｚ → a-z`, `０-９ → 0-9`. All other
    /// scalars pass through.
    #[must_use]
    pub const fn with_full_to_half_ascii(mut self, on: bool) -> Self {
        self.full_to_half_ascii = on;
        self
    }

    /// Toggle the half-width katakana → full-width katakana widening.
    ///
    /// Maps `ｶﾀｶﾅ → カタカナ`, and folds the half-width dakuten (U+FF9E)
    /// / handakuten (U+FF9F) into the combining marks (U+3099 / U+309A)
    /// so the dakuten canonicalization pass can finish the job:
    /// `ｶﾞ → カ + U+3099 → ガ`.
    #[must_use]
    pub const fn with_half_to_full_katakana(mut self, on: bool) -> Self {
        self.half_to_full_katakana = on;
        self
    }

    /// Toggle the katakana → hiragana script fold.
    ///
    /// Maps `カタカナ → かたかな` over the U+30A1..=U+30F6 range. This
    /// is a script-collapsing fold — the caller loses the visual
    /// distinction between the two syllabaries but the two kana share
    /// their entire phonology, so the result is fine for search keys.
    #[must_use]
    pub const fn with_katakana_to_hiragana(mut self, on: bool) -> Self {
        self.katakana_to_hiragana = on;
        self
    }

    /// Toggle the dakuten / handakuten canonicalization pass.
    ///
    /// Collapses `<base kana> + <combining voiced mark U+3099>` into
    /// the precomposed voiced kana (`か + U+3099 → が`), and the same
    /// for handakuten (`は + U+309A → ぱ`). Combining marks that do
    /// not attach to a voice-able base pass through unchanged.
    #[must_use]
    pub const fn with_dakuten_canonicalization(mut self, on: bool) -> Self {
        self.dakuten_canonicalization = on;
        self
    }

    /// Apply every enabled transform to `text` and return the
    /// normalized [`String`].
    ///
    /// The passes run in a fixed order:
    ///
    /// 1. Full-width ASCII → half-width.
    /// 2. Half-width katakana → full-width katakana. (Emits combining
    ///    dakuten / handakuten for the half-width voiced-sound marks;
    ///    the next pass canonicalizes those.)
    /// 3. Dakuten canonicalization.
    /// 4. Katakana → hiragana.
    ///
    /// The order matters — running the katakana → hiragana fold before
    /// the widening pass would miss half-width katakana entirely.
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        let mut work: Cow<'_, str> = Cow::Borrowed(text);
        if self.full_to_half_ascii {
            work = Cow::Owned(full_width_ascii_to_half(&work));
        }
        if self.half_to_full_katakana {
            work = Cow::Owned(widen_halfwidth_katakana(&work));
        }
        if self.dakuten_canonicalization {
            work = Cow::Owned(canonicalize_dakuten(&work));
        }
        if self.katakana_to_hiragana {
            work = Cow::Owned(fold_katakana_to_hiragana(&work));
        }
        work.into_owned()
    }
}

// ---------------------------------------------------------------------
// Single-character conversions.
// ---------------------------------------------------------------------

/// Fold a katakana scalar to the equivalent hiragana scalar.
///
/// Half-width katakana is first widened to full-width, then folded to
/// hiragana. Non-katakana input (including the prolonged sound mark
/// U+30FC, the iteration marks, and the ヴ-family outside the fixed
/// range) is returned unchanged.
#[must_use]
pub fn katakana_to_hiragana_char(c: char) -> char {
    if let Some(fw) = halfwidth_to_fullwidth_katakana_char(c) {
        return katakana_to_hiragana_char(fw);
    }
    if ('\u{30A1}'..='\u{30F6}').contains(&c) {
        let cp = c as u32 - 96;
        return char::from_u32(cp).unwrap_or(c);
    }
    c
}

/// Fold a hiragana scalar to the equivalent katakana scalar.
///
/// The fixed offset is +96 over the range U+3041..=U+3096. Scalars
/// outside that range (`ゔ` U+3094 is inside; the iteration marks
/// U+3099..=U+309F are outside) are returned unchanged.
#[must_use]
pub fn hiragana_to_katakana_char(c: char) -> char {
    if ('\u{3041}'..='\u{3096}').contains(&c) {
        let cp = c as u32 + 96;
        return char::from_u32(cp).unwrap_or(c);
    }
    c
}

/// Widen a half-width katakana scalar (U+FF66..=U+FF9F) to the closest
/// full-width katakana equivalent. Returns `None` for scalars outside
/// the half-width range or for the two half-width voiced-sound marks
/// (which widen to combining U+3099 / U+309A — handled by
/// [`widen_halfwidth_katakana`] rather than as a single-character fold).
#[must_use]
pub fn halfwidth_to_fullwidth_katakana_char(c: char) -> Option<char> {
    const TABLE: &[(char, char)] = &[
        ('\u{FF66}', '\u{30F2}'), // ｦ → ヲ
        ('\u{FF67}', '\u{30A1}'), // ｧ → ァ
        ('\u{FF68}', '\u{30A3}'), // ｨ → ィ
        ('\u{FF69}', '\u{30A5}'), // ｩ → ゥ
        ('\u{FF6A}', '\u{30A7}'), // ｪ → ェ
        ('\u{FF6B}', '\u{30A9}'), // ｫ → ォ
        ('\u{FF6C}', '\u{30E3}'), // ｬ → ャ
        ('\u{FF6D}', '\u{30E5}'), // ｭ → ュ
        ('\u{FF6E}', '\u{30E7}'), // ｮ → ョ
        ('\u{FF6F}', '\u{30C3}'), // ｯ → ッ
        ('\u{FF70}', '\u{30FC}'), // ｰ → ー
        ('\u{FF71}', '\u{30A2}'), // ｱ → ア
        ('\u{FF72}', '\u{30A4}'), // ｲ → イ
        ('\u{FF73}', '\u{30A6}'), // ｳ → ウ
        ('\u{FF74}', '\u{30A8}'), // ｴ → エ
        ('\u{FF75}', '\u{30AA}'), // ｵ → オ
        ('\u{FF76}', '\u{30AB}'), // ｶ → カ
        ('\u{FF77}', '\u{30AD}'), // ｷ → キ
        ('\u{FF78}', '\u{30AF}'), // ｸ → ク
        ('\u{FF79}', '\u{30B1}'), // ｹ → ケ
        ('\u{FF7A}', '\u{30B3}'), // ｺ → コ
        ('\u{FF7B}', '\u{30B5}'), // ｻ → サ
        ('\u{FF7C}', '\u{30B7}'), // ｼ → シ
        ('\u{FF7D}', '\u{30B9}'), // ｽ → ス
        ('\u{FF7E}', '\u{30BB}'), // ｾ → セ
        ('\u{FF7F}', '\u{30BD}'), // ｿ → ソ
        ('\u{FF80}', '\u{30BF}'), // ﾀ → タ
        ('\u{FF81}', '\u{30C1}'), // ﾁ → チ
        ('\u{FF82}', '\u{30C4}'), // ﾂ → ツ
        ('\u{FF83}', '\u{30C6}'), // ﾃ → テ
        ('\u{FF84}', '\u{30C8}'), // ﾄ → ト
        ('\u{FF85}', '\u{30CA}'), // ﾅ → ナ
        ('\u{FF86}', '\u{30CB}'), // ﾆ → ニ
        ('\u{FF87}', '\u{30CC}'), // ﾇ → ヌ
        ('\u{FF88}', '\u{30CD}'), // ﾈ → ネ
        ('\u{FF89}', '\u{30CE}'), // ﾉ → ノ
        ('\u{FF8A}', '\u{30CF}'), // ﾊ → ハ
        ('\u{FF8B}', '\u{30D2}'), // ﾋ → ヒ
        ('\u{FF8C}', '\u{30D5}'), // ﾌ → フ
        ('\u{FF8D}', '\u{30D8}'), // ﾍ → ヘ
        ('\u{FF8E}', '\u{30DB}'), // ﾎ → ホ
        ('\u{FF8F}', '\u{30DE}'), // ﾏ → マ
        ('\u{FF90}', '\u{30DF}'), // ﾐ → ミ
        ('\u{FF91}', '\u{30E0}'), // ﾑ → ム
        ('\u{FF92}', '\u{30E1}'), // ﾒ → メ
        ('\u{FF93}', '\u{30E2}'), // ﾓ → モ
        ('\u{FF94}', '\u{30E4}'), // ﾔ → ヤ
        ('\u{FF95}', '\u{30E6}'), // ﾕ → ユ
        ('\u{FF96}', '\u{30E8}'), // ﾖ → ヨ
        ('\u{FF97}', '\u{30E9}'), // ﾗ → ラ
        ('\u{FF98}', '\u{30EA}'), // ﾘ → リ
        ('\u{FF99}', '\u{30EB}'), // ﾙ → ル
        ('\u{FF9A}', '\u{30EC}'), // ﾚ → レ
        ('\u{FF9B}', '\u{30ED}'), // ﾛ → ロ
        ('\u{FF9C}', '\u{30EF}'), // ﾜ → ワ
        ('\u{FF9D}', '\u{30F3}'), // ﾝ → ン
    ];
    TABLE.iter().find(|(h, _)| *h == c).map(|(_, f)| *f)
}

/// Combine a base kana scalar with a voiced-sound mark into the
/// precomposed voiced kana.
///
/// `mark` is one of the combining marks U+3099 (dakuten, voiced) or
/// U+309A (handakuten, semi-voiced); anything else returns `None`.
/// Returns `None` when `base` has no voiced form for the requested mark
/// (`か + handakuten` → `None`; only h-row supports handakuten).
#[must_use]
pub fn combine_dakuten(base: char, mark: char) -> Option<char> {
    match mark {
        '\u{3099}' => voiced_form(base),
        '\u{309A}' => semi_voiced_form(base),
        _ => None,
    }
}

#[allow(clippy::match_same_arms)]
fn voiced_form(c: char) -> Option<char> {
    Some(match c {
        // Hiragana.
        'う' => 'ゔ',
        'か' => 'が',
        'き' => 'ぎ',
        'く' => 'ぐ',
        'け' => 'げ',
        'こ' => 'ご',
        'さ' => 'ざ',
        'し' => 'じ',
        'す' => 'ず',
        'せ' => 'ぜ',
        'そ' => 'ぞ',
        'た' => 'だ',
        'ち' => 'ぢ',
        'つ' => 'づ',
        'て' => 'で',
        'と' => 'ど',
        'は' => 'ば',
        'ひ' => 'び',
        'ふ' => 'ぶ',
        'へ' => 'べ',
        'ほ' => 'ぼ',
        // Katakana.
        'ウ' => 'ヴ',
        'カ' => 'ガ',
        'キ' => 'ギ',
        'ク' => 'グ',
        'ケ' => 'ゲ',
        'コ' => 'ゴ',
        'サ' => 'ザ',
        'シ' => 'ジ',
        'ス' => 'ズ',
        'セ' => 'ゼ',
        'ソ' => 'ゾ',
        'タ' => 'ダ',
        'チ' => 'ヂ',
        'ツ' => 'ヅ',
        'テ' => 'デ',
        'ト' => 'ド',
        'ハ' => 'バ',
        'ヒ' => 'ビ',
        'フ' => 'ブ',
        'ヘ' => 'ベ',
        'ホ' => 'ボ',
        _ => return None,
    })
}

fn semi_voiced_form(c: char) -> Option<char> {
    Some(match c {
        'は' => 'ぱ',
        'ひ' => 'ぴ',
        'ふ' => 'ぷ',
        'へ' => 'ぺ',
        'ほ' => 'ぽ',
        'ハ' => 'パ',
        'ヒ' => 'ピ',
        'フ' => 'プ',
        'ヘ' => 'ペ',
        'ホ' => 'ポ',
        _ => return None,
    })
}

// ---------------------------------------------------------------------
// Whole-string passes.
// ---------------------------------------------------------------------

/// Fold every full-width digit and Latin letter in `text` to its
/// half-width equivalent; leave everything else alone.
#[must_use]
pub fn full_width_ascii_to_half(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let cp = c as u32;
        if (0xFF10..=0xFF19).contains(&cp)
            || (0xFF21..=0xFF3A).contains(&cp)
            || (0xFF41..=0xFF5A).contains(&cp)
        {
            if let Some(half) = char::from_u32(cp - 0xFEE0) {
                out.push(half);
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Widen every half-width katakana scalar in `text` to full-width
/// katakana. Half-width dakuten and handakuten (U+FF9E / U+FF9F) become
/// the combining marks (U+3099 / U+309A); a downstream
/// [`canonicalize_dakuten`] pass then folds those into the precomposed
/// voiced kana.
#[must_use]
pub fn widen_halfwidth_katakana(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if let Some(fw) = halfwidth_to_fullwidth_katakana_char(c) {
            out.push(fw);
        } else if c == '\u{FF9E}' {
            out.push('\u{3099}');
        } else if c == '\u{FF9F}' {
            out.push('\u{309A}');
        } else {
            out.push(c);
        }
    }
    out
}

/// Collapse every `<base kana> + <combining voiced-sound mark>`
/// sequence in `text` into the precomposed voiced form.
///
/// Sequences whose base does not accept the mark are left alone (the
/// combining mark stays in the output).
#[must_use]
pub fn canonicalize_dakuten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if i + 1 < chars.len() {
            let mark = chars[i + 1];
            if mark == '\u{3099}' || mark == '\u{309A}' {
                if let Some(voiced) = combine_dakuten(c, mark) {
                    out.push(voiced);
                    i += 2;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Fold every katakana scalar in `text` (full-width and half-width)
/// to the hiragana equivalent. Non-katakana characters pass through.
#[must_use]
pub fn fold_katakana_to_hiragana(text: &str) -> String {
    text.chars().map(katakana_to_hiragana_char).collect()
}

/// Fold every hiragana scalar in `text` to the katakana equivalent.
/// Non-hiragana characters pass through.
#[must_use]
pub fn fold_hiragana_to_katakana(text: &str) -> String {
    text.chars().map(hiragana_to_katakana_char).collect()
}

/// Convenience alias for [`fold_katakana_to_hiragana`].
#[must_use]
pub fn to_hiragana(text: &str) -> String {
    fold_katakana_to_hiragana(text)
}

/// Convenience alias for [`fold_hiragana_to_katakana`].
#[must_use]
pub fn to_katakana(text: &str) -> String {
    fold_hiragana_to_katakana(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Single-character conversions.
    // ---------------------------------------------------------------

    #[test]
    fn katakana_folds_to_hiragana() {
        assert_eq!(katakana_to_hiragana_char('カ'), 'か');
        assert_eq!(katakana_to_hiragana_char('タ'), 'た');
        assert_eq!(katakana_to_hiragana_char('ヴ'), 'ゔ');
        // Half-width folds through to hiragana.
        assert_eq!(katakana_to_hiragana_char('\u{FF76}'), 'か');
        // Prolonged sound mark is left alone.
        assert_eq!(katakana_to_hiragana_char('ー'), 'ー');
    }

    #[test]
    fn hiragana_folds_to_katakana() {
        assert_eq!(hiragana_to_katakana_char('か'), 'カ');
        assert_eq!(hiragana_to_katakana_char('ゔ'), 'ヴ');
        // ASCII passes through.
        assert_eq!(hiragana_to_katakana_char('a'), 'a');
    }

    #[test]
    fn halfwidth_to_fullwidth_katakana_maps_common_moras() {
        assert_eq!(halfwidth_to_fullwidth_katakana_char('\u{FF76}'), Some('カ'));
        assert_eq!(halfwidth_to_fullwidth_katakana_char('\u{FF7B}'), Some('サ'));
        assert_eq!(halfwidth_to_fullwidth_katakana_char('\u{FF97}'), Some('ラ'));
        // Non-half-width returns None.
        assert_eq!(halfwidth_to_fullwidth_katakana_char('a'), None);
    }

    #[test]
    fn combine_dakuten_covers_all_voice_able_moras() {
        assert_eq!(combine_dakuten('か', '\u{3099}'), Some('が'));
        assert_eq!(combine_dakuten('し', '\u{3099}'), Some('じ'));
        assert_eq!(combine_dakuten('つ', '\u{3099}'), Some('づ'));
        assert_eq!(combine_dakuten('は', '\u{3099}'), Some('ば'));
        assert_eq!(combine_dakuten('は', '\u{309A}'), Some('ぱ'));
        assert_eq!(combine_dakuten('ハ', '\u{309A}'), Some('パ'));
        // Not-in-h-row + handakuten → None.
        assert_eq!(combine_dakuten('か', '\u{309A}'), None);
        // Non-mark → None.
        assert_eq!(combine_dakuten('か', 'x'), None);
    }

    // ---------------------------------------------------------------
    // Whole-string passes.
    // ---------------------------------------------------------------

    #[test]
    fn ascii_widening_folds_digits_and_letters() {
        assert_eq!(full_width_ascii_to_half("Ａ１ｂ"), "A1b");
        // Punctuation is left alone.
        assert_eq!(full_width_ascii_to_half("。、"), "。、");
    }

    #[test]
    fn halfwidth_katakana_widens_with_dakuten() {
        // ｶ ﾞ → カ + U+3099 (combining voiced sound mark)
        assert_eq!(
            widen_halfwidth_katakana("\u{FF76}\u{FF9E}"),
            "\u{30AB}\u{3099}"
        );
        // ﾊ ﾟ → ハ + U+309A
        assert_eq!(
            widen_halfwidth_katakana("\u{FF8A}\u{FF9F}"),
            "\u{30CF}\u{309A}"
        );
    }

    #[test]
    fn canonicalize_dakuten_precomposes() {
        // か + U+3099 → が
        assert_eq!(canonicalize_dakuten("か\u{3099}"), "が");
        // は + U+309A → ぱ
        assert_eq!(canonicalize_dakuten("は\u{309A}"), "ぱ");
        // Mark on a scalar with no voiced form is left alone.
        assert_eq!(canonicalize_dakuten("a\u{3099}"), "a\u{3099}");
    }

    #[test]
    fn katakana_to_hiragana_string() {
        assert_eq!(fold_katakana_to_hiragana("カタカナ"), "かたかな");
        assert_eq!(to_hiragana("カタカナ"), "かたかな");
    }

    #[test]
    fn hiragana_to_katakana_string() {
        assert_eq!(fold_hiragana_to_katakana("かたかな"), "カタカナ");
        assert_eq!(to_katakana("かたかな"), "カタカナ");
    }

    // ---------------------------------------------------------------
    // KanaNormalizer builder.
    // ---------------------------------------------------------------

    #[test]
    fn default_preset_widens_halfwidth_and_precomposes_dakuten() {
        let n = KanaNormalizer::default();
        // Half-width + dakuten → precomposed full-width katakana.
        assert_eq!(n.normalize("\u{FF76}\u{FF9E}"), "ガ");
        // A pure combining sequence also precomposes.
        assert_eq!(n.normalize("か\u{3099}"), "が");
        // Full-width ASCII is left alone by default.
        assert_eq!(n.normalize("Ａ"), "Ａ");
    }

    #[test]
    fn empty_normalizer_is_identity() {
        let n = KanaNormalizer::new();
        assert_eq!(n.normalize("か\u{3099}"), "か\u{3099}");
        assert_eq!(n.normalize("Ａ"), "Ａ");
    }

    #[test]
    fn full_config_folds_everything() {
        let n = KanaNormalizer::new()
            .with_full_to_half_ascii(true)
            .with_half_to_full_katakana(true)
            .with_dakuten_canonicalization(true)
            .with_katakana_to_hiragana(true);
        // Half-width + dakuten → hiragana with precomposed voicing.
        assert_eq!(n.normalize("\u{FF76}\u{FF9E}"), "が");
        // Full-width ASCII → half-width.
        assert_eq!(n.normalize("Ａ１"), "A1");
        // Katakana → hiragana.
        assert_eq!(n.normalize("カタカナ"), "かたかな");
    }

    #[test]
    fn normalize_is_idempotent_default() {
        let n = KanaNormalizer::default();
        let once = n.normalize("ｶﾞｯｺｳ");
        let twice = n.normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn normalize_is_idempotent_full_config() {
        let n = KanaNormalizer::new()
            .with_full_to_half_ascii(true)
            .with_half_to_full_katakana(true)
            .with_dakuten_canonicalization(true)
            .with_katakana_to_hiragana(true);
        let once = n.normalize("Ａ１\u{FF76}\u{FF9E}カタカナ");
        let twice = n.normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn round_trip_hiragana_katakana_hiragana() {
        let input = "かたかなさくら";
        let round_trip = fold_katakana_to_hiragana(&fold_hiragana_to_katakana(input));
        assert_eq!(round_trip, input);
    }
}
