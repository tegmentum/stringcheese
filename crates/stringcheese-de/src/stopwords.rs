//! The German stopword list.
//!
//! The list contains ~200 common German words drawn primarily from
//! NLTK's `german` list (which itself descends from the widely-cited
//! Snowball / Porter German stopword list, mirrored on
//! [snowballstem.org][ref]). The list is deliberately conservative — no
//! domain-specific jargon, no archaic forms — and covers articles
//! (`der`, `die`, `das`), pronouns (`ich`, `du`, `er`), auxiliaries and
//! modals (`sein`, `haben`, `werden`, `können`), prepositions (`in`,
//! `von`, `zu`), conjunctions (`und`, `oder`, `aber`), and inflected
//! forms of the above.
//!
//! [ref]: https://snowballstem.org/algorithms/german/stop.txt
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific German corpora typically extends the general list.
//!   Downstream applications should carry their own.
//! - **Swiss / Austrian forms.** The list uses the standard German
//!   orthography reformed 1996; Swiss usage drops `ß` for `ss` and
//!   several regional forms of the pronouns and modals are not
//!   included. Swiss corpora need a tailored list.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so
//!   `"der"`, `"Der"`, and `"DER"` are all recognized. Note that this
//!   is ASCII-case-only — the umlaut vowels compare exactly, and words
//!   like `"Über"` need to be lower-cased through Unicode-aware case
//!   folding before the check if the caller wants full case
//!   insensitivity.

/// The German stopword list (~200 entries).
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Articles (der/die/das and their inflected forms).
    "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines", "einem", "einen",
    "kein", "keine", "keiner", "keines", "keinem", "keinen", // Personal pronouns.
    "ich", "mich", "mir", "mein", "meine", "meiner", "meines", "meinem", "meinen", "du", "dich",
    "dir", "dein", "deine", "deiner", "deines", "deinem", "deinen", "er", "ihn", "ihm", "sein",
    "seine", "seiner", "seines", "seinem", "seinen", "sie", "ihr", "ihre", "ihrer", "ihres",
    "ihrem", "ihren", "es", "wir", "uns", "unser", "unsere", "unserer", "unseres", "unserem",
    "unseren", "euch", "euer", "eure", "eurer", "eures", "eurem", "euren",
    // Demonstratives.
    "dieser", "diese", "dieses", "diesem", "diesen", "dies", "jener", "jene", "jenes", "jenem",
    "jenen", "solcher", "solche", "solches", "solchem", "solchen",
    // Relative and interrogative.
    "welcher", "welche", "welches", "welchem", "welchen", "wer", "wen", "wem", "wessen", "was",
    // "sein" (to be) — full paradigm.
    "bin", "bist", "ist", "sind", "seid", "war", "warst", "waren", "wart", "gewesen", "sei",
    "seien", "seiest", "wäre", "wären", "wärest", "wärst", "wäret", "wärt",
    // "haben" (to have) — full paradigm.
    "haben", "habe", "hast", "hat", "habt", "hatte", "hattest", "hatten", "hattet", "gehabt",
    "hätte", "hätten", "hättest", "hättet",
    // "werden" (to become / passive auxiliary).
    "werden", "werde", "wirst", "wird", "werdet", "wurde", "wurdest", "wurden", "wurdet",
    "geworden", "worden", "würde", "würden", "würdest", "würdet", // Modals.
    "können", "kann", "kannst", "könnt", "konnte", "konntest", "konnten", "konntet", "gekonnt",
    "könnte", "könnten", "müssen", "muss", "musst", "müsst", "musste", "mussten", "gemusst",
    "sollen", "soll", "sollst", "sollt", "sollte", "sollten", "wollen", "will", "willst", "wollt",
    "wollte", "wollten", "dürfen", "darf", "darfst", "dürft", "durfte", "durften", "mögen", "mag",
    "magst", "mögt", "mochte", "mochten", // Common prepositions.
    "an", "auf", "aus", "bei", "bis", "durch", "für", "gegen", "hinter", "in", "mit", "nach",
    "neben", "ohne", "seit", "über", "um", "unter", "von", "vor", "während", "wegen", "zu",
    "zwischen", // Conjunctions.
    "und", "oder", "aber", "denn", "sondern", "wenn", "weil", "damit", "dass", "ob", "als", "wie",
    "obwohl", // Adverbs / particles.
    "nicht", "nur", "auch", "noch", "schon", "immer", "sehr", "so", "hier", "da", "dort", "dann",
    "jetzt", "heute", "gestern", "morgen", "wieder", "einmal", "mal", "doch", "ja", "nein", "man",
    "eben", "etwa", "sogar", "kaum", "recht", "wohl",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~200" — assert we're in the
        // ballpark.
        assert!(
            STOPWORDS.len() >= 180 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~200 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        // German stopwords may contain non-ASCII (umlauts, ß). Assert
        // every scalar is either an ASCII lowercase letter or a lowercase
        // German-specific letter.
        for &w in STOPWORDS {
            for ch in w.chars() {
                assert!(
                    ch.is_lowercase() || matches!(ch, 'ß'),
                    "stopword {w:?} contains non-lowercase char {ch:?}"
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
    fn well_known_stopwords_present() {
        // Sanity-check a handful of the words every German stopword
        // list has.
        for w in [
            "der", "die", "das", "und", "in", "zu", "den", "ist", "nicht", "ein",
        ] {
            assert!(
                STOPWORDS.contains(&w),
                "expected {w:?} to be in the stopword list"
            );
        }
    }
}
