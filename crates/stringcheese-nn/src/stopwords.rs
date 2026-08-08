//! The Norwegian (Nynorsk) stopword list.
//!
//! Roughly 130 common Norwegian Nynorsk words drawn from the classic
//! Nynorsk function-word inventory: the ranked head of high-frequency
//! function words (pronouns, articles, prepositions, conjunctions,
//! particles) plus the full paradigms of the copula `vera` "to be", the
//! auxiliary `ha` "have", and the modals `skulle` / `ville` / `kunne`
//! / `måtte` / `burde`.
//!
//! # Bokmål-Nynorsk overlap
//!
//! Nynorsk and Bokmål share large swathes of function-word vocabulary
//! (most prepositions — `i` / `til` / `med` / `av` / `på` / `frå` —
//! and coordinating conjunctions — `og` / `eller` / `men` / `for` —
//! are identical between the two standards). Those shared forms are
//! included in this list. The Nynorsk-specific forms are the highest-
//! value additions:
//!
//! * **Pronouns.** `eg` (I) vs. Bokmål `jeg`; `ho` (she) vs. `hun`;
//!   `me` (we) vs. `vi`; `dei` (they) vs. `de`; `dykk` / `dykkar`
//!   (you-pl / your-pl) vs. Bokmål `dere` / `deres`.
//! * **Articles.** `ein` (a-masc/common) vs. Bokmål `en`; `ei`
//!   (a-fem) vs. Bokmål `en` / `ei`; `eit` (a-neut) vs. Bokmål `et`.
//! * **Negation.** `ikkje` vs. Bokmål `ikke`.
//! * **Interrogatives.** `kva` (what) vs. Bokmål `hva`; `kvifor`
//!   (why) vs. `hvorfor`; `korleis` (how) vs. `hvordan`; `kvar`
//!   (where) vs. `hvor`; `kven` (who) vs. `hvem`.
//! * **Adverbs.** `so` (so/then) vs. Bokmål `så`; `difor` (therefore)
//!   vs. `derfor`; `mykje` (much) vs. `mye`.
//! * **Verb forms.** `vera` (be-infinitive) vs. Bokmål `være`; `vore`
//!   (be-past-participle) vs. `vært`; `verta` / `vert` / `vart` /
//!   `vorte` (become-forms) vs. `bli` / `blir` / `ble` / `blitt`.
//!
//! # Accented characters
//!
//! Nynorsk uses the three additional letters `æ`, `ø`, `å` in everyday
//! vocabulary — including in function words like `på` "on", `frå`
//! "from", `vera` "to be", `å` (infinitive marker), `før` "before".
//! Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — the uppercase variants `Å`, `Æ`, `Ø` are **not**
//! automatically recognized by the default check; only the lowercase
//! spellings stored here match. Callers who need Unicode case-fold on
//! Nynorsk stopwords should wrap the lookup themselves (e.g., via
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

/// The Norwegian (Nynorsk) stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // ---------------------------------------------------------------
    // Conjunctions and coordinators (shared with Bokmål).
    // ---------------------------------------------------------------
    "og", "eller", "men", "at", "som", "fordi", "difor", "då", "medan", "når", "enn", "både",
    "anten",
    // ---------------------------------------------------------------
    // Personal pronouns — Nynorsk-flavoured where they differ.
    // ---------------------------------------------------------------
    "eg",     // I (Bokmål: jeg)
    "meg",    // me
    "du",     // you
    "deg",    // you (obj)
    "han",    // he
    "honom",  // him (Nynorsk-specific obj form)
    "ho",     // she (Bokmål: hun)
    "henne",  // her
    "hennar", // her (poss)
    "det",    // it / that
    "me",     // we (Bokmål: vi)
    "vi",     // we (also accepted)
    "oss",    // us
    "de",     // you-pl
    "dykk",   // you-pl (obj) — Nynorsk-specific
    "dykkar", // your-pl — Nynorsk-specific
    "dei",    // they (Bokmål: de)
    "deim",   // them (older Nynorsk obj)
    "deira",  // their — Nynorsk-specific
    "seg",    // reflexive
    // ---------------------------------------------------------------
    // Possessive pronouns.
    // ---------------------------------------------------------------
    "min", "mi", "mitt", "mine", "din", "di", "ditt", "dine", "hans", "sin", "si", "sitt", "sine",
    "vår", "vårt", "våre",
    // ---------------------------------------------------------------
    // Articles.
    // ---------------------------------------------------------------
    "ein", // masc/common a (Bokmål: en)
    "ei",  // fem a (Bokmål: en / ei)
    "eit", // neut a (Bokmål: et)
    "den",
    // ---------------------------------------------------------------
    // Demonstratives and quantifiers.
    // ---------------------------------------------------------------
    "denne", "dette", "desse", // Nynorsk plural (Bokmål: disse)
    "same", "slik", "slike", "kvar",  // each / every (also interrog. "where")
    "kvart", // each (neut)
    "alle", "all", "alt", "nokon", // some / any (Bokmål: noen)
    "noka", "nokor", "noko", // something (Bokmål: noe)
    "nokre", "ingen", "ingi", "inkje", // nothing (Bokmål: intet)
    "mange", "mykje", // much (Bokmål: mye)
    "lite", "fleire",
    // ---------------------------------------------------------------
    // Interrogatives — Nynorsk uses kv- (Bokmål uses hv-).
    // ---------------------------------------------------------------
    "kva",     // what (Bokmål: hva)
    "kven",    // who (Bokmål: hvem)
    "kvifor",  // why (Bokmål: hvorfor)
    "korleis", // how (Bokmål: hvordan)
    // ---------------------------------------------------------------
    // Negation and affirmation.
    // ---------------------------------------------------------------
    "ikkje", // not (Bokmål: ikke)
    "ja", "nei",
    // ---------------------------------------------------------------
    // Prepositions (largely shared with Bokmål).
    // ---------------------------------------------------------------
    "i", "på", "til", "med", "av", "for", "frå", // from (Bokmål: fra)
    "om", "ved", "mot", "etter", "før", "over", "under", "mellom", "gjennom",
    "utan", // without (Bokmål: uten)
    "hjå",  // by / at (Nynorsk-specific)
    // ---------------------------------------------------------------
    // Adverbs of place, time, degree.
    // ---------------------------------------------------------------
    "her", "der", "no", // now (Bokmål: nå)
    "sidan", "sia", "so", // so / then (Bokmål: så)
    "også", "òg", "opp", "ned", "ut", "inn", "inni", "attende", "sjølv",
    "berre", // only (Bokmål: bare)
    "elles",
    // ---------------------------------------------------------------
    // Infinitive marker.
    // ---------------------------------------------------------------
    "å",
    // ---------------------------------------------------------------
    // Copula `vera` and its full paradigm.
    // ---------------------------------------------------------------
    "vera", // be-inf (Bokmål: være)
    "er", "var", "vore", // been (Bokmål: vært)
    // ---------------------------------------------------------------
    // Auxiliary `ha` paradigm.
    // ---------------------------------------------------------------
    "ha", "har", "hadde", "hatt",
    // ---------------------------------------------------------------
    // Modal paradigms.
    // ---------------------------------------------------------------
    "kan", "kunne", "skal", "skulle", "vil", "ville", "må", "måtte", "bør", "burde",
    // ---------------------------------------------------------------
    // Become-verb `verta` — Nynorsk-flavoured.
    // ---------------------------------------------------------------
    "verta", // become-inf (Bokmål: bli)
    "vert",  // become-pres
    "vart",  // became
    "vorte", // become-pp (Bokmål: blitt)
    "blir",  // Bokmål-style forms sometimes appear in mixed corpora
    "bli", "blei", // became (also Bokmål "ble")
    "blitt",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~130" — assert we're in the
        // ballpark. Range is loose to accommodate paradigm expansion.
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the advertised ~130 range",
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
        // O(n^2) is fine for a static list of ~130.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn nynorsk_specific_pronouns_are_present() {
        // The pronouns that distinguish Nynorsk from Bokmål.
        for w in ["eg", "ho", "me", "dei", "dykk", "dykkar", "deira", "honom"] {
            assert!(STOPWORDS.contains(&w), "Nynorsk pronoun {w:?} is missing");
        }
    }

    #[test]
    fn nynorsk_specific_interrogatives_are_present() {
        for w in ["kva", "kven", "kvifor", "korleis", "kvar"] {
            assert!(
                STOPWORDS.contains(&w),
                "Nynorsk interrogative {w:?} is missing"
            );
        }
    }

    #[test]
    fn nynorsk_specific_negation_is_present() {
        assert!(STOPWORDS.contains(&"ikkje"));
    }

    #[test]
    fn nynorsk_specific_articles_are_present() {
        for w in ["ein", "ei", "eit"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "i", "til", "på", "med", "av", "for", "frå", "ved", "over", "mellom", "mot", "etter",
            "før", "utan",
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
        // The Nynorsk copula VERA, auxiliary HA, and modals paradigm.
        for w in [
            "er", "var", "vera", "vore", "ha", "har", "hadde", "skal", "skulle", "vil", "ville",
            "kan", "kunne", "verta", "vert", "vart", "vorte",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }

    #[test]
    fn norwegian_specific_letters_are_present() {
        // Sanity-check that entries with `å` / `æ` / `ø` are stored as
        // the actual Unicode scalars, not folded to ASCII.
        for w in ["på", "frå", "å", "før", "også"] {
            assert!(
                STOPWORDS.contains(&w),
                "Norwegian-specific-letter stopword {w:?} is missing"
            );
        }
    }
}
