//! The French stopword list.
//!
//! Roughly 200 common French words drawn from the intersection of
//! NLTK's `french` list, the Snowball project's `french/stop.txt`
//! (Martin Porter's own French stoplist), and Lucene's
//! [French analyzer][luc]. The list is deliberately conservative — the
//! most frequent function words (articles, prepositions, pronouns,
//! auxiliaries) plus the elision forms every French tokenizer produces
//! (`l'`, `d'`, `qu'`, `j'`, `m'`, `t'`, `s'`, `n'`, `c'`). No
//! domain-specific jargon; no archaic forms.
//!
//! [luc]: https://lucene.apache.org/core/9_0_0/analysis/common/org/apache/lucene/analysis/fr/FrenchAnalyzer.html
//!
//! # Elision forms
//!
//! The list includes both the stripped clitic (`l`, `d`, `qu`, …) and
//! the apostrophe-suffixed form (`l'`, `d'`, `qu'`, …) so callers whose
//! tokenizer preserves the apostrophe *and* callers whose tokenizer
//! strips it both see the tokens they emit as stopwords. The
//! [`stringcheese_fr::FrenchTokenizer`](crate::FrenchTokenizer) ships
//! the apostrophe-preserving variant (`"l'homme"` → `["l'", "homme"]`);
//! see its module docs for the rationale.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with
//!   [`str::eq_ignore_ascii_case`], so `"le"`, `"Le"`, and `"LE"` are
//!   all recognized. The default trait-level check does not fold
//!   non-ASCII accents.

/// The French stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Articles.
    "le",
    "la",
    "les",
    "un",
    "une",
    "des",
    "du",
    "de",
    "au",
    "aux",
    // Elision clitics: both the apostrophe-suffixed form the shipped
    // tokenizer emits and the bare-letter form a caller who strips the
    // apostrophe would see. Present here so both conventions recognize
    // them as stopwords.
    "l",
    "l'",
    "d",
    "d'",
    "j",
    "j'",
    "m",
    "m'",
    "t",
    "t'",
    "s",
    "s'",
    "n",
    "n'",
    "c",
    "c'",
    "qu",
    "qu'",
    "jusqu",
    "jusqu'",
    "lorsqu",
    "lorsqu'",
    "puisqu",
    "puisqu'",
    "quoiqu",
    "quoiqu'",
    // Personal, demonstrative and possessive pronouns.
    "je",
    "tu",
    "il",
    "elle",
    "on",
    "nous",
    "vous",
    "ils",
    "elles",
    "moi",
    "toi",
    "soi",
    "lui",
    "leur",
    "leurs",
    "me",
    "te",
    "se",
    "y",
    "en",
    "ce",
    "cet",
    "cette",
    "ces",
    "ça",
    "cela",
    "ceci",
    "celui",
    "celle",
    "ceux",
    "celles",
    "mon",
    "ma",
    "mes",
    "ton",
    "ta",
    "tes",
    "son",
    "sa",
    "ses",
    "notre",
    "votre",
    "nos",
    "vos",
    // Relative and interrogative pronouns.
    "qui",
    "que",
    "quoi",
    "dont",
    "où",
    "quel",
    "quelle",
    "quels",
    "quelles",
    // Coordinating and subordinating conjunctions.
    "et",
    "ou",
    "mais",
    "donc",
    "or",
    "ni",
    "car",
    "si",
    "quand",
    "comme",
    "parce",
    "puisque",
    "lorsque",
    "quoique",
    // Prepositions.
    "à",
    "dans",
    "par",
    "pour",
    "vers",
    "avec",
    "sans",
    "sous",
    "sur",
    "chez",
    "entre",
    "contre",
    "avant",
    "après",
    "depuis",
    "pendant",
    // Adverbs of negation, quantity, degree, time.
    "ne",
    "pas",
    "plus",
    "moins",
    "très",
    "trop",
    "peu",
    "assez",
    "beaucoup",
    "bien",
    "mal",
    "mieux",
    "encore",
    "déjà",
    "toujours",
    "jamais",
    "souvent",
    "parfois",
    "aussi",
    "ainsi",
    "alors",
    "aujourd'hui",
    "hier",
    "demain",
    "maintenant",
    "puis",
    "ici",
    "là",
    "voici",
    "voilà",
    "oui",
    "non",
    "seulement",
    "juste",
    "vraiment",
    "surtout",
    // Auxiliary être, avoir, aller — the high-frequency inflected
    // forms every French text is soaked with.
    "être",
    "suis",
    "es",
    "est",
    "sommes",
    "êtes",
    "sont",
    "étais",
    "était",
    "étions",
    "étiez",
    "étaient",
    "serai",
    "seras",
    "sera",
    "serons",
    "serez",
    "seront",
    "soit",
    "soient",
    "sois",
    "été",
    "étant",
    "avoir",
    "ai",
    "as",
    "a",
    "avons",
    "avez",
    "ont",
    "avais",
    "avait",
    "avions",
    "aviez",
    "avaient",
    "aurai",
    "auras",
    "aura",
    "aurons",
    "aurez",
    "auront",
    "aie",
    "aies",
    "ait",
    "ayons",
    "ayez",
    "aient",
    "eu",
    "ayant",
    "aller",
    "vais",
    "vas",
    "va",
    "allons",
    "allez",
    "vont",
    // Common light / modal verbs.
    "faire",
    "fait",
    "faits",
    "faite",
    "faites",
    "dit",
    "dite",
    "dits",
    "dites",
    "peut",
    "peuvent",
    "pouvoir",
    "veut",
    "veulent",
    "vouloir",
    "doit",
    "doivent",
    // Miscellaneous high-frequency function words.
    "tout",
    "toute",
    "tous",
    "toutes",
    "même",
    "mêmes",
    "autre",
    "autres",
    "aucun",
    "aucune",
    "chaque",
    "plusieurs",
    "quelques",
    "quelque",
    "tel",
    "telle",
    "tels",
    "telles",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~200" — assert we're in the
        // ballpark. The upper bound is loose (~250) because the
        // apostrophe-and-bare-clitic doubling adds 20+ entries the
        // English pack doesn't have.
        assert!(
            STOPWORDS.len() >= 180 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~200 range",
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
        // O(n^2) is fine for a static list of ~200.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn elision_forms_are_present() {
        // Both apostrophe-suffixed and stripped forms of the standard
        // elision clitics.
        for w in [
            "l", "l'", "d", "d'", "qu", "qu'", "j", "j'", "m", "m'", "t", "t'", "s", "s'", "n",
            "n'", "c", "c'",
        ] {
            assert!(STOPWORDS.contains(&w), "elision clitic {w:?} is missing");
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["le", "la", "les", "un", "une", "des", "du", "de"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }
}
