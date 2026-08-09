//! Char-to-string lookup table — the classic transliteration
//! pattern.
//!
//! Build a [`TableTransliterator`] from a `(char, &str)` slice;
//! every scalar in the input either maps to its table entry or
//! passes through unchanged. The lookup uses a sorted `Vec` and
//! binary search — for the typical 30-100 char per-script
//! tables that's faster than a `HashMap`, and works under
//! `no_std + alloc`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Transliterator;

/// Char-to-string transliteration table.
///
/// Reusable; cheap to `Clone`. The underlying storage is a
/// sorted `Vec<(char, &'static str)>` — construction sorts it
/// once, then every lookup is `O(log n)`.
#[derive(Clone, Debug)]
pub struct TableTransliterator {
    // Sorted by `char` for binary search.
    entries: Vec<(char, &'static str)>,
}

impl TableTransliterator {
    /// Build a table from a `(char, &str)` slice. Duplicate keys
    /// in the input are preserved as multiple entries — binary
    /// search returns any one of them (undefined which). Callers
    /// with hand-authored tables shouldn't have duplicates.
    #[must_use]
    pub fn new(entries: &[(char, &'static str)]) -> Self {
        let mut v: Vec<(char, &'static str)> = entries.to_vec();
        v.sort_by_key(|&(c, _)| c);
        Self { entries: v }
    }

    /// Look up a single scalar. Returns `None` when the character
    /// isn't in the table.
    #[must_use]
    pub fn lookup(&self, c: char) -> Option<&'static str> {
        self.entries
            .binary_search_by_key(&c, |&(k, _)| k)
            .ok()
            .map(|i| self.entries[i].1)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the table is empty (matches no scalars).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Transliterator for TableTransliterator {
    fn transliterate(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            if let Some(s) = self.lookup(c) {
                out.push_str(s);
            } else {
                out.push(c);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_table_is_identity() {
        let t = TableTransliterator::new(&[]);
        assert_eq!(t.transliterate("hello"), "hello");
        assert!(t.is_empty());
    }

    #[test]
    fn simple_char_to_char_substitution() {
        let t = TableTransliterator::new(&[('a', "@"), ('e', "3"), ('o', "0")]);
        assert_eq!(t.transliterate("hello world"), "h3ll0 w0rld");
    }

    #[test]
    fn multibyte_scalars_and_multichar_replacements() {
        let t = TableTransliterator::new(&[('ß', "ss"), ('Ä', "Ae")]);
        assert_eq!(t.transliterate("STRAßE Ärger"), "STRAssE Aerger");
    }

    #[test]
    fn unlisted_chars_pass_through_unchanged() {
        let t = TableTransliterator::new(&[('a', "α")]);
        assert_eq!(t.transliterate("aaabbb"), "αααbbb");
    }

    #[test]
    fn binary_search_finds_the_right_entry() {
        // Unsorted input — the constructor sorts internally.
        let t = TableTransliterator::new(&[('z', "Z"), ('a', "A"), ('m', "M"), ('c', "C")]);
        assert_eq!(t.lookup('a'), Some("A"));
        assert_eq!(t.lookup('c'), Some("C"));
        assert_eq!(t.lookup('m'), Some("M"));
        assert_eq!(t.lookup('z'), Some("Z"));
        assert_eq!(t.lookup('b'), None);
    }
}
