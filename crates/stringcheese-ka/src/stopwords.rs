//! The Georgian stopword list.
//!
//! Roughly 65 entries covering the high-frequency function words of
//! Modern Georgian (ქართული, `kartuli`): personal pronouns, the
//! high-frequency forms of the copula `არის` "is", demonstratives,
//! interrogatives, conjunctions, negators, sentence particles,
//! quantifiers, and a handful of adverbs.
//!
//! # Georgian is unicase
//!
//! Modern Mkhedruli script has **no case distinction** (Mtavruli
//! U+1C90..=U+1CBF is a modern capitalized style, added to Unicode 11
//! in 2018, but not universally used in typography). All entries are
//! stored in Mkhedruli lowercase; the pack's `is_stopword` fold
//! converts any Mtavruli input to Mkhedruli before comparison, so
//! callers can supply either form.
//!
//! # Non-goals
//!
//! - **Old Georgian (Asomtavruli / Nuskhuri).** Old Georgian used the
//!   Asomtavruli (U+10A0..=U+10CF) and Nuskhuri (U+2D00..=U+2D2F)
//!   scripts. The stopword list targets Modern Georgian; ecclesiastical
//!   / historical texts in the older scripts pass through the pack
//!   unchanged (they still fold correctly at the character level, but
//!   the stopword vocabulary does not attempt to cover the archaic
//!   inflectional forms of Old Georgian).
//! - **Colloquial short forms.** Colloquial Georgian shortens some
//!   frequent copula and pronoun forms (`ვარ` → `ვა`, `არა` → `არ`).
//!   Only the standard-orthography forms are carried here.
//! - **Domain-specific stopwords.** IR practice for legal / medical /
//!   scientific corpora extends the general list; downstream systems
//!   should carry their own.

/// The Georgian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is stored in Mkhedruli
/// lowercase (Modern Georgian's default script, unicase in normal use).
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns (nominative + oblique).
    // -----------------------------------------------------------------
    "მე",     // I
    "შენ",    // you (singular)
    "ის",     // he / she / it — also demonstrative "that"
    "ჩვენ",   // we
    "თქვენ",  // you (plural / formal)
    "ისინი",  // they
    "მას",    // him/her/it (dative)
    "იმას",   // him/her/it (accusative-demonstrative)
    "ამას",   // this (accusative-demonstrative)
    "მან",    // he/she/it (ergative)
    "ჩემი",   // my
    "შენი",   // your (sg)
    "მისი",   // his / her / its
    "ჩვენი",  // our
    "თქვენი", // your (pl / formal)
    "მათი",   // their
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "ეს", // this
    "იმ", // that (declined)
    "ამ", // this (declined)
    // -----------------------------------------------------------------
    // Copula `არის` "is" — high-frequency forms.
    // -----------------------------------------------------------------
    "ვარ",    // I am
    "ხარ",    // you are (sg)
    "არის",   // he/she/it is
    "ვართ",   // we are
    "ხართ",   // you are (pl)
    "არიან",  // they are
    "იყო",    // was
    "იყვნენ", // were
    "იქნება", // will be
    // -----------------------------------------------------------------
    // Negators / affirmatives.
    // -----------------------------------------------------------------
    "არ",   // no / not
    "არა",  // no
    "ვერ",  // cannot
    "ნუ",   // negative imperative
    "დიახ", // yes
    "კი",   // yes / affirmative particle
    "ხო",   // yes (colloquial)
    // -----------------------------------------------------------------
    // Conjunctions.
    // -----------------------------------------------------------------
    "და",     // and
    "ან",     // or
    "მაგრამ", // but
    "ხოლო",   // however / whereas
    "თუ",     // if
    "რომ",    // that (conjunction)
    "როცა",   // when
    "სანამ",  // while / until
    "თუმცა",  // although
    "რადგან", // because
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "რა",    // what
    "ვინ",   // who
    "ვინც",  // who (relative)
    "სად",   // where
    "როგორ", // how
    "რატომ", // why
    "როდის", // when (interrog)
    // -----------------------------------------------------------------
    // Quantifiers / determiners.
    // -----------------------------------------------------------------
    "ერთი",   // one
    "ორი",    // two
    "სამი",   // three
    "ბევრი",  // many
    "ცოტა",   // few
    "ყველა",  // all
    "ყოველი", // every
    // -----------------------------------------------------------------
    // High-frequency adverbs and particles.
    // -----------------------------------------------------------------
    "აქ",     // here
    "იქ",     // there
    "ახლა",   // now
    "მერე",   // later / then
    "უკვე",   // already
    "ჯერ",    // yet / still
    "ისე",    // so / thus
    "ასე",    // like this
    "როგორც", // as
    "ვიდრე",  // than
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // Doc-comment above targets "roughly 65". The task spec targets
        // ~50; the test allows a generous 40-120 to accommodate the
        // actual ~65 count plus future additions.
        assert!(
            STOPWORDS.len() >= 40 && STOPWORDS.len() <= 120,
            "STOPWORDS.len() = {} outside the 40-120 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase_mkhedruli() {
        // Modern Georgian is unicase — Mkhedruli. Mtavruli
        // (U+1C90..=U+1CBF) is a separate capitalized-style block.
        // Entries should live entirely in Mkhedruli or ASCII.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !('\u{1C90}'..='\u{1CBF}').contains(&c),
                    "stopword {w:?} contains Mtavruli character {c:?} — should be stored in Mkhedruli"
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
    fn every_entry_has_at_least_one_georgian_scalar() {
        // A stopword entry with no Georgian content would be an obvious
        // typo — the list should be entirely Georgian.
        for &w in STOPWORDS {
            assert!(
                w.chars().any(|c| ('\u{10D0}'..='\u{10FF}').contains(&c)),
                "stopword {w:?} contains no Mkhedruli scalar"
            );
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["მე", "შენ", "ის", "ჩვენ", "თქვენ", "ისინი"]
        {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["და", "ან", "მაგრამ", "თუ", "რომ"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["ვარ", "ხარ", "არის", "ვართ", "ხართ", "არიან"]
        {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }
}
