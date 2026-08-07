//! Russian Cyrillic → Latin transliteration.
//!
//! # Origin
//!
//! Russian has no widely established Soundex / Metaphone-family
//! phonetic encoder in the way English or German do. What Russian *does*
//! have is a well-known **transliteration** family: ISO 9, GOST 7.79,
//! ALA-LC, BGN/PCGN, and several passport systems, all of which map
//! Cyrillic scalars to Latin ones. This module ships one of them.
//!
//! # Standard chosen: GOST 7.79-2000 System B
//!
//! GOST 7.79 (an ISO 9-adjacent Russian standard) defines two systems:
//!
//! * **System A** — a 1:1 diacritic-rich mapping (e.g. `ж → ž`,
//!   `ч → č`) that is bijective but requires the target alphabet to
//!   carry Latin-with-caron characters. Good for library cataloguing;
//!   awkward for byte-oriented indexes.
//! * **System B** — a multi-character ASCII-only mapping (e.g.
//!   `ж → zh`, `ч → ch`, `щ → shh`) that stays inside plain ASCII
//!   while preserving the source's character content. Good for
//!   byte-oriented indexes, URLs, ASCII-safe search interop.
//!
//! This crate ships **System B**. The output is guaranteed ASCII when
//! the input is entirely composed of scalars in the Russian Cyrillic
//! block; non-Cyrillic input passes through unchanged. Adapter name:
//! `"gost-7.79-b"`.
//!
//! # Why a transliteration instead of a Soundex?
//!
//! The core StringCheese phonetic subsystem is Soundex-shaped —
//! collapse a name to an equivalence class the caller can index on.
//! Russian's phoneme inventory (35 native phonemes across ~33 letters)
//! does not admit a well-established Soundex table the way English's
//! does. GOST 7.79-B is the honest baseline: a **deterministic,
//! reversible-with-caveats, ASCII-only** equivalence class that plays
//! well with byte-oriented indexes and downstream search interop.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words that
//! sound alike do not necessarily encode alike (there are no soundex-style
//! consonant merges). But it *is* a stable, deterministic, ASCII-only
//! key the language trait's `phonetic_encoder` accessor accepts, and
//! for Russian that is the honest baseline. A future
//! `stringcheese-ru` release could add a true Slavic-Metaphone
//! variant alongside this one.
//!
//! # Mapping table
//!
//! The full lowercase mapping — uppercase inputs are lowercased before
//! the table is consulted (Cyrillic `А…Я` and `Ё` fold to `а…я` and
//! `ё` under default Unicode rules; there is no locale-specific
//! tailoring):
//!
//! | Cyrillic | Latin (GOST 7.79-B) |
//! |----------|---------------------|
//! | `а`     | `a`                 |
//! | `б`     | `b`                 |
//! | `в`     | `v`                 |
//! | `г`     | `g`                 |
//! | `д`     | `d`                 |
//! | `е`     | `e`                 |
//! | `ё`     | `yo`                |
//! | `ж`     | `zh`                |
//! | `з`     | `z`                 |
//! | `и`     | `i`                 |
//! | `й`     | `j`                 |
//! | `к`     | `k`                 |
//! | `л`     | `l`                 |
//! | `м`     | `m`                 |
//! | `н`     | `n`                 |
//! | `о`     | `o`                 |
//! | `п`     | `p`                 |
//! | `р`     | `r`                 |
//! | `с`     | `s`                 |
//! | `т`     | `t`                 |
//! | `у`     | `u`                 |
//! | `ф`     | `f`                 |
//! | `х`     | `x`                 |
//! | `ц`     | `cz`                |
//! | `ч`     | `ch`                |
//! | `ш`     | `sh`                |
//! | `щ`     | `shh`               |
//! | `ъ`     | `''`  (double apostrophe) |
//! | `ы`     | `y`                 |
//! | `ь`     | `'`                 |
//! | `э`     | `e'`                |
//! | `ю`     | `yu`                |
//! | `я`     | `ya`                |
//!
//! **Notes on the specific choices.**
//!
//! * The standard's context-sensitive `ц → c` (before `е`, `и`, `й`,
//!   `ь`, `ю`, `я`) rule is **simplified away** — this crate always
//!   emits `cz` for `ц`, so the mapping is context-free and easy to
//!   reason about. The lossy consequence is that a call site that
//!   needs to invert `cz` back to `ц` in isolation cannot; a call
//!   site that only needs a stable, deterministic ASCII key does not
//!   care.
//! * `ы → y` and `ь → '` — the standard's `y'` for `ы` is folded to
//!   plain `y` because `й → j` already claims a distinct letter and
//!   the apostrophe carries no information for byte-oriented indexes.
//!   The lossy consequence is that a call site cannot recover `ы`
//!   from a bare `y` (it might have been `ы` or something else); the
//!   simpler mapping is worth the ambiguity.
//! * The soft-sign `ь` and hard-sign `ъ` are represented with
//!   apostrophes because they carry no phonetic content of their own
//!   (both mark the palatalization / hardness of the preceding
//!   consonant), and dropping them entirely would collapse
//!   otherwise-distinct words.
//!
//! # Byte-length caveat
//!
//! Every Cyrillic scalar in the modern Russian block is **2 bytes** in
//! UTF-8 (U+0410..=U+044F and U+0401 / U+0451 all fall in the 2-byte
//! range). The encoder walks characters via
//! [`str::chars`](str::chars), never raw bytes, so it never risks
//! slicing a scalar apart. Callers that want to compare an encoded
//! output against an original word by byte offset must remember that
//! the ASCII output is 1 byte per character while the Cyrillic input
//! is 2 bytes per character.
//!
//! # Non-Cyrillic pass-through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the Cyrillic mapping pass through unchanged. Mixed-script
//! input carries its non-Cyrillic characters through as-is.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Russian GOST 7.79 System B Cyrillic → Latin transliteration.
///
/// A zero-sized value; construct as [`RussianGost779B`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table.
///
/// # Example
///
/// ```
/// use stringcheese_ru::RussianGost779B;
///
/// // "Москва" → "moskva".
/// assert_eq!(RussianGost779B.encode("Москва"), "moskva");
/// // "Ёж" → "yozh".
/// assert_eq!(RussianGost779B.encode("Ёж"), "yozh");
/// // "Щи" → "shhi".
/// assert_eq!(RussianGost779B.encode("Щи"), "shhi");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RussianGost779B;

impl RussianGost779B {
    /// Encode `text` per the GOST 7.79 System B transliteration.
    ///
    /// Every Cyrillic scalar in the mapping is replaced by its ASCII
    /// counterpart (one or more ASCII characters); every scalar
    /// outside the mapping (ASCII, digits, punctuation, other-script
    /// letters) passes through unchanged.
    ///
    /// The encoder lowercases the input first — the output is always
    /// lowercase-ASCII (plus pass-through characters).
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Preallocate roughly 2x — most Cyrillic scalars map to 1..3
        // ASCII characters, and the input's byte count is already 2x
        // the character count for pure Russian text.
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

/// Map a Russian Cyrillic scalar to its GOST 7.79-B ASCII string, or
/// `None` if the scalar is outside the Russian Cyrillic mapping.
///
/// See the [module-level table](self#mapping-table) for the full list.
/// Input is assumed to already be lowercase; the encoder's public
/// entry point ([`RussianGost779B::encode`]) handles the fold.
#[must_use]
pub const fn cyrillic_to_gost_b(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
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
        'х' => "x",
        'ц' => "cz",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shh",
        'ъ' => "''",
        'ы' => "y",
        'ь' => "'",
        'э' => "e'",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Adapter that exposes [`RussianGost779B`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Russian::phonetic_encoder`](crate::Russian) hands back.
///
/// The adapter always returns `Some((key, None))` — GOST 7.79-B is a
/// single-key encoder — and returns `None` for input with no Cyrillic
/// content (the transliteration would pass through unchanged, which is
/// not a useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RussianGost779BAdapter;

impl LanguagePhoneticEncoder for RussianGost779BAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_cyrillic(word) {
            return None;
        }
        let key = RussianGost779B.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "gost-7.79-b"
    }
}

/// Does `s` contain at least one scalar in the Cyrillic UTF-8 block
/// (U+0400..=U+04FF)?
///
/// This is a superset of the GOST 7.79-B mapping — it includes
/// non-Russian Cyrillic (Ukrainian `і`, Belarusian `ў`, Serbian `љ`, …)
/// that the mapping does not cover — but that is the right shape for
/// the adapter: any Cyrillic-block character makes the input "Cyrillic
/// content" for phonetic-key purposes.
fn contains_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        RussianGost779B.encode(s)
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
        assert_eq!(e("Москва"), "moskva");
        assert_eq!(e("Россия"), "rossiya");
        assert_eq!(e("Санкт-Петербург"), "sankt-peterburg");
    }

    #[test]
    fn every_cyrillic_letter_has_a_mapping() {
        for c in [
            'а', 'б', 'в', 'г', 'д', 'е', 'ё', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п',
            'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ы', 'ь', 'э', 'ю', 'я',
        ] {
            assert!(
                cyrillic_to_gost_b(c).is_some(),
                "no GOST 7.79-B mapping for {c:?}"
            );
        }
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        // "ЁЖ" → "yozh" — the encoder lowercases before consulting the
        // table.
        assert_eq!(e("ЁЖ"), "yozh");
    }

    #[test]
    fn hard_and_soft_signs_encode_to_apostrophes() {
        // ъ → '' (double apostrophe). Word "объект".
        assert_eq!(e("объект"), "ob''ekt");
        // ь → ' (single apostrophe). Word "конь".
        assert_eq!(e("конь"), "kon'");
    }

    #[test]
    fn tsch_shch_sh_ch_encode_correctly() {
        // ш → sh, щ → shh, ч → ch, ц → cz.
        assert_eq!(e("щит"), "shhit");
        assert_eq!(e("час"), "chas");
        assert_eq!(e("шаг"), "shag");
        assert_eq!(e("царь"), "czar'");
    }

    #[test]
    fn yo_encodes_to_yo() {
        assert_eq!(e("ёж"), "yozh");
        assert_eq!(e("ёлка"), "yolka");
    }

    #[test]
    fn e_reverse_encodes_to_apostrophe_e() {
        // э → e'
        assert_eq!(e("это"), "e'to");
    }

    #[test]
    fn mixed_content_passes_through_non_cyrillic() {
        assert_eq!(e("hello Москва"), "hello moskva");
        assert_eq!(e("год 2026"), "god 2026");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_name_is_gost_779_b() {
        assert_eq!(RussianGost779BAdapter.name(), "gost-7.79-b");
    }

    #[test]
    fn adapter_returns_some_for_cyrillic() {
        let out = RussianGost779BAdapter.encode("Москва");
        assert_eq!(out, Some((String::from("moskva"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_cyrillic() {
        assert!(RussianGost779BAdapter.encode("").is_none());
        assert!(RussianGost779BAdapter.encode("hello").is_none());
        assert!(RussianGost779BAdapter.encode("123").is_none());
    }
}
