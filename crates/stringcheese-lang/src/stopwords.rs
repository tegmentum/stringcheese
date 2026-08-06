//! [`Stopwords`] — a thin wrapper over a static slice of stopwords.
//!
//! The wrapper's job is a case-insensitive [`contains`](Stopwords::contains)
//! check that language packs would otherwise re-implement every time. It
//! stores nothing but a `&'static [&'static str]`, so it is
//! `no_std`-friendly and lives entirely in the caller's stopword table.

/// A case-insensitive stopword set backed by a static slice.
///
/// A language pack constructs one of these from a hand-authored
/// stopword table and stores it in a `const`; the trait's
/// [`stopwords`](crate::Language::stopwords) accessor returns the
/// underlying slice. Callers use [`contains`](Self::contains) to test
/// membership.
///
/// The membership check is deliberately linear (`O(n)`) — stopword
/// lists are small (dozens to a few hundred entries) and ASCII, and a
/// linear scan beats a hash lookup at these sizes while avoiding the
/// `hashbrown` / `std::collections::HashSet` dependency. Language packs
/// that need faster lookup can maintain their own perfect-hash table
/// and override [`Language::is_stopword`](crate::Language::is_stopword).
///
/// # Case handling
///
/// [`contains`](Self::contains) compares its argument against the
/// stopword table with an ASCII-case-insensitive equality
/// ([`str::eq_ignore_ascii_case`]). Language packs whose stopwords
/// live outside ASCII (Turkish `İ`, German `ß`, …) should override
/// [`Language::is_stopword`](crate::Language::is_stopword) with a
/// Unicode-aware equality — the default is intentionally the
/// English-shaped baseline.
///
/// # Example
///
/// ```
/// use stringcheese_lang::Stopwords;
///
/// const WORDS: &[&str] = &["the", "and", "of"];
/// const SW: Stopwords = Stopwords::new(WORDS);
///
/// assert!(SW.contains("the"));
/// assert!(SW.contains("THE"));
/// assert!(!SW.contains("cheese"));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stopwords {
    words: &'static [&'static str],
}

impl Stopwords {
    /// Wraps the supplied static slice as a [`Stopwords`] set.
    #[inline]
    #[must_use]
    pub const fn new(words: &'static [&'static str]) -> Self {
        Self { words }
    }

    /// Returns the underlying slice.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &'static [&'static str] {
        self.words
    }

    /// Returns the number of stopwords.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.words.len()
    }

    /// Returns `true` if the stopword slice is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Returns `true` if `word` matches any stopword under ASCII
    /// case-insensitive equality.
    ///
    /// See the type-level docs for the rationale (linear scan; ASCII
    /// case folding is the default so language packs whose stopword
    /// alphabet extends beyond ASCII should override
    /// [`Language::is_stopword`](crate::Language::is_stopword)).
    #[must_use]
    pub fn contains(&self, word: &str) -> bool {
        self.words
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: &[&str] = &["the", "and", "of", "a"];

    #[test]
    fn contains_exact_match() {
        let sw = Stopwords::new(WORDS);
        assert!(sw.contains("the"));
        assert!(sw.contains("and"));
    }

    #[test]
    fn contains_is_case_insensitive() {
        let sw = Stopwords::new(WORDS);
        assert!(sw.contains("The"));
        assert!(sw.contains("THE"));
        assert!(sw.contains("tHe"));
    }

    #[test]
    fn contains_returns_false_on_miss() {
        let sw = Stopwords::new(WORDS);
        assert!(!sw.contains("cheese"));
        assert!(!sw.contains(""));
    }

    #[test]
    fn as_slice_returns_the_wrapped_data() {
        let sw = Stopwords::new(WORDS);
        assert_eq!(sw.as_slice(), WORDS);
        assert_eq!(sw.len(), WORDS.len());
        assert!(!sw.is_empty());
    }

    #[test]
    fn empty_stopwords_is_recognized() {
        let sw = Stopwords::new(&[]);
        assert!(sw.is_empty());
        assert_eq!(sw.len(), 0);
        assert!(!sw.contains("the"));
    }
}
