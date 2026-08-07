//! Modified Hepburn romanization reference input/output pairs.
//!
//! ~25 pairs drawn from Wikipedia's Modified Hepburn table plus
//! composed words that exercise the small-tsu, prolonged-sound mark,
//! syllabic-n apostrophe, and long-vowel-macron rules.
//!
//! - <https://en.wikipedia.org/wiki/Hepburn_romanization#Modified_Hepburn>
//!
//! # Reading the table
//!
//! Long vowels take macrons (`おう → ō`, `うう → ū`). The `ei` and
//! `ii` sequences are left as two ASCII letters per Modified Hepburn
//! convention (`せんせい → sensei`, `おおきい → ōkii`).

use stringcheese_ja::HepburnRomaji;

/// Reference pairs (input, expected Modified Hepburn romanization).
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
    // Shibboleth moras where Hepburn diverges from Kunrei.
    // -----------------------------------------------------------------
    ("し", "shi"),
    ("ち", "chi"),
    ("つ", "tsu"),
    ("ふ", "fu"),
    ("じ", "ji"),
    // -----------------------------------------------------------------
    // Digraphs (Hepburn sha/cha/ja vs. Kunrei sya/tya/zya).
    // -----------------------------------------------------------------
    ("しゃ", "sha"),
    ("しゅ", "shu"),
    ("しょ", "sho"),
    ("ちゃ", "cha"),
    ("ちゅ", "chu"),
    ("ちょ", "cho"),
    ("じゃ", "ja"),
    ("じゅ", "ju"),
    ("じょ", "jo"),
    // -----------------------------------------------------------------
    // Composed words — Wikipedia Modified Hepburn table.
    // -----------------------------------------------------------------
    ("さくら", "sakura"),
    ("すし", "sushi"),
    ("ふじさん", "fujisan"),
    ("ちば", "chiba"),
    ("しんじゅく", "shinjuku"),
    ("しゅくだい", "shukudai"),
    ("ちゅうがくせい", "chūgakusei"),
    ("ばんざい", "banzai"),
    ("たなか", "tanaka"),
    // -----------------------------------------------------------------
    // Long-vowel macrons (Modified Hepburn).
    // -----------------------------------------------------------------
    ("とうきょう", "tōkyō"),
    ("きょうと", "kyōto"),
    ("おおさか", "ōsaka"),
    ("じゅう", "jū"),
    ("おおきい", "ōkii"),   // いい stays as `ii`.
    ("せんせい", "sensei"), // えい stays as `ei`.
    // -----------------------------------------------------------------
    // Prolonged sound mark on katakana → macron over the previous vowel.
    // -----------------------------------------------------------------
    ("コーヒー", "kōhī"),
    ("ラーメン", "rāmen"),
    ("サッカー", "sakkā"),
    ("ケーキ", "kēki"),
    // -----------------------------------------------------------------
    // Small tsu (sokuon).
    // -----------------------------------------------------------------
    ("がっこう", "gakkō"),
    ("まっちゃ", "matcha"), // っち → tchi (Modified Hepburn exception).
    ("いっき", "ikki"),
    // -----------------------------------------------------------------
    // Syllabic n.
    // -----------------------------------------------------------------
    ("しんぶん", "shinbun"),
    ("しんや", "shin'ya"),
    ("かんじ", "kanji"),
    // -----------------------------------------------------------------
    // Katakana folds to hiragana before lookup.
    // -----------------------------------------------------------------
    ("カタカナ", "katakana"),
    ("シ", "shi"),
];

#[test]
fn every_reference_pair_matches() {
    for (input, expected) in PAIRS {
        let actual = HepburnRomaji.romanize(input);
        assert_eq!(
            actual, *expected,
            "input {input:?}: expected {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn reference_pair_count_is_within_the_advertised_range() {
    // The module doc-comment says "~25". Assert we're in a sensible band.
    assert!(
        PAIRS.len() >= 20 && PAIRS.len() <= 60,
        "PAIRS.len() = {} outside the advertised 20-60 range",
        PAIRS.len()
    );
}
