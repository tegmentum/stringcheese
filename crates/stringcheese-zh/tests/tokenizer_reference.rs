//! [`ChineseTokenizer`] reference sentences with expected token lists.
//!
//! Reference pairs exercising the character-level rule for Han
//! characters, the same-type-run rule for Latin / digit sequences,
//! and the whitespace/punctuation separator handling. Every expected
//! token list was hand-derived from the tokenizer's stated rules and
//! is what the character-level tokenizer *should* produce
//! (deliberately coarse for Chinese; dictionary-driven word
//! segmentation is deferred to a follow-up sibling crate).

use stringcheese_zh::ChineseTokenizer;

/// Reference pairs — `(input, expected_tokens)`.
///
/// Each entry is derived from the tokenizer's rules by hand:
///
/// 1. Skip leading whitespace + punctuation.
/// 2. Han characters emit as their own one-scalar token; never merge.
/// 3. Latin / digit / Other runs stay together as one token.
const PAIRS: &[(&str, &[&str])] = &[
    // -----------------------------------------------------------------
    // Trivial empty and single-type inputs.
    // -----------------------------------------------------------------
    ("", &[]),
    ("hello", &["hello"]),
    ("123", &["123"]),
    // -----------------------------------------------------------------
    // Pure Han input — every character is its own token.
    // -----------------------------------------------------------------
    ("你好", &["你", "好"]),
    ("中国", &["中", "国"]),
    ("北京", &["北", "京"]),
    ("你好世界", &["你", "好", "世", "界"]),
    ("中国人", &["中", "国", "人"]),
    // -----------------------------------------------------------------
    // Whitespace and punctuation as separators.
    // -----------------------------------------------------------------
    ("你 好", &["你", "好"]),
    ("你好，世界。", &["你", "好", "世", "界"]),
    ("北京。上海", &["北", "京", "上", "海"]),
    ("你好！你好？", &["你", "好", "你", "好"]),
    ("　中　国　", &["中", "国"]), // ideographic space
    // -----------------------------------------------------------------
    // Mixed script — Han is character-split, Latin/digit runs stay
    // together.
    // -----------------------------------------------------------------
    ("中文hello", &["中", "文", "hello"]),
    ("hello中国", &["hello", "中", "国"]),
    ("2026年", &["2026", "年"]),
    ("中文hello123", &["中", "文", "hello", "123"]),
    ("abc123", &["abc", "123"]),
    ("Java中国", &["Java", "中", "国"]),
    // -----------------------------------------------------------------
    // Traditional Han still tokenizes character-by-character.
    // -----------------------------------------------------------------
    ("國家", &["國", "家"]),
    // -----------------------------------------------------------------
    // Full-width Latin letters and digits.
    // -----------------------------------------------------------------
    ("Ｈｅｌｌｏ", &["Ｈｅｌｌｏ"]),
    ("１２３", &["１２３"]),
];

#[test]
fn every_reference_pair_matches() {
    for (input, expected) in PAIRS {
        let actual: Vec<&str> = ChineseTokenizer::new().tokenize(input).collect();
        assert_eq!(
            actual, *expected,
            "input {input:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn reference_pair_count_is_within_the_advertised_range() {
    // The task requires at least 10 pairs; we ship more.
    assert!(
        PAIRS.len() >= 10 && PAIRS.len() <= 40,
        "PAIRS.len() = {} outside the advertised 10-40 range",
        PAIRS.len()
    );
}
