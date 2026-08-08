//! The Armenian (Eastern) stopword list.
//!
//! Roughly 55 entries of common Eastern Armenian words drawn from the
//! canonical personal / demonstrative / interrogative pronouns,
//! conjunctions, prepositions, particles, the copula (`եմ` / `ես` /
//! `է` / `ենք` / `եք` / `են`), the negator (`չէ` / `չեմ`), and a
//! handful of high-frequency adverbs / quantifiers.
//!
//! # Armenian stopwords
//!
//! Every entry is a lowercase Armenian-script string. Armenian
//! orthography uses both upper (`Ա-Ֆ`) and lower (`ա-ֆ`) case; Rust's
//! default [`char::to_lowercase`] fold handles the case mapping
//! correctly for the full 39-letter inventory. The pack's
//! `is_stopword` override applies the case fold plus a `եւ → և`
//! normalization before comparison so an uppercase query like `ԵՒ`
//! (Ե + Ւ upper Yiwn) lowercases to `եւ` and then normalizes to `և`
//! for matching against the lowercase entry.
//!
//! # Non-goals
//!
//! - **Western Armenian.** Western Armenian (spoken across the
//!   Armenian diaspora) has a distinct phonology (the plain / voiced
//!   stop distinction of Eastern Armenian collapses: Western reads `բ`
//!   as /pʰ/ where Eastern reads it as /b/) and some morphological
//!   divergence. This pack targets **Eastern Armenian** (the standard
//!   of the Republic of Armenia); a future `stringcheese-hyw` sibling
//!   could take Western.
//! - **Classical Armenian (Grabar).** The 5th-century literary
//!   language preserved in the New Testament and the Bible translations
//!   uses a much richer morphology (7 cases with distinct sg/pl forms,
//!   an aorist/imperfect/perfect distinction, participles that inflect
//!   for case) and a different stopword profile. Classical is out of
//!   scope; a future `stringcheese-xcl` (ISO 639-3 for Classical
//!   Armenian) could take it.
//! - **Multi-word phrases.** Entries like `այն որ` (that which)
//!   would never match a single token — they are left out.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.

/// The Armenian (Eastern) stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in lowercase
/// Armenian script.
pub const STOPWORDS: &[&str] = &[
    // -------------------------------------------------------------
    // Personal pronouns (nominative).
    // -------------------------------------------------------------
    "ես",    // I
    "դու",   // you (sg)
    "նա",    // he / she
    "մենք",  // we
    "դուք",  // you (pl / formal)
    "նրանք", // they
    // -------------------------------------------------------------
    // Demonstratives — three-way deixis (proximal / medial / distal).
    // -------------------------------------------------------------
    "այս", // this (near speaker)
    "այդ", // that (near listener)
    "այն", // that (far)
    "սա",  // this-one
    "դա",  // that-one (near listener)
    "նա",  // that-one / he (already listed but harmless)
    // -------------------------------------------------------------
    // Interrogatives.
    // -------------------------------------------------------------
    "ով",     // who
    "ինչ",    // what
    "որ",     // which / that (relativizer)
    "որտեղ",  // where
    "երբ",    // when
    "ինչու",  // why
    "ինչպես", // how
    "որքան",  // how much / how many
    // -------------------------------------------------------------
    // Conjunctions.
    // -------------------------------------------------------------
    "և",        // and (ligature ech-yiwn, U+0587)
    "եւ",       // and (spelled out — same word, alternate spelling)
    "կամ",      // or
    "բայց",     // but
    "որովհետև", // because
    "եթե",      // if
    "որպեսզի",  // so that / in order that
    "թե",       // whether / that
    "թեև",      // although
    "երբ",      // when (already listed — some overlap by design)
    // -------------------------------------------------------------
    // Prepositions / postpositions.
    // -------------------------------------------------------------
    "մեջ",   // in
    "վրա",   // on
    "տակ",   // under
    "մոտ",   // near
    "հետ",   // with
    "առանց", // without
    "համար", // for / for the sake of
    "մասին", // about
    // -------------------------------------------------------------
    // Copula եմ — high-frequency forms.
    // -------------------------------------------------------------
    "եմ",  // am
    "ես",  // are (2sg) — already listed as pronoun; harmless duplicate
    "է",   // is
    "ենք", // are (1pl)
    "եք",  // are (2pl)
    "են",  // are (3pl)
    // -------------------------------------------------------------
    // Negator forms.
    // -------------------------------------------------------------
    "չէ",   // no / is-not
    "չեմ",  // am-not
    "չես",  // are-not (2sg)
    "չենք", // are-not (1pl)
    "չեք",  // are-not (2pl)
    "չեն",  // are-not (3pl)
    "ոչ",   // no / not
    // -------------------------------------------------------------
    // Affirmation.
    // -------------------------------------------------------------
    "այո", // yes
    "իսկ", // and / whereas / really
    // -------------------------------------------------------------
    // Particles + high-frequency adverbs.
    // -------------------------------------------------------------
    "շատ",      // very / many
    "քիչ",      // few / little
    "այսօր",    // today
    "երեկ",     // yesterday
    "վաղը",     // tomorrow
    "արդեն",    // already
    "նաև",      // also
    "միայն",    // only
    "դեռ",      // still / yet
    "նույնիսկ", // even
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 55". Allow 40..=120 so
        // future additions do not immediately flip the test.
        assert!(
            STOPWORDS.len() >= 40 && STOPWORDS.len() <= 120,
            "STOPWORDS.len() = {} outside the 40-120 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !c.is_uppercase(),
                    "stopword {w:?} contains an uppercase character {c:?}"
                );
            }
        }
    }

    #[test]
    fn no_entries_contain_ascii_whitespace() {
        for &w in STOPWORDS {
            assert!(
                !w.chars().any(char::is_whitespace),
                "stopword {w:?} contains whitespace — must be a single word"
            );
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["ես", "դու", "նա", "մենք", "դուք", "նրանք"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["և", "կամ", "բայց", "եթե", "որպեսզի"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn demonstratives_are_present() {
        for w in ["այս", "այդ", "այն"] {
            assert!(STOPWORDS.contains(&w), "demonstrative {w:?} is missing");
        }
    }

    #[test]
    fn copula_forms_are_present() {
        for w in ["եմ", "է", "ենք", "եք", "են"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn negator_forms_are_present() {
        for w in ["չէ", "չեմ", "չեն"] {
            assert!(STOPWORDS.contains(&w), "negator form {w:?} is missing");
        }
    }
}
