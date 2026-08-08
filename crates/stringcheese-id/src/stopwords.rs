//! The Indonesian (Bahasa Indonesia) stopword list.
//!
//! Roughly 90 common Indonesian words: conjunctions, prepositions,
//! personal / demonstrative / interrogative pronouns, auxiliary and
//! copular verbs, common adverbs, negations, and numerals up to
//! `sepuluh`. Compiled from the intersection of published Indonesian
//! IR stopword collections (Tala 2003 stemmer companion list; the
//! Sastrawi library's stopword file; the Snowball-community `id`
//! candidate list).
//!
//! # ASCII-only orthography
//!
//! Indonesian is written in the modern 26-letter Latin alphabet with
//! **no diacritics** — every entry below fits in ASCII. The Indonesian
//! pack therefore does not need a locale-specific case fold, and the
//! default [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! (which uses [`str::eq_ignore_ascii_case`]) works correctly without
//! an override.
//!
//! # Non-goals
//!
//! - **Regional / dialectal variants.** No Javanese, Sundanese, Malay
//!   (`ms`), or other regional-language vocabulary. Malay is a
//!   separate BCP-47 code; a `stringcheese-ms` pack is out of scope
//!   here.
//! - **Colloquial / SMS-register forms.** No `gw`/`gue` for `saya`,
//!   no `lu`/`elo` for `kamu`, no `bgt` for `banget`. Add them
//!   downstream if your corpus is informal.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or news corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Indonesian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in lowercase
/// ASCII; the [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
/// default (case-insensitive ASCII match) is correct without an
/// override.
pub const STOPWORDS: &[&str] = &[
    // Coordinating / subordinating conjunctions.
    "dan",
    "atau",
    "tetapi",
    "tapi",
    "namun",
    "melainkan",
    "sedangkan",
    "serta",
    "karena",
    "sebab",
    "jika",
    "kalau",
    "bila",
    "apabila",
    "supaya",
    "agar",
    "walaupun",
    "meskipun",
    "sehingga",
    "maka",
    "yang",
    // Prepositions.
    "di",
    "ke",
    "dari",
    "pada",
    "kepada",
    "dalam",
    "untuk",
    "dengan",
    "tanpa",
    "oleh",
    "atas",
    "bawah",
    "antara",
    "tentang",
    "hingga",
    "sampai",
    // Personal pronouns.
    "saya",
    "aku",
    "kamu",
    "engkau",
    "anda",
    "dia",
    "beliau",
    "ia",
    "kami",
    "kita",
    "kalian",
    "mereka",
    // Demonstratives / determiners.
    "ini",
    "itu",
    "sini",
    "situ",
    "sana",
    "begini",
    "begitu",
    // Interrogatives.
    "apa",
    "siapa",
    "mana",
    "kapan",
    "bagaimana",
    "mengapa",
    "kenapa",
    "berapa",
    // Copula / existential / auxiliary.
    "adalah",
    "ialah",
    "ada",
    "tidak",
    "tak",
    "bukan",
    "belum",
    "sudah",
    "telah",
    "akan",
    "sedang",
    "masih",
    "pernah",
    "boleh",
    "harus",
    "wajib",
    "mesti",
    "bisa",
    "dapat",
    // Adverbs (degree, quantity, time, place).
    "sangat",
    "amat",
    "sekali",
    "cukup",
    "hampir",
    "lebih",
    "kurang",
    "sekitar",
    "sekarang",
    "kemarin",
    "besok",
    "lalu",
    "juga",
    "pula",
    "hanya",
    "saja",
    "cuma",
    "pun",
    // Numerals (low).
    "satu",
    "dua",
    "tiga",
    "empat",
    "lima",
    "enam",
    "tujuh",
    "delapan",
    "sembilan",
    "sepuluh",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~90" — assert we're in the
        // ballpark.
        assert!(
            STOPWORDS.len() >= 80 && STOPWORDS.len() <= 140,
            "STOPWORDS.len() = {} outside the advertised ~90 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase_ascii() {
        for &w in STOPWORDS {
            assert!(
                w.chars().all(|c| c.is_ascii_lowercase()),
                "stopword {w:?} contains a non-ASCII-lowercase character"
            );
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~90.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["dan", "atau", "yang", "tetapi", "karena", "jika"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["di", "ke", "dari", "pada", "dalam", "untuk", "dengan"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["saya", "aku", "kamu", "dia", "kami", "kita", "mereka"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_negations_and_auxiliaries_are_present() {
        for w in ["tidak", "bukan", "belum", "sudah", "akan", "adalah", "ada"] {
            assert!(
                STOPWORDS.contains(&w),
                "negation/auxiliary {w:?} is missing"
            );
        }
    }
}
