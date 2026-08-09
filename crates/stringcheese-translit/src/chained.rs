//! Combinator: apply two transliterators in sequence.

use alloc::string::String;

use crate::Transliterator;

/// Compose two transliterators. `Chained::new(a, b).transliterate(s)`
/// is equivalent to `b.transliterate(&a.transliterate(s))`.
///
/// Order matters. Transliteration stages don't commute — Cyrillic
/// → Latin followed by ASCII case-fold produces something
/// different from case-fold-then-transliterate (the case-fold
/// path has nothing to fold before the Cyrillic gets romanised).
#[derive(Clone, Debug)]
pub struct Chained<A: Transliterator, B: Transliterator> {
    first: A,
    second: B,
}

impl<A: Transliterator, B: Transliterator> Chained<A, B> {
    /// Compose `first` then `second`.
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A: Transliterator, B: Transliterator> Transliterator for Chained<A, B> {
    fn transliterate(&self, input: &str) -> String {
        let intermediate = self.first.transliterate(input);
        self.second.transliterate(&intermediate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TableTransliterator;

    #[test]
    fn chain_applies_in_order() {
        // First: replace 'a' with "X"; second: replace 'X' with "!".
        let a = TableTransliterator::new(&[('a', "X")]);
        let b = TableTransliterator::new(&[('X', "!")]);
        let chained = Chained::new(a, b);
        assert_eq!(chained.transliterate("banana"), "b!n!n!");
    }

    #[test]
    fn chain_is_not_commutative() {
        // First: 'a' → "b"; second: 'b' → "a". Applied one way
        // the input `a` → `b` → `a` (identity); the other way,
        // `a` (no match in first table) → `a`.
        let ab = TableTransliterator::new(&[('a', "b")]);
        let ba = TableTransliterator::new(&[('b', "a")]);
        let ab_then_ba = Chained::new(ab.clone(), ba.clone());
        let ba_then_ab = Chained::new(ba, ab);
        // ab_then_ba: "a" → "b" → "a" ; "b" → "b" (no match in ab) → "a"
        assert_eq!(ab_then_ba.transliterate("ab"), "aa");
        // ba_then_ab: "a" → "a" → "b" ; "b" → "a" → "b"
        assert_eq!(ba_then_ab.transliterate("ab"), "bb");
    }
}
