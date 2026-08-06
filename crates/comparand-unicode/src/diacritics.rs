//! Diacritic stripping — remove combining marks after NFD.
//!
//! [`strip_diacritics`] implements the classic "flatten accents" pass
//! useful for search and fuzzy matching:
//!
//! 1. Decompose the input to NFD so precomposed accented characters
//!    become base + combining-mark sequences.
//! 2. Drop every character that Unicode classifies as a combining mark
//!    (categories `Mn`, `Mc`, `Me`).
//! 3. Recompose the residue to NFC.
//!
//! The result is: `"café"` → `"cafe"`, `"naïve"` → `"naive"`, `"résumé"`
//! → `"resume"`.
//!
//! # This is a lossy operation
//!
//! Diacritic stripping is deliberately lossy — that is the whole point —
//! and it should be used only when the caller wants that behavior.
//! Applied indiscriminately it silently corrupts meaning in scripts
//! whose combining marks are not "diacritics" in the Latin-script sense
//! (Devanagari matras, Hebrew and Arabic diacritics, tone marks). This
//! function does not attempt to be smart about that: if you pass it
//! `"नमस्ते"` it will strip the vowel marks and produce nonsense. The
//! caller is expected to know that stripping is appropriate for the
//! text it is stripping.
//!
//! # What is *not* stripped
//!
//! Only *combining marks* are removed. Characters like `Æ` (U+00C6),
//! `Œ` (U+0152), `ø` (U+00F8), or the Icelandic `þ` (U+00FE) are single
//! scalar values with no decomposition into base + mark, so they are
//! left as-is. If you want `Æ → AE`, `ø → o`, and so on, you want
//! **transliteration**, which is a different (and much more opinionated)
//! operation.
//!
//! Transliteration is a future concern for this crate; when it lands it
//! will live in its own module because the choices it forces on the
//! caller (source script, target script, romanization scheme, mapping
//! table) are orthogonal to the mechanical NFD-based approach here.

use crate::normalization::{nfc, nfd};
use alloc::string::String;
use unicode_normalization::char::is_combining_mark;

/// Removes combining marks from `input`, producing an NFC string.
///
/// Implementation: NFD-decompose the input, drop each character for
/// which [`is_combining_mark`] returns `true`, then NFC-recompose the
/// remainder.
///
/// See the [module documentation](self) for what this does and does not
/// cover — in particular, that non-Latin scripts are not automatically
/// handled and that single-scalar-value ligatures such as `Æ` are not
/// decomposed by this pass.
///
/// # Examples
///
/// ```
/// # use comparand_unicode::strip_diacritics;
/// assert_eq!(strip_diacritics("café"), "cafe");
/// assert_eq!(strip_diacritics("naïve"), "naive");
/// assert_eq!(strip_diacritics("résumé"), "resume");
/// // Æ is not decomposable into base + mark, so it is preserved.
/// assert_eq!(strip_diacritics("Ærøskøbing"), "Ærøskøbing");
/// // ASCII is untouched.
/// assert_eq!(strip_diacritics("Hello, World!"), "Hello, World!");
/// ```
#[must_use]
pub fn strip_diacritics(input: &str) -> String {
    let decomposed = nfd(input);
    let filtered: String = decomposed
        .chars()
        .filter(|c| !is_combining_mark(*c))
        .collect();
    nfc(&filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        assert_eq!(strip_diacritics(""), "");
    }

    #[test]
    fn ascii_is_preserved() {
        for s in ["", "hello", "A quick brown fox jumps 12 3.", "!@#$%^&*()"] {
            assert_eq!(strip_diacritics(s), s);
        }
    }

    #[test]
    fn cafe_strips() {
        assert_eq!(strip_diacritics("café"), "cafe");
    }

    #[test]
    fn cafe_decomposed_strips() {
        // "cafe" + combining acute — the acute is a Mn character and
        // gets dropped.
        assert_eq!(strip_diacritics("cafe\u{0301}"), "cafe");
    }

    #[test]
    fn naive_strips() {
        assert_eq!(strip_diacritics("naïve"), "naive");
    }

    #[test]
    fn ae_ligature_is_not_transliterated() {
        assert_eq!(strip_diacritics("Æ"), "Æ");
        assert_eq!(strip_diacritics("Ærøskøbing"), "Ærøskøbing");
    }

    #[test]
    fn cyrillic_without_precomposed_short_i_is_untouched() {
        // Cyrillic characters in these strings have no decomposable
        // marks, so they round-trip unchanged.
        for s in ["Москва", "Санкт-Петербург"] {
            assert_eq!(strip_diacritics(s), s);
        }
    }

    #[test]
    fn cyrillic_short_i_is_lossy() {
        // Cyrillic "й" (U+0439) is precomposed as И + combining short
        // (U+0306). NFD decomposes it, `strip_diacritics` drops the
        // combining short, and the recomposition leaves bare И — the
        // same lossy behavior Latin diacritic stripping produces.
        //
        // This is documented as a limitation of the mechanical NFD-based
        // approach: any script whose letters carry diacritic-like
        // combining marks will be affected by `strip_diacritics`.
        assert_eq!(strip_diacritics("й"), "и");
    }

    #[test]
    fn cjk_is_untouched() {
        for s in ["東京", "日本語", "中文简体"] {
            assert_eq!(strip_diacritics(s), s);
        }
    }
}
