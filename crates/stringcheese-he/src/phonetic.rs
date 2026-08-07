//! [`Iso259`] — a simplified ISO 259 style ASCII transliteration for
//! Hebrew.
//!
//! # Origin and the ASCII trade-off
//!
//! ISO 259 (1984), ISO 259-2 (1994 "simplified"), and ISO 259-3 (1999
//! "phonetic") are the ISO family of Latin-script Hebrew
//! transliterations. All three use diacritics (`ẖ`, `ṭ`, `ṣ`, `š`) to
//! disambiguate emphatic, pharyngeal, and sibilant consonants that
//! have no single-character ASCII equivalent. That's the right choice
//! for scholarly typography and the wrong choice for a byte-oriented
//! phonetic key (diacritics defeat ASCII-only indexes, and the two
//! encodings a naive caller can produce — precomposed NFC vs.
//! decomposed NFD — hash differently).
//!
//! This pack ships a **simplified single-character ASCII variant** of
//! the ISO 259 mapping, adapter name `"iso-259-he"`. Every one of the
//! 22 base consonants and 5 final forms maps to exactly one printable
//! ASCII character; the emphatic / pharyngeal / sibilant consonants
//! that ISO 259 marks with a diacritic get an uppercase-Latin
//! approximation (`H` for `ח`, `T` for `ט`) or a punctuation stand-in
//! (`$` for `ש`) — the same design choice Buckwalter made for the
//! Arabic transliteration this pack takes its overall shape from.
//!
//! # Why an ASCII transliteration as the phonetic key?
//!
//! Hebrew has ~27 letters (22 base + 5 final forms), several of which
//! do not correspond to any single Latin phoneme (`ח`, `ט`, `ע`, `צ`,
//! `ש`). English-first phonetic encoders (Soundex, Metaphone) collapse
//! those into approximations that lose the very distinctions Hebrew IR
//! needs. A deterministic, ASCII-only equivalence-class key preserves
//! those distinctions and plays well with the phonetic subsystem's
//! `String` return type and with any downstream byte-oriented index.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words
//! that sound alike do not necessarily encode alike. But it *is* a
//! stable, deterministic, ASCII-only equivalence class the language
//! trait's `phonetic_encoder` accessor accepts; and for Hebrew that is
//! the honest baseline. A future `stringcheese-he` release could add
//! a true phonetic encoder (`HebrewSoundex`, `PhonexHe`) alongside
//! this transliteration.
//!
//! # Mapping
//!
//! The full table (Hebrew scalar → ASCII character). Final letter
//! forms fold to the same code as their base form, so the mapping is a
//! *many-to-one* function from Hebrew scalars to ASCII (the inverse
//! picks the base form).
//!
//! | Codepoint | Hebrew | ASCII | Notes                       |
//! |-----------|--------|-------|-----------------------------|
//! | U+05D0    | `א`   | `'`   | Alef (glottal stop)         |
//! | U+05D1    | `ב`   | `b`   | Bet                         |
//! | U+05D2    | `ג`   | `g`   | Gimel                       |
//! | U+05D3    | `ד`   | `d`   | Dalet                       |
//! | U+05D4    | `ה`   | `h`   | He                          |
//! | U+05D5    | `ו`   | `w`   | Vav (ISO 259 `w`)           |
//! | U+05D6    | `ז`   | `z`   | Zayin                       |
//! | U+05D7    | `ח`   | `H`   | Chet (pharyngeal — upper H) |
//! | U+05D8    | `ט`   | `T`   | Tet (emphatic — upper T)    |
//! | U+05D9    | `י`   | `y`   | Yod                         |
//! | U+05DA    | `ך`   | `k`   | Final Kaf (folds to Kaf)    |
//! | U+05DB    | `כ`   | `k`   | Kaf                         |
//! | U+05DC    | `ל`   | `l`   | Lamed                       |
//! | U+05DD    | `ם`   | `m`   | Final Mem (folds to Mem)    |
//! | U+05DE    | `מ`   | `m`   | Mem                         |
//! | U+05DF    | `ן`   | `n`   | Final Nun (folds to Nun)    |
//! | U+05E0    | `נ`   | `n`   | Nun                         |
//! | U+05E1    | `ס`   | `s`   | Samekh                      |
//! | U+05E2    | `ע`   | `` ` ``   | Ayin (backtick)             |
//! | U+05E3    | `ף`   | `p`   | Final Pe (folds to Pe)      |
//! | U+05E4    | `פ`   | `p`   | Pe                          |
//! | U+05E5    | `ץ`   | `c`   | Final Tsadi (folds to Tsadi)|
//! | U+05E6    | `צ`   | `c`   | Tsadi (`c` from *ç*)        |
//! | U+05E7    | `ק`   | `q`   | Qof                         |
//! | U+05E8    | `ר`   | `r`   | Resh                        |
//! | U+05E9    | `ש`   | `$`   | Shin/Sin (`$` from Buckwalter) |
//! | U+05EA    | `ת`   | `t`   | Tav                         |
//!
//! # Final-form folding is unconditional in the phonetic key
//!
//! The five final letter forms (`ך ם ן ף ץ`) always encode to the same
//! ASCII code as their base-form counterparts (`k`, `m`, `n`, `p`,
//! `c`). This is deliberate: final vs. non-final is a *position*
//! distinction (the same phoneme, written differently at the end of a
//! word), so a phonetic key must collapse them. The normalizer's
//! [`with_final_form_folding`](crate::normalize::HebrewNormalizer::with_final_form_folding)
//! flag controls the *surface* form; this encoder folds regardless.
//!
//! # Niqqud and cantillation pass through
//!
//! Combining niqqud (U+05B0..=U+05C7) and cantillation (U+0591..=U+05AF)
//! scalars are not in the mapping and pass through unchanged. A pipeline
//! that wants an ASCII-only phonetic key should
//! [`normalize`](crate::normalize::normalize) the input first (the
//! default normalizer strips both classes).
//!
//! # Non-Hebrew passes through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the mapping pass through the encoder unchanged. The output
//! is guaranteed ASCII when the input is entirely composed of Hebrew
//! scalars in the mapping table — mixed-script input carries its
//! non-Hebrew characters over as-is.
//!
//! # Inverse
//!
//! The mapping is a *many-to-one* function from Hebrew scalars to
//! ASCII (both `ך` and `כ` encode to `k`), so the inverse must pick a
//! canonical representative for each ASCII code. This module's
//! [`Iso259::inverse`] picks the **base form** (non-final) for `k`,
//! `m`, `n`, `p`, `c`. Round-trip `inverse(encode(x))` therefore
//! equals `x` for input `x` that uses no final forms; it equals the
//! final-form-folded version of `x` otherwise.
//!
//! # RTL note
//!
//! The encoder walks the input in **logical order** (UTF-8 byte
//! order). Each Hebrew scalar produces one ASCII character; the
//! resulting ASCII string is naturally rendered left-to-right by any
//! renderer, but it represents the Hebrew word in its
//! first-consonant-first logical order — which is what a matching /
//! indexing pipeline wants.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The simplified ISO 259 Hebrew transliteration.
///
/// A zero-sized value; construct as [`Iso259`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the full mapping table.
///
/// # Example
///
/// ```
/// use stringcheese_he::Iso259;
///
/// // Shalom — with final mem folded to base mem.
/// assert_eq!(Iso259.encode("שלום"), "$lwm");
/// // Melekh — with final kaf folded to base kaf.
/// assert_eq!(Iso259.encode("מלך"), "mlk");
/// // With ayin (backtick).
/// assert_eq!(Iso259.encode("עולם"), "`wlm");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Iso259;

impl Iso259 {
    /// Encode `text` per the simplified ISO 259 transliteration.
    ///
    /// Every Hebrew scalar in the mapping is replaced by one ASCII
    /// character (with final forms folded to their base form's code);
    /// every scalar outside the mapping (niqqud, cantillation, ASCII,
    /// digits, punctuation, other-script letters) passes through
    /// unchanged.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match hebrew_to_iso259(c) {
                Some(ascii) => out.push(ascii),
                None => out.push(c),
            }
        }
        out
    }

    /// Reverse the transliteration (best-effort — final forms fold
    /// into their base form).
    ///
    /// Every ASCII character in the ISO 259 alphabet maps back to
    /// exactly one Hebrew scalar (the base form, never the final
    /// form); every character outside the alphabet passes through
    /// unchanged. The round-trip
    /// `Iso259.inverse(&Iso259.encode(x))` equals `x` for input `x`
    /// that uses no final forms; it equals the final-form-folded
    /// version of `x` otherwise.
    #[must_use]
    pub fn inverse(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() * 2);
        for c in text.chars() {
            match iso259_to_hebrew(c) {
                Some(hebrew) => out.push(hebrew),
                None => out.push(c),
            }
        }
        out
    }
}

/// Map a Hebrew scalar to its simplified ISO 259 ASCII counterpart, or
/// `None` if the scalar is outside the mapping.
///
/// Final letter forms fold to the same code as their base form.
///
/// See the [module-level table](self#mapping) for the full list.
#[must_use]
pub const fn hebrew_to_iso259(c: char) -> Option<char> {
    Some(match c {
        // Base 22 consonants. Kaf / mem / nun / pe / tsadi are folded
        // together with their final-form counterpart (`ך ם ן ף ץ`) in
        // a single arm each — the phonetic key collapses the position
        // distinction, and clippy's `match_same_arms` would flag
        // separate arms with identical bodies.
        '\u{05D0}' => '\'',             // א
        '\u{05D1}' => 'b',              // ב
        '\u{05D2}' => 'g',              // ג
        '\u{05D3}' => 'd',              // ד
        '\u{05D4}' => 'h',              // ה
        '\u{05D5}' => 'w',              // ו
        '\u{05D6}' => 'z',              // ז
        '\u{05D7}' => 'H',              // ח
        '\u{05D8}' => 'T',              // ט
        '\u{05D9}' => 'y',              // י
        '\u{05DA}' | '\u{05DB}' => 'k', // ך → כ
        '\u{05DC}' => 'l',              // ל
        '\u{05DD}' | '\u{05DE}' => 'm', // ם → מ
        '\u{05DF}' | '\u{05E0}' => 'n', // ן → נ
        '\u{05E1}' => 's',              // ס
        '\u{05E2}' => '`',              // ע
        '\u{05E3}' | '\u{05E4}' => 'p', // ף → פ
        '\u{05E5}' | '\u{05E6}' => 'c', // ץ → צ
        '\u{05E7}' => 'q',              // ק
        '\u{05E8}' => 'r',              // ר
        '\u{05E9}' => '$',              // ש
        '\u{05EA}' => 't',              // ת
        _ => return None,
    })
}

/// Map a simplified ISO 259 ASCII scalar to its canonical Hebrew
/// counterpart (base form), or `None` if the scalar is outside the
/// mapping.
///
/// This is the (partial) inverse of [`hebrew_to_iso259`]: the mapping
/// is many-to-one (final and base forms share a code), so the inverse
/// always picks the base form.
#[must_use]
pub const fn iso259_to_hebrew(c: char) -> Option<char> {
    Some(match c {
        '\'' => '\u{05D0}', // א
        'b' => '\u{05D1}',  // ב
        'g' => '\u{05D2}',  // ג
        'd' => '\u{05D3}',  // ד
        'h' => '\u{05D4}',  // ה
        'w' => '\u{05D5}',  // ו
        'z' => '\u{05D6}',  // ז
        'H' => '\u{05D7}',  // ח
        'T' => '\u{05D8}',  // ט
        'y' => '\u{05D9}',  // י
        'k' => '\u{05DB}',  // כ (base form)
        'l' => '\u{05DC}',  // ל
        'm' => '\u{05DE}',  // מ (base form)
        'n' => '\u{05E0}',  // נ (base form)
        's' => '\u{05E1}',  // ס
        '`' => '\u{05E2}',  // ע
        'p' => '\u{05E4}',  // פ (base form)
        'c' => '\u{05E6}',  // צ (base form)
        'q' => '\u{05E7}',  // ק
        'r' => '\u{05E8}',  // ר
        '$' => '\u{05E9}',  // ש
        't' => '\u{05EA}',  // ת
        _ => return None,
    })
}

/// Adapter that exposes [`Iso259`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Hebrew::phonetic_encoder`](crate::Hebrew) hands back.
///
/// The adapter always returns `Some((key, None))` — the simplified ISO
/// 259 mapping is a single-key encoder — and returns `None` for input
/// with no Hebrew content (the transliteration would pass through
/// unchanged, which is not a phonetic key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Iso259Adapter;

impl LanguagePhoneticEncoder for Iso259Adapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_hebrew(word) {
            return None;
        }
        let key = Iso259.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "iso-259-he"
    }
}

/// Does `s` contain at least one scalar in the Hebrew UTF-8 block
/// (U+0590..=U+05FF)?
///
/// This is a *superset* of the ISO 259 mapping — it includes scalars
/// the mapping does not cover (niqqud, cantillation, Hebrew
/// punctuation, Yiddish digraphs) — but that is the right shape for
/// the adapter: any Hebrew-block character makes the input "Hebrew
/// content" for phonetic-key purposes.
fn contains_hebrew(s: &str) -> bool {
    s.chars().any(|c| ('\u{0590}'..='\u{05FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        Iso259.encode(s)
    }

    fn i(s: &str) -> String {
        Iso259.inverse(s)
    }

    // -------------------------------------------------------------
    // Canonical reference words.
    // -------------------------------------------------------------

    #[test]
    fn encodes_shalom() {
        // שלום — final mem folds to base mem in the phonetic key.
        assert_eq!(e("שלום"), "$lwm");
    }

    #[test]
    fn encodes_bayit() {
        // בית — house.
        assert_eq!(e("בית"), "byt");
    }

    #[test]
    fn encodes_ani() {
        assert_eq!(e("אני"), "'ny");
    }

    #[test]
    fn encodes_melekh_with_final_kaf() {
        // מלך — final kaf folds to k.
        assert_eq!(e("מלך"), "mlk");
    }

    #[test]
    fn encodes_erets_with_final_tsadi() {
        // ארץ — final tsadi folds to c.
        assert_eq!(e("ארץ"), "'rc");
    }

    #[test]
    fn encodes_ayin_with_backtick() {
        // עולם — ayin → backtick.
        assert_eq!(e("עולם"), "`wlm");
    }

    // -------------------------------------------------------------
    // Mapping totality — every table entry round-trips (base form).
    // -------------------------------------------------------------

    #[test]
    fn every_base_letter_round_trips() {
        // Base letters (U+05D0..=U+05EA minus the five final slots)
        // round-trip through encode/inverse.
        for cp in 0x05D0u32..=0x05EAu32 {
            let c = char::from_u32(cp).unwrap();
            // Skip the five final-form slots — those fold and don't
            // round-trip to themselves; they round-trip to their base
            // form.
            if matches!(cp, 0x05DA | 0x05DD | 0x05DF | 0x05E3 | 0x05E5) {
                continue;
            }
            let a = hebrew_to_iso259(c).unwrap_or_else(|| panic!("no code for base {c:?}"));
            let back = iso259_to_hebrew(a);
            assert_eq!(
                back,
                Some(c),
                "hebrew {c:?} → iso259 {a:?} → {back:?} did not round-trip"
            );
        }
    }

    #[test]
    fn final_forms_fold_to_base_form_codes() {
        // ך → k → כ
        assert_eq!(hebrew_to_iso259('\u{05DA}'), Some('k'));
        assert_eq!(iso259_to_hebrew('k'), Some('\u{05DB}'));
        // ם → m → מ
        assert_eq!(hebrew_to_iso259('\u{05DD}'), Some('m'));
        assert_eq!(iso259_to_hebrew('m'), Some('\u{05DE}'));
        // ן → n → נ
        assert_eq!(hebrew_to_iso259('\u{05DF}'), Some('n'));
        assert_eq!(iso259_to_hebrew('n'), Some('\u{05E0}'));
        // ף → p → פ
        assert_eq!(hebrew_to_iso259('\u{05E3}'), Some('p'));
        assert_eq!(iso259_to_hebrew('p'), Some('\u{05E4}'));
        // ץ → c → צ
        assert_eq!(hebrew_to_iso259('\u{05E5}'), Some('c'));
        assert_eq!(iso259_to_hebrew('c'), Some('\u{05E6}'));
    }

    // -------------------------------------------------------------
    // Pass-through.
    // -------------------------------------------------------------

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e("hello"), "hello");
        assert_eq!(e(""), "");
    }

    #[test]
    fn mixed_hebrew_and_ascii() {
        assert_eq!(e("hello שלום"), "hello $lwm");
    }

    #[test]
    fn niqqud_passes_through() {
        // The encoder does not strip niqqud — that's the normalizer's
        // job. Combining marks are outside the mapping and pass through.
        let input = "אָ";
        let out = e(input);
        assert!(out.starts_with('\''));
        assert!(out.contains('\u{05B8}')); // qamats passed through
    }

    // -------------------------------------------------------------
    // Inverse.
    // -------------------------------------------------------------

    #[test]
    fn inverse_of_reference_words() {
        // Both "$lwm" and its round-trip normalize the final mem to
        // the base form.
        assert_eq!(i("$lwm"), "שלומ"); // note: base mem, not final mem
        assert_eq!(i("byt"), "בית");
        assert_eq!(i("'ny"), "אני");
    }

    #[test]
    fn round_trip_normalizes_final_forms_to_base() {
        // Words without final forms round-trip exactly.
        for w in ["בית", "אני", "ספר", "כתב"] {
            let enc = e(w);
            let back = i(&enc);
            assert_eq!(back, w, "round-trip failed on {w:?}");
        }
        // Words with final forms round-trip to the folded form.
        // מלך (final kaf) → mlk → מלכ (base kaf)
        let enc = e("מלך");
        let back = i(&enc);
        assert_eq!(back, "מלכ");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_iso259_he() {
        assert_eq!(Iso259Adapter.name(), "iso-259-he");
    }

    #[test]
    fn adapter_returns_some_for_hebrew() {
        let out = Iso259Adapter.encode("שלום");
        assert_eq!(out, Some((String::from("$lwm"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_hebrew() {
        assert!(Iso259Adapter.encode("").is_none());
        assert!(Iso259Adapter.encode("hello").is_none());
        assert!(Iso259Adapter.encode("123").is_none());
    }
}
