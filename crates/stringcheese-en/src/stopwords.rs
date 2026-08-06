//! The English stopword list.
//!
//! The list contains 152 common English words drawn from the union of
//! the traditional van Rijsbergen list (as taught in the classic IR
//! literature), NLTK's `english` list, and scikit-learn's
//! `ENGLISH_STOP_WORDS`. It is deliberately modest — no domain-specific
//! jargon, no archaic forms, no contractions beyond the most common
//! (see `n't`, `'ll`, `'re` handling in the tokenizer roadmap).
//!
//! # Non-goals
//!
//! - **Contraction fragments.** Words like `n't`, `'ll`, `'re`, `'ve`
//!   are absent because the shipped tokenizer does not split
//!   contractions (`"don't"` yields `["don", "t"]`, not
//!   `["do", "n't"]`). A future English tokenizer that does split
//!   them will bring its own extension of this list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with
//!   [`str::eq_ignore_ascii_case`], so `"the"`, `"The"`, and `"THE"`
//!   are all recognized as stopwords.

/// The English stopword list (152 entries).
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "am",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "did",
    "do",
    "does",
    "doing",
    "don",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "just",
    "me",
    "more",
    "most",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "s",
    "same",
    "she",
    "should",
    "so",
    "some",
    "such",
    "t",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
    // Additions from the scikit-learn / SMART lists that are widely
    // useful across IR corpora.
    "also",
    "another",
    "back",
    "come",
    "could",
    "even",
    "get",
    "give",
    "go",
    "like",
    "make",
    "many",
    "must",
    "never",
    "new",
    "one",
    "say",
    "see",
    "since",
    "still",
    "take",
    "well",
    "would",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~150" — assert we're in the
        // ballpark.
        assert!(
            STOPWORDS.len() >= 140 && STOPWORDS.len() <= 170,
            "STOPWORDS.len() = {} outside the advertised ~150 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase_ascii() {
        for &w in STOPWORDS {
            assert!(
                w.bytes().all(|b| b.is_ascii_lowercase() || b == b'\''),
                "stopword {w:?} is not lowercase ASCII"
            );
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~150.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
