//! The Tamil stopword list.
//!
//! ~55 common Tamil function words: personal / demonstrative /
//! interrogative pronouns, postpositions, conjunctions, negation and
//! affirmation particles, high-frequency forms of the auxiliary
//! *இரு-* ("to be") and *ஆகு-* ("to become"), and common adverbs. The
//! list is drawn from the intersection of several open Tamil stopword
//! corpora; it deliberately omits content words.
//!
//! # Tamil script — a note on storage width
//!
//! Every entry here is written in the Tamil script. Tamil scalars
//! live in the range **U+0B80..=U+0BFF**, which falls in UTF-8's
//! 3-byte range (U+0800..=U+FFFF). A word like `"நான்"` (4
//! characters: `ந` + `ா` + `ன` + `்`) is **12 bytes**. Any code
//! that mixes byte offsets with character-boundary logic will
//! silently corrupt token or entry boundaries. The stopword list
//! itself is a plain `&[&str]` of static strings, so this concern
//! only surfaces in downstream iteration; every consumer in this
//! crate walks scalars via [`str::chars`] or [`Vec<char>`], not raw
//! bytes.
//!
//! # Case folding
//!
//! Tamil has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Tamil scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Tamil input.
//!
//! # Non-goals
//!
//! - **Colloquial / spoken forms.** Written Tamil (centamiḻ) diverges
//!   from spoken Tamil (koṭuntamiḻ) in the surface form of many
//!   function words. The list targets the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Tamil stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Tamil string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "நான்",    // I
    "என்",    // my / me (short)
    "எனக்கு", // to me
    "என்னை",  // me (accusative)
    "நீ",     // you (familiar)
    "உன்",    // your (familiar)
    "நீங்கள்",  // you (polite / plural)
    "உங்கள்",  // your (polite / plural)
    "அவன்",   // he
    "அவள்",   // she
    "அவர்",   // he/she (polite / honorific)
    "அது",   // it / that
    "இது",   // this
    "நாம்",    // we (inclusive)
    "நாங்கள்",  // we (exclusive)
    "அவர்கள்", // they
    "அவை",   // they (inanimate)
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "இந்த", // this (adj)
    "அந்த", // that (adj)
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "எது",    // which
    "என்ன",    // what
    "எப்படி",  // how
    "ஏன்",     // why
    "எப்போது", // when
    "எங்கே",   // where
    "யார்",     // who
    "எவ்வளவு", // how much
    // -----------------------------------------------------------------
    // Conjunctions.
    // -----------------------------------------------------------------
    "மற்றும்",  // and
    "அல்லது",  // or
    "ஆனால்",    // but
    "எனவே",   // therefore / so
    "ஏனெனில்", // because
    "என்று",   // that (complementizer)
    // -----------------------------------------------------------------
    // Negation / affirmation particles.
    // -----------------------------------------------------------------
    "இல்லை", // no / not
    "ஆம்",   // yes
    "அல்ல",  // is not (nominal)
    // -----------------------------------------------------------------
    // "To be" (இரு-) / "to become" (ஆகு-) — high-frequency forms.
    // -----------------------------------------------------------------
    "உள்ளது",     // is (present)
    "இருக்கிறது", // is (progressive)
    "இருந்தது",   // was
    "ஆகிறது",    // becomes
    "ஆயிற்று",    // became
    // -----------------------------------------------------------------
    // Postpositions / spatial adverbs.
    // -----------------------------------------------------------------
    "இருந்து", // from
    "வரை",    // until
    "மேல்",    // on / above
    "கீழ்",     // under / below
    "உள்ளே",   // inside
    "வெளியே", // outside
    "இங்கே",   // here
    "அங்கே",   // there
    "இப்போது", // now
    "அப்போது", // then
    "போது",   // when (subordinator)
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "ஒரு",   // one / a (article)
    "பல",    // many
    "எல்லாம்",  // all
    "சில",   // some / a few
    "மிக",   // very
    "தான்",    // emphatic particle
    "கூட",   // also / even
    "மட்டும்", // only
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
    fn every_stopword_contains_tamil() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Tamil block U+0B80..=U+0BFF.
        for &w in STOPWORDS {
            let has_tamil = w.chars().any(|c| ('\u{0B80}'..='\u{0BFF}').contains(&c));
            assert!(has_tamil, "stopword {w:?} has no Tamil scalar");
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
        for &w in &["நான்", "நீ", "அவன்", "அவள்", "அவர்"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["மற்றும்", "அல்லது", "ஆனால்"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["உள்ளது", "இருந்தது"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_and_affirmation() {
        for &w in &["இல்லை", "ஆம்"] {
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
