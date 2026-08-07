//! The Hebrew stopword list.
//!
//! ~130 common Hebrew function words drawn from the intersection of
//! several well-known open lists (the NLTK Hebrew stopword list and the
//! `MILA` / `HebMorph` function-word inventory). The list targets
//! prepositions, conjunctions, coordinating and subordinating
//! particles, demonstrative and personal pronouns, high-frequency
//! forms of the copula *to be* (`להיות`), the negation particles, and
//! the most-common interrogatives; it deliberately omits content words.
//!
//! # Right-to-left script — a note on storage order
//!
//! Every entry here is written in **logical order** — the order in
//! which the scalars appear in the UTF-8 encoded byte stream, which is
//! *first-consonant-first* regardless of how the string is displayed.
//! Rust source files, `str::eq_ignore_ascii_case`, and the default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) all work
//! on logical order; RTL rendering is a display-layer concern that
//! affects only what a human sees on screen, not what the byte
//! comparison sees.
//!
//! # Case folding
//!
//! Hebrew has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Hebrew scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Hebrew input. This is the
//! right behavior — comparing Hebrew function words byte-for-byte
//! against the list is what a stopword filter wants.
//!
//! # Niqqud, cantillation, and orthographic variants
//!
//! Hebrew input in the wild comes with several orthographic
//! uncertainties the stopword list does not enumerate:
//!
//! - **Niqqud (vowel points).** Fully-pointed text (Biblical, poetic,
//!   pedagogical) writes vowels with combining marks
//!   (U+05B0..=U+05BC, U+05BF, U+05C1..=U+05C2, U+05C4..=U+05C7);
//!   newswire and web text almost always strips them. The stopword
//!   list uses the *unpointed* surface form only. Callers whose input
//!   may carry niqqud should run [`crate::normalize::normalize`]
//!   before consulting the stopword list.
//! - **Te'amim (cantillation marks).** Biblical text carries
//!   ~30 combining marks in U+0591..=U+05AF that annotate chant.
//!   Same rule: normalize before consulting.
//! - **Final letter forms.** `ך ם ן ף ץ` (U+05DA, U+05DD, U+05DF,
//!   U+05E3, U+05E5) are the terminal-position variants of `כ מ נ פ צ`.
//!   The stopword list uses the correct final-form spelling for each
//!   entry (Hebrew orthography is strict about position); a caller
//!   whose input has been folded via
//!   [`HebrewNormalizer::with_final_form_folding`](crate::normalize::HebrewNormalizer::with_final_form_folding)
//!   would find the stopword lookup miss its normalized entries, so
//!   apply final-form folding *after* the stopword check or normalize
//!   the stopword list itself.
//!
//! # Prefix particles — a note on set semantics
//!
//! Hebrew's coordinating particles (`ו-` "and-", `ב-` "in-",
//! `כ-` "like-", `ל-` "to-", `מ-` "from-", `ש-` "that-") and the
//! definite article (`ה-` "the-") are written *fused* to the following
//! word: `והספר` ("and-the-book") is one orthographic word but three
//! morphemes. This list does *not* enumerate every prefix combination
//! — that is a combinatorial explosion best handled by the light
//! stemmer's prefix-strip pass. The list carries the *bare* function
//! words plus a small handful of extremely-high-frequency fused forms
//! (`של` "of", `את` accusative marker, `אין` "there is not") where the
//! fused form is the canonical dictionary headword.
//!
//! # Coverage rationale
//!
//! - **Prepositions** (`של`, `על`, `אל`, `עם`, `בין`, `אצל`, `לפני`,
//!   `אחרי`, `תחת`).
//! - **Coordinating conjunctions** (`ו`, `או`, `אבל`, `אך`, `רק`, `וגם`).
//! - **Subordinating conjunctions** (`אם`, `כי`, `כאשר`, `לפי`, `אלא`,
//!   `כמו`, `כדי`).
//! - **Demonstrative pronouns** (`זה`, `זאת`, `אלה`, `הזה`, `ההיא`).
//! - **Personal pronouns** (`אני`, `אתה`, `את`, `הוא`, `היא`, `אנחנו`,
//!   `אתם`, `אתן`, `הם`, `הן`).
//! - **Interrogatives** (`מה`, `מי`, `איפה`, `מתי`, `איך`, `כמה`,
//!   `למה`, `האם`).
//! - **Negation** (`לא`, `אל`, `אין`).
//! - **Common "to be" surface forms** (`היה`, `היתה`, `היו`, `יהיה`,
//!   `יהיו`, `יש`, `אינו`, `אינה`).
//! - **Adverbs and quantifiers** (`כל`, `רק`, `מאוד`, `גם`, `עוד`,
//!   `כבר`, `אולי`, `בטח`).
//!
//! # Non-goals
//!
//! - **Fully-pointed entries.** The list stores the unpointed surface
//!   form only; input with niqqud should be passed through
//!   [`crate::normalize::normalize`] before the stopword check.
//! - **Yiddish / Ladino / Judeo-Aramaic.** These languages use the
//!   Hebrew script but have distinct grammars and function-word
//!   inventories. They belong in their own `stringcheese-yi` /
//!   `stringcheese-lad` / `stringcheese-jrb` packs.
//! - **Biblical-specific stopwords.** Biblical Hebrew has function
//!   words (`הנה`, `אשר`, particles like `נא`) that appear less
//!   commonly in Modern Hebrew corpora. A `stringcheese-he-biblical`
//!   pack would extend the list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Hebrew stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice.
///
/// Entries are stored in *logical* (byte) order; RTL rendering is a
/// display-layer concern.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Prepositions.
    // -----------------------------------------------------------------
    "של",   // of
    "על",   // on / about
    "אל",   // to / toward
    "את",   // (definite direct object marker)
    "עם",   // with
    "בין",  // between
    "אצל",  // at / by (someone)
    "לפני", // before
    "אחרי", // after
    "תחת",  // under
    "מעל",  // above
    "ליד",  // next to
    "מול",  // opposite / in front of
    "בלי",  // without
    "עד",   // until
    "מן",   // from
    "בתוך", // inside
    "מתוך", // out of
    // -----------------------------------------------------------------
    // Coordinating conjunctions.
    // -----------------------------------------------------------------
    "ו",   // and (bare particle — usually fused as prefix)
    "או",  // or
    "אבל", // but
    "אך",  // but / only
    "אלא", // rather / but
    "וגם", // and also
    // -----------------------------------------------------------------
    // Subordinating conjunctions and complementizers.
    // -----------------------------------------------------------------
    "אם",    // if
    "כי",    // because / that
    "כאשר",  // when
    "כש",    // when- (fused prefix form)
    "לפי",   // according to
    "כמו",   // like / as
    "כדי",   // in order to
    "בגלל",  // because of
    "למרות", // despite
    "אף",    // even / also
    "אפילו", // even
    // -----------------------------------------------------------------
    // Demonstrative pronouns.
    // -----------------------------------------------------------------
    "זה",   // this (m.sg.)
    "זאת",  // this (f.sg.)
    "זו",   // this (f.sg. alt)
    "אלה",  // these
    "אלו",  // these (alt)
    "הזה",  // this (m.sg. with article)
    "הזאת", // this (f.sg. with article)
    "ההוא", // that (m.sg.)
    "ההיא", // that (f.sg.)
    "ההם",  // those (m.pl.)
    "ההן",  // those (f.pl.)
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "אני",   // I
    "אתה",   // you (m.sg.) — homograph with את (acc.) already listed above under prepositions
    "הוא",   // he
    "היא",   // she
    "אנחנו", // we
    "אנו",   // we (formal)
    "אתם",   // you (m.pl.)
    "אתן",   // you (f.pl.)
    "הם",    // they (m.)
    "הן",    // they (f.)
    // -----------------------------------------------------------------
    // Object-suffixed pronouns (attached-form dictionary entries).
    // -----------------------------------------------------------------
    "אותו",  // him
    "אותה",  // her
    "אותם",  // them (m.)
    "אותן",  // them (f.)
    "אותי",  // me
    "אותך",  // you (acc.)
    "אותנו", // us
    "אתכם",  // you all (acc. m.pl.)
    // -----------------------------------------------------------------
    // Possessive-suffixed forms of של.
    // -----------------------------------------------------------------
    "שלי",  // mine
    "שלך",  // yours (sg.)
    "שלו",  // his
    "שלה",  // hers
    "שלנו", // ours
    "שלכם", // yours (m.pl.)
    "שלכן", // yours (f.pl.)
    "שלהם", // theirs (m.)
    "שלהן", // theirs (f.)
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "מה",   // what
    "מי",   // who
    "איפה", // where
    "היכן", // where (formal)
    "מתי",  // when
    "איך",  // how
    "כמה",  // how many / how much
    "למה",  // why
    "מדוע", // why (formal)
    "האם",  // (yes/no question particle)
    "איזה", // which (m.)
    "איזו", // which (f.)
    // -----------------------------------------------------------------
    // Negation and modality particles.
    // -----------------------------------------------------------------
    "לא",   // no / not
    "אין",  // there is not
    "אינו", // is not (m.sg.)
    "אינה", // is not (f.sg.)
    "אינם", // are not (m.pl.)
    "אינן", // are not (f.pl.)
    "אסור", // forbidden / must not
    // -----------------------------------------------------------------
    // Common surface forms of the copula "to be".
    // -----------------------------------------------------------------
    "היה",   // was (m.sg.)
    "היתה",  // was (f.sg.)
    "היינו", // we were
    "הייתם", // you-mp were
    "היו",   // they were
    "יהיה",  // will be (m.sg.)
    "תהיה",  // will be (f.sg.)
    "יהיו",  // will be (pl.)
    "יש",    // there is / has
    "להיות", // to be (infinitive)
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "כל",     // all / every
    "רק",     // only
    "מאוד",   // very
    "גם",     // also
    "עוד",    // still / more
    "כבר",    // already
    "אולי",   // maybe
    "בטח",    // certainly
    "בוודאי", // certainly
    "פה",     // here
    "שם",     // there
    "כאן",    // here
    "עכשיו",  // now
    "היום",   // today
    "אתמול",  // yesterday
    "מחר",    // tomorrow
    "תמיד",   // always
    "לפעמים", // sometimes
    "לעולם",  // never / forever
    "יותר",   // more
    "פחות",   // less
    "הכי",    // most (superlative particle)
    "כך",     // so / thus
    "לכן",    // therefore
    "אז",     // then
    // -----------------------------------------------------------------
    // "Yes" / "no" / discourse markers.
    // -----------------------------------------------------------------
    "כן",   // yes
    "טוב",  // good / well (discourse)
    "בסדר", // OK
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~130" — assert we're in that
        // ballpark. Has grown slightly during hand-curation.
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
    fn stopwords_contain_core_prepositions() {
        for &w in &["של", "על", "אל", "עם", "בין"] {
            assert!(STOPWORDS.contains(&w), "core preposition {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_personal_pronouns() {
        for &w in &["אני", "אתה", "הוא", "היא", "אנחנו", "הם"] {
            assert!(STOPWORDS.contains(&w), "personal pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_demonstratives() {
        for &w in &["זה", "זאת", "אלה", "הזה"] {
            assert!(STOPWORDS.contains(&w), "demonstrative {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["ו", "או", "אבל", "כי"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["היה", "יהיה", "יש", "להיות"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation() {
        for &w in &["לא", "אין"] {
            assert!(STOPWORDS.contains(&w), "negation {w:?} missing");
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
