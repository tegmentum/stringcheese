//! Ukrainian Cyrillic → Latin transliteration.
//!
//! # Origin
//!
//! Ukrainian has no widely established Soundex / Metaphone-family
//! phonetic encoder. What Ukrainian *does* have is a well-known
//! **transliteration** family: ISO 9, GOST 7.79, the Ukrainian
//! Government's 2010 national transliteration, BGN/PCGN, and several
//! passport systems, all of which map Cyrillic scalars to Latin ones.
//! This module ships an adaptation of GOST 7.79 System B tailored to
//! the Ukrainian letter set.
//!
//! # Standard chosen: GOST 7.79-2000 System B, Ukrainian adaptation
//!
//! GOST 7.79 (an ISO 9-adjacent Slavic-Cyrillic standard) originally
//! covered Russian; extensions were later added for the other Slavic
//! Cyrillic alphabets, but the Ukrainian portion is not as widely
//! adopted as the Russian one. This module ships a **Ukrainian
//! adaptation** of System B — a context-free, ASCII-only mapping
//! designed for byte-oriented indexes and search interop — with the
//! following Ukrainian-specific choices:
//!
//! * **`г → h`** and **`ґ → g`**. Ukrainian phonology draws a
//!   distinction that Russian does not: `г` is a voiced glottal
//!   fricative /ɦ/ (best rendered as `h`), and `ґ` is a voiced velar
//!   stop /ɡ/ (best rendered as `g`). Russian GOST 7.79-B collapses
//!   `г → g` because Russian only has the velar stop. This is the
//!   most consequential Ukrainian-vs-Russian divergence in the table.
//! * **`є → ye`**. `є` is a distinct Ukrainian letter representing
//!   /je/, absent from the Russian inventory (Russian uses `е` for the
//!   same sound in some positions and a bare /e/ in others). `ye`
//!   preserves the /j/-onset that distinguishes it from `е`.
//! * **`і → i`**. Ukrainian `і` is the /i/ vowel; its Russian
//!   counterpart is `и`. Straight `i` is the natural ASCII rendering.
//! * **`ї → yi`**. `ї` is a distinct Ukrainian letter representing
//!   /ji/, absent from Russian. `yi` preserves the /j/-onset the same
//!   way `ye` does for `є`.
//! * **`и → y`**. Ukrainian `и` is /ɪ/ (a lax high-front vowel), not
//!   the /i/ of Russian `и`. `y` is the standard Ukrainian
//!   romanization and matches every published Ukrainian
//!   transliteration system.
//! * **`х → kh`**. Ukrainian romanization convention favours the
//!   digraph `kh` over the single-letter `x` used in Russian GOST-B.
//!   The two are pronounced the same /x/, but Ukrainian romanization
//!   traditions (including the 2010 national standard) always spell
//!   it out.
//! * **`щ → shch`**. Ukrainian `щ` is pronounced /ʃtʃ/ — a fricative
//!   plus affricate cluster — and is uniformly romanized `shch` in
//!   Ukrainian conventions. Russian's simpler `shh` is not used here.
//!
//! The letters **`ъ` (hard sign, U+044A)**, **`ы` (U+044B)**,
//! **`ё` (U+0451)**, and **`э` (U+044D)** are **not part of the
//! Ukrainian alphabet** and pass through unchanged — matching the
//! Russian pack's behaviour for its non-Russian Cyrillic scalars
//! (Ukrainian `і`, Belarusian `ў`, etc.).
//!
//! Adapter name: `"gost-7.79-b-uk"`. The `-uk` disambiguates it from
//! the Russian `"gost-7.79-b"` — the two encoders differ on `г` /
//! `ґ` / `х` / `щ` and cover disjoint letter sets, so callers should
//! never mix them.
//!
//! # Why a transliteration instead of a Soundex?
//!
//! The core StringCheese phonetic subsystem is Soundex-shaped —
//! collapse a name to an equivalence class the caller can index on.
//! Ukrainian's phoneme inventory does not admit a well-established
//! Soundex table the way English's does. GOST 7.79-B (Ukrainian
//! adaptation) is the honest baseline: a **deterministic,
//! reversible-with-caveats, ASCII-only** equivalence class that plays
//! well with byte-oriented indexes and downstream search interop.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words
//! that sound alike do not necessarily encode alike (there are no
//! Soundex-style consonant merges). But it *is* a stable,
//! deterministic, ASCII-only key the language trait's
//! `phonetic_encoder` accessor accepts, and for Ukrainian that is the
//! honest baseline.
//!
//! # Mapping table
//!
//! The full lowercase mapping — uppercase inputs are lowercased before
//! the table is consulted (Ukrainian `А…Я`, `Ґ`, `Є`, `І`, `Ї` fold to
//! `а…я`, `ґ`, `є`, `і`, `ї` under default Unicode rules; there is no
//! locale-specific tailoring):
//!
//! | Cyrillic | Latin (Ukrainian GOST 7.79-B) |
//! |----------|-------------------------------|
//! | `а`     | `a`   |
//! | `б`     | `b`   |
//! | `в`     | `v`   |
//! | `г`     | `h`   |
//! | `ґ`     | `g`   |
//! | `д`     | `d`   |
//! | `е`     | `e`   |
//! | `є`     | `ye`  |
//! | `ж`     | `zh`  |
//! | `з`     | `z`   |
//! | `и`     | `y`   |
//! | `і`     | `i`   |
//! | `ї`     | `yi`  |
//! | `й`     | `j`   |
//! | `к`     | `k`   |
//! | `л`     | `l`   |
//! | `м`     | `m`   |
//! | `н`     | `n`   |
//! | `о`     | `o`   |
//! | `п`     | `p`   |
//! | `р`     | `r`   |
//! | `с`     | `s`   |
//! | `т`     | `t`   |
//! | `у`     | `u`   |
//! | `ф`     | `f`   |
//! | `х`     | `kh`  |
//! | `ц`     | `cz`  |
//! | `ч`     | `ch`  |
//! | `ш`     | `sh`  |
//! | `щ`     | `shch` |
//! | `ь`     | `'`   |
//! | `ю`     | `yu`  |
//! | `я`     | `ya`  |
//!
//! # Byte-length caveat
//!
//! Every Cyrillic scalar in the Ukrainian block is **2 bytes** in
//! UTF-8 (U+0400..=U+04FF and the U+0491 / U+0490 `ґ` / `Ґ` pair all
//! fall in the 2-byte range). The encoder walks characters via
//! [`str::chars`](str::chars), never raw bytes, so it never risks
//! slicing a scalar apart. Callers that want to compare an encoded
//! output against an original word by byte offset must remember that
//! the ASCII output is 1 byte per character while the Cyrillic input
//! is 2 bytes per character.
//!
//! # Non-Cyrillic pass-through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the Ukrainian mapping pass through unchanged. Mixed-script
//! input carries its non-Cyrillic characters through as-is. Russian-
//! only letters (`ъ`, `ы`, `ё`, `э`) also pass through — the encoder
//! is scoped to the Ukrainian alphabet.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Ukrainian GOST 7.79 System B (Ukrainian adaptation) Cyrillic →
/// Latin transliteration.
///
/// A zero-sized value; construct as [`UkrainianGost779B`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and the
/// rationale behind the Ukrainian-specific choices (`г → h`, `ґ → g`,
/// `є → ye`, `ї → yi`, `х → kh`, `щ → shch`, `и → y`).
///
/// # Example
///
/// ```
/// use stringcheese_uk::UkrainianGost779B;
///
/// // "Київ" → "kyyiv" (к → k, и → y, ї → yi, в → v).
/// assert_eq!(UkrainianGost779B.encode("Київ"), "kyyiv");
/// // "Ґудзик" → "gudzyk" (ґ → g, distinguishing it from Russian г).
/// assert_eq!(UkrainianGost779B.encode("Ґудзик"), "gudzyk");
/// // "Харків" → "kharkiv" (х → kh, per Ukrainian convention).
/// assert_eq!(UkrainianGost779B.encode("Харків"), "kharkiv");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UkrainianGost779B;

impl UkrainianGost779B {
    /// Encode `text` per the GOST 7.79 System B (Ukrainian) transliteration.
    ///
    /// Every Cyrillic scalar in the mapping is replaced by its ASCII
    /// counterpart (one or more ASCII characters); every scalar
    /// outside the mapping (ASCII, digits, punctuation, other-script
    /// letters, Russian-only Cyrillic letters) passes through
    /// unchanged.
    ///
    /// The encoder lowercases the input first — the output is always
    /// lowercase-ASCII (plus pass-through characters).
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Preallocate roughly 2x — most Cyrillic scalars map to 1..4
        // ASCII characters, and the input's byte count is already 2x
        // the character count for pure Ukrainian text.
        let mut out = String::with_capacity(text.len().saturating_mul(2));
        for c in text.chars() {
            // Lowercase first — Cyrillic case-fold is well-behaved
            // under default Unicode rules; unlike Turkish there is no
            // locale tailoring to apply.
            for lc in c.to_lowercase() {
                if let Some(ascii) = cyrillic_to_gost_b(lc) {
                    out.push_str(ascii);
                } else {
                    out.push(lc);
                }
            }
        }
        out
    }
}

/// Map a Ukrainian Cyrillic scalar to its GOST 7.79-B (Ukrainian
/// adaptation) ASCII string, or `None` if the scalar is outside the
/// Ukrainian Cyrillic mapping.
///
/// See the [module-level table](self#mapping-table) for the full list.
/// Input is assumed to already be lowercase; the encoder's public
/// entry point ([`UkrainianGost779B::encode`]) handles the fold.
#[must_use]
pub const fn cyrillic_to_gost_b(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "h",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'є' => "ye",
        'ж' => "zh",
        'з' => "z",
        'и' => "y",
        'і' => "i",
        'ї' => "yi",
        'й' => "j",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "cz",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ь' => "'",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Adapter that exposes [`UkrainianGost779B`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Ukrainian::phonetic_encoder`](crate::Ukrainian) hands back.
///
/// The adapter always returns `Some((key, None))` — GOST 7.79-B is a
/// single-key encoder — and returns `None` for input with no Cyrillic
/// content (the transliteration would pass through unchanged, which is
/// not a useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UkrainianGost779BAdapter;

impl LanguagePhoneticEncoder for UkrainianGost779BAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_cyrillic(word) {
            return None;
        }
        let key = UkrainianGost779B.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "gost-7.79-b-uk"
    }
}

/// Does `s` contain at least one scalar in the Cyrillic UTF-8 block
/// (U+0400..=U+04FF)?
///
/// This is a superset of the Ukrainian mapping — it includes non-
/// Ukrainian Cyrillic (Russian `ъ`, Belarusian `ў`, Serbian `љ`, …)
/// that the mapping does not cover — but that is the right shape for
/// the adapter: any Cyrillic-block character makes the input
/// "Cyrillic content" for phonetic-key purposes.
fn contains_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        UkrainianGost779B.encode(s)
    }

    #[test]
    fn empty_input_encodes_to_empty() {
        assert_eq!(e(""), "");
    }

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e("hello"), "hello");
        assert_eq!(e("123"), "123");
    }

    #[test]
    fn common_place_names() {
        assert_eq!(e("Київ"), "kyyiv");
        assert_eq!(e("Львів"), "l'viv");
        assert_eq!(e("Харків"), "kharkiv");
        assert_eq!(e("Україна"), "ukrayina");
    }

    #[test]
    fn every_ukrainian_letter_has_a_mapping() {
        for c in [
            'а', 'б', 'в', 'г', 'ґ', 'д', 'е', 'є', 'ж', 'з', 'и', 'і', 'ї', 'й', 'к', 'л', 'м',
            'н', 'о', 'п', 'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ь', 'ю', 'я',
        ] {
            assert!(
                cyrillic_to_gost_b(c).is_some(),
                "no Ukrainian GOST 7.79-B mapping for {c:?}"
            );
        }
    }

    #[test]
    fn russian_only_letters_pass_through() {
        // The Ukrainian mapping deliberately excludes ъ, ы, ё, э —
        // they should pass through unchanged rather than get a
        // Russian-style transliteration.
        assert_eq!(cyrillic_to_gost_b('ъ'), None);
        assert_eq!(cyrillic_to_gost_b('ы'), None);
        assert_eq!(cyrillic_to_gost_b('ё'), None);
        assert_eq!(cyrillic_to_gost_b('э'), None);
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        // "ҐУДЗИК" → "gudzyk".
        assert_eq!(e("ҐУДЗИК"), "gudzyk");
    }

    #[test]
    fn g_h_distinction() {
        // The Ukrainian-vs-Russian divergence: г → h, ґ → g.
        assert_eq!(e("гора"), "hora");
        assert_eq!(e("ґанок"), "ganok");
    }

    #[test]
    fn ie_yi_encodings() {
        // є → ye, ї → yi — Ukrainian-only letters.
        assert_eq!(e("Європа"), "yevropa");
        assert_eq!(e("Їжак"), "yizhak");
    }

    #[test]
    fn shch_ch_sh_ts_encodings() {
        // ш → sh, щ → shch (Ukrainian /ʃtʃ/), ч → ch, ц → cz.
        assert_eq!(e("щастя"), "shchastya");
        assert_eq!(e("час"), "chas");
        assert_eq!(e("шапка"), "shapka");
        assert_eq!(e("цар"), "czar");
    }

    #[test]
    fn kh_encoding() {
        // х → kh, per Ukrainian convention (Russian uses x in GOST-B).
        assert_eq!(e("хата"), "khata");
    }

    #[test]
    fn soft_sign_encodes_to_apostrophe() {
        // ь → '. Word "мати" has no soft sign; "мить" (moment) does.
        assert_eq!(e("мить"), "myt'");
    }

    #[test]
    fn mixed_content_passes_through_non_cyrillic() {
        assert_eq!(e("hello Київ"), "hello kyyiv");
        assert_eq!(e("рік 2026"), "rik 2026");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_name_is_gost_779_b_uk() {
        assert_eq!(UkrainianGost779BAdapter.name(), "gost-7.79-b-uk");
    }

    #[test]
    fn adapter_returns_some_for_cyrillic() {
        let out = UkrainianGost779BAdapter.encode("Київ");
        assert_eq!(out, Some((String::from("kyyiv"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_cyrillic() {
        assert!(UkrainianGost779BAdapter.encode("").is_none());
        assert!(UkrainianGost779BAdapter.encode("hello").is_none());
        assert!(UkrainianGost779BAdapter.encode("123").is_none());
    }
}
