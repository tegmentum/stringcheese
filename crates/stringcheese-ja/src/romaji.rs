//! Kunrei-shiki (訓令式) romanization — ISO 3602.
//!
//! # Origin
//!
//! Kunrei-shiki is the Japanese government's official kana-to-Latin
//! transliteration standard, promulgated by the Cabinet Order of 1954
//! and adopted internationally as **ISO 3602:1989** *Documentation —
//! Romanization of Japanese (kana script)*. It differs from Hepburn
//! (the alternative in wide English-facing use) by preserving the
//! phonological regularity of Japanese consonant series:
//!
//! | Kana | Kunrei | Hepburn |
//! |------|--------|---------|
//! | し   | `si`   | `shi`   |
//! | ち   | `ti`   | `chi`   |
//! | つ   | `tu`   | `tsu`   |
//! | ふ   | `hu`   | `fu`    |
//! | じ   | `zi`   | `ji`    |
//! | しゃ | `sya`  | `sha`   |
//! | じゃ | `zya`  | `ja`    |
//!
//! # Why Kunrei-shiki over Hepburn?
//!
//! The language pack's phonetic encoder wants a *deterministic*
//! kana → Latin mapping where the output is a stable equivalence-class
//! representative for lookup, not a pronunciation guide. Kunrei-shiki's
//! consonant regularity (same column, same consonant letter across the
//! whole row) is exactly the property a phonetic key wants: `し`, `シ`,
//! and `si` all end up with the same encoded key. Hepburn's row-specific
//! spellings (`shi` from し, `sha` from しゃ) are pronunciation-first
//! and would produce a marginally more English-readable key at the cost
//! of collision with any Latin-alphabet `shi` in the same corpus.
//!
//! Hepburn is a legitimate alternate encoder and is called out as a
//! follow-up in the crate's roadmap.
//!
//! # Algorithm
//!
//! 1. **Normalize.** Fold every half-width katakana scalar to its
//!    full-width equivalent (U+FF66..=U+FF9F → U+30A0..=U+30FF).
//! 2. **Fold katakana to hiragana.** Katakana and hiragana carry
//!    identical phonology; the mapping table is keyed on hiragana.
//! 3. **Table lookup.** For each character (or, for palatalized moras,
//!    for each *two-character* sequence like `きゃ`, `しゃ`, `じゅ`),
//!    look up the Kunrei syllable and emit it.
//! 4. **Special-case digraphs before regular scalars.**
//!    - **Small tsu (っ/ッ, U+3063 / U+30C3).** Doubles the following
//!      consonant: `かっぱ → kappa`.
//!    - **Prolonged sound mark (ー, U+30FC).** Marks the preceding vowel
//!      as long. In Kunrei-shiki the long vowel is written with a
//!      circumflex (`kôhî`). We use the ASCII fallback of doubling the
//!      vowel: `コーヒー → koohii`. This keeps the output ASCII, which
//!      matters for the phonetic encoder's "ASCII-only key" contract.
//!    - **Syllabic n (ん/ン).** Written `n`. Kunrei-shiki inserts an
//!      apostrophe before a following vowel or `y` to avoid ambiguity
//!      (`shinya → sin'ya`), matching the ISO 3602 rule.
//! 5. **Anything else passes through unchanged.**
//!
//! # Non-goals
//!
//! - **Hepburn output.** The alternate spelling table is a follow-up.
//! - **Katakana-only extension moras.** Foreign-loan phonemes like
//!   `ヴ` (vu), `ティ` (ti), `ファ` (fa), `フォ` (fo), etc., are handled
//!   only to the extent that they're in the standard mora table
//!   (`ヴ → vu`); more exotic ones fall through as-is.
//! - **Kanji.** This encoder only romanizes kana. Kanji characters pass
//!   through unchanged — the ASCII-only output guarantee applies only
//!   to the *kana* content; kanji remain as-is. Callers who need
//!   kanji-to-reading conversion need a dictionary (`kakasi` /
//!   `kuromoji`-scale infrastructure) that this crate deliberately
//!   does not ship.
//! - **Compound-word boundary insertion.** Kunrei-shiki has no formal
//!   spec for word boundaries; the caller controls tokenization.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Kunrei-shiki romanizer.
///
/// A zero-sized value; construct as [`KunreiRomaji`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the mapping tables and the
/// small-tsu / prolonged-sound / syllabic-n handling.
///
/// # Example
///
/// ```
/// use stringcheese_ja::KunreiRomaji;
///
/// assert_eq!(KunreiRomaji.romanize("さくら"), "sakura");
/// assert_eq!(KunreiRomaji.romanize("シ"), "si");
/// assert_eq!(KunreiRomaji.romanize("かっぱ"), "kappa");
/// assert_eq!(KunreiRomaji.romanize("コーヒー"), "koohii");
/// assert_eq!(KunreiRomaji.romanize("しんや"), "sin'ya");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KunreiRomaji;

impl KunreiRomaji {
    /// Romanize `text` per Kunrei-shiki (ISO 3602).
    ///
    /// The output is ASCII when the input is entirely kana; kanji and
    /// other non-kana scalars pass through unchanged (the encoder does
    /// not consult a dictionary).
    #[must_use]
    pub fn romanize(&self, text: &str) -> String {
        // Step 1 & 2: fold to hiragana. We collect into a Vec<char>
        // for two-character lookahead (digraphs like きゃ) and for the
        // small-tsu / long-vowel rules that need positional peeking.
        //
        // Working with `char` values (not byte offsets) is fine: kana
        // are 3 bytes each in UTF-8, and the mapping tables are keyed
        // on hiragana scalars.
        let hiragana: alloc::vec::Vec<char> = text.chars().map(katakana_to_hiragana).collect();

        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < hiragana.len() {
            let c = hiragana[i];

            // Small tsu (U+3063 っ) — double the initial consonant of
            // the next mora.
            if c == '\u{3063}' {
                if let Some(next_syll) = try_syllable(&hiragana, i + 1) {
                    let (syllable, consumed) = next_syll;
                    let first = syllable.as_bytes()[0];
                    // Only double if the next syllable starts with a
                    // consonant. If the next syllable begins with a
                    // vowel (rare / malformed), fall back to writing a
                    // literal `t` (an approximation) and continuing.
                    if first.is_ascii_lowercase() && !is_vowel_byte(first) {
                        out.push(first as char);
                        out.push_str(&syllable);
                        i += 1 + consumed;
                        continue;
                    }
                    // Vowel-initial follower: emit `t` and let the
                    // outer loop handle the vowel on the next pass.
                    out.push('t');
                    i += 1;
                    continue;
                }
                // Trailing small tsu with nothing to double — emit `t`
                // as the closest single-letter proxy.
                out.push('t');
                i += 1;
                continue;
            }

            // Prolonged sound mark (U+30FC ー) — double the previous
            // vowel. This is the ASCII-fallback convention; a full
            // Kunrei-shiki implementation would emit the circumflexed
            // form (â/î/û/ê/ô).
            if c == '\u{30FC}' {
                if let Some(last) = out.chars().next_back() {
                    if is_vowel_byte(last as u8) {
                        out.push(last);
                        i += 1;
                        continue;
                    }
                }
                // No preceding vowel to lengthen — the mark has no
                // meaning in isolation. Drop it silently (this keeps
                // the output ASCII, which the phonetic-encoder
                // contract requires; the alternative — passing the
                // non-ASCII mark through — would break that
                // guarantee).
                i += 1;
                continue;
            }

            // Syllabic n (U+3093 ん) — emit `n`, plus an apostrophe
            // before a following vowel or `y` per ISO 3602.
            if c == '\u{3093}' {
                out.push('n');
                if let Some(next) = hiragana.get(i + 1).copied() {
                    // The apostrophe rule fires when the next mora
                    // begins with a vowel or `y`. In hiragana that
                    // means: あいうえお (a/i/u/e/o) or the や-row
                    // (ya/yu/yo).
                    if needs_apostrophe_after_n(next) {
                        out.push('\'');
                    }
                }
                i += 1;
                continue;
            }

            // Regular mora — try the two-character digraphs first
            // (きゃ, しゃ, じゅ, …), then the single-character mora.
            if let Some((syllable, consumed)) = try_syllable(&hiragana, i) {
                out.push_str(&syllable);
                i += consumed;
                continue;
            }

            // Pass-through: kanji, ASCII, punctuation, or an
            // unrecognized scalar.
            out.push(c);
            i += 1;
        }
        out
    }
}

/// Fold a half-width or full-width katakana scalar to the equivalent
/// hiragana scalar. Non-kana input is returned unchanged.
#[inline]
fn katakana_to_hiragana(c: char) -> char {
    // Half-width katakana → full-width katakana is not a fixed offset,
    // so we handle it with a small table (only the common moras — for
    // less-common half-width forms the fall-through is fine because
    // the caller will typically get half-width text only from legacy
    // input methods).
    if let Some(fw) = halfwidth_to_fullwidth_katakana(c) {
        return katakana_to_hiragana(fw);
    }
    // Full-width katakana → hiragana is a fixed offset of 96 code
    // points. Range U+30A1..=U+30F6 maps to U+3041..=U+3096.
    // (U+30FB, U+30FC, U+30FD, U+30FE, U+30F7..U+30FA fall outside
    // the fold — the middle dot, prolonged mark, iteration marks, and
    // the vu-family are left alone.)
    if ('\u{30A1}'..='\u{30F6}').contains(&c) {
        // Safe: c is a valid Unicode scalar and c - 96 lands inside
        // the hiragana block.
        let cp = c as u32 - 96;
        return char::from_u32(cp).unwrap_or(c);
    }
    c
}

/// Map a half-width katakana scalar (U+FF66..=U+FF9F) to its
/// closest full-width katakana equivalent. Returns `None` for scalars
/// outside the half-width range or for the small handful of half-width
/// forms with no single-character full-width equivalent.
#[inline]
fn halfwidth_to_fullwidth_katakana(c: char) -> Option<char> {
    // Table adapted from Unicode Standard Annex #11 (East Asian Width)
    // reference chart; each row is `(half-width, full-width)`.
    // Only the mora-carrying scalars are listed — dakuten (ﾞ, U+FF9E)
    // and handakuten (ﾟ, U+FF9F) are combining marks and are left
    // as-is (they don't romanize to anything on their own).
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

/// Try to match a mora starting at position `i` in `chars`. Returns
/// `Some((romaji, consumed))` on match; `None` if the character at `i`
/// is not a recognized hiragana mora.
fn try_syllable(chars: &[char], i: usize) -> Option<(alloc::string::String, usize)> {
    if i >= chars.len() {
        return None;
    }
    // Two-character digraph (きゃ, しゃ, ...) — small ゃ/ゅ/ょ after
    // an -i mora.
    if i + 1 < chars.len() {
        if let Some(rom) = digraph_romaji(chars[i], chars[i + 1]) {
            return Some((alloc::string::String::from(rom), 2));
        }
    }
    // Single mora.
    single_romaji(chars[i]).map(|r| (alloc::string::String::from(r), 1))
}

/// Return the Kunrei-shiki romaji for a hiragana digraph (mora starting
/// character + small ゃ/ゅ/ょ), or `None` if the pair is not a digraph.
fn digraph_romaji(a: char, b: char) -> Option<&'static str> {
    // Small ゃ (U+3083), ゅ (U+3085), ょ (U+3087).
    let (small_a, small_u, small_o) = ('\u{3083}', '\u{3085}', '\u{3087}');
    Some(match (a, b) {
        // k-column
        ('き', c) if c == small_a => "kya",
        ('き', c) if c == small_u => "kyu",
        ('き', c) if c == small_o => "kyo",
        // s-column (Kunrei: sya/syu/syo, not sha/shu/sho)
        ('し', c) if c == small_a => "sya",
        ('し', c) if c == small_u => "syu",
        ('し', c) if c == small_o => "syo",
        // t-column (Kunrei: tya/tyu/tyo, not cha/chu/cho)
        ('ち', c) if c == small_a => "tya",
        ('ち', c) if c == small_u => "tyu",
        ('ち', c) if c == small_o => "tyo",
        // n-column
        ('に', c) if c == small_a => "nya",
        ('に', c) if c == small_u => "nyu",
        ('に', c) if c == small_o => "nyo",
        // h-column
        ('ひ', c) if c == small_a => "hya",
        ('ひ', c) if c == small_u => "hyu",
        ('ひ', c) if c == small_o => "hyo",
        // m-column
        ('み', c) if c == small_a => "mya",
        ('み', c) if c == small_u => "myu",
        ('み', c) if c == small_o => "myo",
        // r-column
        ('り', c) if c == small_a => "rya",
        ('り', c) if c == small_u => "ryu",
        ('り', c) if c == small_o => "ryo",
        // Voiced series.
        ('ぎ', c) if c == small_a => "gya",
        ('ぎ', c) if c == small_u => "gyu",
        ('ぎ', c) if c == small_o => "gyo",
        // Kunrei writes じゃ/じゅ/じょ as zya/zyu/zyo (Hepburn ja/ju/jo).
        ('じ', c) if c == small_a => "zya",
        ('じ', c) if c == small_u => "zyu",
        ('じ', c) if c == small_o => "zyo",
        // Kunrei writes ぢゃ/ぢゅ/ぢょ as dya/dyu/dyo (Hepburn ja/ju/jo).
        ('ぢ', c) if c == small_a => "dya",
        ('ぢ', c) if c == small_u => "dyu",
        ('ぢ', c) if c == small_o => "dyo",
        ('び', c) if c == small_a => "bya",
        ('び', c) if c == small_u => "byu",
        ('び', c) if c == small_o => "byo",
        ('ぴ', c) if c == small_a => "pya",
        ('ぴ', c) if c == small_u => "pyu",
        ('ぴ', c) if c == small_o => "pyo",
        _ => return None,
    })
}

/// Return the Kunrei-shiki romaji for a single hiragana mora, or `None`
/// if `c` is not a recognized single-mora hiragana scalar.
///
/// The `match_same_arms` allow is deliberate: several kana pairs
/// legitimately map to the same romaji (`お`/`を` both to `o`;
/// `じ`/`ぢ` both to `zi`; `ず`/`づ` both to `zu`; the small `ゃゅょ`
/// pass through as `ya`/`yu`/`yo` when they appear outside a digraph
/// context). Collapsing the arms would obscure which kana produced
/// which romaji and would make future edits (e.g., adding a
/// `di`/`du` alternate for `ぢ`/`づ`) needlessly disruptive.
#[allow(clippy::match_same_arms)]
fn single_romaji(c: char) -> Option<&'static str> {
    // Kunrei-shiki basic table (ISO 3602).
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
        // s-column (Kunrei: si, not shi).
        'さ' => "sa",
        'し' => "si",
        'す' => "su",
        'せ' => "se",
        'そ' => "so",
        // t-column (Kunrei: ti/tu, not chi/tsu).
        'た' => "ta",
        'ち' => "ti",
        'つ' => "tu",
        'て' => "te",
        'と' => "to",
        // n-column.
        'な' => "na",
        'に' => "ni",
        'ぬ' => "nu",
        'ね' => "ne",
        'の' => "no",
        // h-column (Kunrei: hu, not fu).
        'は' => "ha",
        'ひ' => "hi",
        'ふ' => "hu",
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
        'を' => "o", // Kunrei-shiki writes を as `o` (topic marker); ISO 3602 allows `wo` as an alternate.
        // Voiced g-column.
        'が' => "ga",
        'ぎ' => "gi",
        'ぐ' => "gu",
        'げ' => "ge",
        'ご' => "go",
        // Voiced z-column (Kunrei: zi, not ji).
        'ざ' => "za",
        'じ' => "zi",
        'ず' => "zu",
        'ぜ' => "ze",
        'ぞ' => "zo",
        // Voiced d-column (Kunrei: zi/zu for ぢ/づ, matching じ/ず;
        // the transcriptions preserve the mora's orthographic identity
        // via `di`/`du` in one alternate spec, but ISO 3602 §5 rule 2
        // permits `zi`/`zu`).
        'だ' => "da",
        'ぢ' => "zi",
        'づ' => "zu",
        'で' => "de",
        'ど' => "do",
        // Voiced b-column.
        'ば' => "ba",
        'び' => "bi",
        'ぶ' => "bu",
        'べ' => "be",
        'ぼ' => "bo",
        // Voiced p-column (semi-voiced).
        'ぱ' => "pa",
        'ぴ' => "pi",
        'ぷ' => "pu",
        'ぺ' => "pe",
        'ぽ' => "po",
        // Small forms — pass-through when they didn't attach to a
        // digraph. These are rare in isolation.
        'ゃ' => "ya",
        'ゅ' => "yu",
        'ょ' => "yo",
        // ヴ (vu) is katakana-only; it folds to hiragana at U+3094.
        '\u{3094}' => "vu",
        _ => return None,
    })
}

/// Is byte `b` an ASCII vowel (`a`, `i`, `u`, `e`, `o`)?
#[inline]
const fn is_vowel_byte(b: u8) -> bool {
    matches!(b, b'a' | b'i' | b'u' | b'e' | b'o')
}

/// Should syllabic `ん` be followed by an apostrophe before this
/// character (ISO 3602 disambiguation rule)?
#[inline]
fn needs_apostrophe_after_n(c: char) -> bool {
    // Direct hiragana vowels and the や-row triggers the apostrophe.
    // (The rule prevents `sinya` from being read as `si-nya` vs.
    // `sin-ya`.)
    matches!(c, 'あ' | 'い' | 'う' | 'え' | 'お' | 'や' | 'ゆ' | 'よ')
}

/// Adapter that exposes [`KunreiRomaji`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Japanese::phonetic_encoder`](crate::Japanese) hands back.
///
/// The adapter always returns `Some((key, None))` — Kunrei-shiki is a
/// single-key encoder — and considers an all-non-kana input (kanji-only
/// text, ASCII-only text) as producing no phonetic key (returns
/// `None`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KunreiRomajiAdapter;

impl LanguagePhoneticEncoder for KunreiRomajiAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        // If the input has no kana content at all, the romanizer would
        // return the input unchanged (pass-through) — that isn't a
        // phonetic key. The trait's `encode` contract says "return
        // None if the algorithm cannot produce a key".
        if !contains_kana(word) {
            return None;
        }
        let key = KunreiRomaji.romanize(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "kunrei-romaji"
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
        KunreiRomaji.romanize(s)
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
    fn hiragana_k_row() {
        assert_eq!(r("か"), "ka");
        assert_eq!(r("き"), "ki");
        assert_eq!(r("く"), "ku");
        assert_eq!(r("け"), "ke");
        assert_eq!(r("こ"), "ko");
    }

    #[test]
    fn kunrei_versus_hepburn_shibboleth_moras() {
        // The row of moras that distinguishes Kunrei from Hepburn.
        assert_eq!(r("し"), "si"); // NOT shi
        assert_eq!(r("ち"), "ti"); // NOT chi
        assert_eq!(r("つ"), "tu"); // NOT tsu
        assert_eq!(r("ふ"), "hu"); // NOT fu
        assert_eq!(r("じ"), "zi"); // NOT ji
    }

    #[test]
    fn katakana_folds_to_hiragana_before_lookup() {
        assert_eq!(r("シ"), "si");
        assert_eq!(r("カタカナ"), "katakana");
    }

    // ---------------------------------------------------------------
    // Digraphs.
    // ---------------------------------------------------------------

    #[test]
    fn kunrei_digraphs() {
        assert_eq!(r("きゃ"), "kya");
        assert_eq!(r("しゅ"), "syu"); // NOT shu
        assert_eq!(r("ちょ"), "tyo"); // NOT cho
        assert_eq!(r("じゃ"), "zya"); // NOT ja
    }

    // ---------------------------------------------------------------
    // Small tsu (sokuon).
    // ---------------------------------------------------------------

    #[test]
    fn small_tsu_doubles_next_consonant() {
        assert_eq!(r("かっぱ"), "kappa");
        assert_eq!(r("しっぽ"), "sippo");
        assert_eq!(r("いっき"), "ikki");
    }

    // ---------------------------------------------------------------
    // Prolonged sound mark.
    // ---------------------------------------------------------------

    #[test]
    fn prolonged_sound_mark_doubles_vowel() {
        assert_eq!(r("コーヒー"), "koohii");
        assert_eq!(r("ケーキ"), "keeki");
    }

    // ---------------------------------------------------------------
    // Syllabic n.
    // ---------------------------------------------------------------

    #[test]
    fn syllabic_n_bare() {
        assert_eq!(r("ほん"), "hon");
        assert_eq!(r("かんじ"), "kanzi");
    }

    #[test]
    fn syllabic_n_apostrophe_before_vowel_or_y() {
        assert_eq!(r("しんや"), "sin'ya");
        assert_eq!(r("こんいち"), "kon'iti");
    }

    #[test]
    fn syllabic_n_no_apostrophe_before_consonant() {
        // `hondana` — n followed by d, no apostrophe.
        assert_eq!(r("ほんだな"), "hondana");
    }

    // ---------------------------------------------------------------
    // Pass-through.
    // ---------------------------------------------------------------

    #[test]
    fn kanji_passes_through_unchanged() {
        // The encoder does not consult a dictionary.
        assert_eq!(r("日本"), "日本");
    }

    #[test]
    fn ascii_passes_through_unchanged() {
        assert_eq!(r("hello"), "hello");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_returns_name_kunrei() {
        assert_eq!(KunreiRomajiAdapter.name(), "kunrei-romaji");
    }

    #[test]
    fn adapter_returns_some_for_kana() {
        let out = KunreiRomajiAdapter.encode("さくら");
        assert_eq!(out, Some((String::from("sakura"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_kana() {
        assert!(KunreiRomajiAdapter.encode("").is_none());
        assert!(KunreiRomajiAdapter.encode("hello").is_none());
        assert!(KunreiRomajiAdapter.encode("日本").is_none());
        assert!(KunreiRomajiAdapter.encode("...").is_none());
    }

    #[test]
    fn halfwidth_katakana_folds_to_hiragana() {
        // ｻｸﾗ (half-width) → sakura
        assert_eq!(r("\u{FF7B}\u{FF78}\u{FF97}"), "sakura");
    }
}
