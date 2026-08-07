//! The Chinese (Simplified) stopword list.
//!
//! ~80 common Modern Standard Chinese function words drawn from the
//! intersection of several well-known open lists (the Harbin Institute
//! of Technology / HIT stopword list, the Baidu stopword list, and the
//! subset of function characters that jieba and thulac ship in their
//! own filter files). The list targets structural particles (助词),
//! pronouns, high-frequency conjunctions, adverbs, and the most common
//! adpositions; it deliberately omits content words.
//!
//! # Simplified vs Traditional
//!
//! Entries are written in **Simplified Chinese** (`简` not `簡`, `国`
//! not `國`, `们` not `們`). A Traditional-facing pack would use
//! `stringcheese-zh-hant` and carry its own list. Callers who need to
//! recognise both forms should compose an S↔T converter (deferred) or
//! ship both `stringcheese-zh` and `stringcheese-zh-hant`.
//!
//! # Character-type composition
//!
//! Chinese function words are almost universally one or two
//! Han characters (`的`, `了`, `是`, `我们`, `什么`, `因为`, …).
//! The list contains no Latin and no ASCII outside a couple of
//! punctuation-adjacent entries. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation is ASCII-case-insensitive byte compare — for CJK
//! entries the ASCII case-fold is a no-op, which is the right
//! behaviour (Chinese has no case).
//!
//! # Coverage rationale
//!
//! - **Structural particles (结构助词).** `的`, `地`, `得`, `了`, `着`,
//!   `过`, `们`.
//! - **Pronouns.** The personal series (`我`, `你`, `他`, `她`, `它`,
//!   `我们`, `你们`, `他们`, `她们`, `它们`, `咱们`, `自己`) and the
//!   demonstrative pair (`这`, `那`, plus `这个` / `那个` / `这些` /
//!   `那些`).
//! - **Copula and existentials.** `是`, `有`, `没有`, `不是`, `就是`.
//! - **Common function words.** `不`, `也`, `都`, `很`, `再`, `就`,
//!   `还`, `又`, `只`, `只是`.
//! - **Conjunctions.** `和`, `与`, `或`, `但`, `但是`, `而`, `而且`,
//!   `虽然`, `因为`, `所以`, `如果`, `因此`, `不过`, `然后`, `或者`.
//! - **Prepositions.** `在`, `从`, `到`, `向`, `对`, `把`, `被`, `跟`,
//!   `给`, `为`, `为了`, `按`, `按照`, `根据`, `关于`, `除了`, `由`.
//! - **Interrogatives.** `什么`, `哪`, `哪里`, `谁`, `怎么`, `多少`,
//!   `为什么`.
//! - **Adverbs of degree / time / manner (high-frequency subset).**
//!   `很`, `太`, `更`, `最`, `已经`, `马上`, `刚`, `现在`.
//! - **Sentence-final particles.** `吗`, `呢`, `吧`, `啊`, `呀`, `哦`,
//!   `啦`, `嘛`.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Sentence-boundary punctuation.** Chinese full-width period
//!   `。`, comma `，`, and enumeration comma `、` are handled by the
//!   tokenizer as separators; they are not stopwords.
//! - **Dictionary-inferred multi-character stopwords.** A jieba-lite
//!   dictionary layer would expose more legitimate multi-character
//!   stopwords (`然而`, `因而`, `不但…而且`) as a byproduct of
//!   segmentation, but the base pack ships no segmenter, so only
//!   entries recognizable at the character-transition boundary appear
//!   here.

/// The Chinese (Simplified) stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Structural particles (结构助词) — mostly single characters.
    // -----------------------------------------------------------------
    "的",
    "地",
    "得",
    "了",
    "着",
    "过",
    "们",
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "我",
    "你",
    "他",
    "她",
    "它",
    "我们",
    "你们",
    "他们",
    "她们",
    "它们",
    "咱们",
    "自己",
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "这",
    "那",
    "这个",
    "那个",
    "这些",
    "那些",
    // -----------------------------------------------------------------
    // Copula and existentials.
    // -----------------------------------------------------------------
    "是",
    "有",
    "没",
    "没有",
    "不是",
    "就是",
    // -----------------------------------------------------------------
    // High-frequency function adverbs / negators.
    // -----------------------------------------------------------------
    "不",
    "也",
    "都",
    "很",
    "再",
    "就",
    "还",
    "又",
    "只",
    "只是",
    // -----------------------------------------------------------------
    // Conjunctions.
    // -----------------------------------------------------------------
    "和",
    "与",
    "或",
    "或者",
    "但",
    "但是",
    "而",
    "而且",
    "虽然",
    "因为",
    "所以",
    "如果",
    "因此",
    "不过",
    "然后",
    // -----------------------------------------------------------------
    // Prepositions / coverbs.
    // -----------------------------------------------------------------
    "在",
    "从",
    "到",
    "向",
    "对",
    "把",
    "被",
    "跟",
    "给",
    "为",
    "为了",
    "按",
    "按照",
    "根据",
    "关于",
    "除了",
    "由",
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "什么",
    "哪",
    "哪里",
    "谁",
    "怎么",
    "多少",
    "为什么",
    // -----------------------------------------------------------------
    // Adverbs of degree / time (high-frequency subset).
    // -----------------------------------------------------------------
    "太",
    "更",
    "最",
    "已经",
    "现在",
    // -----------------------------------------------------------------
    // Sentence-final and modal particles.
    // -----------------------------------------------------------------
    "吗",
    "呢",
    "吧",
    "啊",
    "呀",
    "哦",
    "啦",
    "嘛",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~80" — assert we're in the
        // 50-120 ballpark.
        assert!(
            STOPWORDS.len() >= 50 && STOPWORDS.len() <= 120,
            "STOPWORDS.len() = {} outside the advertised 50-120 range",
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
    fn stopwords_contain_core_particles() {
        // The core structural particles — any pack author reordering
        // the list should trip this test if they lose one accidentally.
        for &w in &["的", "了", "是", "在", "有", "我", "和", "不"] {
            assert!(STOPWORDS.contains(&w), "core function word {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_pronouns() {
        for &w in &["我", "你", "他", "她", "它", "我们"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~80.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
