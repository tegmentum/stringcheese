//! The Malayalam stopword list.
//!
//! ~55 common Malayalam function words: personal / demonstrative /
//! interrogative pronouns, postpositions, conjunctions, negation and
//! affirmation particles, high-frequency forms of the copula *ആണ്* /
//! *ഉണ്ട്* ("to be") and the auxiliary *ചെയ്യുക* ("to do"), and
//! common adverbs. The list deliberately omits content words.
//!
//! # Malayalam script — a note on storage width
//!
//! Every entry here is written in the Malayalam script. Malayalam
//! scalars live in the range **U+0D00..=U+0D7F**, which falls in
//! UTF-8's 3-byte range (U+0800..=U+FFFF). A word like `"ഞാൻ"` (3
//! characters: `ഞ` + `ാ` + `ൻ`) is **9 bytes**. Any code that mixes
//! byte offsets with character-boundary logic will silently corrupt
//! token or entry boundaries. The stopword list itself is a plain
//! `&[&str]` of static strings, so this concern only surfaces in
//! downstream iteration; every consumer in this crate walks scalars
//! via [`str::chars`] or [`Vec<char>`], not raw bytes.
//!
//! # Chillu letters
//!
//! Several entries end with a **chillu** letter — the atomic
//! "pure consonant" forms in U+0D7A..=U+0D7F (`ൺ ൻ ർ ൽ ൾ ൿ`) that
//! represent a word-final consonant without any following vowel or
//! virama. `ഞാൻ` "I" ends in `ൻ` (chillu n, U+0D7B); `അവർ` "they"
//! ends in `ർ` (chillu r, U+0D7C). Chillu letters are inside the
//! Malayalam block, so the tokenizer keeps them word-internal
//! automatically; the stemmer treats them as ordinary scalars and
//! its suffix table matches the multi-scalar chillu-carrying
//! endings (`-ിൽ`, `-ന്റെ`, `-ന്`) rather than stripping bare
//! chillu at word-end.
//!
//! # Case folding
//!
//! Malayalam has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Malayalam scalars (they are non-ASCII and pass through
//! the comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Malayalam input.
//!
//! # Non-goals
//!
//! - **Colloquial / spoken forms.** Written Malayalam
//!   (*grantha-Malayalam* register) diverges from spoken Malayalam
//!   in the surface form of many function words. The list targets
//!   the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.
//!   Downstream applications should carry their own.

/// The Malayalam stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Malayalam string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "ഞാൻ",    // I
    "എന്റെ",  // my
    "എനിക്ക്", // to me
    "എന്നെ",  // me (accusative)
    "നീ",    // you (familiar)
    "നിന്റെ", // your (familiar)
    "നിങ്ങൾ", // you (polite / plural)
    "അവൻ",   // he
    "അവൾ",   // she
    "അവർ",   // they / he-she (polite)
    "അത്",    // it / that
    "ഇത്",    // this
    "നമ്മൾ",  // we (inclusive)
    "ഞങ്ങൾ",  // we (exclusive)
    "അവരുടെ", // their
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "ഈ", // this (adj)
    "ആ", // that (adj)
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "എന്ത്",     // what
    "എന്ത",     // what (colloquial)
    "എങ്ങനെ",   // how
    "എന്തുകൊണ്ട്", // why
    "എപ്പോൾ",   // when
    "എവിടെ",   // where
    "ആര്",      // who
    "ഏത്",      // which
    // -----------------------------------------------------------------
    // Conjunctions.
    // -----------------------------------------------------------------
    "ഒപ്പം",     // with / along
    "അല്ലെങ്കിൽ", // or
    "പക്ഷേ",     // but
    "എന്നാൽ",     // but / however
    "അതിനാൽ",    // therefore / so
    "കാരണം",     // because
    "എന്ന്",      // that (complementizer)
    "കൂടാതെ",     // besides / in addition
    // -----------------------------------------------------------------
    // Negation / affirmation particles.
    // -----------------------------------------------------------------
    "ഇല്ല",  // no / not
    "അല്ല",  // is not (nominal)
    "ഉവ്വ്",  // yes
    "വേണ്ട", // don't want / need not
    // -----------------------------------------------------------------
    // "To be" (ആണ് / ഉണ്ട്) / common copula forms.
    // -----------------------------------------------------------------
    "ആണ്",     // is
    "ആയിരുന്നു", // was
    "ഉണ്ട്",    // exists / has
    "ആകുന്നു",   // becomes
    "ആയി",    // became
    // -----------------------------------------------------------------
    // Postpositions / spatial adverbs.
    // -----------------------------------------------------------------
    "നിന്ന്",  // from
    "വരെ",   // until
    "മുകളിൽ", // above
    "താഴെ",   // below
    "ഉള്ളിൽ", // inside
    "പുറത്ത്",  // outside
    "ഇവിടെ", // here
    "അവിടെ", // there
    "ഇപ്പോൾ", // now
    "അപ്പോൾ", // then
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "ഒരു",   // one / a (article)
    "പല",   // many
    "എല്ലാം", // all
    "ചില",  // some / a few
    "വളരെ", // very
    "മാത്രം", // only
    "കൂടി",  // also / too
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~55" — assert we're in a
        // reasonable ballpark.
        assert!(
            STOPWORDS.len() >= 50 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised 50-150 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_non_empty() {
        for &w in STOPWORDS {
            assert!(!w.is_empty(), "empty stopword entry");
        }
    }

    #[test]
    fn every_stopword_contains_malayalam() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Malayalam block U+0D00..=U+0D7F.
        for &w in STOPWORDS {
            let has_ml = w.chars().any(|c| ('\u{0D00}'..='\u{0D7F}').contains(&c));
            assert!(has_ml, "stopword {w:?} has no Malayalam scalar");
        }
    }

    #[test]
    fn no_entries_contain_ascii_whitespace() {
        // Multi-word phrases would never match a single token; keep the
        // list to single orthographic words.
        for &w in STOPWORDS {
            assert!(
                !w.chars().any(char::is_whitespace),
                "stopword {w:?} contains whitespace — must be a single word"
            );
        }
    }

    #[test]
    fn stopwords_contain_core_pronouns() {
        for &w in &["ഞാൻ", "നീ", "അവൻ", "അവൾ", "അവർ"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["ഒപ്പം", "അല്ലെങ്കിൽ", "പക്ഷേ"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["ആണ്", "ഉണ്ട്"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_and_affirmation() {
        for &w in &["ഇല്ല", "അല്ല"] {
            assert!(STOPWORDS.contains(&w), "particle {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~55.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
