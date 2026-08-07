//! [`EnglishCollator`] — dictionary-order English collation.
//!
//! Implements [`stringcheese_lang::Collator`] with three configurable
//! rules, all opt-in and independent:
//!
//! * **Ignore leading articles.** `"The Beatles"`,
//!   `"A Hard Day's Night"`, and `"An American in Paris"` sort as
//!   `"Beatles"`, `"Hard Day's Night"`, and `"American in Paris"` — the
//!   leading article and its following whitespace are skipped for
//!   comparison purposes. Matches the way a library card catalog, a
//!   movie index, or a bibliography orders titles. Only `a`, `an`, and
//!   `the` are stripped; the collator does *not* also treat `some`,
//!   `any`, or `all` as articles.
//! * **ASCII case-fold.** Uppercase and lowercase ASCII letters compare
//!   equal. Non-ASCII case is left alone (no Unicode case-folding
//!   tables — English's own alphabet is ASCII).
//! * **Digits after letters.** `"z" < "1"` in the collator's total
//!   order — the reverse of raw code-point order. This matches the
//!   convention English dictionaries and glossaries use for entries
//!   containing numerals: a section for words, then a trailing section
//!   for numeric entries.
//!
//! # Presets
//!
//! Two shipped configurations cover the common cases:
//!
//! * [`EnglishCollator::DICTIONARY`] — all three rules on. This is the
//!   default entry point ([`ENGLISH_DICTIONARY_COLLATOR`](crate::ENGLISH_DICTIONARY_COLLATOR))
//!   and what [`English::collator`](crate::English) hands back.
//! * [`EnglishCollator::ASCII`] — all three rules off. Equivalent to
//!   raw [`str::cmp`] ordering; useful as a baseline for tests or when
//!   the caller wants a locale-neutral comparator carried through the
//!   [`Language`](stringcheese_lang::Language) trait rather than
//!   plumbed separately.
//!
//! # Zero allocation
//!
//! [`EnglishCollator::compare`] walks both inputs character-by-character
//! and never allocates. The article-strip step returns a
//! sub-slice of the input rather than building a fresh `String`, and
//! ASCII case-fold is a per-character bit-twiddle. The collator carries
//! only three `bool` fields.
//!
//! # Example
//!
//! ```
//! use stringcheese_en::ENGLISH_DICTIONARY_COLLATOR;
//! use stringcheese_lang::Collator;
//! use core::cmp::Ordering;
//!
//! // Leading articles are ignored: "The Beatles" sorts as "Beatles",
//! // which comes after "Abbey Road" alphabetically.
//! assert_eq!(
//!     ENGLISH_DICTIONARY_COLLATOR.compare("Abbey Road", "The Beatles"),
//!     Ordering::Less,
//! );
//!
//! // Case is folded: "banana" and "BANANA" compare equal.
//! assert_eq!(
//!     ENGLISH_DICTIONARY_COLLATOR.compare("banana", "BANANA"),
//!     Ordering::Equal,
//! );
//!
//! // Digits sort after letters: "banana" comes before "1st".
//! assert_eq!(
//!     ENGLISH_DICTIONARY_COLLATOR.compare("banana", "1st"),
//!     Ordering::Less,
//! );
//! ```

use core::cmp::Ordering;

use stringcheese_lang::Collator;

/// A dictionary-order English collator.
///
/// See the [module-level docs](self) for the three configurable rules
/// and their rationale, and the two shipped presets
/// ([`DICTIONARY`](Self::DICTIONARY) and [`ASCII`](Self::ASCII)).
///
/// Constructed either through the presets, through
/// [`new`](Self::new) (which returns
/// [`DICTIONARY`](Self::DICTIONARY)), or through the builder-style
/// `with_*` methods that toggle a single flag at a time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnglishCollator {
    ignore_leading_articles: bool,
    case_insensitive: bool,
    digits_after_letters: bool,
}

impl EnglishCollator {
    /// The dictionary-order preset: every rule on.
    ///
    /// This is what [`ENGLISH_DICTIONARY_COLLATOR`](crate::ENGLISH_DICTIONARY_COLLATOR)
    /// exposes and what [`English::collator`](crate::English) hands
    /// back.
    pub const DICTIONARY: EnglishCollator = EnglishCollator {
        ignore_leading_articles: true,
        case_insensitive: true,
        digits_after_letters: true,
    };

    /// The raw-ASCII preset: every rule off.
    ///
    /// Equivalent to [`str::cmp`] as a total order — useful as a
    /// baseline for tests or when the caller wants a locale-neutral
    /// comparator carried through the
    /// [`Language`](stringcheese_lang::Language) trait rather than
    /// plumbed separately.
    pub const ASCII: EnglishCollator = EnglishCollator {
        ignore_leading_articles: false,
        case_insensitive: false,
        digits_after_letters: false,
    };

    /// Construct a new collator with the [`DICTIONARY`](Self::DICTIONARY)
    /// preset.
    ///
    /// Equivalent to `EnglishCollator::DICTIONARY`; provided as a
    /// familiar entry point for `Default`-shaped code.
    #[must_use]
    pub const fn new() -> Self {
        Self::DICTIONARY
    }

    /// Toggle the article-stripping rule.
    ///
    /// See the [module-level docs](self) for the list of articles
    /// (`a`, `an`, `the`).
    #[must_use]
    pub const fn with_ignore_leading_articles(mut self, v: bool) -> Self {
        self.ignore_leading_articles = v;
        self
    }

    /// Toggle the ASCII case-fold rule.
    #[must_use]
    pub const fn with_case_insensitive(mut self, v: bool) -> Self {
        self.case_insensitive = v;
        self
    }

    /// Toggle the digits-after-letters rule.
    #[must_use]
    pub const fn with_digits_after_letters(mut self, v: bool) -> Self {
        self.digits_after_letters = v;
        self
    }

    /// Character-comparison key for a single scalar.
    ///
    /// Combines the case-fold and digit-reordering rules into a single
    /// `u32` sort key. ASCII digits (`0`..=`9`) are lifted above the
    /// entire ASCII letter range when `digits_after_letters` is set;
    /// ASCII letters are collapsed to their lowercase form when
    /// `case_insensitive` is set. Non-ASCII scalars pass through
    /// unchanged.
    #[inline]
    fn char_key(self, c: char) -> u32 {
        if self.digits_after_letters && c.is_ascii_digit() {
            // Lift digits above ASCII 'z' (0x7A). Using 0x80 as the
            // base keeps digits below the Unicode block used for
            // non-ASCII characters (U+0080..=U+00FF Latin-1
            // Supplement) — a non-ASCII input scalar will always sort
            // above a repositioned ASCII digit, which matches the
            // "ASCII first, then Unicode" convention English
            // dictionaries and glossaries follow.
            return 0x80 + (c as u32 - '0' as u32);
        }
        if self.case_insensitive && c.is_ascii_alphabetic() {
            return c.to_ascii_lowercase() as u32;
        }
        c as u32
    }
}

impl Default for EnglishCollator {
    fn default() -> Self {
        Self::DICTIONARY
    }
}

impl Collator for EnglishCollator {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        let a = if self.ignore_leading_articles {
            strip_leading_article(a)
        } else {
            a
        };
        let b = if self.ignore_leading_articles {
            strip_leading_article(b)
        } else {
            b
        };

        let mut ai = a.chars();
        let mut bi = b.chars();
        loop {
            match (ai.next(), bi.next()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(ac), Some(bc)) => {
                    let ak = self.char_key(ac);
                    let bk = self.char_key(bc);
                    match ak.cmp(&bk) {
                        Ordering::Equal => {}
                        ord => return ord,
                    }
                }
            }
        }
    }
}

/// Strip a leading English article (`a`, `an`, `the`) plus any
/// following ASCII whitespace from `s`.
///
/// Returns the input unchanged when it does not start with any of the
/// three articles (case-insensitive) followed by ASCII whitespace.
/// Only these three articles are recognized — not `some`, `any`, or
/// `all` — matching the convention used by library card catalogs and
/// movie indexes.
fn strip_leading_article(s: &str) -> &str {
    // Longest-first: check "the" before "an" before "a" so a title
    // starting with "an " isn't mistakenly matched as "a" + "n ".
    for article in ["the", "an", "a"] {
        let alen = article.len();
        if s.len() > alen && s.is_char_boundary(alen) && s[..alen].eq_ignore_ascii_case(article) {
            // Byte at position `alen` must be an ASCII whitespace
            // separator — otherwise the article is a prefix of a
            // longer word ("theater", "another", "apple") and we
            // leave the string alone.
            let sep = s.as_bytes()[alen];
            if sep == b' ' || sep == b'\t' {
                // Skip the article + all subsequent ASCII whitespace.
                let mut rest = &s[alen + 1..];
                while let Some(stripped) =
                    rest.strip_prefix(' ').or_else(|| rest.strip_prefix('\t'))
                {
                    rest = stripped;
                }
                return rest;
            }
        }
    }
    s
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    // ---- article stripping ------------------------------------------

    #[test]
    fn strip_leading_article_recognizes_the() {
        assert_eq!(strip_leading_article("The Beatles"), "Beatles");
        assert_eq!(strip_leading_article("the beatles"), "beatles");
        assert_eq!(strip_leading_article("THE BEATLES"), "BEATLES");
    }

    #[test]
    fn strip_leading_article_recognizes_a_and_an() {
        assert_eq!(
            strip_leading_article("A Tale of Two Cities"),
            "Tale of Two Cities"
        );
        assert_eq!(
            strip_leading_article("An American in Paris"),
            "American in Paris"
        );
    }

    #[test]
    fn strip_leading_article_leaves_prefix_words_alone() {
        // "theater" starts with "the" but is a longer word.
        assert_eq!(strip_leading_article("Theater"), "Theater");
        // "another" starts with "an" but is a longer word.
        assert_eq!(strip_leading_article("another"), "another");
        // "apple" starts with "a" but is a longer word.
        assert_eq!(strip_leading_article("apple"), "apple");
    }

    #[test]
    fn strip_leading_article_ignores_extra_whitespace() {
        assert_eq!(strip_leading_article("The   Matrix"), "Matrix");
        assert_eq!(strip_leading_article("A\tPage"), "Page");
    }

    #[test]
    fn strip_leading_article_does_not_recognize_some_or_any() {
        assert_eq!(
            strip_leading_article("Some Like It Hot"),
            "Some Like It Hot"
        );
        assert_eq!(
            strip_leading_article("Any Given Sunday"),
            "Any Given Sunday"
        );
        assert_eq!(
            strip_leading_article("All The Presidents Men"),
            "All The Presidents Men"
        );
    }

    // ---- individual rule flags --------------------------------------

    #[test]
    fn ascii_preset_is_raw_str_cmp() {
        let c = EnglishCollator::ASCII;
        assert_eq!(c.compare("A", "a"), "A".cmp("a"));
        assert_eq!(c.compare("banana", "1st"), "banana".cmp("1st"));
        assert_eq!(
            c.compare("The Beatles", "Abbey"),
            "The Beatles".cmp("Abbey")
        );
    }

    #[test]
    fn case_insensitive_flag_only() {
        let c = EnglishCollator::ASCII.with_case_insensitive(true);
        assert_eq!(c.compare("Apple", "apple"), Ordering::Equal);
        assert_eq!(c.compare("BANANA", "apple"), Ordering::Greater);
        assert_eq!(c.compare("Apple", "banana"), Ordering::Less);
    }

    #[test]
    fn digits_after_letters_flag_only() {
        let c = EnglishCollator::ASCII.with_digits_after_letters(true);
        // Digits sort AFTER letters — reversed from raw code-point order.
        assert_eq!(c.compare("z", "1"), Ordering::Less);
        assert_eq!(c.compare("1", "z"), Ordering::Greater);
        // Within digits, natural ordering is preserved.
        assert_eq!(c.compare("1", "2"), Ordering::Less);
        // Uppercase letter still compares against lowercase per raw
        // code-point (case_insensitive is off).
        assert_eq!(c.compare("A", "a"), "A".cmp("a"));
    }

    #[test]
    fn ignore_leading_articles_flag_only() {
        let c = EnglishCollator::ASCII.with_ignore_leading_articles(true);
        // "The Beatles" strips to "Beatles"; compare "Beatles" vs "Abbey".
        assert_eq!(c.compare("The Beatles", "Abbey Road"), Ordering::Greater);
        // "A Hard Day's Night" strips to "Hard Day's Night".
        assert_eq!(c.compare("A Hard Day's Night", "Grease"), Ordering::Greater);
    }

    // ---- dictionary preset ------------------------------------------

    #[test]
    fn dictionary_sort_ignores_leading_articles() {
        let mut albums: Vec<&str> = ["The Beatles", "Abbey Road", "A Hard Day's Night"].into();
        albums.sort_by(|a, b| EnglishCollator::DICTIONARY.compare(a, b));
        // "Abbey Road" (A) < "The Beatles" (B) < "A Hard Day's Night" (H).
        assert_eq!(albums, ["Abbey Road", "The Beatles", "A Hard Day's Night"]);
    }

    #[test]
    fn dictionary_sort_case_insensitive() {
        let mut words: Vec<&str> = ["Banana", "apple", "CHERRY"].into();
        words.sort_by(|a, b| EnglishCollator::DICTIONARY.compare(a, b));
        assert_eq!(words, ["apple", "Banana", "CHERRY"]);
    }

    #[test]
    fn dictionary_sort_digits_after_letters() {
        let mut mixed: Vec<&str> = ["banana", "1st place", "apple"].into();
        mixed.sort_by(|a, b| EnglishCollator::DICTIONARY.compare(a, b));
        assert_eq!(mixed, ["apple", "banana", "1st place"]);
    }

    #[test]
    fn dictionary_sort_combined_rules() {
        // Combines all three rules on a fresh set.
        let mut titles: Vec<&str> = [
            "The Matrix",
            "1984",
            "a Clockwork Orange",
            "Bambi",
            "AN American Tail",
        ]
        .into();
        titles.sort_by(|a, b| EnglishCollator::DICTIONARY.compare(a, b));
        // Article-stripped, case-folded: "american tail" (AN), "bambi",
        // "clockwork orange" (a), "matrix" (The), "1984".
        assert_eq!(
            titles,
            [
                "AN American Tail",
                "Bambi",
                "a Clockwork Orange",
                "The Matrix",
                "1984",
            ]
        );
    }

    #[test]
    fn dictionary_new_equals_dictionary_preset() {
        assert_eq!(EnglishCollator::new(), EnglishCollator::DICTIONARY);
        assert_eq!(EnglishCollator::default(), EnglishCollator::DICTIONARY);
    }

    #[test]
    fn builder_composes_flags() {
        let c = EnglishCollator::ASCII
            .with_case_insensitive(true)
            .with_digits_after_letters(true);
        assert!(!c.ignore_leading_articles);
        assert!(c.case_insensitive);
        assert!(c.digits_after_letters);
        // Round-trip: turning a flag back off restores the earlier state.
        let c2 = c.with_case_insensitive(false);
        assert!(!c2.case_insensitive);
    }

    // ---- Collator contract axioms (spot checks) ---------------------

    #[test]
    fn compare_is_reflexive() {
        let c = EnglishCollator::DICTIONARY;
        for s in ["", "The Beatles", "1984", "banana"] {
            assert_eq!(c.compare(s, s), Ordering::Equal);
        }
    }

    #[test]
    fn compare_is_antisymmetric() {
        let c = EnglishCollator::DICTIONARY;
        for (a, b) in [
            ("apple", "banana"),
            ("The Beatles", "Abbey Road"),
            ("1", "z"),
            ("A", "a"),
        ] {
            let ab = c.compare(a, b);
            let ba = c.compare(b, a);
            assert_eq!(ab, ba.reverse(), "compare({a:?}, {b:?}) antisymmetry");
        }
    }

    #[test]
    fn empty_strings_compare_equal() {
        let c = EnglishCollator::DICTIONARY;
        assert_eq!(c.compare("", ""), Ordering::Equal);
        assert_eq!(c.compare("", "a"), Ordering::Less);
        assert_eq!(c.compare("a", ""), Ordering::Greater);
    }

    #[test]
    fn char_key_case_folds_and_reorders_digits() {
        let c = EnglishCollator::DICTIONARY;
        // Case folding: 'A' and 'a' produce the same key.
        assert_eq!(c.char_key('A'), c.char_key('a'));
        // Order: 'a' < 'b'.
        assert!(c.char_key('a') < c.char_key('b'));
        // Digits sort above all ASCII letters.
        assert!(c.char_key('z') < c.char_key('0'));
        assert!(c.char_key('0') < c.char_key('9'));
    }
}
