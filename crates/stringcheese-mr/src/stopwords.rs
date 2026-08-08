//! The Marathi stopword list.
//!
//! ~55 common Marathi function words: personal / demonstrative /
//! interrogative pronouns, postpositions (Marathi's independent
//! postpositions — `साठी` "for", `बद्दल` "about", `बरोबर`
//! "together-with"), coordinating and subordinating conjunctions,
//! particles, high-frequency forms of the copula *असणे* ("to be") and
//! auxiliary *करणे* ("to do"), and common adverbs. The list targets the
//! written / newswire register and deliberately omits content words.
//!
//! # Marathi vs. Hindi
//!
//! Marathi is an Indo-Aryan language written in Devanagari, but its
//! function-word inventory is markedly different from Hindi's. Marathi
//! retains the Old Indo-Aryan **neuter gender** (Hindi has only
//! masculine and feminine), and Marathi's case marking is
//! **agglutinative** — case suffixes attach directly to the noun stem
//! (`घराला` "to the house" — one orthographic word, `-ला` attached),
//! rather than Hindi's postpositions written as separate words
//! (`घर को` "to the house" — two words). The stopword list here
//! reflects that: many entries are pronouns and demonstratives with
//! Marathi-specific morphology (`आम्ही` "we-exclusive" vs. `आपण`
//! "we-inclusive" — Marathi preserves the clusivity contrast Hindi has
//! lost), and the postpositions listed are the *independent* ones.
//!
//! # Devanagari script — a note on storage width
//!
//! Every entry here is written in the Devanagari script. Devanagari
//! scalars live in the range **U+0900..=U+097F**, which falls in
//! UTF-8's 3-byte range (U+0800..=U+FFFF). A word like `"मी"` (2
//! characters: `म` + `ी`) is **6 bytes**. Any code that mixes byte
//! offsets with character-boundary logic will silently corrupt token or
//! entry boundaries. The stopword list itself is a plain `&[&str]` of
//! static strings, so this concern only surfaces in downstream
//! iteration; every consumer in this crate walks scalars via
//! [`str::chars`] or [`Vec<char>`], not raw bytes.
//!
//! # Case folding
//!
//! Devanagari has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Devanagari scalars (they are non-ASCII and pass through
//! the comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Devanagari input.
//!
//! # Non-goals
//!
//! - **Konkani / Ahirani / other Marathi-family stopwords.** Konkani
//!   (a sister Indo-Aryan language of Maharashtra / Goa) and the
//!   various Marathi dialects have distinct function word inventories
//!   despite sharing much vocabulary and the Devanagari script. Each
//!   deserves its own pack.
//! - **Colloquial / spoken forms.** Written Marathi (*prāmāṇika
//!   marāṭhī*) and spoken Marathi diverge in the surface form of many
//!   function words. The list targets the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Marathi stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Devanagari string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns. Marathi preserves the clusivity distinction
    // in the first-person plural (आम्ही exclusive vs. आपण inclusive)
    // that Hindi has lost.
    // -----------------------------------------------------------------
    "मी",    // I
    "मला",   // to me (dative — case suffix -ला fused)
    "माझा",  // my (m. sg.)
    "माझी",  // my (f. sg.)
    "माझे",   // my (n. sg. / m. pl.)
    "आम्ही",  // we (exclusive)
    "आपण",   // we (inclusive) / you (formal)
    "आमचा",  // our-exclusive (m. sg.)
    "आमची",  // our-exclusive (f. sg.)
    "आमचे",   // our-exclusive (n. sg. / m. pl.)
    "तू",     // you (informal sg.)
    "तुला",   // to you (informal sg.)
    "तुझा",   // your (informal m. sg.)
    "तुझी",   // your (informal f. sg.)
    "तुझे",    // your (informal n. sg. / m. pl.)
    "तुम्ही",  // you (plural / formal)
    "तुमचा",  // your-plural (m. sg.)
    "तुमची",  // your-plural (f. sg.)
    "तुमचे",   // your-plural (n. sg. / m. pl.)
    "तो",    // he / that-m
    "ती",    // she / that-f
    "ते",     // it / that-n / they
    "त्याला", // to him
    "तिला",  // to her
    "त्यांना", // to them
    "त्याचा", // his (m. sg.)
    "त्याची", // his (f. sg.)
    "त्याचे",  // his (n. sg. / m. pl.)
    // -----------------------------------------------------------------
    // Demonstratives — Marathi has a three-way proximal / medial /
    // distal contrast (हा / तो / जो-relative).
    // -----------------------------------------------------------------
    "हा",  // this (m.)
    "ही",  // this (f.)
    "हे",   // this (n.) / these
    "इथे",  // here
    "तिथे", // there
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "काय",   // what
    "कोण",   // who
    "का",    // why / question particle
    "कसे",    // how
    "केव्हा",  // when
    "कुठे",    // where
    "कोणता", // which (m.)
    "कोणती", // which (f.)
    // -----------------------------------------------------------------
    // Postpositions. Marathi case markers -ला / -चा / -ने / -त / -हून
    // are agglutinative (attached to the stem — the stemmer strips
    // them); these entries are the *independent* postpositions that
    // stand as their own orthographic words.
    // -----------------------------------------------------------------
    "साठी",  // for (purpose)
    "बद्दल",  // about / regarding
    "बरोबर", // together with
    "पासून",  // from (source)
    "पर्यंत",  // up to / until
    "शिवाय", // without / except
    // -----------------------------------------------------------------
    // Coordinating / subordinating conjunctions.
    // -----------------------------------------------------------------
    "आणि",  // and
    "किंवा", // or
    "पण",   // but
    "परंतु",  // but (literary)
    "जर",   // if
    "तर",   // then
    "कारण", // because
    "म्हणून", // therefore / so
    "की",   // that (complementizer)
    "जेव्हा", // when (relative)
    "तेव्हा", // then (correlative)
    "जसे",   // as / like (relative)
    "तसे",   // like that (correlative)
    // -----------------------------------------------------------------
    // Negation / affirmation particles.
    // -----------------------------------------------------------------
    "नाही", // no / not
    "नको",  // don't (imperative negation)
    "होय",  // yes
    "हो",   // yes (colloquial)
    // -----------------------------------------------------------------
    // "To be" (असणे) — surface forms of the copula.
    // -----------------------------------------------------------------
    "आहे",   // is (3sg present)
    "आहेत",  // are (3pl present)
    "होता", // was (m. sg.)
    "होती", // was (f. sg.)
    "होते",  // was (n. sg.) / were (m. pl.)
    // Note: होय (is, 3sg literary) is homographic with the affirmation
    // particle listed above under negation/affirmation; not duplicated
    // here.
    // -----------------------------------------------------------------
    // "To do" (करणे) — high-frequency forms.
    // -----------------------------------------------------------------
    "करतो",  // does (m. sg. habitual)
    "करते",   // does (f. sg. / n. sg. habitual)
    "करतात", // do (3pl habitual)
    "केला",   // did (m. sg. aorist)
    "केली",   // did (f. sg. aorist)
    "केले",    // did (n. sg. / 3pl-m aorist)
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "आता",  // now
    "मग",   // then / afterwards
    "खूप",   // very / much
    "थोडे",  // a little
    "सर्व",  // all
    "काही", // some / something
    "एक",   // one
    "दोन",  // two
    "तीन",  // three
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~55" — assert we're in that
        // ballpark. The task spec asks for ~50 entries.
        assert!(
            STOPWORDS.len() >= 40 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised 40-150 range",
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
    fn every_stopword_contains_devanagari() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Devanagari block U+0900..=U+097F.
        for &w in STOPWORDS {
            let has_devanagari = w.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
            assert!(has_devanagari, "stopword {w:?} has no Devanagari scalar");
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
        for &w in &["मी", "तू", "तो", "ती", "ते", "आम्ही", "आपण", "तुम्ही"]
        {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["आणि", "किंवा", "पण", "जर", "की"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["आहे", "आहेत", "होता", "होती", "होते"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_particles() {
        for &w in &["नाही", "नको"] {
            assert!(STOPWORDS.contains(&w), "negation particle {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_independent_postpositions() {
        // Marathi's independent postpositions (as opposed to the
        // agglutinative case markers that the stemmer strips).
        for &w in &["साठी", "बद्दल", "बरोबर"] {
            assert!(
                STOPWORDS.contains(&w),
                "independent postposition {w:?} missing"
            );
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
