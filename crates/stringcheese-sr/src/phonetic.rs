//! Serbian phonetic encoder — Cyrillic-to-Latin transliteration reused
//! as the phonetic key.
//!
//! # Design
//!
//! Serbian is written in two scripts (Vukovica and Gaj), and the
//! standard [`to_latin`] transliteration is
//! **deterministic and script-normalizing** — every Cyrillic form
//! maps to a canonical Latin form, and Latin forms pass through
//! unchanged. That property is exactly what a phonetic index for
//! Serbian needs:
//!
//! * A record indexed under Cyrillic (`Београд`) collapses to the
//!   same key (`beograd`) as a record indexed under Latin (`Beograd`).
//! * The output is lowercase-normalized and script-unified, so
//!   downstream comparisons match across the script boundary.
//!
//! Serbian orthography is already highly phonetic — each letter maps
//! to one phoneme, mostly one-to-one — so there is no obvious
//! Soundex-family sound-alike step to add on top. The Cyrillic <->
//! Latin transliteration is the honest baseline phonetic key.
//!
//! Adapter name: `"sr-latin"`.
//!
//! # Non-goals
//!
//! * **Consonant / vowel merges.** No `k/c` collapse, no `ž/š`
//!   collapse, no vowel reduction. Two words that share a stem but
//!   differ in a consonant get different keys.
//! * **Ekavian / ijekavian merge.** `vek` and `vijek` produce
//!   different keys. A follow-up wave could add an ijekavian → ekavian
//!   fold under a separate adapter name.
//! * **Cross-language merge.** The encoder is Serbian-tuned; feeding
//!   Croatian, Bosnian, or Montenegrin text gets the transliteration
//!   applied but no language-specific accommodation.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::scripts::{contains_cyrillic, to_latin};

/// The Serbian Cyrillic-to-Latin phonetic encoder.
///
/// A zero-sized value; construct as [`SerbianLatin`] and reuse across
/// threads and calls.
///
/// # Example
///
/// ```
/// use stringcheese_sr::SerbianLatin;
///
/// // Cyrillic input transliterates to its lowercase Latin form.
/// assert_eq!(SerbianLatin.encode("Београд"), "beograd");
///
/// // Latin input is lowercased only.
/// assert_eq!(SerbianLatin.encode("Beograd"), "beograd");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerbianLatin;

impl SerbianLatin {
    /// Encode `text` as its lowercase Serbian Latin form.
    ///
    /// Cyrillic input is transliterated to Latin via
    /// [`to_latin`] and then lowercased.
    /// Latin input is just lowercased. Every scalar outside the
    /// Serbian alphabet passes through unchanged.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let latin = to_latin(text);
        latin.chars().flat_map(char::to_lowercase).collect()
    }
}

/// Adapter that exposes [`SerbianLatin`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type the pack's
/// `Language::phonetic_encoder` hands back.
///
/// Returns `Some((key, None))` for any non-empty input. The encoder
/// has no secondary key.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerbianLatinAdapter;

impl LanguagePhoneticEncoder for SerbianLatinAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if word.is_empty() {
            return None;
        }
        let key = SerbianLatin.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "sr-latin"
    }
}

/// Adapter that exposes the cross-Slavic
/// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
/// Metaphone-family sound-alike encoder from `stringcheese-phonetic`
/// through the object-safe [`LanguagePhoneticEncoder`] trait — this is
/// the type
/// [`Serbian::phonetic_encoder`](crate::Serbian) hands back when the
/// pack was built with
/// [`SerbianPhoneticChoice::SlavicMetaphone`](crate::SerbianPhoneticChoice).
///
/// Uses [`SlavicMetaphone::new`](stringcheese_phonetic::SlavicMetaphone::new)
/// with the default options (drop non-initial vowels, cap at the
/// default key length). Returns `Some((key, None))` on success and
/// `None` when the encoder produces the empty key (input with no
/// mappable Slavic-family letter).
///
/// Only available when the crate's `slavic-metaphone` Cargo feature is
/// enabled.
#[cfg(feature = "slavic-metaphone")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlavicMetaphoneAdapter;

#[cfg(feature = "slavic-metaphone")]
impl LanguagePhoneticEncoder for SlavicMetaphoneAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        let key = stringcheese_phonetic::SlavicMetaphone::new().encode(word);
        if key.is_empty() {
            None
        } else {
            Some((key, None))
        }
    }

    fn name(&self) -> &'static str {
        "slavic-metaphone-2026"
    }
}

/// Returns `true` if `text` contains any Serbian (Cyrillic or Latin)
/// letter. Used by tests / callers who want a quick "is this Serbian?"
/// probe; the adapter itself does not gate on this — it happily
/// encodes any input.
#[must_use]
pub fn looks_like_serbian(text: &str) -> bool {
    if contains_cyrillic(text) {
        return true;
    }
    text.chars().any(|c| {
        matches!(
            c,
            'a'..='z'
                | 'A'..='Z'
                | 'č'
                | 'ć'
                | 'đ'
                | 'š'
                | 'ž'
                | 'Č'
                | 'Ć'
                | 'Đ'
                | 'Š'
                | 'Ž'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_encodes_to_empty() {
        assert_eq!(SerbianLatin.encode(""), "");
    }

    #[test]
    fn cyrillic_encodes_to_lowercase_latin() {
        assert_eq!(SerbianLatin.encode("Београд"), "beograd");
        assert_eq!(SerbianLatin.encode("ЉУБАВ"), "ljubav");
        assert_eq!(SerbianLatin.encode("Његош"), "njegoš");
    }

    #[test]
    fn latin_input_is_lowercased() {
        assert_eq!(SerbianLatin.encode("Beograd"), "beograd");
        assert_eq!(SerbianLatin.encode("LJUBAV"), "ljubav");
    }

    #[test]
    fn cyrillic_and_latin_forms_produce_same_key() {
        // The whole point of using the transliteration as the phonetic
        // key: a record filed under either script collapses to the
        // same normalized form.
        assert_eq!(
            SerbianLatin.encode("Београд"),
            SerbianLatin.encode("Beograd")
        );
        assert_eq!(SerbianLatin.encode("ЉУБАВ"), SerbianLatin.encode("ljubav"));
    }

    #[test]
    fn adapter_name_is_sr_latin() {
        assert_eq!(SerbianLatinAdapter.name(), "sr-latin");
    }

    #[test]
    fn adapter_returns_some_for_serbian_input() {
        assert_eq!(
            SerbianLatinAdapter.encode("Београд"),
            Some((String::from("beograd"), None))
        );
        assert_eq!(
            SerbianLatinAdapter.encode("Beograd"),
            Some((String::from("beograd"), None))
        );
    }

    #[test]
    fn adapter_returns_none_for_empty_input() {
        assert!(SerbianLatinAdapter.encode("").is_none());
    }

    #[test]
    fn looks_like_serbian_predicate() {
        assert!(looks_like_serbian("Београд"));
        assert!(looks_like_serbian("Beograd"));
        assert!(looks_like_serbian("čаša"));
        assert!(!looks_like_serbian("123"));
        assert!(!looks_like_serbian(""));
    }
}
