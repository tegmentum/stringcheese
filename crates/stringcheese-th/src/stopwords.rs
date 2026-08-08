//! The Thai stopword list.
//!
//! ~55 common Thai function words drawn from the intersection of
//! several open Thai NLP stopword lists (`PyThaiNLP`'s default list, the
//! Thai NECTEC stopword set, and the subset of tokens the ICU Thai
//! break-iterator's post-processing normalizes away in IR pipelines).
//! The list targets pronouns, conjunctions, high-frequency
//! prepositions, sentence-final particles, and the copula / auxiliary
//! forms of `เป็น` "to be" and `ได้` "to be able / to receive"; it
//! deliberately omits content words.
//!
//! # Script and case
//!
//! Every entry is written in **Thai script** — Thai has **no case**
//! (there is no uppercase / lowercase distinction) and every scalar in
//! the block U+0E00..=U+0E7F is 3 bytes in UTF-8 (falling in the
//! U+0800..=U+FFFF three-byte range). The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) is
//! ASCII-case-insensitive byte-compare, which is a no-op on Thai
//! scalars and is therefore the right behaviour.
//!
//! # Coverage rationale
//!
//! - **Personal pronouns.** `ฉัน` (I, informal-fem.), `ผม` (I,
//!   informal-masc.), `เรา` (we / I, colloquial), `คุณ` (you, polite),
//!   `เขา` (he / she / they), `มัน` (it).
//! - **Demonstratives.** `นี้` (this), `นั้น` (that), `นี่` (this-one),
//!   `โน้น` (over-there).
//! - **Interrogatives.** `อะไร` (what), `ทำไม` (why), `ใคร` (who),
//!   `เมื่อไร` (when), `ที่ไหน` (where), `อย่างไร` (how).
//! - **Conjunctions.** `และ` (and), `แต่` (but), `หรือ` (or), `ถ้า`
//!   (if), `เพราะ` (because), `เพื่อ` (in order to), `แล้ว` (then).
//! - **Prepositions.** `ใน` (in), `บน` (on), `ที่` (at / that /
//!   which), `ของ` (of), `จาก` (from), `ถึง` (to / until), `กับ`
//!   (with), `โดย` (by), `เพื่อ` (for), `ตาม` (according-to).
//! - **Verbs / copulae as function words.** `เป็น` (to be),
//!   `ไม่` (not — the primary negator), `ไม่ใช่` (is not),
//!   `ให้` (to give / for), `ได้` (to be able / receive), `จะ`
//!   (future marker), `กำลัง` (progressive marker), `แล้ว`
//!   (already / perfective).
//! - **Motion / directional light verbs.** `ไป` (to go), `มา` (to
//!   come).
//! - **Sentence-final particles.** `นะ`, `ครับ`, `ค่ะ`, `คะ`, `จ้ะ`,
//!   `จ๊ะ`.
//! - **Quantifiers / degree adverbs.** `มาก` (very / much), `น้อย`
//!   (little), `บาง` (some), `ทุก` (every), `หลาย` (several).
//!
//! # Non-goals
//!
//! - **Classifiers (ลักษณนาม).** Thai uses hundreds of classifiers
//!   (`คน` "person", `ตัว` "animal / thing", `อัน` "item", `เครื่อง`
//!   "machine", …). They are content-adjacent function words that
//!   change meaning by nominal class; a downstream IR pipeline that
//!   wants to strip them should carry its own list.
//! - **Numerals.** `หนึ่ง` (one), `สอง` (two), … are content when they
//!   appear as counted amounts; leaving them out of the stopword list
//!   preserves their information for search.
//! - **Sentence-boundary punctuation.** Thai uses the ASCII space as
//!   a phrase / sentence break (see [`crate::tokenizer`]); it is not
//!   a stopword.
//! - **Dictionary-inferred multi-word stopwords.** A future
//!   `stringcheese-th-newmm` sibling with a dictionary would expose
//!   more legitimate multi-syllable stopwords (`ทั้งหมด`, `เช่นเดียวกัน`)
//!   as a byproduct of segmentation; the base pack ships no
//!   dictionary, so multi-syllable entries appear here only if they
//!   are commonly whitespace-adjacent or single-cluster in surface
//!   Thai.

/// The Thai stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "ฉัน",
    "ผม",
    "เรา",
    "คุณ",
    "เขา",
    "มัน",
    "ท่าน",
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "นี้",
    "นั้น",
    "นี่",
    "โน้น",
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "อะไร",
    "ทำไม",
    "ใคร",
    "เมื่อไร",
    "ที่ไหน",
    "อย่างไร",
    // -----------------------------------------------------------------
    // Conjunctions.
    // -----------------------------------------------------------------
    "และ",
    "แต่",
    "หรือ",
    "ถ้า",
    "เพราะ",
    "แล้ว",
    // -----------------------------------------------------------------
    // Prepositions.
    // -----------------------------------------------------------------
    "ใน",
    "บน",
    "ที่",
    "ของ",
    "จาก",
    "ถึง",
    "กับ",
    "โดย",
    "เพื่อ",
    "ตาม",
    // -----------------------------------------------------------------
    // Copula / auxiliaries / negators / TAM markers.
    // -----------------------------------------------------------------
    "เป็น",
    "ไม่",
    "ไม่ใช่",
    "ให้",
    "ได้",
    "จะ",
    "กำลัง",
    "อยู่",
    // -----------------------------------------------------------------
    // Motion / directional light verbs.
    // -----------------------------------------------------------------
    "ไป",
    "มา",
    // -----------------------------------------------------------------
    // Sentence-final particles.
    // -----------------------------------------------------------------
    "นะ",
    "ครับ",
    "ค่ะ",
    "คะ",
    "จ้ะ",
    "จ๊ะ",
    // -----------------------------------------------------------------
    // Quantifiers / degree adverbs.
    // -----------------------------------------------------------------
    "มาก",
    "น้อย",
    "บาง",
    "ทุก",
    "หลาย",
    "ก็",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~55" — assert we're in the
        // 40-90 ballpark.
        assert!(
            STOPWORDS.len() >= 40 && STOPWORDS.len() <= 90,
            "STOPWORDS.len() = {} outside the advertised 40-90 range",
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
    fn stopwords_contain_core_function_words() {
        // The core function words — any pack author reordering the
        // list should trip this test if they lose one accidentally.
        for &w in &["และ", "แต่", "หรือ", "เป็น", "ที่", "ของ", "ใน", "ไม่"]
        {
            assert!(STOPWORDS.contains(&w), "core function word {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_pronouns() {
        for &w in &["ฉัน", "ผม", "คุณ", "เรา", "เขา"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~55.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
