//! The Korean stopword list.
//!
//! ~60 common Korean function words drawn from the intersection of
//! several well-known open Korean IR stopword lists (the KAIST Korean
//! stopword collection, common Korean NLP toolkit `komoran` /
//! `mecab-ko` inventories). The list targets demonstratives,
//! interrogatives, common conjunctions, and the highest-frequency
//! adverbs and quantifiers. It deliberately omits the case particles
//! (`은/는/이/가/을/를/에/에서/…`) because those are attached at the
//! syllable end and are stripped by the stemmer, not filtered as
//! whole-word stopwords.
//!
//! # Coverage rationale
//!
//! - **Demonstratives.** The 이 / 그 / 저 series (this / that /
//!   yonder), including the free-standing pronoun forms `이것`, `그것`,
//!   `저것` and their determiner forms `이`, `그`, `저`.
//! - **Interrogatives.** `누구` who, `무엇` / `뭐` what, `어디` where,
//!   `언제` when, `어떻게` how, `왜` why.
//! - **Conjunctions.** `그리고` and, `하지만` but, `그러나` however,
//!   `또는` or, `그래서` so, `따라서` therefore.
//! - **Common adverbs / quantifiers.** `매우` very, `아주` very,
//!   `너무` too, `모든` all, `많은` many, `조금` a little, `잘` well,
//!   `또` again, `다시` again, `이미` already.
//! - **Copula / auxiliary function words.** `있다` to be / exist,
//!   `없다` to not be / not exist, `되다` to become, `하다` to do — in
//!   their dictionary forms.
//!
//! # Non-goals
//!
//! - **Case particles as free words.** Korean particles like `은/는`
//!   attach directly to the preceding noun with no space and are not
//!   pronounced as separate words. The stemmer strips them from the
//!   syllable end; they are not carried as stopword list entries.
//! - **Verb-conjugation stopwords.** The polite endings `-습니다` /
//!   `-습니까` / `-어요` / `-아요` and the many other conjugated verb
//!   suffixes are not stopwords — they attach to verb stems. Stripping
//!   them is the stemmer's job.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Korean stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is Hangul (or ASCII when the
/// word is habitually written in Latin script, which does not happen in
/// this list — every entry is Hangul).
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Demonstratives (지시 대명사 / 지시 관형사).
    // -----------------------------------------------------------------
    "이",   // this (determiner)
    "그",   // that (determiner)
    "저",   // yonder / that over there (determiner)
    "이것", // this (pronoun)
    "그것", // that (pronoun)
    "저것", // that over there (pronoun)
    "여기", // here
    "거기", // there
    "저기", // over there
    // -----------------------------------------------------------------
    // Interrogatives (의문사).
    // -----------------------------------------------------------------
    "누구",   // who
    "무엇",   // what
    "뭐",     // what (short form)
    "어디",   // where
    "언제",   // when
    "어떻게", // how
    "왜",     // why
    "어느",   // which
    "얼마",   // how much
    // -----------------------------------------------------------------
    // Common bound nouns / dependent nouns treated as noise.
    // -----------------------------------------------------------------
    "것", // thing (bound noun)
    "수", // possibility / way (as in `할 수 있다`)
    "등", // etc.
    "때", // time / when
    "곳", // place
    "말", // word / speech
    "일", // work / matter
    // -----------------------------------------------------------------
    // Conjunctions (접속사).
    // -----------------------------------------------------------------
    "그리고", // and
    "하지만", // but
    "그러나", // however
    "그런데", // however / by the way
    "그래서", // so / therefore
    "따라서", // therefore / accordingly
    "또는",   // or
    "혹은",   // or
    "및",     // and (formal)
    "즉",     // that is / namely
    "만약",   // if
    // -----------------------------------------------------------------
    // Adverbs / quantifiers.
    // -----------------------------------------------------------------
    "매우", // very
    "아주", // very
    "너무", // too / very
    "정말", // really
    "진짜", // really
    "잘",   // well
    "다시", // again
    "또",   // also / again
    "이미", // already
    "아직", // still / yet
    "곧",   // soon
    "먼저", // first
    "함께", // together
    "다",   // all
    "모든", // all / every
    "많은", // many
    "적은", // few
    "조금", // a little
    "약간", // slightly
    "거의", // almost
    "가장", // most
    "더",   // more
    // -----------------------------------------------------------------
    // High-frequency verb dictionary forms treated as function words.
    // -----------------------------------------------------------------
    "있다",   // to be / exist
    "없다",   // to not be / not exist
    "되다",   // to become
    "하다",   // to do
    "이다",   // to be (copula)
    "아니다", // to not be
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~60" — assert we're in the
        // ballpark. A wider band gives future maintainers room to
        // extend without immediately tripping the test.
        assert!(
            STOPWORDS.len() >= 50 && STOPWORDS.len() <= 100,
            "STOPWORDS.len() = {} outside the advertised 50-100 range",
            STOPWORDS.len(),
        );
    }

    #[test]
    fn every_stopword_is_non_empty() {
        for &w in STOPWORDS {
            assert!(!w.is_empty(), "empty stopword entry");
        }
    }

    #[test]
    fn stopwords_contain_core_demonstratives() {
        for &w in &["이", "그", "저", "이것", "그것"] {
            assert!(STOPWORDS.contains(&w), "core demonstrative {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["그리고", "하지만", "또는"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) over ~60 entries is fine for a static-list check.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
