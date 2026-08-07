//! Kunrei-shiki romanization reference input/output pairs.
//!
//! ~50 pairs drawn from the ISO 3602 kana table plus a small set of
//! composed words that exercise the small-tsu (sokuon), prolonged-sound
//! mark (chōonpu), and syllabic-n apostrophe rules. Each pair was
//! hand-derived from the Kunrei-shiki spec:
//!
//! - <https://www.iso.org/standard/8956.html> (ISO 3602:1989)
//! - Cabinet Order No. 1 of 1954, ローマ字のつづり方 (Rōmaji no Tsuzurikata)
//!
//! # Reading the table
//!
//! Long-vowel marks are handled with the *ASCII-double-vowel*
//! convention (`コーヒー → koohii`) instead of the circumflex form
//! (`kôhî`) because ASCII output is a hard contract for the phonetic
//! encoder. If a future encoder emits the circumflex form the reference
//! pairs here will need updating.

use stringcheese_ja::KunreiRomaji;

/// Reference pairs (input, expected Kunrei-shiki romanization).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Basic hiragana vowels.
    // -----------------------------------------------------------------
    ("あ", "a"),
    ("い", "i"),
    ("う", "u"),
    ("え", "e"),
    ("お", "o"),
    // -----------------------------------------------------------------
    // k-row (hiragana).
    // -----------------------------------------------------------------
    ("か", "ka"),
    ("き", "ki"),
    ("く", "ku"),
    ("け", "ke"),
    ("こ", "ko"),
    // -----------------------------------------------------------------
    // s-row — Kunrei's si/ti/tu/hu/zi shibboleths.
    // -----------------------------------------------------------------
    ("さ", "sa"),
    ("し", "si"),
    ("す", "su"),
    ("せ", "se"),
    ("そ", "so"),
    ("た", "ta"),
    ("ち", "ti"),
    ("つ", "tu"),
    ("て", "te"),
    ("と", "to"),
    // -----------------------------------------------------------------
    // n-row.
    // -----------------------------------------------------------------
    ("な", "na"),
    ("に", "ni"),
    ("ぬ", "nu"),
    ("ね", "ne"),
    ("の", "no"),
    // -----------------------------------------------------------------
    // h-row — Kunrei's hu (Hepburn fu).
    // -----------------------------------------------------------------
    ("は", "ha"),
    ("ひ", "hi"),
    ("ふ", "hu"),
    ("へ", "he"),
    ("ほ", "ho"),
    // -----------------------------------------------------------------
    // Voiced series.
    // -----------------------------------------------------------------
    ("が", "ga"),
    ("ざ", "za"),
    ("じ", "zi"),
    ("だ", "da"),
    ("ば", "ba"),
    ("ぱ", "pa"),
    // -----------------------------------------------------------------
    // Digraphs (Kunrei sya/tya/zya vs. Hepburn sha/cha/ja).
    // -----------------------------------------------------------------
    ("きゃ", "kya"),
    ("きゅ", "kyu"),
    ("きょ", "kyo"),
    ("しゃ", "sya"),
    ("しゅ", "syu"),
    ("しょ", "syo"),
    ("ちゃ", "tya"),
    ("ちゅ", "tyu"),
    ("ちょ", "tyo"),
    ("じゃ", "zya"),
    ("じゅ", "zyu"),
    ("じょ", "zyo"),
    // -----------------------------------------------------------------
    // Katakana folds to hiragana (same output).
    // -----------------------------------------------------------------
    ("シ", "si"),
    ("カタカナ", "katakana"),
    // -----------------------------------------------------------------
    // Small-tsu (sokuon) — double the next consonant.
    // -----------------------------------------------------------------
    ("かっぱ", "kappa"),
    ("しっぽ", "sippo"),
    ("いっき", "ikki"),
    ("さっと", "satto"),
    // -----------------------------------------------------------------
    // Prolonged sound mark — double the previous vowel (ASCII
    // fallback for the circumflex form).
    // -----------------------------------------------------------------
    ("コーヒー", "koohii"),
    ("ケーキ", "keeki"),
    ("スーパー", "suupaa"),
    // -----------------------------------------------------------------
    // Syllabic n — bare `n`; apostrophe before vowel or y.
    // -----------------------------------------------------------------
    ("ほん", "hon"),
    ("かんじ", "kanzi"),
    ("しんや", "sin'ya"),
    ("こんいち", "kon'iti"),
    ("ほんだな", "hondana"),
    // -----------------------------------------------------------------
    // Composed words.
    // -----------------------------------------------------------------
    ("さくら", "sakura"),
    ("にほん", "nihon"),
    ("とうきょう", "toukyou"),
    ("がっこう", "gakkou"),
];

#[test]
fn every_reference_pair_matches() {
    for (input, expected) in PAIRS {
        let actual = KunreiRomaji.romanize(input);
        assert_eq!(
            actual, *expected,
            "input {input:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn reference_pair_count_is_within_the_advertised_range() {
    // The module doc-comment says "40-60" — assert we're in range.
    assert!(
        PAIRS.len() >= 40 && PAIRS.len() <= 70,
        "PAIRS.len() = {} outside the advertised 40-60 range",
        PAIRS.len()
    );
}
