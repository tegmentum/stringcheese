//! [`deunicode`]-backed general-purpose transliterator.

use alloc::string::String;

use crate::Transliterator;

/// General-purpose transliterator: every non-ASCII scalar gets a
/// deunicode-approximate ASCII rendering.
///
/// Cheap to construct (stateless); the deunicode table is a
/// static.
#[derive(Copy, Clone, Debug, Default)]
pub struct DeunicodeTransliterator;

impl DeunicodeTransliterator {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Transliterator for DeunicodeTransliterator {
    fn transliterate(&self, input: &str) -> String {
        deunicode::deunicode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        let t = DeunicodeTransliterator::new();
        assert_eq!(t.transliterate("hello"), "hello");
    }

    #[test]
    fn accents_stripped() {
        let t = DeunicodeTransliterator::new();
        assert_eq!(t.transliterate("Café résumé"), "Cafe resume");
    }

    #[test]
    fn cjk_gets_a_rendering() {
        // deunicode gives some transliteration for every scalar;
        // the specific form isn't part of our contract, but the
        // output must be non-empty ASCII.
        let t = DeunicodeTransliterator::new();
        let out = t.transliterate("日本語");
        assert!(!out.is_empty());
        assert!(out.is_ascii());
    }
}
