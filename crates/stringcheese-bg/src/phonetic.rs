//! Bulgarian Cyrillic → Latin transliteration.
//!
//! # Origin
//!
//! Bulgarian has no widely established Soundex / Metaphone-family
//! phonetic encoder. What Bulgarian *does* have is a well-known
//! **transliteration** family: the 2006 Bulgarian government
//! Streamlined System (a.k.a. BDS ISO/DIS 9:1995 modification),
//! ISO 9, GOST 7.79, and BGN/PCGN, all of which map Cyrillic scalars
//! to Latin ones. This module ships an adaptation of GOST 7.79
//! System B tailored to the Bulgarian letter set and pronunciation.
//!
//! # Standard chosen: GOST 7.79-2000 System B, Bulgarian adaptation
//!
//! GOST 7.79 (an ISO 9-adjacent Slavic-Cyrillic standard) originally
//! covered Russian; extensions for the other Slavic Cyrillic
//! alphabets exist but are less widely adopted. This module ships a
//! **Bulgarian adaptation** of System B — a context-free, ASCII-only
//! mapping designed for byte-oriented indexes and search interop —
//! blending GOST-B conventions with Bulgarian-specific choices where
//! Bulgarian phonology diverges from Russian:
//!
//! * **`щ → sht`**. This is the single most Bulgarian-specific
//!   choice. Russian `щ` is a long soft /ʃː/ (rendered `shh` in
//!   Russian GOST-B); Bulgarian `щ` is a consonant cluster /ʃt/
//!   (rendered `sht`, matching every published Bulgarian
//!   romanization convention including the 2006 government
//!   Streamlined System).
//! * **`ъ → a`**. In Bulgarian, `ъ` is not the Russian hard-sign
//!   glyph — it is a full vowel /ɤ/, the mid-central sound that
//!   appears in `България` ("Bulgaria") itself. The Bulgarian
//!   Streamlined System renders it as bare `a`, and this pack
//!   follows suit. (Russian GOST-B renders `ъ` as `''` because in
//!   Russian it carries no phonetic content of its own.)
//! * **`х → h`**. Bulgarian `х` is the voiceless velar fricative
//!   /x/. Russian GOST-B renders it as `x` (a common transliteration
//!   choice for Slavic-x); Bulgarian romanization conventions favour
//!   `h` (matching the 2006 Streamlined System and BGN/PCGN
//!   Bulgarian). The two are pronounced the same but the Bulgarian
//!   convention is `h`.
//! * **`ц → ts`**. Bulgarian romanization conventions render `ц` as
//!   the digraph `ts`, matching the affricate's pronunciation
//!   directly. (Russian GOST-B uses `cz` — a compact ASCII rendering
//!   that avoids collision with the k-family — but the Bulgarian
//!   convention is `ts` and every published Bulgarian romanization
//!   standard uses it.)
//! * **`ь → '`**. The soft sign is rare in Bulgarian — it appears
//!   almost exclusively in the sequence `ьо` (as in `Гьоте` —
//!   "Goethe"). Rendered as an apostrophe for consistency with the
//!   Russian pack.
//! * **`ю → yu`**, **`я → ya`**, **`й → j`**. Standard.
//!
//! The letters **`ё` (U+0451)**, **`ы` (U+044B)**, and
//! **`э` (U+044D)** are **not part of the Bulgarian alphabet** and
//! pass through unchanged — matching the Ukrainian pack's behaviour
//! for its non-Ukrainian Cyrillic scalars.
//!
//! Adapter name: `"gost-7.79-b-bg"`. The `-bg` disambiguates it from
//! the Russian `"gost-7.79-b"` and the Ukrainian `"gost-7.79-b-uk"` —
//! the three encoders differ on `ъ`, `щ`, `х`, `ц`, and cover
//! overlapping but distinct letter sets, so callers should never mix
//! them.
//!
//! # Why a transliteration instead of a Soundex?
//!
//! The core StringCheese phonetic subsystem is Soundex-shaped —
//! collapse a name to an equivalence class the caller can index on.
//! Bulgarian's phoneme inventory does not admit a well-established
//! Soundex table the way English's does. GOST 7.79-B (Bulgarian
//! adaptation) is the honest baseline: a **deterministic,
//! reversible-with-caveats, ASCII-only** equivalence class that
//! plays well with byte-oriented indexes and downstream search
//! interop.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words
//! that sound alike do not necessarily encode alike (there are no
//! Soundex-style consonant merges). But it *is* a stable,
//! deterministic, ASCII-only key the language trait's
//! `phonetic_encoder` accessor accepts, and for Bulgarian that is
//! the honest baseline.
//!
//! # Mapping table
//!
//! The full lowercase mapping — uppercase inputs are lowercased
//! before the table is consulted (Bulgarian `А…Я` fold to `а…я`
//! under default Unicode rules; there is no locale-specific
//! tailoring):
//!
//! | Cyrillic | Latin (Bulgarian GOST 7.79-B) |
//! |----------|-------------------------------|
//! | `а`     | `a`   |
//! | `б`     | `b`   |
//! | `в`     | `v`   |
//! | `г`     | `g`   |
//! | `д`     | `d`   |
//! | `е`     | `e`   |
//! | `ж`     | `zh`  |
//! | `з`     | `z`   |
//! | `и`     | `i`   |
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
//! | `х`     | `h`   |
//! | `ц`     | `ts`  |
//! | `ч`     | `ch`  |
//! | `ш`     | `sh`  |
//! | `щ`     | `sht` |
//! | `ъ`     | `a`   |
//! | `ь`     | `'`   |
//! | `ю`     | `yu`  |
//! | `я`     | `ya`  |
//!
//! # Byte-length caveat
//!
//! Every Cyrillic scalar in the modern Bulgarian block is **2 bytes**
//! in UTF-8 (U+0410..=U+044F falls in the 2-byte range). The
//! encoder walks characters via [`str::chars`](str::chars), never raw
//! bytes, so it never risks slicing a scalar apart. Callers that want
//! to compare an encoded output against an original word by byte
//! offset must remember that the ASCII output is 1 byte per character
//! while the Cyrillic input is 2 bytes per character.
//!
//! # Non-Cyrillic pass-through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the Bulgarian mapping pass through unchanged. Mixed-script
//! input carries its non-Cyrillic characters through as-is. Russian-
//! only letters (`ё`, `ы`, `э`) also pass through — the encoder is
//! scoped to the Bulgarian alphabet.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Bulgarian GOST 7.79 System B (Bulgarian adaptation) Cyrillic →
/// Latin transliteration.
///
/// A zero-sized value; construct as [`BulgarianGost779B`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the rationale behind the Bulgarian-specific choices (`щ → sht`,
/// `ъ → a`, `х → h`, `ц → ts`).
///
/// # Example
///
/// ```
/// use stringcheese_bg::BulgarianGost779B;
///
/// // "София" → "sofiya" (с о ф и я).
/// assert_eq!(BulgarianGost779B.encode("София"), "sofiya");
/// // "България" → "balgariya" (ъ → a, showing the Bulgarian schwa).
/// assert_eq!(BulgarianGost779B.encode("България"), "balgariya");
/// // "щастие" → "shtastie" (щ → sht, the Bulgarian-specific mapping).
/// assert_eq!(BulgarianGost779B.encode("щастие"), "shtastie");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BulgarianGost779B;

impl BulgarianGost779B {
    /// Encode `text` per the GOST 7.79 System B (Bulgarian)
    /// transliteration.
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
        // Preallocate roughly 2x — most Cyrillic scalars map to 1..3
        // ASCII characters, and the input's byte count is already 2x
        // the character count for pure Bulgarian text.
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

/// Map a Bulgarian Cyrillic scalar to its GOST 7.79-B (Bulgarian
/// adaptation) ASCII string, or `None` if the scalar is outside the
/// Bulgarian Cyrillic mapping.
///
/// See the [module-level table](self#mapping-table) for the full list.
/// Input is assumed to already be lowercase; the encoder's public
/// entry point ([`BulgarianGost779B::encode`]) handles the fold.
///
/// The `а` / `ъ` arms deliberately share the ASCII string `"a"` and
/// are kept separate here so the table reads as a per-letter mapping.
/// Callers who need to distinguish the two vowels on the ASCII side
/// need a different transliteration standard (Bulgarian ISO 9 uses
/// `ǎ` for `ъ`, at the cost of losing pure-ASCII output).
#[allow(clippy::match_same_arms)]
#[must_use]
pub const fn cyrillic_to_gost_b(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => "e",
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
        'х' => "h",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "sht",
        'ъ' => "a",
        'ь' => "'",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Adapter that exposes [`BulgarianGost779B`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Bulgarian::phonetic_encoder`](crate::Bulgarian) hands back.
///
/// The adapter always returns `Some((key, None))` — GOST 7.79-B is a
/// single-key encoder — and returns `None` for input with no Cyrillic
/// content (the transliteration would pass through unchanged, which
/// is not a useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BulgarianGost779BAdapter;

impl LanguagePhoneticEncoder for BulgarianGost779BAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_cyrillic(word) {
            return None;
        }
        let key = BulgarianGost779B.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "gost-7.79-b-bg"
    }
}

/// Does `s` contain at least one scalar in the Cyrillic UTF-8 block
/// (U+0400..=U+04FF)?
///
/// This is a superset of the Bulgarian mapping — it includes non-
/// Bulgarian Cyrillic (Russian `ё`, Ukrainian `і`, Belarusian `ў`,
/// Serbian `љ`, …) that the mapping does not cover — but that is the
/// right shape for the adapter: any Cyrillic-block character makes
/// the input "Cyrillic content" for phonetic-key purposes.
fn contains_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        BulgarianGost779B.encode(s)
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
        assert_eq!(e("София"), "sofiya");
        assert_eq!(e("България"), "balgariya");
        assert_eq!(e("Пловдив"), "plovdiv");
        assert_eq!(e("Варна"), "varna");
    }

    #[test]
    fn every_bulgarian_letter_has_a_mapping() {
        for c in [
            'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п', 'р',
            'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ь', 'ю', 'я',
        ] {
            assert!(
                cyrillic_to_gost_b(c).is_some(),
                "no Bulgarian GOST 7.79-B mapping for {c:?}"
            );
        }
    }

    #[test]
    fn russian_only_letters_pass_through() {
        // The Bulgarian mapping deliberately excludes ё, ы, э — they
        // are not part of the Bulgarian alphabet.
        assert_eq!(cyrillic_to_gost_b('ё'), None);
        assert_eq!(cyrillic_to_gost_b('ы'), None);
        assert_eq!(cyrillic_to_gost_b('э'), None);
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        // "СОФИЯ" → "sofiya".
        assert_eq!(e("СОФИЯ"), "sofiya");
    }

    #[test]
    fn shht_bulgarian_specific() {
        // The signature Bulgarian mapping: щ → sht (Russian's щ → shh).
        assert_eq!(e("щастие"), "shtastie");
        assert_eq!(e("Пещера"), "peshtera");
    }

    #[test]
    fn er_goljam_vowel() {
        // ъ is a full vowel in Bulgarian, rendered as `a`.
        assert_eq!(e("България"), "balgariya");
        assert_eq!(e("път"), "pat");
    }

    #[test]
    fn kh_ts_encodings() {
        // х → h and ц → ts, per Bulgarian romanization convention
        // (Russian GOST-B uses x and cz respectively).
        assert_eq!(e("храна"), "hrana");
        assert_eq!(e("цар"), "tsar");
    }

    #[test]
    fn ch_sh_encodings() {
        // ч → ch, ш → sh.
        assert_eq!(e("час"), "chas");
        assert_eq!(e("шапка"), "shapka");
    }

    #[test]
    fn ju_ja_j_encodings() {
        // ю → yu, я → ya, й → j.
        assert_eq!(e("юг"), "yug");
        assert_eq!(e("ябълка"), "yabalka");
        assert_eq!(e("май"), "maj");
    }

    #[test]
    fn soft_sign_encodes_to_apostrophe() {
        // ь is rare in Bulgarian and appears mainly in the sequence ьо.
        assert_eq!(e("Гьоте"), "g'ote");
    }

    #[test]
    fn mixed_content_passes_through_non_cyrillic() {
        assert_eq!(e("hello София"), "hello sofiya");
        assert_eq!(e("година 2026"), "godina 2026");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_name_is_gost_779_b_bg() {
        assert_eq!(BulgarianGost779BAdapter.name(), "gost-7.79-b-bg");
    }

    #[test]
    fn adapter_returns_some_for_cyrillic() {
        let out = BulgarianGost779BAdapter.encode("София");
        assert_eq!(out, Some((String::from("sofiya"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_cyrillic() {
        assert!(BulgarianGost779BAdapter.encode("").is_none());
        assert!(BulgarianGost779BAdapter.encode("hello").is_none());
        assert!(BulgarianGost779BAdapter.encode("123").is_none());
    }
}
