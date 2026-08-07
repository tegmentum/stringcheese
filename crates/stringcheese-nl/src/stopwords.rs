//! The Dutch stopword list.
//!
//! Roughly 160 common Dutch words drawn from the Snowball project's
//! `dutch/stop.txt` (the same source the Portuguese pack pulls its list
//! from). The list combines the ranked head of high-frequency function
//! words with the full paradigms of the auxiliary verbs `zijn` / `hebben`
//! / `zullen` / `worden` / `kunnen` / `moeten` / `mogen` / `willen`
//! / `doen`.
//!
//! # Accented characters
//!
//! Dutch is nearly ASCII-clean in surface text — the diaeresis
//! (`ë`, `ï`, `ü`) and the acute (`é`) show up on borrowed or
//! disambiguated words but almost never on function words. The one
//! entry in this list that carries a diacritic is `één` (the stressed
//! form of the article/numeral `een`); the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — so accented forms only match when spelled exactly as
//! stored here.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Regional variants.** No Belgian-Dutch (Flemish) or Afrikaans
//!   variants — the list targets standard Netherlands Dutch
//!   vocabulary of the Snowball `dutch/stop.txt` distribution.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so `"de"`,
//!   `"De"`, and `"DE"` are all recognized. The default trait-level
//!   check does not fold non-ASCII accents.

/// The Dutch stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the commonest Dutch function words per the
    // Snowball project's stop.txt.
    "de",
    "en",
    "van",
    "ik",
    "te",
    "dat",
    "die",
    "in",
    "een",
    "hij",
    "het",
    "niet",
    "zijn",
    "is",
    "was",
    "op",
    "aan",
    "met",
    "als",
    "voor",
    "had",
    "er",
    "maar",
    "om",
    "hem",
    "dan",
    "zou",
    "of",
    "wat",
    "mijn",
    "men",
    "dit",
    "zo",
    "door",
    "over",
    "ze",
    "zich",
    "bij",
    "ook",
    "tot",
    "je",
    "mij",
    "uit",
    "der",
    "daar",
    "haar",
    "naar",
    "heb",
    "hoe",
    "heeft",
    "hebben",
    "deze",
    "u",
    "want",
    "nog",
    "zal",
    "me",
    "zij",
    "nu",
    "ge",
    "geen",
    "omdat",
    "iets",
    "worden",
    "toch",
    "al",
    "waren",
    "veel",
    "meer",
    "doen",
    "toen",
    "moet",
    "ben",
    "zonder",
    "kan",
    "hun",
    "dus",
    "alles",
    "onder",
    "ja",
    "eens",
    "hier",
    "wie",
    "werd",
    "altijd",
    "doch",
    "wordt",
    "wezen",
    "kunnen",
    "ons",
    "zelf",
    "tegen",
    "na",
    "reeds",
    "wil",
    "kon",
    "niets",
    "uw",
    "iemand",
    "geweest",
    "andere",
    // Extras — additional pronouns and demonstratives.
    "we",
    "jij",
    "jullie",
    "hen",
    "hunne",
    "onze",
    "onzen",
    "onzer",
    "onzes",
    "zulke",
    "zulks",
    "zulk",
    "dezelfde",
    "hetzelfde",
    "diezelfde",
    "datzelfde",
    "welke",
    "welk",
    "welken",
    // Forms of ZIJN (to be) not already present above.
    "zijnde",
    "ware",
    // Forms of HEBBEN (to have) not already present above.
    "hebt",
    "hadden",
    "gehad",
    "hebbende",
    // Forms of ZULLEN (shall) not already present above.
    "zult",
    "zullen",
    "zouden",
    "zoudt",
    // Forms of WORDEN (to become / passive auxiliary) not already present.
    "werden",
    "geworden",
    "wordende",
    // Forms of KUNNEN (can) not already present.
    "kunt",
    "konden",
    "gekund",
    // Forms of MOETEN (must) not already present.
    "moeten",
    "moest",
    "moesten",
    "gemoeten",
    // Forms of MOGEN (may).
    "mag",
    "mogen",
    "mocht",
    "mochten",
    "gemogen",
    // Forms of WILLEN (want) not already present.
    "wilt",
    "willen",
    "wilde",
    "wilden",
    "wou",
    "wouden",
    "gewild",
    // Forms of DOEN (to do) not already present.
    "doe",
    "doet",
    "deed",
    "deden",
    "gedaan",
    "doende",
    // Adverbial and connective extras.
    "echter",
    "steeds",
    "dikwijls",
    "vaak",
    "misschien",
    "wel",
    "eigenlijk",
    "vooral",
    "ergens",
    "nergens",
    "overal",
    "waarom",
    "wanneer",
    "waarheen",
    "waar",
    // The stressed article/numeral `één`.
    "één",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~160" — assert we're in the
        // ballpark. Range is loose to accommodate the auxiliary-verb
        // paradigm expansion (zijn/hebben/zullen/worden/kunnen/moeten/
        // mogen/willen/doen).
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~160 range",
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
        // O(n^2) is fine for a static list of ~160.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["de", "het", "een"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "van", "in", "op", "aan", "met", "voor", "door", "over", "bij", "tot", "uit", "naar",
            "onder", "tegen", "na",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["en", "of", "maar", "want", "omdat", "dus", "als", "dan"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // The Snowball `dutch/stop.txt` covers ZIJN / HEBBEN / ZULLEN /
        // WORDEN paradigms. This asserts a representative subset that
        // lets a callsite trust the pack for the common auxiliary
        // conjugations.
        for w in [
            "is", "was", "zijn", "waren", "heb", "heeft", "hebben", "hadden", "zal", "zullen",
            "worden", "wordt", "werd",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }
}
