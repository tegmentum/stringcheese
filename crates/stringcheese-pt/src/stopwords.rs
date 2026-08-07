//! The Portuguese stopword list.
//!
//! Roughly 200 common Portuguese words drawn from the Snowball
//! project's `portuguese/stop.txt` (Martin Porter's own Portuguese
//! stoplist). The list combines a ranked head of high-frequency
//! function words with the full paradigms of the auxiliary verbs
//! `ser` / `estar` / `haver` / `ter`.
//!
//! # Accented characters
//!
//! Several stopwords carry acute or tilde accents (`é`, `há`, `está`,
//! `também`, `então`, `são`, `hão`, …). These are stored in their
//! canonical accented form. The default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — so `é` matches `É` **only if the input carries the
//! accented `É`**; the ASCII probe `E` does not match the accented
//! stopword `é`. Callers whose input pipeline already applies Unicode
//! case folding will see the expected match; callers who feed
//! accent-stripped input must be aware that accented stopwords will
//! silently miss.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Regional variants.** No pt-BR-only or pt-PT-only forms; the
//!   list targets the shared standard vocabulary of the Snowball
//!   `portuguese/stop.txt` distribution.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so
//!   `"o"`, `"O"`, and (for accent-free stopwords) all ASCII case
//!   variants are recognized. The default trait-level check does not
//!   fold non-ASCII accents.

/// The Portuguese stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the ~100 commonest Portuguese function words per
    // the Snowball project's stop.txt.
    "de",
    "a",
    "o",
    "que",
    "e",
    "do",
    "da",
    "em",
    "um",
    "para",
    "com",
    "não",
    "uma",
    "os",
    "no",
    "se",
    "na",
    "por",
    "mais",
    "as",
    "dos",
    "como",
    "mas",
    "ao",
    "ele",
    "das",
    "à",
    "seu",
    "sua",
    "ou",
    "quando",
    "muito",
    "nos",
    "já",
    "eu",
    "também",
    "só",
    "pelo",
    "pela",
    "até",
    "isso",
    "ela",
    "entre",
    "depois",
    "sem",
    "mesmo",
    "aos",
    "seus",
    "quem",
    "nas",
    "me",
    "esse",
    "eles",
    "você",
    "essa",
    "num",
    "nem",
    "suas",
    "meu",
    "às",
    "minha",
    "numa",
    "pelos",
    "elas",
    "qual",
    "nós",
    "lhe",
    "deles",
    "essas",
    "esses",
    "pelas",
    "este",
    "dele",
    // Extras — extended pronouns and demonstratives.
    "tu",
    "te",
    "vocês",
    "vos",
    "lhes",
    "meus",
    "minhas",
    "teu",
    "tua",
    "teus",
    "tuas",
    "nosso",
    "nossa",
    "nossos",
    "nossas",
    "dela",
    "delas",
    "esta",
    "estes",
    "estas",
    "aquele",
    "aquela",
    "aqueles",
    "aquelas",
    "isto",
    "aquilo",
    // Forms of ESTAR (excluding the infinitive itself).
    "estou",
    "está",
    "estamos",
    "estão",
    "estive",
    "esteve",
    "estivemos",
    "estiveram",
    "estava",
    "estávamos",
    "estavam",
    "estivera",
    "estivéramos",
    "esteja",
    "estejamos",
    "estejam",
    "estivesse",
    "estivéssemos",
    "estivessem",
    "estiver",
    "estivermos",
    "estiverem",
    // Forms of HAVER.
    "hei",
    "há",
    "havemos",
    "hão",
    "houve",
    "houvemos",
    "houveram",
    "houvera",
    "houvéramos",
    "haja",
    "hajamos",
    "hajam",
    "houvesse",
    "houvéssemos",
    "houvessem",
    "houver",
    "houvermos",
    "houverem",
    "houverei",
    "houverá",
    "houveremos",
    "houverão",
    "houveria",
    "houveríamos",
    "houveriam",
    // Forms of SER.
    "sou",
    "somos",
    "são",
    "era",
    "éramos",
    "eram",
    "fui",
    "foi",
    "fomos",
    "foram",
    "fora",
    "fôramos",
    "seja",
    "sejamos",
    "sejam",
    "fosse",
    "fôssemos",
    "fossem",
    "for",
    "formos",
    "forem",
    "serei",
    "será",
    "seremos",
    "serão",
    "seria",
    "seríamos",
    "seriam",
    // Forms of TER.
    "tenho",
    "tem",
    "temos",
    "tém",
    "tinha",
    "tínhamos",
    "tinham",
    "tive",
    "teve",
    "tivemos",
    "tiveram",
    "tivera",
    "tivéramos",
    "tenha",
    "tenhamos",
    "tenham",
    "tivesse",
    "tivéssemos",
    "tivessem",
    "tiver",
    "tivermos",
    "tiverem",
    "terei",
    "terá",
    "teremos",
    "terão",
    "teria",
    "teríamos",
    "teriam",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~200" — assert we're in the
        // ballpark. Range is loose because Portuguese has a lot of
        // auxiliary-verb conjugations plus gender/number-inflected
        // pronouns and demonstratives.
        assert!(
            STOPWORDS.len() >= 180 && STOPWORDS.len() <= 320,
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
    fn common_articles_are_present() {
        for w in ["o", "a", "os", "as", "um", "uma"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "de", "em", "por", "para", "com", "sem", "entre", "até", "no", "na", "do", "da",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["e", "ou", "mas", "que", "se", "quando", "como"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // The Snowball `portuguese/stop.txt` covers SER / ESTAR /
        // HAVER / TER paradigms. This asserts a representative subset
        // that lets a callsite trust the pack for the common auxiliary
        // conjugations.
        for w in [
            "sou", "são", "era", "foi", "está", "estão", "há", "hei", "houve", "tenho", "tem",
            "tinha",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }
}
