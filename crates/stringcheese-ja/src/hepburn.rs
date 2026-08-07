//! Hepburn romanization (Modified Hepburn).
//!
//! Sibling encoder to [`crate::romaji::KunreiRomaji`] with the same
//! shape but Hepburn's English-facing spellings: `si → shi`,
//! `ti → chi`, `tu → tsu`, `hu → fu`, `zya → ja`, and Modified Hepburn's
//! macron treatment of long vowels.
//!
//! # Kunrei vs. Hepburn
//!
//! | Kana | Kunrei | Hepburn |
//! |------|--------|---------|
//! | し   | `si`   | `shi`   |
//! | ち   | `ti`   | `chi`   |
//! | つ   | `tu`   | `tsu`   |
//! | ふ   | `hu`   | `fu`    |
//! | じ   | `zi`   | `ji`    |
//! | しゃ | `sya`  | `sha`   |
//! | ちゃ | `tya`  | `cha`   |
//! | じゃ | `zya`  | `ja`    |
//!
//! Hepburn is the English-facing convention (English-speaking readers
//! recognize `sushi`, `Tokyo`, and `Fuji` at a glance). Kunrei-shiki
//! is Japan's official government standard and preserves consonant
//! regularity. The `stringcheese-ja` pack ships both; Kunrei is the
//! default (see [`crate::JAPANESE`]), Hepburn is an alternate that
//! callers opt into with [`crate::JAPANESE_WITH_HEPBURN`] or by
//! constructing a [`HepburnRomajiAdapter`] directly.
//!
//! # Modified Hepburn conventions this encoder follows
//!
//! * **Long vowels take macrons.** `おう → ō`, `おお → ō`, `うう → ū`,
//!   `ええ → ē`, `ああ → ā`. The prolonged-sound mark `ー` becomes the
//!   macron over the preceding vowel: `コーヒー → kōhī`.
//! * **`えい` and `いい` are left as-is.** The Modified Hepburn
//!   convention leaves the `ei` and `ii` sequences as two ASCII letters
//!   rather than emitting `ē` or `ī`. `せんせい → sensei`,
//!   `おおきい → ōkii`.
//! * **Small tsu (`っ`) doubles the following consonant.** With the
//!   Modified Hepburn exception for `ch`: `っち → tchi`, `まっちゃ →
//!   matcha`. The `sh` and `ts` digraphs double only their first
//!   consonant (`っし → sshi`, `っつ → ttsu`).
//! * **Syllabic `ん` is `n`.** Modified Hepburn writes `n` before
//!   `b`/`m`/`p` (traditional Hepburn writes `m`). An apostrophe
//!   separates `n` from a following vowel or `y`: `しんや → shin'ya`.
//!
//! # Non-goals
//!
//! * **Word-boundary macrons.** `おう` at a morpheme boundary should
//!   emit `ou` rather than `ō`, but this encoder can't see morpheme
//!   boundaries without a dictionary. Called out in the roadmap.
//! * **Traditional Hepburn `m` before b/m/p.** This encoder emits `n`
//!   uniformly.
//! * **Kanji.** Kanji pass through unchanged — the encoder is kana-only.
//!
//! # Adapter
//!
//! The [`HepburnRomajiAdapter`] wraps the encoder for the
//! [`LanguagePhoneticEncoder`] trait so callers who route through the
//! [`Language`](stringcheese_lang::Language) trait see a Hepburn key
//! when they select the [`crate::JAPANESE_WITH_HEPBURN`] pack.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::normalize::katakana_to_hiragana_char;

/// The Modified Hepburn romanizer.
///
/// Zero-sized; construct as [`HepburnRomaji`] and reuse the value
/// freely across threads and calls. See the [module-level docs](self)
/// for the mapping tables, the small-tsu / prolonged-sound / syllabic-n
/// handling, and the long-vowel conventions.
///
/// # Example
///
/// ```
/// use stringcheese_ja::HepburnRomaji;
///
/// assert_eq!(HepburnRomaji.romanize("さくら"), "sakura");
/// assert_eq!(HepburnRomaji.romanize("シ"), "shi");
/// assert_eq!(HepburnRomaji.romanize("まっちゃ"), "matcha");
/// assert_eq!(HepburnRomaji.romanize("コーヒー"), "kōhī");
/// assert_eq!(HepburnRomaji.romanize("しんや"), "shin'ya");
/// assert_eq!(HepburnRomaji.romanize("とうきょう"), "tōkyō");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HepburnRomaji;

impl HepburnRomaji {
    /// Romanize `text` per Modified Hepburn.
    ///
    /// Kanji, ASCII, and any scalar that isn't kana pass through
    /// unchanged. The output is ASCII plus, at most, the vowel macrons
    /// `ā`, `ī`, `ū`, `ē`, `ō`.
    #[must_use]
    pub fn romanize(&self, text: &str) -> String {
        // Fold katakana (full-width and half-width) to hiragana so the
        // mapping tables only have to cover the hiragana side. Leaves
        // the prolonged sound mark U+30FC, the iteration marks, and
        // any non-kana scalar alone.
        let chars: Vec<char> = text.chars().map(katakana_to_hiragana_char).collect();

        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];

            // Small tsu (sokuon) — doubles the initial consonant of
            // the next mora. Modified Hepburn writes `っち → tchi`,
            // not `cchi`.
            if c == '\u{3063}' {
                if let Some((syll, consumed)) = try_syllable(&chars, i + 1) {
                    if syll.starts_with("ch") {
                        // Modified Hepburn exception: `っち → tchi`.
                        out.push('t');
                        out.push_str(&syll);
                        i = extend_long_vowel_and_advance(&mut out, &chars, i + 1 + consumed);
                        continue;
                    }
                    let first = syll.as_bytes()[0];
                    if first.is_ascii_lowercase() && !is_vowel_byte(first) {
                        out.push(first as char);
                        out.push_str(&syll);
                        i = extend_long_vowel_and_advance(&mut out, &chars, i + 1 + consumed);
                        continue;
                    }
                    // Vowel-initial follower (rare / malformed) —
                    // emit `t` as the closest single-letter proxy and
                    // let the outer loop handle the vowel next round.
                    out.push('t');
                    i += 1;
                    continue;
                }
                // Trailing small tsu with nothing to double — proxy `t`.
                out.push('t');
                i += 1;
                continue;
            }

            // Prolonged sound mark (ー) — replace the previous vowel
            // with its macron form.
            if c == '\u{30FC}' {
                apply_prolongation(&mut out);
                i += 1;
                continue;
            }

            // Syllabic n (ん) — write `n`, plus an apostrophe before a
            // following vowel or `y` to disambiguate.
            if c == '\u{3093}' {
                out.push('n');
                if let Some(next) = chars.get(i + 1).copied() {
                    if needs_apostrophe_after_n(next) {
                        out.push('\'');
                    }
                }
                i += 1;
                continue;
            }

            // Regular mora — try the two-character digraph first
            // (きゃ, しゃ, じゅ, …), then the single-character mora.
            if let Some((syll, consumed)) = try_syllable(&chars, i) {
                out.push_str(&syll);
                i = extend_long_vowel_and_advance(&mut out, &chars, i + consumed);
                continue;
            }

            // Pass-through: kanji, ASCII, punctuation, or any scalar
            // outside the mapping tables.
            out.push(c);
            i += 1;
        }
        out
    }
}

/// Peek at `chars[i]`; if it forms a long vowel with the last vowel in
/// `out`, replace that vowel with its macron form and advance `i` past
/// the peeked scalar. Returns the (possibly advanced) index to resume
/// scanning from.
fn extend_long_vowel_and_advance(out: &mut String, chars: &[char], i: usize) -> usize {
    if i >= chars.len() {
        return i;
    }
    let next = chars[i];
    let Some(last) = out.chars().last() else {
        return i;
    };
    // Modified Hepburn's long-vowel table. Note the deliberate absence
    // of `('i', 'い')` and `('e', 'い')` — the `ii` and `ei` sequences
    // are left as two ASCII letters per the Modified Hepburn
    // convention.
    let macron = match (last, next) {
        ('a', 'あ') => 'ā',
        ('u', 'う') => 'ū',
        ('e', 'え') => 'ē',
        ('o', 'う' | 'お') => 'ō',
        _ => return i,
    };
    out.pop();
    out.push(macron);
    i + 1
}

/// Replace the last vowel in `out` with its macron form. Called when
/// the scanner hits the prolonged sound mark ー.
fn apply_prolongation(out: &mut String) {
    let Some(last) = out.chars().last() else {
        return;
    };
    let macron = match last {
        'a' => 'ā',
        'i' => 'ī',
        'u' => 'ū',
        'e' => 'ē',
        'o' => 'ō',
        _ => return,
    };
    out.pop();
    out.push(macron);
}

/// Try to match a mora starting at position `i` in `chars`. Returns
/// `Some((romaji, consumed))` on match, `None` otherwise.
fn try_syllable(chars: &[char], i: usize) -> Option<(String, usize)> {
    if i >= chars.len() {
        return None;
    }
    if i + 1 < chars.len() {
        if let Some(rom) = digraph_romaji(chars[i], chars[i + 1]) {
            return Some((String::from(rom), 2));
        }
    }
    single_romaji(chars[i]).map(|r| (String::from(r), 1))
}

/// Modified Hepburn digraphs (a mora starting character + small
/// ゃ/ゅ/ょ).
fn digraph_romaji(a: char, b: char) -> Option<&'static str> {
    let (small_a, small_u, small_o) = ('\u{3083}', '\u{3085}', '\u{3087}');
    Some(match (a, b) {
        // k-column.
        ('き', c) if c == small_a => "kya",
        ('き', c) if c == small_u => "kyu",
        ('き', c) if c == small_o => "kyo",
        // s-column (Hepburn: sha/shu/sho).
        ('し', c) if c == small_a => "sha",
        ('し', c) if c == small_u => "shu",
        ('し', c) if c == small_o => "sho",
        // t-column (Hepburn: cha/chu/cho).
        ('ち', c) if c == small_a => "cha",
        ('ち', c) if c == small_u => "chu",
        ('ち', c) if c == small_o => "cho",
        // n-column.
        ('に', c) if c == small_a => "nya",
        ('に', c) if c == small_u => "nyu",
        ('に', c) if c == small_o => "nyo",
        // h-column.
        ('ひ', c) if c == small_a => "hya",
        ('ひ', c) if c == small_u => "hyu",
        ('ひ', c) if c == small_o => "hyo",
        // m-column.
        ('み', c) if c == small_a => "mya",
        ('み', c) if c == small_u => "myu",
        ('み', c) if c == small_o => "myo",
        // r-column.
        ('り', c) if c == small_a => "rya",
        ('り', c) if c == small_u => "ryu",
        ('り', c) if c == small_o => "ryo",
        // Voiced g-column.
        ('ぎ', c) if c == small_a => "gya",
        ('ぎ', c) if c == small_u => "gyu",
        ('ぎ', c) if c == small_o => "gyo",
        // Voiced z-column (Hepburn: ja/ju/jo).
        ('じ', c) if c == small_a => "ja",
        ('じ', c) if c == small_u => "ju",
        ('じ', c) if c == small_o => "jo",
        // Voiced d-column (Hepburn: ja/ju/jo, matching じゃ/じゅ/じょ).
        ('ぢ', c) if c == small_a => "ja",
        ('ぢ', c) if c == small_u => "ju",
        ('ぢ', c) if c == small_o => "jo",
        // Voiced b-column.
        ('び', c) if c == small_a => "bya",
        ('び', c) if c == small_u => "byu",
        ('び', c) if c == small_o => "byo",
        // Voiced (semi-voiced) p-column.
        ('ぴ', c) if c == small_a => "pya",
        ('ぴ', c) if c == small_u => "pyu",
        ('ぴ', c) if c == small_o => "pyo",
        _ => return None,
    })
}

/// Modified Hepburn single-mora table.
///
/// The `match_same_arms` allow is deliberate: several kana pairs
/// legitimately share a romaji (`お`/`を` → `o`; `じ`/`ぢ` → `ji`;
/// `ず`/`づ` → `zu`).
#[allow(clippy::match_same_arms)]
fn single_romaji(c: char) -> Option<&'static str> {
    Some(match c {
        // Vowels.
        'あ' => "a",
        'い' => "i",
        'う' => "u",
        'え' => "e",
        'お' => "o",
        // k-column.
        'か' => "ka",
        'き' => "ki",
        'く' => "ku",
        'け' => "ke",
        'こ' => "ko",
        // s-column (Hepburn: shi, not si).
        'さ' => "sa",
        'し' => "shi",
        'す' => "su",
        'せ' => "se",
        'そ' => "so",
        // t-column (Hepburn: chi/tsu, not ti/tu).
        'た' => "ta",
        'ち' => "chi",
        'つ' => "tsu",
        'て' => "te",
        'と' => "to",
        // n-column.
        'な' => "na",
        'に' => "ni",
        'ぬ' => "nu",
        'ね' => "ne",
        'の' => "no",
        // h-column (Hepburn: fu, not hu).
        'は' => "ha",
        'ひ' => "hi",
        'ふ' => "fu",
        'へ' => "he",
        'ほ' => "ho",
        // m-column.
        'ま' => "ma",
        'み' => "mi",
        'む' => "mu",
        'め' => "me",
        'も' => "mo",
        // y-column.
        'や' => "ya",
        'ゆ' => "yu",
        'よ' => "yo",
        // r-column.
        'ら' => "ra",
        'り' => "ri",
        'る' => "ru",
        'れ' => "re",
        'ろ' => "ro",
        // w-column.
        'わ' => "wa",
        // Modern Hepburn writes を (topic marker) as `o`.
        'を' => "o",
        // Voiced g-column.
        'が' => "ga",
        'ぎ' => "gi",
        'ぐ' => "gu",
        'げ' => "ge",
        'ご' => "go",
        // Voiced z-column (Hepburn: ji, not zi).
        'ざ' => "za",
        'じ' => "ji",
        'ず' => "zu",
        'ぜ' => "ze",
        'ぞ' => "zo",
        // Voiced d-column (Hepburn: ji/zu for ぢ/づ, matching じ/ず).
        'だ' => "da",
        'ぢ' => "ji",
        'づ' => "zu",
        'で' => "de",
        'ど' => "do",
        // Voiced b-column.
        'ば' => "ba",
        'び' => "bi",
        'ぶ' => "bu",
        'べ' => "be",
        'ぼ' => "bo",
        // Voiced (semi-voiced) p-column.
        'ぱ' => "pa",
        'ぴ' => "pi",
        'ぷ' => "pu",
        'ぺ' => "pe",
        'ぽ' => "po",
        // Small forms — pass-through when they didn't attach to a
        // digraph.
        'ゃ' => "ya",
        'ゅ' => "yu",
        'ょ' => "yo",
        // ヴ (vu). Folded to hiragana ゔ (U+3094) upstream.
        '\u{3094}' => "vu",
        _ => return None,
    })
}

/// Is `b` an ASCII vowel byte?
#[inline]
const fn is_vowel_byte(b: u8) -> bool {
    matches!(b, b'a' | b'i' | b'u' | b'e' | b'o')
}

/// Should syllabic `ん` be followed by an apostrophe before this
/// character?
#[inline]
fn needs_apostrophe_after_n(c: char) -> bool {
    matches!(c, 'あ' | 'い' | 'う' | 'え' | 'お' | 'や' | 'ゆ' | 'よ')
}

/// Free-function shortcut: `to_hepburn(text)` is
/// `HepburnRomaji.romanize(text)`.
#[must_use]
pub fn to_hepburn(text: &str) -> String {
    HepburnRomaji.romanize(text)
}

/// Adapter that exposes [`HepburnRomaji`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — the type
/// [`crate::JapaneseWithHepburn::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
/// hands back.
///
/// The adapter always returns `Some((key, None))` — Hepburn is a
/// single-key encoder — and considers input with no kana content as
/// producing no phonetic key (returns `None`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HepburnRomajiAdapter;

impl LanguagePhoneticEncoder for HepburnRomajiAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_kana(word) {
            return None;
        }
        let key = HepburnRomaji.romanize(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "hepburn-romaji"
    }
}

/// Does `s` contain at least one kana scalar (hiragana, full-width
/// katakana, or half-width katakana)?
fn contains_kana(s: &str) -> bool {
    s.chars().any(|c| {
        ('\u{3041}'..='\u{309F}').contains(&c)
            || ('\u{30A0}'..='\u{30FF}').contains(&c)
            || ('\u{FF66}'..='\u{FF9F}').contains(&c)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> String {
        HepburnRomaji.romanize(s)
    }

    // ---------------------------------------------------------------
    // Basic single-mora table.
    // ---------------------------------------------------------------

    #[test]
    fn hiragana_vowels() {
        assert_eq!(r("あ"), "a");
        assert_eq!(r("い"), "i");
        assert_eq!(r("う"), "u");
        assert_eq!(r("え"), "e");
        assert_eq!(r("お"), "o");
    }

    #[test]
    fn hepburn_versus_kunrei_shibboleth_moras() {
        // The row of moras that distinguishes Hepburn from Kunrei.
        assert_eq!(r("し"), "shi");
        assert_eq!(r("ち"), "chi");
        assert_eq!(r("つ"), "tsu");
        assert_eq!(r("ふ"), "fu");
        assert_eq!(r("じ"), "ji");
    }

    #[test]
    fn hepburn_digraphs() {
        assert_eq!(r("きゃ"), "kya");
        assert_eq!(r("しゅ"), "shu");
        assert_eq!(r("ちょ"), "cho");
        assert_eq!(r("じゃ"), "ja");
    }

    // ---------------------------------------------------------------
    // Modified Hepburn reference pairs (Wikipedia table).
    // ---------------------------------------------------------------

    /// Twenty-plus reference pairs drawn from the Modified Hepburn
    /// row of the Wikipedia romanization table, covering the
    /// shibboleth moras, the long-vowel conventions, small-tsu
    /// geminates, syllabic-n apostrophes, and the prolonged-sound
    /// mark.
    #[test]
    fn modified_hepburn_reference_pairs() {
        // Basic mora sequences.
        assert_eq!(r("さくら"), "sakura");
        assert_eq!(r("すし"), "sushi");
        assert_eq!(r("ちば"), "chiba");
        assert_eq!(r("ばんざい"), "banzai");
        assert_eq!(r("たなか"), "tanaka");

        // Shibboleth digraphs.
        assert_eq!(r("しんじゅく"), "shinjuku");
        assert_eq!(r("しゅくだい"), "shukudai");
        assert_eq!(r("ちゅうがくせい"), "chūgakusei");

        // Long vowels via おう / うう / おお / ええ / ああ.
        assert_eq!(r("とうきょう"), "tōkyō");
        assert_eq!(r("きょうと"), "kyōto");
        assert_eq!(r("おおさか"), "ōsaka");
        assert_eq!(r("じゅう"), "jū");
        assert_eq!(r("おおきい"), "ōkii"); // いい stays as `ii`.
        assert_eq!(r("せんせい"), "sensei"); // えい stays as `ei`.

        // Prolonged sound mark on katakana.
        assert_eq!(r("コーヒー"), "kōhī");
        assert_eq!(r("ラーメン"), "rāmen");
        assert_eq!(r("サッカー"), "sakkā");

        // Small tsu (sokuon).
        assert_eq!(r("がっこう"), "gakkō");
        assert_eq!(r("まっちゃ"), "matcha"); // っち → tchi.

        // Syllabic n.
        assert_eq!(r("しんぶん"), "shinbun");
        assert_eq!(r("しんや"), "shin'ya");
        assert_eq!(r("かんじ"), "kanji");

        // Kana pass-through and script folding.
        assert_eq!(r("カタカナ"), "katakana");
        assert_eq!(r("ふじさん"), "fujisan");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_returns_name_hepburn() {
        assert_eq!(HepburnRomajiAdapter.name(), "hepburn-romaji");
    }

    #[test]
    fn adapter_returns_some_for_kana() {
        let out = HepburnRomajiAdapter.encode("さくら");
        assert_eq!(out, Some((String::from("sakura"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_kana() {
        assert!(HepburnRomajiAdapter.encode("").is_none());
        assert!(HepburnRomajiAdapter.encode("hello").is_none());
        assert!(HepburnRomajiAdapter.encode("日本").is_none());
        assert!(HepburnRomajiAdapter.encode("...").is_none());
    }

    #[test]
    fn to_hepburn_matches_romanize() {
        assert_eq!(to_hepburn("さくら"), HepburnRomaji.romanize("さくら"));
    }

    // ---------------------------------------------------------------
    // Half-width katakana folds through to hiragana before lookup.
    // ---------------------------------------------------------------

    #[test]
    fn halfwidth_katakana_folds_to_hiragana() {
        // ｻｸﾗ (half-width) → sakura
        assert_eq!(r("\u{FF7B}\u{FF78}\u{FF97}"), "sakura");
    }

    // ---------------------------------------------------------------
    // Pass-through.
    // ---------------------------------------------------------------

    #[test]
    fn kanji_passes_through_unchanged() {
        assert_eq!(r("日本"), "日本");
    }

    #[test]
    fn ascii_passes_through_unchanged() {
        assert_eq!(r("hello"), "hello");
    }
}
