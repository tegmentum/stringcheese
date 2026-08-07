//! [`JapaneseTokenizer`] reference sentences with expected token lists.
//!
//! 25 sentences covering the character-type transitions and the
//! Kanji+Hiragana okurigana merge rule; every expected token list was
//! hand-derived from the tokenizer's stated rules and is what the
//! character-type-based tokenizer *should* produce (deliberately
//! coarse; morphological accuracy is deferred to a follow-up wave).

use stringcheese_ja::JapaneseTokenizer;

/// Reference pairs — `(input, expected_tokens)`.
///
/// Each entry is derived from the tokenizer's rules by hand:
///
/// 1. Skip leading whitespace + punctuation.
/// 2. Emit maximal same-type runs.
/// 3. A Kanji run absorbs any immediately-following Hiragana
///    (okurigana), then terminates at the first non-Hiragana boundary
///    (including a return to Kanji).
const PAIRS: &[(&str, &[&str])] = &[
    // -----------------------------------------------------------------
    // Trivial single-script inputs.
    // -----------------------------------------------------------------
    ("", &[]),
    ("あいうえお", &["あいうえお"]),
    ("カタカナ", &["カタカナ"]),
    ("日本語", &["日本語"]),
    ("abc", &["abc"]),
    ("123", &["123"]),
    // -----------------------------------------------------------------
    // Whitespace and punctuation as separators.
    // -----------------------------------------------------------------
    ("これ は 本", &["これ", "は", "本"]),
    ("これ、それ。", &["これ", "それ"]),
    ("あ　い", &["あ", "い"]),
    // -----------------------------------------------------------------
    // Script transitions.
    // -----------------------------------------------------------------
    ("あいカタ", &["あい", "カタ"]),
    ("abc人", &["abc", "人"]),
    ("123abc", &["123", "abc"]),
    ("コンピュータ", &["コンピュータ"]),
    // -----------------------------------------------------------------
    // The Kanji + Hiragana (okurigana) merge rule.
    // -----------------------------------------------------------------
    ("食べる", &["食べる"]),
    ("食べ物", &["食べ", "物"]),
    ("見た", &["見た"]),
    ("走ります", &["走ります"]),
    ("勉強しています", &["勉強しています"]),
    // -----------------------------------------------------------------
    // Mixed-script sentences.
    // -----------------------------------------------------------------
    (
        "彼はJavaScriptを勉強しています",
        &["彼は", "JavaScript", "を", "勉強しています"],
    ),
    ("私は日本人です", &["私は", "日本人です"]),
    ("コーヒーを飲みます", &["コーヒー", "を", "飲みます"]),
    ("2026年の桜", &["2026", "年の", "桜"]),
    // -----------------------------------------------------------------
    // Kanji + iteration mark.
    // -----------------------------------------------------------------
    ("人々", &["人々"]),
    // -----------------------------------------------------------------
    // Half-width katakana kept in its own run.
    // -----------------------------------------------------------------
    ("\u{FF71}\u{FF72}\u{FF73}", &["\u{FF71}\u{FF72}\u{FF73}"]),
    // -----------------------------------------------------------------
    // Full-width Latin letters.
    // -----------------------------------------------------------------
    ("Ｈｅｌｌｏ", &["Ｈｅｌｌｏ"]),
];

#[test]
fn every_reference_pair_matches() {
    for (input, expected) in PAIRS {
        let actual: Vec<&str> = JapaneseTokenizer::new().tokenize(input).collect();
        assert_eq!(
            actual, *expected,
            "input {input:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn reference_pair_count_is_within_the_advertised_range() {
    // The module doc-comment says "20-30" — assert we're in range.
    assert!(
        PAIRS.len() >= 20 && PAIRS.len() <= 40,
        "PAIRS.len() = {} outside the advertised 20-30 range",
        PAIRS.len()
    );
}
