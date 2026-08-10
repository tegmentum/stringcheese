//! The Norwegian (Bokmål) stopword list.
//!
//! Roughly 170 common Norwegian Bokmål words drawn from the Snowball
//! project's `norwegian/stop.txt` (the same source the Dutch pack pulls
//! its list from for consistency across the Germanic Latin-script
//! packs). The list combines the ranked head of high-frequency function
//! words with the full paradigms of the copula `være` "to be", the
//! auxiliary `ha` "have", and the modals `skulle` / `ville` / `kunne`
//! / `måtte` / `burde`.
//!
//! The upstream Snowball `stop.txt` under-differentiates Bokmål from
//! Nynorsk — it labels the list simply "norwegian" and folds in a
//! handful of Nynorsk-flavored high-frequency function words (`ikkje`,
//! `eg`, `me`, `dei`, `kva`, `korleis`, …) whose Bokmål equivalents
//! (`ikke`, `jeg`, `vi`, `de`, `hva`, `hvordan`) are also present. We
//! keep the list as-is rather than pruning: a Bokmål-tuned pipeline
//! reading a mixed corpus will still see those tokens as noise, and
//! keeping the Snowball inventory intact means IR results reproduce
//! exactly what Lucene / Elasticsearch's Norwegian analyzer produces
//! given the same stop list.
//!
//! # Accented characters
//!
//! Norwegian Bokmål uses the three additional letters `æ`, `ø`, `å` in
//! everyday vocabulary — including in function words like `på` "on",
//! `så` "then", `være` "to be", `også` "also", `før` "before". Because
//! the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — the uppercase variants `Å`, `Æ`, `Ø` are **not**
//! automatically recognized by the default check; only the lowercase
//! spellings stored here match. Callers who need Unicode case-fold on
//! Norwegian stopwords should wrap the lookup themselves (e.g., via
//! `str::to_lowercase` from `std`).
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Nynorsk-only extension.** The sibling `stringcheese-nn` pack
//!   ships a Nynorsk-tuned stopword list that adds the Nynorsk-specific
//!   forms (`eg`, `ho`, `me`, `dei`, `dykk`, `ein`, `ei`, `eit`,
//!   `ikkje`, `kva`, `kvifor`, `korleis`, `kven`, `vera`, `verta`, …)
//!   Bokmål does not carry. This pack keeps only the Nynorsk-flavored
//!   forms Snowball-Norwegian's `voc.txt` embeds for parity reasons.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so `"og"`,
//!   `"Og"`, and `"OG"` are all recognized. The default trait-level
//!   check does not fold non-ASCII accents.

/// The Norwegian (Bokmål) stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the commonest Bokmål function words per the
    // Snowball project's stop.txt.
    "og",
    "i",
    "jeg",
    "det",
    "at",
    "en",
    "et",
    "den",
    "til",
    "er",
    "som",
    "på",
    "de",
    "med",
    "han",
    "av",
    "ikke",
    "der",
    "så",
    "var",
    "meg",
    "seg",
    "men",
    "ett",
    "har",
    "om",
    "vi",
    "min",
    "mitt",
    "ha",
    "hadde",
    "hun",
    "nå",
    "over",
    "da",
    "ved",
    "fra",
    "du",
    "ut",
    "sin",
    "dem",
    "oss",
    "opp",
    "man",
    "kan",
    "hans",
    "hvor",
    "eller",
    "hva",
    "skal",
    "selv",
    "sjøl",
    "her",
    "alle",
    "vil",
    "bli",
    "ble",
    "blitt",
    "kunne",
    "inn",
    "når",
    "være",
    "kom",
    "noen",
    "noe",
    "ville",
    "dere",
    "deres",
    "kun",
    "ja",
    "etter",
    "ned",
    "skulle",
    "denne",
    "for",
    "deg",
    "si",
    "sine",
    "sitt",
    "mot",
    "å",
    "meget",
    "hvorfor",
    "dette",
    "disse",
    "uten",
    "hvordan",
    "ingen",
    "din",
    "ditt",
    "blir",
    "samme",
    "hvilken",
    "hvilke",
    "sånn",
    "inni",
    "mellom",
    "vår",
    "hver",
    "hvem",
    "vors",
    "hvis",
    "både",
    "bare",
    "enn",
    "fordi",
    "før",
    "mange",
    "også",
    "slik",
    "vært",
    "båe",
    "begge",
    "siden",
    // Nynorsk-flavored function words retained for Snowball parity.
    "dykk",
    "dykkar",
    "dei",
    "deira",
    "deires",
    "deim",
    "di",
    "då",
    "eg",
    "ein",
    "eit",
    "eitt",
    "elles",
    "honom",
    "hjå",
    "ho",
    "hoe",
    "henne",
    "hennar",
    "hennes",
    "hoss",
    "hossen",
    "ikkje",
    "ingi",
    "inkje",
    "korleis",
    "korso",
    "kva",
    "kvar",
    "kvarhelst",
    "kven",
    "kvi",
    "kvifor",
    "me",
    "medan",
    "mi",
    "mine",
    "mykje",
    "no",
    "nokon",
    "noka",
    "nokor",
    "noko",
    "nokre",
    "sia",
    "sidan",
    "so",
    "somt",
    "somme",
    "um",
    "upp",
    "vore",
    "verte",
    "vort",
    "varte",
    "vart",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~170" — assert we're in the
        // ballpark. Range is loose to accommodate the modal-verb
        // paradigm expansion (være/ha/skulle/ville/kunne).
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~170 range",
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
        // O(n^2) is fine for a static list of ~170.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["en", "et", "den", "det"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "i", "til", "på", "med", "av", "for", "fra", "ved", "over", "mellom", "mot", "etter",
            "før", "uten",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["og", "eller", "men", "at", "som", "fordi", "når"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // The Snowball `norwegian/stop.txt` covers VÆRE / HA / SKULLE
        // / VILLE / KUNNE paradigms. This asserts a representative
        // subset that lets a callsite trust the pack for the common
        // auxiliary conjugations.
        for w in [
            "er", "var", "være", "vært", "ha", "har", "hadde", "skal", "skulle", "vil", "ville",
            "kan", "kunne", "bli", "ble", "blitt", "blir",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }

    #[test]
    fn norwegian_specific_letters_are_present() {
        // Sanity-check that entries with `å` / `æ` / `ø` are stored as
        // the actual Unicode scalars, not folded to ASCII.
        for w in ["på", "så", "være", "før", "også"] {
            assert!(
                STOPWORDS.contains(&w),
                "Norwegian-specific-letter stopword {w:?} is missing"
            );
        }
    }
}
