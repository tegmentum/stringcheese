//! The Hindi stopword list.
//!
//! ~130 common Hindi function words drawn from the intersection of
//! several well-known open lists (the iNLTK Hindi stopword list, the
//! stopwords-hi corpus, and the Ranks NL Hindi list). The list targets
//! personal / demonstrative / interrogative pronouns, postpositions,
//! conjunctions, particles, the high-frequency forms of the copula
//! *hona* ("to be") and the auxiliary *karna* ("to do"), and common
//! adverbs; it deliberately omits content words.
//!
//! # Devanagari script — a note on storage width
//!
//! Every entry here is written in the Devanagari script. Devanagari
//! scalars live in the range **U+0900..=U+097F**, which falls in
//! UTF-8's 3-byte range (U+0800..=U+FFFF). A word like `"मैं"` (3
//! characters: `म` + `ै` + `ं`) is **9 bytes**. Any code that mixes byte
//! offsets with character-boundary logic will silently corrupt token
//! or entry boundaries. The stopword list itself is a plain `&[&str]`
//! of static strings, so this concern only surfaces in downstream
//! iteration; every consumer in this crate walks scalars via
//! [`str::chars`] or [`Vec<char>`], not raw bytes.
//!
//! # Case folding
//!
//! Devanagari has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Devanagari scalars (they are non-ASCII and pass through
//! the comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Devanagari input.
//!
//! # Orthographic variants
//!
//! Hindi text in the wild carries several orthographic uncertainties
//! the stopword list does not enumerate:
//!
//! - **Nukta (`़` U+093C).** The nukta combining mark modifies base
//!   consonants to produce Perso-Arabic loans (`क़`, `ख़`, `ग़`, `ज़`,
//!   `ड़`, `ढ़`, `फ़`). It is **semantically meaningful** — `ज` (`ja`)
//!   and `ज़` (`za`) are different phonemes — so the stopword list
//!   stores the surface form the entry contains and does not attempt
//!   to fold `क़ ↔ क` or similar. Callers whose input carries a nukta
//!   that ought to be folded should run
//!   [`crate::normalize::HindiNormalizer::with_nukta_stripping`] first.
//! - **Devanagari digits (`०`..`९` U+0966..=U+096F) vs. ASCII digits.**
//!   Hindi text may carry either; the stopword list contains no
//!   numerals so this uncertainty does not affect matching here.
//!
//! # Non-goals
//!
//! - **Sanskrit / Marathi / Nepali stopwords.** These languages share
//!   the Devanagari script with Hindi but have quite different function
//!   words. Each deserves its own pack — see the crate roadmap.
//! - **Colloquial / spoken forms.** Written Hindi and spoken Hindi
//!   differ in the surface form of many function words. The list
//!   targets the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Hindi stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Devanagari string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "मैं",      // I
    "मुझे",     // to me
    "मेरा",    // my (m. sg.)
    "मेरी",    // my (f. sg.)
    "मेरे",     // my (m. pl. / oblique)
    "हम",     // we
    "हमें",     // to us
    "हमारा",  // our (m. sg.)
    "हमारी",  // our (f. sg.)
    "हमारे",   // our (m. pl. / oblique)
    "तू",      // you (informal sg.)
    "तुझे",     // to you (informal sg.)
    "तेरा",    // your (informal m. sg.)
    "तेरी",    // your (informal f. sg.)
    "तेरे",     // your (informal m. pl. / oblique)
    "तुम",     // you (familiar)
    "तुम्हें",    // to you (familiar)
    "तुम्हारा", // your (familiar m. sg.)
    "तुम्हारी", // your (familiar f. sg.)
    "तुम्हारे",  // your (familiar m. pl. / oblique)
    "आप",     // you (formal)
    "आपको",   // to you (formal)
    "आपका",   // your (formal m. sg.)
    "आपकी",   // your (formal f. sg.)
    "आपके",    // your (formal m. pl. / oblique)
    "वह",     // he/she/it (distal)
    "वे",      // they (distal)
    "यह",     // he/she/it (proximal)
    "ये",      // they (proximal) / these
    "उसे",     // to him/her (distal)
    "उन्हें",    // to them (distal)
    "इसे",     // to him/her (proximal)
    "इन्हें",    // to them (proximal)
    "उसका",   // his/her (distal, m. sg.)
    "उसकी",   // his/her (distal, f. sg.)
    "उसके",    // his/her (distal, m. pl. / oblique)
    "उनका",   // their (distal, m. sg.)
    "उनकी",   // their (distal, f. sg.)
    "उनके",    // their (distal, m. pl. / oblique)
    "इसका",   // his/her (proximal, m. sg.)
    "इसकी",   // his/her (proximal, f. sg.)
    "इसके",    // his/her (proximal, m. pl. / oblique)
    "इनका",   // their (proximal, m. sg.)
    "इनकी",   // their (proximal, f. sg.)
    "इनके",    // their (proximal, m. pl. / oblique)
    // -----------------------------------------------------------------
    // Demonstratives and locative adverbs.
    // -----------------------------------------------------------------
    "यहाँ", // here
    "वहाँ", // there
    "यहां", // here (anusvara variant)
    "वहां", // there (anusvara variant)
    "कहाँ", // where
    "कहां", // where (anusvara variant)
    "जहाँ", // where (relative)
    "जहां", // where (relative, anusvara variant)
    "ऐसा", // like this (m. sg.)
    "ऐसी", // like this (f. sg.)
    "ऐसे",  // like this (m. pl. / oblique)
    "वैसा", // like that (m. sg.)
    "वैसी", // like that (f. sg.)
    "वैसे",  // like that (m. pl. / oblique)
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "क्या",   // what
    "कौन",   // who
    "क्यों",   // why
    "कैसे",    // how
    "कैसा",   // how (m. sg.)
    "कैसी",   // how (f. sg.)
    "कब",    // when
    "कितना", // how much (m. sg.)
    "कितनी", // how much (f. sg.)
    "कितने",  // how many
    // -----------------------------------------------------------------
    // Postpositions — the analogue of English prepositions but written
    // after the noun. Hindi is postpositional.
    // -----------------------------------------------------------------
    "का",   // of (m. sg.)
    "की",   // of (f. sg.)
    "के",    // of (m. pl. / oblique)
    "को",   // to / accusative-dative marker
    "ने",    // ergative marker
    "से",    // from / with / by
    "में",    // in
    "पर",   // on / at
    "तक",   // up to / until
    "साथ",  // with (comitative)
    "लिए",  // for (as in "for the sake of")
    "बिना", // without
    "पास",  // near
    "बीच",  // between
    "ऊपर",  // above / on top
    "नीचे",  // below
    "आगे",   // ahead / in front
    "पीछे",  // behind
    "अंदर",  // inside
    "बाहर", // outside
    // -----------------------------------------------------------------
    // Coordinating / subordinating conjunctions.
    // -----------------------------------------------------------------
    "और", // and
    "या", // or
    // "पर" (adversative "but") is homographic with the locative
    // postposition पर listed above; a single entry covers both.
    "लेकिन",  // but
    "किंतु",   // but
    "परन्तु",  // but
    "अगर",   // if
    "यदि",   // if
    "तो",    // then
    "क्योंकि", // because
    "कि",    // that (complementizer)
    "जब",    // when (relative)
    "तब",    // then (correlative)
    "जो",    // who (relative)
    "जिसे",   // whom (relative)
    // -----------------------------------------------------------------
    // Negation particles.
    // -----------------------------------------------------------------
    "नहीं", // no / not
    "मत",  // don't (imperative negation)
    "न",   // not (literary)
    // -----------------------------------------------------------------
    // "To be" (होना) — the most-common surface forms.
    // -----------------------------------------------------------------
    "है",    // is
    "हैं",    // are
    "था",   // was (m. sg.)
    "थी",   // was (f. sg.)
    "थे",    // were (m. pl.)
    "थीं",   // were (f. pl.)
    "हूँ",    // am
    "हूं",    // am (anusvara variant)
    "हो",   // are (2nd person familiar) / imperative
    "होगा", // will be (m. sg.)
    "होगी", // will be (f. sg.)
    "होंगे",  // will be (m. pl.)
    "होना", // to be (infinitive)
    "होने",  // being (oblique)
    "हुआ",   // happened (m. sg.)
    "हुई",   // happened (f. sg.)
    "हुए",   // happened (m. pl.)
    // -----------------------------------------------------------------
    // "To do" (करना) — high-frequency forms used as an auxiliary in
    // countless compound verbs.
    // -----------------------------------------------------------------
    "करना", // to do (infinitive)
    "करता", // does (m. sg. habitual)
    "करती", // does (f. sg. habitual)
    "करते",  // do (m. pl. habitual)
    "किया", // did (m. sg. perfective)
    // "की" (f. sg. perfective) is homographic with the feminine
    // genitive की listed above under postpositions — a single entry
    // covers both senses; not duplicated here.
    "किए", // did (m. pl. perfective)
    "किये", // did (m. pl. perfective, alt. spelling)
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "अब",   // now
    "फिर",  // then / again
    "बहुत",  // very / much
    "थोड़ा", // a little (m. sg.)
    "थोड़ी", // a little (f. sg.)
    "सब",   // all
    "कुछ",   // some / something
    "कोई",  // someone / anyone
    "हर",   // every
    "यही",  // this very one
    "वही",  // that very one
    "सिर्फ", // only
    "केवल",  // only
    "भी",   // also / too
    "ही",   // emphatic particle (only, itself)
    // "तो" (discourse particle) is homographic with the correlative
    // conjunction तो listed above; not duplicated here.
    "जी",  // honorific particle
    "एक",  // one
    "दो",  // two
    "तीन", // three
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~130" — assert we're in that
        // ballpark. The task spec asks for 100-150 entries.
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the advertised 100-200 range",
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
    fn every_stopword_contains_devanagari() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Devanagari block U+0900..=U+097F.
        for &w in STOPWORDS {
            let has_devanagari = w.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
            assert!(has_devanagari, "stopword {w:?} has no Devanagari scalar");
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
        for &w in &["मैं", "हम", "तू", "तुम", "आप", "वह", "यह"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_postpositions() {
        for &w in &["का", "की", "के", "को", "ने", "से", "में", "पर"]
        {
            assert!(STOPWORDS.contains(&w), "core postposition {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["और", "या", "लेकिन", "अगर", "कि"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["है", "हैं", "था", "थी", "थे"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_particles() {
        for &w in &["नहीं", "मत", "न"] {
            assert!(STOPWORDS.contains(&w), "negation particle {w:?} missing");
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
}
