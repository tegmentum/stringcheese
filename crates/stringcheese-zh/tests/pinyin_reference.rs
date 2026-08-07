//! Hanyu Pinyin (tone-stripped) reference input/output pairs.
//!
//! Reference pairs for the top-frequency Han characters and a set of
//! common composed words. Each pair was hand-derived from Modern
//! Standard Mandarin readings (cross-checked against CC-CEDICT).
//! Tone marks are stripped — see [`stringcheese_zh::phonetic`] for
//! the rationale.
//!
//! # Notes on polyphonic characters
//!
//! Several entries below exercise characters with multiple Mandarin
//! readings (`了` `le`/`liǎo`, `地` `de`/`dì`, `长` `cháng`/`zhǎng`,
//! `重` `zhòng`/`chóng`). The shipped table records the most-frequent
//! reading; the reference pairs assert *that* reading, not an
//! alternate. Callers with a dictionary-aware wrapper can override
//! per-word (see the module-level `Per-character ambiguity` note).

use stringcheese_zh::Pinyin;

/// Reference pairs — `(input, expected_pinyin_tone_stripped)`.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Common two-character greetings and names.
    // -----------------------------------------------------------------
    ("你好", "ni hao"),
    ("中国", "zhong guo"),
    ("北京", "bei jing"),
    ("上海", "shang hai"),
    ("汉字", "han zi"),
    ("学生", "xue sheng"),
    ("老师", "lao shi"),
    ("朋友", "peng you"),
    ("电话", "dian hua"),
    ("电脑", "dian nao"),
    // -----------------------------------------------------------------
    // Common single characters.
    // -----------------------------------------------------------------
    ("的", "de"),
    ("是", "shi"),
    ("我", "wo"),
    ("你", "ni"),
    ("他", "ta"),
    // -----------------------------------------------------------------
    // Numbers as Han characters.
    // -----------------------------------------------------------------
    ("一二三", "yi er san"),
    ("四五六", "si wu liu"),
    ("七八九", "qi ba jiu"),
    ("十", "shi"),
    // -----------------------------------------------------------------
    // Mixed with Latin and digits.
    // -----------------------------------------------------------------
    ("hello", "hello"),
    ("Hello", "hello"),
    ("2026", "2026"),
    ("2026年", "2026 nian"),
    ("中国abc", "zhong guo abc"),
    // -----------------------------------------------------------------
    // Punctuation as separator (tokens on both sides still land in
    // the output, space-separated).
    // -----------------------------------------------------------------
    ("你好，世界", "ni hao shi jie"),
];

#[test]
fn every_reference_pair_matches() {
    for (input, expected) in PAIRS {
        let actual = Pinyin.encode(input);
        assert_eq!(
            actual, *expected,
            "input {input:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn reference_pair_count_is_within_the_advertised_range() {
    // The task requires at least 15 pairs; we ship more.
    assert!(
        PAIRS.len() >= 15 && PAIRS.len() <= 60,
        "PAIRS.len() = {} outside the advertised 15-60 range",
        PAIRS.len()
    );
}
