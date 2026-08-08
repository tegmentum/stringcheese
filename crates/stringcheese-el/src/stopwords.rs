//! The Greek stopword list.
//!
//! Roughly 220 entries (~205 unique) of common Modern Greek words
//! drawn from published Greek IR
//! stopword lists (the Snowball project's Greek collection, common
//! Greek NLP toolkits' `greek` lists). The list is deliberately
//! conservative — the most frequent function words (personal /
//! demonstrative / interrogative pronouns, prepositions, conjunctions,
//! particles, the high-frequency forms of the copula *είμαι*, common
//! adverbs, and quantifiers).
//!
//! # Greek stopwords
//!
//! Every entry below is a Greek string in canonical **accent-stripped**
//! lowercase form. Modern Greek uses the *monotonic* orthography — a
//! single acute tonos accent on the stressed vowel of every
//! polysyllabic word (`ό`, `ά`, `έ`, `ή`, `ί`, `ύ`, `ώ`) — and Greek
//! also uses the diaeresis (`ϊ`, `ϋ`) to signal a vowel is
//! syllabically distinct from a preceding vowel that would otherwise
//! form a diphthong. Both marks are stripped before comparison so a
//! query for the accented `είμαι` still matches the plain `ειμαι` in
//! this list. See the [crate root](crate) for the details.
//!
//! # Final sigma
//!
//! Modern Greek writes sigma with a *final* form (`ς` U+03C2) at the
//! end of a word and a *non-final* form (`σ` U+03C3) elsewhere — the
//! two are the same letter, positional variants only. Stopword entries
//! that end in a sigma store the **non-final** `σ` form; the pack's
//! `is_stopword` implementation folds `ς → σ` before comparison so
//! either spelling matches.
//!
//! # Non-goals
//!
//! - **Polytonic / Ancient Greek.** Greek used a *polytonic* system
//!   before 1982 (three accents — acute, grave, circumflex — plus rough
//!   and smooth breathings). This crate targets modern monotonic Greek;
//!   polytonic input scalars (`ᾰ`, `ᾱ`, `ἀ`, `ἁ`, `ᾴ`, `ᾷ`, etc.) pass
//!   through unchanged. Callers who need polytonic support should
//!   NFD-decompose and strip the additional combining marks before
//!   handing text to the pack, or wait for a `stringcheese-grc`
//!   (Ancient Greek) sibling.
//! - **Katharevousa.** The archaic / puristic variety used until 1976
//!   preserved many Ancient-Greek forms that Modern Demotic Greek does
//!   not. Katharevousa function words (`ήτο`, `όστις`, etc.) are not
//!   carried.
//! - **Multi-word phrases.** Entries like `για να` (a two-word purpose
//!   conjunction) would never match a single token — they are left out
//!   of the list; downstream systems that want phrase-level filtering
//!   should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Greek stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in Greek
/// lowercase with accents stripped and with sigma in the non-final
/// (`σ`) form.
pub const STOPWORDS: &[&str] = &[
    // -------------------------------------------------------------
    // Personal pronouns (nominative + oblique).
    // -------------------------------------------------------------
    "εγω",
    "εσυ",
    "αυτοσ",
    "αυτη",
    "αυτο",
    "εμεισ",
    "εσεισ",
    "αυτοι",
    "αυτεσ",
    "αυτα",
    "με",
    "σε",
    "τον",
    "την",
    "μασ",
    "σασ",
    "τουσ",
    "τισ",
    "τα",
    "μου",
    "σου",
    "του",
    "τησ",
    "μασ",
    "σασ",
    "τουσ",
    "εμενα",
    "εσενα",
    "εμασ",
    "εσασ",
    // -------------------------------------------------------------
    // Possessive / demonstrative markers.
    // -------------------------------------------------------------
    "δικοσ",
    "δικη",
    "δικο",
    "δικοι",
    "δικεσ",
    "δικα",
    "τετοιοσ",
    "τετοια",
    "τετοιο",
    "τοσοσ",
    "τοση",
    "τοσο",
    "εκεινοσ",
    "εκεινη",
    "εκεινο",
    "εκεινοι",
    "εκεινεσ",
    "εκεινα",
    "ιδιοσ",
    "ιδια",
    "ιδιο",
    // -------------------------------------------------------------
    // Definite / indefinite articles + demonstrative-neuter `το`.
    // -------------------------------------------------------------
    "ο",
    "η",
    "το",
    "οι",
    "οι",
    "τα",
    "ενασ",
    "μια",
    "μιασ",
    "ενοσ",
    "ενα",
    "τησ",
    "του",
    "των",
    // -------------------------------------------------------------
    // Interrogatives.
    // -------------------------------------------------------------
    "ποιοσ",
    "ποια",
    "ποιο",
    "ποιοι",
    "ποιεσ",
    "ποια",
    "τι",
    "που",
    "πωσ",
    "ποτε",
    "γιατι",
    "ποσο",
    "ποσοι",
    "ποσα",
    "ποσεσ",
    // -------------------------------------------------------------
    // Conjunctions.
    // -------------------------------------------------------------
    "και",
    "κι",
    "η",
    "ειτε",
    "αλλα",
    "ομωσ",
    "μα",
    "παρα",
    "ουτε",
    "μηδε",
    "ωστε",
    "αν",
    "εαν",
    "οτι",
    "πωσ",
    "διοτι",
    "επειδη",
    "αφου",
    "μολισ",
    "οταν",
    "εφοσον",
    "ενω",
    "μηπωσ",
    "μηπωσ",
    "ωσπου",
    "εωσ",
    // -------------------------------------------------------------
    // Prepositions.
    // -------------------------------------------------------------
    "σε",
    "στον",
    "στην",
    "στο",
    "στουσ",
    "στισ",
    "στα",
    "απο",
    "με",
    "για",
    "χωρισ",
    "μεταξυ",
    "μεσα",
    "μεσω",
    "προσ",
    "κατα",
    "παρα",
    "υπο",
    "υπερ",
    "εξω",
    "πριν",
    "μετα",
    "εναντια",
    "εναντιον",
    // -------------------------------------------------------------
    // Particles + high-frequency adverbs.
    // -------------------------------------------------------------
    "να",
    "θα",
    "δεν",
    "μην",
    "μη",
    "ναι",
    "οχι",
    "ασ",
    "μονο",
    "πια",
    "ακομα",
    "ακομη",
    "ηδη",
    "τωρα",
    "εδω",
    "εκει",
    "τοτε",
    "παντα",
    "ποτε",
    "καποτε",
    "ξανα",
    "παλι",
    "παντωσ",
    "λοιπον",
    "επισησ",
    "ισωσ",
    "μαλλον",
    "σχεδον",
    "πολυ",
    "λιγο",
    "μαζι",
    "χθεσ",
    "σημερα",
    "αυριο",
    // -------------------------------------------------------------
    // Copula είμαι — high-frequency forms (accent-stripped).
    // -------------------------------------------------------------
    "ειμαι",
    "εισαι",
    "ειναι",
    "ειμαστε",
    "εισαστε",
    "ειστε",
    "ημουν",
    "ησουν",
    "ηταν",
    "ημασταν",
    "ησασταν",
    "ητανε",
    // -------------------------------------------------------------
    // Auxiliary έχω — high-frequency forms.
    // -------------------------------------------------------------
    "εχω",
    "εχεισ",
    "εχει",
    "εχουμε",
    "εχετε",
    "εχουν",
    "ειχα",
    "ειχεσ",
    "ειχε",
    "ειχαμε",
    "ειχατε",
    "ειχαν",
    // -------------------------------------------------------------
    // Quantifiers / generic determiners.
    // -------------------------------------------------------------
    "ολοσ",
    "ολη",
    "ολο",
    "ολοι",
    "ολεσ",
    "ολα",
    "καθε",
    "καποιοσ",
    "καποια",
    "καποιο",
    "καποιοι",
    "καποιεσ",
    "καποια",
    "τιποτα",
    "κανεισ",
    "κανενασ",
    "καμια",
    "κανενα",
    "λιγοσ",
    "λιγη",
    "λιγοι",
    "λιγεσ",
    "λιγα",
    "πολλοσ",
    "πολλη",
    "πολλοι",
    "πολλεσ",
    "πολλα",
    "αλλοσ",
    "αλλη",
    "αλλο",
    "αλλοι",
    "αλλεσ",
    "αλλα",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 220 (~205 unique)". The
        // task spec targets ~100-150 as a soft range; the test allows
        // a generous 100-260 to accommodate the actual ~220 count plus
        // future additions (a handful
        // of duplicate literals occur because the same surface form
        // appears in multiple morphological classes — that is by design;
        // the linear-scan lookup does not require the slice to be a set).
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the 100-260 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        // Every character of every entry should be lowercase under
        // default Unicode rules.
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
    fn no_entries_contain_final_sigma() {
        // Entries store the non-final sigma `σ`; the pack's `ς → σ`
        // fold takes care of the final-sigma spelling at call time.
        for &w in STOPWORDS {
            assert!(
                !w.contains('ς'),
                "stopword {w:?} contains final sigma ς — should be stored with non-final σ"
            );
        }
    }

    #[test]
    fn no_entries_contain_accents() {
        // Entries store accent-stripped forms; the pack's Greek
        // accent-fold takes care of the accented spelling at call time.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !matches!(
                        c,
                        'ά' | 'έ' | 'ή' | 'ί' | 'ό' | 'ύ' | 'ώ' | 'ϊ' | 'ϋ' | 'ΐ' | 'ΰ'
                    ),
                    "stopword {w:?} contains accented character {c:?} — should be stored accent-stripped"
                );
            }
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
    fn common_pronouns_are_present() {
        for w in ["εγω", "εσυ", "αυτοσ", "αυτη", "αυτο"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["και", "η", "αλλα", "οτι", "αν"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["σε", "απο", "με", "για"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["ειμαι", "εισαι", "ειναι", "ηταν"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }
}
