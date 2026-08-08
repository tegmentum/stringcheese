//! The Danish stopword list.
//!
//! Roughly 120 common Danish words drawn from the Snowball project's
//! `danish/stop.txt` distribution — the ranked head of Danish function
//! words plus the full paradigms of the copula `være` "to be", the
//! auxiliary `have` "have", and the modals `skulle` / `ville` / `kunne`
//! / `måtte` / `burde`.
//!
//! # Accented characters
//!
//! Danish uses the three additional letters `æ`, `ø`, `å` in everyday
//! vocabulary — including in function words like `på` "on", `så` "then",
//! `være` "to be", `også` "also", `før` "before". Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — the uppercase variants `Å`, `Æ`, `Ø` are **not**
//! automatically recognized by the default check; only the lowercase
//! spellings stored here match. Callers who need Unicode case-fold on
//! Danish stopwords should wrap the lookup themselves (e.g., via
//! `str::to_lowercase` from `std`).
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so `"og"`,
//!   `"Og"`, and `"OG"` are all recognized. The default trait-level
//!   check does not fold non-ASCII accents.

/// The Danish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the commonest Danish function words per the
    // Snowball project's stop.txt.
    "og", "i", "jeg", "det", "at", "en", "et", "den", "til", "er", "som", "på", "de", "med", "han",
    "af", "for", "ikke", "der", "var", "mig", "sig", "men", "har", "om", "vi", "min", "havde",
    "ham", "hun", "nu", "over", "da", "fra", "du", "ud", "sin", "dem", "os", "op", "man", "hans",
    "hvor", "eller", "hvad", "skal", "selv", "her", "alle", "vil", "blev", "kunne", "ind", "når",
    "være", "dog", "noget", "ville", "jo", "deres", "efter", "ned", "skulle", "denne", "end",
    "dette", "mit", "også", "under", "have", "dig", "anden", "hende", "mine", "alt", "meget",
    "sit", "sine", "vor", "mod", "disse", "hvis", "din", "nogle", "hos", "blive", "mange", "ad",
    "bliver", "hendes", "været", "thi", "jer", "sådan",
    // Additional forms of the auxiliaries VÆRE "to be" and HAVE
    // "have" not covered above, plus the full modal paradigms
    // (KAN/KUNNE/VIL/VILLE/SKAL/SKULLE/MÅ/MÅTTE/BØR/BURDE).
    "vær", "haft", "kan", "kunnet", "villet", "skullet", "må", "måtte", "måttet", "bør", "burde",
    // Adverbial / connective extras common in stop lists.
    "aldrig", "altid", "bare", "hvorfor", "hvornår", "således", "derfor", "endnu", "kun", "kanske",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~120" — assert we're in the
        // ballpark. Range is loose to accommodate the modal-verb
        // paradigm expansion (være / have / skulle / ville / kunne /
        // måtte / burde).
        assert!(
            STOPWORDS.len() >= 90 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the advertised ~120 range",
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
        // O(n^2) is fine for a static list of ~120.
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
            "i", "til", "på", "med", "af", "for", "fra", "over", "under", "mod", "efter",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["og", "eller", "men", "at", "som", "når"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // The Snowball `danish/stop.txt` covers VÆRE / HAVE / SKULLE /
        // VILLE / KUNNE / MÅTTE / BURDE paradigms. This asserts a
        // representative subset that lets a callsite trust the pack for
        // the common auxiliary conjugations.
        for w in [
            "er", "var", "være", "været", "have", "har", "havde", "skal", "skulle", "vil", "ville",
            "kan", "kunne", "må", "måtte", "bør", "burde",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }

    #[test]
    fn danish_specific_letters_are_present() {
        // Sanity-check that entries with `å` / `æ` / `ø` are stored as
        // the actual Unicode scalars, not folded to ASCII.
        for w in ["på", "være", "også", "må"] {
            assert!(
                STOPWORDS.contains(&w),
                "Danish-specific-letter stopword {w:?} is missing"
            );
        }
    }
}
