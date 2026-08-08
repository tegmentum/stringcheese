//! The Swedish stopword list.
//!
//! Roughly 140 common Swedish words drawn from the Snowball project's
//! `swedish/stop.txt` distribution. The list combines the ranked head
//! of high-frequency function words with the full paradigms of the
//! auxiliary verbs `vara` (to be), `ha` (to have), `bli` (to become),
//! `kunna` (can), `skola` (shall / will), `vilja` (want), `måste`
//! (must), and `göra` (to do).
//!
//! # Accented characters
//!
//! Swedish carries three letters beyond the base Latin 26: `å`, `ä`,
//! `ö`. Every function word that contains one of these letters (`är`,
//! `så`, `där`, `här`, `över`, …) is stored with the accent in place;
//! the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — so accented forms only match when spelled exactly as
//! stored here (case-folded on the ASCII subset). Callers who need
//! Unicode case-folded lookups can override
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! or lower-case their input before calling.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Regional variants.** No Finland-Swedish (fi-SV) variants — the
//!   list targets standard Sverigesvenska ("Sweden Swedish").
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so
//!   `"och"`, `"Och"`, and `"OCH"` are all recognized. The default
//!   trait-level check does not fold uppercase `Å`, `Ä`, `Ö` to their
//!   lowercase forms.

/// The Swedish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the commonest Swedish function words per the
    // Snowball project's stop.txt.
    "och", "det", "att", "i", "en", "jag", "hon", "som", "han", "på", "den", "med", "var", "sig",
    "för", "så", "till", "är", "men", "ett", "om", "hade", "de", "av", "icke", "mig", "du",
    "henne", "då", "sin", "nu", "har", "inte", "hans", "honom", "skulle", "hennes", "där", "min",
    "man", "ej", "vid", "kunde", "något", "från", "ut", "när", "efter", "upp", "vi", "dem", "vara",
    "vad", "över", "än", "dig", "kan", "sina", "här", "ha", "mot", "alla", "under", "någon",
    "eller", "allt", "mycket", "sedan", "ju", "denna", "själv", "detta", "åt", "utan", "varit",
    "hur", "ingen", "mitt", "ni", "bli", "blev", "oss", "din", "dessa", "några", "deras", "blir",
    "mina", "samma", "vilken", "er", "sådan", "vår", "blivit", "dess", "inom", "mellan", "sådant",
    "varför", "varje", "vilka", "ditt", "vem", "vilket", "sitta", "sådana", "vart", "dina", "vars",
    "vårt", "våra", "ert", "era", "vilkas",
    // Forms of HA (to have) not already present above.
    "haft", "hava", // Forms of VARA (to be) not already present above.
    "vore", "varande", // Forms of BLI (to become) not already present above.
    "bliva",   // Forms of KUNNA (can).
    "kunna", "kunnat", // Forms of SKOLA (shall / will).
    "skola", "skall", "ska", // Forms of VILJA (want).
    "vilja", "vill", "ville", "velat", // Forms of MÅSTE (must) — invariant.
    "måste", // Forms of GÖRA (to do).
    "göra", "gör", "gjorde", "gjort", // Adverbial and connective extras.
    "ännu", "aldrig", "alltid", "också", "bara", "kanske", "redan", "således", "därför", "helt",
    "endast", "hela",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~140" — assert we're in the
        // ballpark. Range is loose to accommodate the auxiliary-verb
        // paradigm expansion (vara / ha / bli / kunna / skola / vilja
        // / måste / göra).
        assert!(
            STOPWORDS.len() >= 120 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the advertised ~140 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !c.is_uppercase(),
                    "stopword {w:?} contains an uppercase character"
                );
            }
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~140.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["den", "det", "en", "ett", "de"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "i", "på", "till", "för", "av", "med", "från", "vid", "över", "under", "mot", "utan",
            "efter", "om",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["och", "eller", "men", "att", "som", "när", "då"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // The Snowball `swedish/stop.txt` covers VARA / HA / BLI /
        // KUNNA / SKOLA / VILJA paradigms. This asserts a representative
        // subset that lets a callsite trust the pack for the common
        // auxiliary conjugations.
        for w in [
            "är", "var", "vara", "varit", "har", "hade", "haft", "bli", "blir", "blev", "blivit",
            "kan", "kunde", "skulle", "vill", "måste",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }

    #[test]
    fn swedish_specific_letters_are_present() {
        // The Swedish letter set carries `å`, `ä`, `ö`. Assert that at
        // least one representative stopword carrying each is on the list.
        assert!(STOPWORDS.contains(&"på"), "å-carrying stopword missing");
        assert!(STOPWORDS.contains(&"är"), "ä-carrying stopword missing");
        assert!(STOPWORDS.contains(&"över"), "ö-carrying stopword missing");
    }
}
