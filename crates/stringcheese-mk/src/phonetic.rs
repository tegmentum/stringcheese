//! PHONEX-Macedonian — a Soundex-shape phonetic encoder tuned to the
//! Macedonian Cyrillic letter set.
//!
//! # Origin
//!
//! Macedonian, like Bulgarian and Serbian, has **no widely established
//! Soundex / Metaphone-family phonetic encoder** shipped with a public
//! reference implementation. What Macedonian *does* have, phonologically,
//! is a highly regular grapheme-to-phoneme mapping — every letter is
//! pronounced (unlike Russian, which has silent-vowel-reduction
//! patterns), and the seven Macedonian-specific letters (`ѓ`, `ќ`, `љ`,
//! `њ`, `џ`, `ѕ`, `ј`) each denote a single phoneme. This makes a
//! Soundex-shape encoder practical: no digraph preprocessing is needed
//! (Macedonian orthography already assigns one letter per phoneme), so
//! the classification table can be applied directly.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Macedonian** encoder for consistency
//! with the other packs that group by consonant class (`phonex-hu`,
//! `phonex-cs`, `phonex-nl`, `phonex-pt`, `phonex-es`, `phonex-fr`,
//! `phonex-pl`, `phonex-sk`, `phonex-tr`, `phonex-vi`, `phonex-bn`).
//! Concretely, the algorithm is a 4-character
//! `<letter><digit><digit><digit>` Soundex-shape key with
//! Macedonian-tuned classification:
//!
//! 1. **Fold to lowercase.** Cyrillic case-fold is well-behaved under
//!    Rust's default rules; no locale-specific tailoring is needed for
//!    Macedonian.
//! 2. **Drop non-letter scalars.** Digits, punctuation, and
//!    whitespace fall out.
//! 3. **Soundex-shape encoding.** Retain the first letter as the seed;
//!    classify each subsequent letter; drop the zero class (vowels);
//!    collapse consecutive equal codes; truncate to three digits and
//!    left-pad with `'0'` to reach length four.
//!
//! **Classification table** — the Macedonian-specific letters are
//! folded to their nearest Slavic-Soundex class:
//!
//! | Code | Cyrillic letters | Rationale |
//! |------|------------------|-----------|
//! | 1    | Б П Ф В         | Labials |
//! | 2    | Г К Х Ѓ Ќ Ј     | Gutturals + palatals — `ѓ` is a palatal voiced stop, `ќ` is a palatal voiceless stop, `ј` is a palatal glide; all group with the k/g class per Slavic-Metaphone convention |
//! | 3    | Д Т             | Dentals |
//! | 4    | Л Љ             | Laterals — `љ` is the palatalized `l` and folds to the same class |
//! | 5    | М Н Њ           | Nasals — `њ` is the palatalized `n` and folds to the same class |
//! | 6    | Р               | Rhotic |
//! | 7    | С З Ц Ч Ш Ж Ѕ Џ | Sibilants + affricates — `ѕ` is /dz/, `џ` is /dʒ/, both fold to the sibilant class |
//! | 0    | А Е И О У       | Vowels (dropped except at the seed) |
//!
//! The Macedonian-specific letters (`ѓ`, `ќ`, `љ`, `њ`, `џ`, `ѕ`, `ј`)
//! are **treated as distinct scalars** — the seed of the key preserves
//! the letter exactly (a word starting with `Ѓ` produces a key whose
//! first character is `Ѓ`, not `Г`), so `ѓавол` and `гавол` are not
//! collapsed at the seed. Only in the interior classification do the
//! Macedonian letters fold to their nearest Slavic-Soundex class.
//!
//! # Adapter name
//!
//! `"phonex-mk"` — chosen for consistency with the other PHONEX
//! adapters.
//!
//! # Deferred to a follow-up wave
//!
//! * **GOST 7.79-B Macedonian adaptation.** A parallel Cyrillic → Latin
//!   transliteration adapter for library-catalog interop. Would ship
//!   alongside the PHONEX encoder as an alternative.
//! * **Slavic-Metaphone Macedonian.** A variable-length encoder with
//!   better discrimination across Slavic Cyrillic packs; heavier to
//!   reference-test.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Macedonian PHONEX encoder.
///
/// A zero-sized value; construct as [`MacedonianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_mk::MacedonianPhonex;
///
/// // "Скопје" — С seed, К=2, О vow drop, П=1, Ј=2 → "С212".
/// let key = MacedonianPhonex.encode("Скопје").unwrap();
/// assert_eq!(key, "с212");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MacedonianPhonex;

impl MacedonianPhonex {
    /// Encodes `word` per the PHONEX-Macedonian algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form
    /// `<lowercase Cyrillic letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Lowercase and drop non-letters into a Vec<char>. `char` (not
        // byte) because every Cyrillic scalar is 2 bytes in UTF-8; byte
        // arithmetic would silently corrupt boundaries.
        let letters: alloc::vec::Vec<char> = word
            .chars()
            .flat_map(char::to_lowercase)
            .filter(|c| c.is_alphabetic())
            .collect();

        if letters.is_empty() {
            return None;
        }

        let mut out = String::with_capacity(4 + 3); // seed char + 3 digits
        out.push(letters[0]);
        let mut last_code = code_of(letters[0]);

        for &c in &letters[1..] {
            let code = code_of(c);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state.
                last_code = b'0';
                continue;
            }
            if code == last_code {
                // Duplicate consonant class — collapse.
                continue;
            }
            out.push(code as char);
            last_code = code;
            // The key's shape is seed + 3 digits. `out.chars().count()`
            // is the right measure because the seed can be a multi-byte
            // Cyrillic scalar; `out.len()` in bytes would over-count.
            if out.chars().count() == 4 {
                break;
            }
        }
        while out.chars().count() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Soundex-family digit for Cyrillic letter `c`.
///
/// Returns an ASCII byte in `b'0'..=b'7'`. See the classification table
/// in the [module-level docs](self).
#[inline]
#[must_use]
pub const fn code_of(c: char) -> u8 {
    match c {
        // Labials.
        'б' | 'п' | 'ф' | 'в' => b'1',
        // Gutturals + palatals (Macedonian-specific ѓ, ќ, ј).
        'г' | 'к' | 'х' | 'ѓ' | 'ќ' | 'ј' => b'2',
        // Dentals.
        'д' | 'т' => b'3',
        // Laterals (Macedonian-specific љ).
        'л' | 'љ' => b'4',
        // Nasals (Macedonian-specific њ).
        'м' | 'н' | 'њ' => b'5',
        // Rhotic.
        'р' => b'6',
        // Sibilants + affricates (Macedonian-specific ѕ, џ).
        'с' | 'з' | 'ц' | 'ч' | 'ш' | 'ж' | 'ѕ' | 'џ' => b'7',
        // Vowels + everything else (dropped except at the seed).
        _ => b'0',
    }
}

/// Adapter that exposes [`MacedonianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Macedonian::phonetic_encoder`](crate::Macedonian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MacedonianPhonexAdapter;

impl LanguagePhoneticEncoder for MacedonianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        MacedonianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-mk"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        MacedonianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(MacedonianPhonex.encode("").is_none());
        assert!(MacedonianPhonex.encode("   ").is_none());
        assert!(MacedonianPhonex.encode("---").is_none());
    }

    #[test]
    fn seed_letter_is_preserved_verbatim() {
        // The seed of the key preserves the input letter exactly.
        // `Скопје` → seed `с`, not folded to `с`-class.
        assert!(p("Скопје").starts_with('с'));
        // Macedonian-specific ѓ carried through as the seed.
        assert!(p("ѓавол").starts_with('ѓ'));
        // Uppercase folds to lowercase.
        assert!(p("СКОПЈЕ").starts_with('с'));
    }

    #[test]
    fn skopje_encodes_to_c212_style_key() {
        // "Скопје" — с seed, к=2, о(drop), п=1, ј=2 → "с212".
        assert_eq!(p("Скопје"), "с212");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("Скопје"), p("скопје"));
        assert_eq!(p("СКОПЈЕ"), p("скопје"));
    }

    #[test]
    fn short_input_pads_to_four_char_count() {
        // The output has char-count 4 (seed + 3 digits). Byte-length
        // is larger because the seed is a multi-byte Cyrillic scalar.
        let k = p("а");
        assert_eq!(k.chars().count(), 4);
        assert_eq!(k, "а000");
        let k = p("не");
        assert_eq!(k.chars().count(), 4);
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "асс" — а seed last=0. с code=7 push → "а7" last=7. с dup drop.
        //   pad → "а700".
        assert_eq!(p("асс"), "а700");
    }

    #[test]
    fn macedonian_specific_ѓ_ќ_encode_as_guttural_class() {
        // ѓ and ќ are palatal stops; they group with к/г as class 2.
        // "ѓ" alone at seed → "ѓ000".
        assert_eq!(p("ѓ"), "ѓ000");
        // "аѓа" — а seed, ѓ=2 push, а vow reset → "а2" pad → "а200".
        assert_eq!(p("аѓа"), "а200");
        // "аќа" — а seed, ќ=2 push, а vow reset → "а2" pad → "а200".
        assert_eq!(p("аќа"), "а200");
    }

    #[test]
    fn macedonian_specific_љ_њ_encode_as_liquid_nasal_classes() {
        // љ folds to class 4 (like л); њ folds to class 5 (like н).
        // "аља" — а seed, љ=4 push, а reset → "а400".
        assert_eq!(p("аља"), "а400");
        // "ања" — а seed, њ=5 push, а reset → "а500".
        assert_eq!(p("ања"), "а500");
    }

    #[test]
    fn macedonian_specific_ѕ_џ_encode_as_sibilant_class() {
        // ѕ (/dz/) and џ (/dʒ/) both fold to class 7.
        // "аѕа" — а seed, ѕ=7 push, а reset → "а700".
        assert_eq!(p("аѕа"), "а700");
        // "аџа" — а seed, џ=7 push, а reset → "а700".
        assert_eq!(p("аџа"), "а700");
    }

    #[test]
    fn macedonian_specific_ј_encodes_as_guttural_class() {
        // ј folds to class 2 (palatal glide, groups with the palatal
        // stops).
        // "ајв" — а seed, ј=2 push, в=1 push → "а21" pad → "а210".
        assert_eq!(p("ајв"), "а210");
    }

    #[test]
    fn vowels_reset_the_collapse_state() {
        // "аба" — а seed, б=1 push, а reset → "а100".
        assert_eq!(p("аба"), "а100");
        // "абаба" — а seed, б=1 push, а reset, б=1 push, а reset →
        //   "а11" but duplicate-collapse fires because b just after a
        //   vowel reset still has last_code=0 → the second б pushes as
        //   a fresh 1. Result: "а11" pad → "а110".
        assert_eq!(p("абаба"), "а110");
    }

    #[test]
    fn adapter_returns_name_phonex_mk() {
        assert_eq!(MacedonianPhonexAdapter.name(), "phonex-mk");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(MacedonianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = MacedonianPhonexAdapter.encode("Скопје").unwrap();
        assert_eq!(primary, "с212");
        assert!(alt.is_none());
    }
}
