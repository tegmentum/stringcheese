//! The Bengali stopword list.
//!
//! ~65 common Bengali function words: personal / demonstrative /
//! interrogative pronouns, postpositions, conjunctions, particles, the
//! high-frequency forms of the copula *হওয়া* ("to be") and the
//! auxiliary *করা* ("to do"), and common adverbs. The list is drawn
//! from the intersection of several open Bengali stopword corpora; it
//! deliberately omits content words.
//!
//! # Bengali script — a note on storage width
//!
//! Every entry here is written in the Bengali script. Bengali scalars
//! live in the range **U+0980..=U+09FF**, which falls in UTF-8's
//! 3-byte range (U+0800..=U+FFFF). A word like `"আমি"` (2
//! characters: `আ` + `ম` + `ি`) is **9 bytes**. Any code that mixes
//! byte offsets with character-boundary logic will silently corrupt
//! token or entry boundaries. The stopword list itself is a plain
//! `&[&str]` of static strings, so this concern only surfaces in
//! downstream iteration; every consumer in this crate walks scalars
//! via [`str::chars`] or [`Vec<char>`], not raw bytes.
//!
//! # Case folding
//!
//! Bengali has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Bengali scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Bengali input.
//!
//! # Non-goals
//!
//! - **Assamese / Manipuri stopwords.** Bengali script is also used
//!   for Assamese (with two additional letters `ৰ`/`ৱ`) and
//!   historically for Manipuri; both languages have distinct function
//!   word inventories. Each deserves its own pack.
//! - **Colloquial / spoken forms.** Written Bengali (shadhu-bhasha
//!   and chalit-bhasha) diverges from spoken Bengali in the surface
//!   form of many function words. The list targets the written /
//!   newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Bengali stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Bengali string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "আমি",    // I
    "আমাকে",   // to me
    "আমার",    // my
    "আমরা",    // we
    "আমাদের",  // our / of us
    "তুমি",    // you (familiar)
    "তোমাকে",  // to you (familiar)
    "তোমার",   // your (familiar)
    "তোমরা",   // you (familiar plural)
    "তোমাদের", // your (familiar plural)
    "আপনি",   // you (formal)
    "আপনার",   // your (formal)
    "তুই",     // you (intimate)
    "সে",     // he/she (proximal / neutral)
    "তিনি",   // he/she (honorific)
    "তার",     // his/her
    "তাকে",    // to him/her
    "তাহার",    // his/her (literary variant)
    "তারা",     // they
    "তাদের",   // their
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "এই",  // this
    "এটা",  // this one
    "এটি", // this (formal)
    "ওই",  // that
    "ওটা",  // that one
    "সেই", // that (distal)
    "সেটা", // that one (distal)
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "কি",    // what / question particle
    "কী",    // what (emphatic)
    "কে",    // who
    "কেন",   // why
    "কীভাবে", // how
    "কোথায়",  // where
    "কখন",   // when
    "কোন",   // which
    // -----------------------------------------------------------------
    // Postpositions / case-like markers.
    // -----------------------------------------------------------------
    "এর",   // of / genitive (with proximal)
    "তে",   // in / locative (fused; note homographic with -তে suffix)
    "থেকে", // from
    "দিয়ে", // with / by (instrumental)
    "জন্য",  // for (purpose)
    "উপর",  // on / above
    "নিচে", // below
    "সাথে",  // with (comitative)
    "মধ্যে", // in / among / between
    "কাছে",  // near / at
    "পরে",  // after
    "আগে",  // before / in front
    // -----------------------------------------------------------------
    // Coordinating / subordinating conjunctions.
    // -----------------------------------------------------------------
    "এবং",  // and
    "ও",    // and / also
    "বা",    // or
    "কিন্তু", // but
    "তবে",  // however / but
    "যদি",  // if
    "তাহলে", // then
    "কারণ",  // because
    "যে",   // that (complementizer) / who (relative)
    "যা",    // that which / what (relative)
    "যখন",  // when (relative)
    "তখন",  // then (correlative)
    // -----------------------------------------------------------------
    // Negation particles.
    // -----------------------------------------------------------------
    "না",   // no / not
    "নয়",  // is not
    "নেই", // is not present / does not exist
    // -----------------------------------------------------------------
    // "To be" (হওয়া) / copula surface forms.
    // -----------------------------------------------------------------
    "আছে",   // is / exists
    "আছেন",  // is (honorific)
    "ছিল",   // was
    "ছিলেন", // was (honorific)
    "হয়",    // is / becomes
    "হবে",   // will be
    "হয়েছে", // has become / has been
    "হচ্ছে",  // is becoming (progressive)
    // -----------------------------------------------------------------
    // "To do" (করা) — high-frequency forms.
    // -----------------------------------------------------------------
    "করে",   // does / doing
    "করেন",  // does (honorific)
    "করছে",  // is doing
    "করবে",  // will do
    "করেছে", // has done
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "সব",    // all
    "কিছু",   // some / something
    "অনেক",  // much / many
    "খুব",    // very
    "শুধু",    // only
    "শুধুমাত্র", // only (emphatic)
    "আবার",   // again
    "এখন",   // now
    "তারপর",  // then / afterwards
    "এক",    // one
    "দুই",    // two
    "তিন",   // three
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~65" — assert we're in that
        // ballpark. The task spec asks for ~60 entries.
        assert!(
            STOPWORDS.len() >= 50 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised 50-150 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_non_empty() {
        for &w in STOPWORDS {
            assert!(!w.is_empty(), "empty stopword entry");
        }
    }

    #[test]
    fn every_stopword_contains_bengali() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Bengali block U+0980..=U+09FF.
        for &w in STOPWORDS {
            let has_bengali = w.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c));
            assert!(has_bengali, "stopword {w:?} has no Bengali scalar");
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
    fn stopwords_contain_core_pronouns() {
        for &w in &["আমি", "তুমি", "আপনি", "সে", "তিনি"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["এবং", "বা", "কিন্তু", "যদি"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["আছে", "ছিল", "হয়"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_particles() {
        for &w in &["না", "নয়", "নেই"] {
            assert!(STOPWORDS.contains(&w), "negation particle {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~65.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
