//! The Vietnamese stopword list.
//!
//! Roughly 180 common Vietnamese *syllables* (Vietnamese orthography
//! writes every syllable as a whitespace-delimited word — see the
//! module doc on the tokenizer for the reason) drawn from the union
//! of the `stopwords-iso` Vietnamese list and NLTK-adjacent Southeast
//! Asian function-word inventories, augmented with the copula `là`
//! (to be), the possessive marker `của` (of), the tense / aspect
//! markers (`đã` / `đang` / `sẽ`), the most-common classifiers
//! (`cái`, `con`, `chiếc`, `quyển`, `cuốn`), and the personal
//! pronouns.
//!
//! # One entry = one syllable
//!
//! Vietnamese multi-syllable compounds like `chúng tôi` "we",
//! `bởi vì` "because", `tất cả` "all" are written as two
//! whitespace-separated syllables in the standard orthography, and
//! the [`VietnameseTokenizer`](crate::VietnameseTokenizer) emits
//! them as two tokens. This list therefore stores every entry as a
//! single syllable — the multi-syllable stopword expressions are
//! covered by their component syllables, each of which is on the
//! list independently.
//!
//! # Accented characters
//!
//! Vietnamese surface text carries an extensive two-layer diacritic
//! system:
//!
//! * **Letter modifiers** (part of the letter's identity): `ă â đ ê
//!   ô ơ ư`.
//! * **Tone marks** (applied to any vowel): grave `à`, acute `á`,
//!   hook-above `ả`, tilde `ã`, dot-below `ạ`.
//!
//! The list stores every entry in its **exact Vietnamese spelling**,
//! including diacritics, in the **NFC** (precomposed) form that the
//! web overwhelmingly delivers. The default trait-level
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`], which does not
//! fold non-ASCII accents; the [`Vietnamese`](crate::Vietnamese) trait
//! implementation therefore **overrides** `is_stopword` to apply
//! [`char::to_lowercase`] (Unicode) before comparison, so uppercase
//! Vietnamese queries like `VÀ`, `ĐƯỢC`, `NHỮNG` all match the plain
//! lowercase list entries.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.
//!   Downstream applications should carry their own.
//! - **Toneless spellings.** The list stores tone-marked entries in
//!   their canonical Vietnamese form. Callers who work with
//!   toneless text (some fuzzy-search pipelines) should apply
//!   [`crate::normalize::VietnameseNormalizer::with_strip_tone_marks`]
//!   to both the query and the stopword list at lookup time — the
//!   list itself is not toneless-folded because Vietnamese-typed
//!   text is overwhelmingly tone-marked in practice.
//! - **Regional variants.** No dialect-specific entries — Northern /
//!   Central / Southern regional vocabulary variation is out of
//!   scope for the general list.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with a Unicode-aware lowercase pass, so
//!   `"và"`, `"Và"`, `"VÀ"`, and any other case variant all resolve
//!   to the same list entry.

/// The Vietnamese stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns (single-syllable; multi-syllable compounds
    // like `chúng tôi` are covered by the individual syllables).
    // -----------------------------------------------------------------
    "tôi",
    "tao",
    "tớ",
    "mình",
    "ta",
    "chúng",
    "bạn",
    "cậu",
    "mày",
    "các",
    "nó",
    "họ",
    "hắn",
    "y",
    "ông",
    "bà",
    "anh",
    "chị",
    "em",
    "cô",
    "chú",
    "bác",
    // -----------------------------------------------------------------
    // Possessives and demonstratives.
    // -----------------------------------------------------------------
    "của",
    "này",
    "nay",
    "đây",
    "đó",
    "kia",
    "nọ",
    "ấy",
    "vậy",
    "thế",
    "như",
    // -----------------------------------------------------------------
    // Copula and auxiliary verbs.
    // -----------------------------------------------------------------
    "là",
    "có",
    "được",
    "phải",
    "cần",
    "nên",
    "muốn",
    "định",
    "làm",
    "cho",
    "bị",
    "chịu",
    // -----------------------------------------------------------------
    // Tense / aspect / modality particles.
    // -----------------------------------------------------------------
    "đã",
    "đang",
    "sẽ",
    "vừa",
    "mới",
    "sắp",
    "từng",
    "chưa",
    "còn",
    "hết",
    "xong",
    "rồi",
    // -----------------------------------------------------------------
    // Negation and interrogatives.
    // -----------------------------------------------------------------
    "không",
    "chẳng",
    "chả",
    "đừng",
    "chớ",
    "gì",
    "ai",
    "nào",
    "sao",
    "đâu",
    "bao",
    "nhiêu",
    "khi",
    "tại",
    // -----------------------------------------------------------------
    // Prepositions and directional / locative words.
    // -----------------------------------------------------------------
    "trong",
    "ngoài",
    "trên",
    "dưới",
    "trước",
    "sau",
    "giữa",
    "bên",
    "cạnh",
    "gần",
    "xa",
    "từ",
    "đến",
    "tới",
    "về",
    "qua",
    "theo",
    "với",
    "bằng",
    "ở",
    "vào",
    "ra",
    "lên",
    "xuống",
    // -----------------------------------------------------------------
    // Conjunctions and connectives.
    // -----------------------------------------------------------------
    "và",
    "hoặc",
    "hay",
    "nhưng",
    "mà",
    "nếu",
    "lúc",
    "vì",
    "bởi",
    "do",
    "để",
    "tuy",
    "dù",
    "dẫu",
    "mặc",
    "song",
    "thì",
    "rằng",
    // -----------------------------------------------------------------
    // Classifiers (common noun-class markers).
    // -----------------------------------------------------------------
    "cái",
    "con",
    "chiếc",
    "quyển",
    "cuốn",
    "tờ",
    "bức",
    "cây",
    "quả",
    "trái",
    "hạt",
    "củ",
    "người",
    "vị",
    "ngôi",
    "tòa",
    // -----------------------------------------------------------------
    // Quantifiers.
    // -----------------------------------------------------------------
    "một",
    "hai",
    "ba",
    "nhiều",
    "ít",
    "vài",
    "mỗi",
    "cả",
    "toàn",
    "tất",
    "mọi",
    "những",
    "số",
    "bộ",
    "thể",
    // -----------------------------------------------------------------
    // Adverbs / discourse markers / degrees.
    // -----------------------------------------------------------------
    "rất",
    "quá",
    "khá",
    "hơi",
    "cực",
    "hơn",
    "kém",
    "nhất",
    "cũng",
    "đều",
    "chỉ",
    "thôi",
    "vẫn",
    "luôn",
    "thường",
    "đôi",
    "ngay",
    "liền",
    "lại",
    "nữa",
    "thêm",
    "hầu",
    // -----------------------------------------------------------------
    // Sentence-final / hedging particles.
    // -----------------------------------------------------------------
    "à",
    "ạ",
    "nhé",
    "nha",
    "nhỉ",
    "đấy",
    "chứ",
    "cơ",
    // -----------------------------------------------------------------
    // Yes / no / affirmation.
    // -----------------------------------------------------------------
    "vâng",
    "dạ",
    "ừ",
    // -----------------------------------------------------------------
    // High-frequency verbs of communication / cognition often used as
    // function words.
    // -----------------------------------------------------------------
    "nói",
    "bảo",
    "biết",
    "nghĩ",
    "tưởng",
    "thấy",
    "gọi",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        let n = STOPWORDS.len();
        assert!(
            (150..=250).contains(&n),
            "STOPWORDS.len() = {n} outside the advertised ~180 range",
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
        // O(n^2) is fine for a static list of ~180.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn every_entry_is_a_single_syllable() {
        // Vietnamese multi-syllable compounds are two whitespace-
        // separated words in the orthography; the list stores each
        // syllable as its own entry rather than a joined form.
        for &w in STOPWORDS {
            assert!(!w.contains(' '), "stopword {w:?} contains whitespace");
            assert!(!w.contains('_'), "stopword {w:?} contains an underscore");
        }
    }

    #[test]
    fn common_prepositions_and_connectives_are_present() {
        for w in [
            "và", "hoặc", "nhưng", "mà", "nếu", "vì", "để", "trong", "trên", "dưới", "với", "của",
            "từ", "đến",
        ] {
            assert!(
                STOPWORDS.contains(&w),
                "preposition/conjunction {w:?} is missing"
            );
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["tôi", "bạn", "nó", "họ", "chúng", "ta"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_and_aux_are_present() {
        for w in ["là", "có", "được", "đã", "đang", "sẽ", "không"] {
            assert!(STOPWORDS.contains(&w), "aux/copula {w:?} is missing");
        }
    }

    #[test]
    fn common_classifiers_are_present() {
        for w in ["cái", "con", "chiếc", "quyển", "cuốn", "người"] {
            assert!(STOPWORDS.contains(&w), "classifier {w:?} is missing");
        }
    }

    #[test]
    fn diacritic_carrying_entries_are_stored_with_diacritics() {
        // Confirm the list actually carries the Vietnamese diacritics.
        for w in ["và", "được", "những", "đã", "đang", "sẽ"] {
            assert!(STOPWORDS.contains(&w), "diacritic entry {w:?} is missing");
        }
        // Look for at least a few obviously diacritic-carrying entries.
        assert!(STOPWORDS.iter().any(|w| w.contains('à')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ư')));
        assert!(STOPWORDS.iter().any(|w| w.contains('đ')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ê')));
        assert!(STOPWORDS.iter().any(|w| w.contains('ơ')));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn all_entries_are_nfc_form() {
        // Every Vietnamese entry ships in NFC (precomposed) form.
        // We check that by round-tripping through NFC and asserting
        // string equality — if any entry were in NFD form, the NFC
        // pass would rewrite it.
        extern crate alloc;
        use unicode_normalization::UnicodeNormalization;
        for &w in STOPWORDS {
            let nfc: alloc::string::String = w.nfc().collect();
            assert_eq!(nfc, w, "stopword {w:?} is not stored in NFC form");
        }
    }
}
