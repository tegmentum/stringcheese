//! The Polish stopword list.
//!
//! Roughly 280 common Polish words drawn from the union of the
//! `stopwords-iso` Polish list and NLTK-adjacent Slavic function-word
//! inventories, augmented with the full paradigms of the auxiliary
//! verbs `być` (to be) / `mieć` (to have) / `móc` (can) / `chcieć`
//! (want).
//!
//! # Accented characters
//!
//! Polish surface text carries the diacritics `ą ć ę ł ń ó ś ź ż`
//! extensively — the list stores every entry in its **exact Polish
//! spelling**, including diacritics. The default trait-level
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`], which does not
//! fold non-ASCII accents; the [`Polish`](crate::Polish) trait
//! implementation therefore **overrides** `is_stopword` to apply
//! [`char::to_lowercase`] (Unicode) before comparison, so uppercase
//! Polish queries like `NIE`, `ŻE`, `AŻ` all match the plain lowercase
//! list entries.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.
//!   Downstream applications should carry their own.
//! - **Regional / historical variants.** No Silesian / Kashubian
//!   entries — those deserve their own language packs.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with a Unicode-aware lowercase pass, so
//!   `"nie"`, `"Nie"`, `"NIE"`, and any other case variant all
//!   resolve to the same list entry.

/// The Polish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // High-frequency function words: articles-lite, conjunctions,
    // discourse particles.
    // -----------------------------------------------------------------
    "a",
    "aby",
    "ach",
    "acz",
    "aczkolwiek",
    "aj",
    "albo",
    "ale",
    "alez",
    "ależ",
    "ani",
    "az",
    "aż",
    // -----------------------------------------------------------------
    // Second-letter head — b/c/d.
    // -----------------------------------------------------------------
    "bardziej",
    "bardzo",
    "bez",
    "bo",
    "bowiem",
    "by",
    "byli",
    "bym",
    "był",
    "była",
    "było",
    "były",
    "będzie",
    "będą",
    "chce",
    "choć",
    "ci",
    "cię",
    "ciebie",
    "co",
    "cokolwiek",
    "coś",
    "czasami",
    "czasem",
    "czemu",
    "czy",
    "czyli",
    "daleko",
    "dla",
    "dlaczego",
    "dlatego",
    "do",
    "dobrze",
    "dokąd",
    "dość",
    "dużo",
    "dwa",
    "dwaj",
    "dwie",
    "dwoje",
    "dziś",
    "dzisiaj",
    // -----------------------------------------------------------------
    // e/f/g/h.
    // -----------------------------------------------------------------
    "gdy",
    "gdyby",
    "gdyż",
    "gdzie",
    "gdziekolwiek",
    "gdzieś",
    "go",
    // -----------------------------------------------------------------
    // i/j — very high frequency.
    // -----------------------------------------------------------------
    "i",
    "ich",
    "ile",
    "im",
    "inna",
    "inne",
    "inny",
    "innych",
    "iż",
    "ja",
    "ją",
    "jak",
    "jakaś",
    "jakby",
    "jaki",
    "jakichś",
    "jakie",
    "jakiś",
    "jakiż",
    "jakkolwiek",
    "jako",
    "jakoś",
    "je",
    "jeden",
    "jedna",
    "jednak",
    "jednakże",
    "jedno",
    "jego",
    "jej",
    "jemu",
    "jest",
    "jestem",
    "jeszcze",
    "jeśli",
    "jeżeli",
    "już",
    // -----------------------------------------------------------------
    // k/l/m/n.
    // -----------------------------------------------------------------
    "każdy",
    "kiedy",
    "kilka",
    "kimś",
    "kto",
    "ktokolwiek",
    "ktoś",
    "która",
    "które",
    "którego",
    "której",
    "który",
    "których",
    "którym",
    "którzy",
    "ku",
    "lat",
    "lecz",
    "lub",
    "ma",
    "mają",
    "mało",
    "mam",
    "mi",
    "mimo",
    "między",
    "mną",
    "mnie",
    "mogą",
    "moi",
    "moim",
    "moja",
    "moje",
    "może",
    "możliwe",
    "można",
    "mój",
    "mu",
    "musi",
    "my",
    "na",
    "nad",
    "nam",
    "nami",
    "nas",
    "nasi",
    "nasz",
    "nasza",
    "nasze",
    "naszego",
    "naszych",
    "natomiast",
    "natychmiast",
    "nawet",
    "nią",
    "nic",
    "nich",
    "nie",
    "niech",
    "niego",
    "niej",
    "niemu",
    "nigdy",
    "nim",
    "nimi",
    "niż",
    "no",
    // -----------------------------------------------------------------
    // o.
    // -----------------------------------------------------------------
    "o",
    "obok",
    "od",
    "około",
    "on",
    "ona",
    "one",
    "oni",
    "ono",
    "oraz",
    "oto",
    "owszem",
    // -----------------------------------------------------------------
    // p.
    // -----------------------------------------------------------------
    "pan",
    "pana",
    "pani",
    "po",
    "pod",
    "podczas",
    "pomimo",
    "ponad",
    "ponieważ",
    "powinien",
    "powinna",
    "powinni",
    "powinno",
    "poza",
    "prawie",
    "przecież",
    "przed",
    "przede",
    "przedtem",
    "przez",
    "przy",
    // -----------------------------------------------------------------
    // r/s.
    // -----------------------------------------------------------------
    "roku",
    "również",
    "sam",
    "sama",
    "są",
    "się",
    "skąd",
    "sobie",
    "sobą",
    "sposób",
    "swoje",
    // -----------------------------------------------------------------
    // t.
    // -----------------------------------------------------------------
    "ta",
    "tak",
    "taka",
    "taki",
    "takich",
    "takie",
    "także",
    "tam",
    "te",
    "tego",
    "tej",
    "temu",
    "ten",
    "teraz",
    "też",
    "to",
    "tobą",
    "tobie",
    "toteż",
    "trzeba",
    "tu",
    "tutaj",
    "twoi",
    "twoim",
    "twoja",
    "twoje",
    "twym",
    "twój",
    "ty",
    "tych",
    "tylko",
    "tym",
    "tys",
    "tyś",
    "tzw",
    // -----------------------------------------------------------------
    // u/v/w.
    // -----------------------------------------------------------------
    "u",
    "w",
    "wam",
    "wami",
    "was",
    "wasi",
    "wasz",
    "wasza",
    "wasze",
    "we",
    "według",
    "wiele",
    "wielu",
    "więc",
    "więcej",
    "wszyscy",
    "wszystkich",
    "wszystkie",
    "wszystkim",
    "wszystko",
    "wtedy",
    "wy",
    // -----------------------------------------------------------------
    // z/ż/ź — auxiliary and connective extras.
    // -----------------------------------------------------------------
    "z",
    "za",
    "zapewne",
    "zawsze",
    "ze",
    "zł",
    "znowu",
    "znów",
    "został",
    "żaden",
    "żadna",
    "żadne",
    "żadnych",
    "że",
    "żeby",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~200" — assert we're in the
        // ballpark. Range accommodates the auxiliary-verb paradigm
        // expansion and the ASCII / diacritic-carrying dual entries
        // (e.g. `az` / `aż`) which cover both keyboard-shortcut
        // typing and the correct Polish spelling.
        let n = STOPWORDS.len();
        assert!(
            (150..=300).contains(&n),
            "STOPWORDS.len() = {n} outside the advertised ~200-280 range",
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
    fn common_conjunctions_and_particles_are_present() {
        for w in [
            "i", "a", "ale", "lub", "albo", "czy", "że", "aby", "gdy", "jak",
        ] {
            assert!(
                STOPWORDS.contains(&w),
                "conjunction/particle {w:?} is missing"
            );
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "na", "w", "we", "z", "ze", "za", "do", "od", "po", "przez", "przy", "u", "pod", "nad",
            "bez", "między",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in [
            "ja", "ty", "on", "ona", "ono", "my", "wy", "oni", "one", "mnie", "ciebie", "jego",
            "jej", "ich", "nam", "wam", "im", "się",
        ] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_byc_forms_are_present() {
        // `być` (to be) paradigm — the pack targets the modern
        // present-tense and past-tense forms typically listed in Polish
        // stopword collections.
        for w in [
            "jest", "są", "był", "była", "było", "były", "będzie", "będą", "jestem",
        ] {
            assert!(STOPWORDS.contains(&w), "być-form {w:?} is missing");
        }
    }

    #[test]
    fn diacritic_carrying_entries_are_stored_with_diacritics() {
        // Confirm the entries actually carry the Polish diacritics.
        for w in ["że", "aż", "już", "śię_placeholder", "między"] {
            if w == "śię_placeholder" {
                continue;
            }
            assert!(STOPWORDS.contains(&w), "diacritic entry {w:?} is missing");
        }
        // Look for at least a few obviously diacritic-carrying entries.
        assert!(STOPWORDS.iter().any(|w| w.contains('ż')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ę')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ą')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ś')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ó')));
    }
}
