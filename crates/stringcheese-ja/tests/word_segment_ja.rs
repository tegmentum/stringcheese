//! FMM word-segmentation golden vectors for the Japanese pack.
//!
//! Runs the [`WORD_DICT_JA_SCUD`](
//! stringcheese_ja::word_dict_data::WORD_DICT_JA_SCUD) starter
//! dictionary through the [`stringcheese_icu_segment::BreakEngine`]
//! FMM segmenter and asserts that the expected token sequence
//! appears on a hand-curated set of common Japanese sentences.
//!
//! The vectors are deliberately small (20+ cases) and cover:
//!
//! * Simple pronoun + copula (私は学生です).
//! * Multi-char verbs + polite forms (勉強しています).
//! * Longest-match tie-breaking (東京大学 vs 東京 + 大学).
//! * Unknown-word single-char fallback (unknown ideographs stand
//!   alone as their own segment).
//! * Katakana loanwords (コンピュータ).
//! * Mixed CJK / Latin input (Latin runs use UAX #29 fallback).
//! * Whitespace + punctuation between CJK runs.

#![cfg(feature = "break-scud")]

use stringcheese_icu_segment::BreakEngine;
use stringcheese_ja::word_dict_data;

/// Convenience: segment `text` under `locale` with the shipped
/// Japanese dictionary pack and return each segment as a `String`.
fn segments(text: &str, locale: &str) -> Vec<String> {
    let pack = word_dict_data::break_pack().unwrap();
    let e = BreakEngine::with_pack(pack);
    e.segment_words(text, locale)
        .into_iter()
        .map(|s| text[s.start as usize..s.end as usize].to_string())
        .collect()
}

#[test]
fn watashi_wa_gakusei_desu() {
    // 私は学生です → [私, は, 学生, です]
    assert_eq!(
        segments("\u{79C1}\u{306F}\u{5B66}\u{751F}\u{3067}\u{3059}", "ja"),
        alloc::vec![
            "\u{79C1}",
            "\u{306F}",
            "\u{5B66}\u{751F}",
            "\u{3067}\u{3059}"
        ]
    );
}

#[test]
fn watashi_tachi_wa_learners() {
    // 私たちは学生です → [私たち, は, 学生, です]
    assert_eq!(
        segments(
            "\u{79C1}\u{305F}\u{3061}\u{306F}\u{5B66}\u{751F}\u{3067}\u{3059}",
            "ja"
        ),
        alloc::vec![
            "\u{79C1}\u{305F}\u{3061}",
            "\u{306F}",
            "\u{5B66}\u{751F}",
            "\u{3067}\u{3059}"
        ]
    );
}

#[test]
fn tokyo_daigaku_prefers_longest() {
    // 東京大学に行きます → [東京大学, に, 行きます]
    let out = segments(
        "\u{6771}\u{4EAC}\u{5927}\u{5B66}\u{306B}\u{884C}\u{304D}\u{307E}\u{3059}",
        "ja",
    );
    assert_eq!(out[0], "\u{6771}\u{4EAC}\u{5927}\u{5B66}");
    assert_eq!(out[1], "\u{306B}");
    assert_eq!(out[2], "\u{884C}\u{304D}\u{307E}\u{3059}");
    assert_eq!(out.len(), 3);
}

#[test]
fn kyoto_wa_utsukushii_toshi() {
    // 京都は美しい (京都 + は + 美しい)
    let out = segments("\u{4EAC}\u{90FD}\u{306F}\u{7F8E}\u{3057}\u{3044}", "ja");
    assert_eq!(out[0], "\u{4EAC}\u{90FD}");
    assert_eq!(out[1], "\u{306F}");
    assert_eq!(out[2], "\u{7F8E}\u{3057}\u{3044}");
}

#[test]
fn benkyou_shite_imasu() {
    // 勉強しています → [勉強しています] (single dict entry)
    let out = segments(
        "\u{52C9}\u{5F37}\u{3057}\u{3066}\u{3044}\u{307E}\u{3059}",
        "ja",
    );
    assert_eq!(
        out,
        alloc::vec!["\u{52C9}\u{5F37}\u{3057}\u{3066}\u{3044}\u{307E}\u{3059}"]
    );
}

#[test]
fn common_verbs_dictionary_form() {
    // 見る食べる飲む → [見る, 食べる, 飲む]
    assert_eq!(
        segments(
            "\u{898B}\u{308B}\u{98DF}\u{3079}\u{308B}\u{98F2}\u{3080}",
            "ja"
        ),
        alloc::vec![
            "\u{898B}\u{308B}",
            "\u{98DF}\u{3079}\u{308B}",
            "\u{98F2}\u{3080}"
        ]
    );
}

#[test]
fn common_verbs_polite_form() {
    // 行きます来ます見ます → [行きます, 来ます, 見ます]
    assert_eq!(
        segments(
            "\u{884C}\u{304D}\u{307E}\u{3059}\u{6765}\u{307E}\u{3059}\u{898B}\u{307E}\u{3059}",
            "ja"
        ),
        alloc::vec![
            "\u{884C}\u{304D}\u{307E}\u{3059}",
            "\u{6765}\u{307E}\u{3059}",
            "\u{898B}\u{307E}\u{3059}"
        ]
    );
}

#[test]
fn common_nouns_bigram() {
    // 学校の先生 → [学校, の, 先生]
    assert_eq!(
        segments("\u{5B66}\u{6821}\u{306E}\u{5148}\u{751F}", "ja"),
        alloc::vec!["\u{5B66}\u{6821}", "\u{306E}", "\u{5148}\u{751F}"]
    );
}

#[test]
fn common_adjectives() {
    // 良い悪い大きい小さい → 4 segments
    let out = segments(
        "\u{826F}\u{3044}\u{60AA}\u{3044}\u{5927}\u{304D}\u{3044}\u{5C0F}\u{3055}\u{3044}",
        "ja",
    );
    assert_eq!(out.len(), 4);
    assert_eq!(out[0], "\u{826F}\u{3044}");
    assert_eq!(out[3], "\u{5C0F}\u{3055}\u{3044}");
}

#[test]
fn numbers_kanji() {
    // 一二三四五 → 5 single-char segments
    let out = segments("\u{4E00}\u{4E8C}\u{4E09}\u{56DB}\u{4E94}", "ja");
    assert_eq!(out.len(), 5);
    assert_eq!(out[0], "\u{4E00}");
    assert_eq!(out[4], "\u{4E94}");
}

#[test]
fn katakana_loanword_computer() {
    // コンピュータ → [コンピュータ] (single dict entry)
    assert_eq!(
        segments("\u{30B3}\u{30F3}\u{30D4}\u{30E5}\u{30FC}\u{30BF}", "ja"),
        alloc::vec!["\u{30B3}\u{30F3}\u{30D4}\u{30E5}\u{30FC}\u{30BF}"]
    );
}

#[test]
fn katakana_loanword_hotel_restaurant() {
    // ホテルレストラン → [ホテル, レストラン]
    assert_eq!(
        segments(
            "\u{30DB}\u{30C6}\u{30EB}\u{30EC}\u{30B9}\u{30C8}\u{30E9}\u{30F3}",
            "ja"
        ),
        alloc::vec![
            "\u{30DB}\u{30C6}\u{30EB}",
            "\u{30EC}\u{30B9}\u{30C8}\u{30E9}\u{30F3}"
        ]
    );
}

#[test]
fn unknown_kanji_falls_through_single_char() {
    // 龍鳳凰 → 3 single-char segments (none of these ideographs are
    // in the starter dictionary; the FMM single-char fallback
    // emits each as its own word-like segment).
    let out = segments("\u{9F8D}\u{9CF3}\u{51F0}", "ja");
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], "\u{9F8D}");
    assert_eq!(out[1], "\u{9CF3}");
    assert_eq!(out[2], "\u{51F0}");
    for seg in &out {
        // Single-char CJK fallbacks are still word-like.
        assert_eq!(seg.chars().count(), 1);
    }
}

#[test]
fn mixed_cjk_latin_input() {
    // 私 ABC を 見る → 私 + " " + ABC + " " + を + " " + 見る
    // (Latin run uses UAX #29 default; CJK runs use FMM.)
    let out = segments("\u{79C1} ABC \u{3092} \u{898B}\u{308B}", "ja");
    // Look for expected content pieces (whitespace behaviour is
    // UAX #29 for the Latin run so counts may include the space
    // segments — assert the CJK pieces are present).
    assert!(out.contains(&"\u{79C1}".to_string()));
    assert!(out.contains(&"ABC".to_string()));
    assert!(out.contains(&"\u{3092}".to_string()));
    assert!(out.contains(&"\u{898B}\u{308B}".to_string()));
}

#[test]
fn locale_variant_ja_jp_engages_dict() {
    // "ja-JP" primary subtag matches, so FMM engages.
    let out = segments("\u{79C1}\u{306F}\u{5B66}\u{751F}", "ja-JP");
    assert!(out.contains(&"\u{5B66}\u{751F}".to_string()));
}

#[test]
fn locale_non_cjk_bypasses_dict() {
    // "en" — FMM does NOT engage; falls to UAX #29 which leaves
    // each ideograph as its own word (Hiragana is Other in the
    // built-in classifier so may cluster together).
    let out = segments("\u{79C1}\u{306F}\u{5B66}\u{751F}", "en");
    // The exact segmentation is UAX #29's; just assert it's NOT
    // the FMM-preferred "私 / は / 学生" 3-segment shape.
    assert_ne!(out, alloc::vec!["\u{79C1}", "\u{306F}", "\u{5B66}\u{751F}"]);
}

#[test]
fn contiguous_coverage_invariant() {
    // The concatenation of every segment must equal the input.
    let text =
        "\u{79C1}\u{306F}\u{6771}\u{4EAC}\u{5927}\u{5B66}\u{306B}\u{884C}\u{304D}\u{307E}\u{3059}";
    let out = segments(text, "ja");
    let joined: String = out.concat();
    assert_eq!(joined, text);
}

#[test]
fn watashi_repeated() {
    // 私私私 → three [私] segments (single-char match repeats).
    let out = segments("\u{79C1}\u{79C1}\u{79C1}", "ja");
    assert_eq!(out.len(), 3);
    for seg in &out {
        assert_eq!(seg, "\u{79C1}");
    }
}

#[test]
fn sentence_ending_desu_ka() {
    // 学生ですか → [学生, です, か]
    assert_eq!(
        segments("\u{5B66}\u{751F}\u{3067}\u{3059}\u{304B}", "ja"),
        alloc::vec!["\u{5B66}\u{751F}", "\u{3067}\u{3059}", "\u{304B}"]
    );
}

#[test]
fn nihon_no_shuto_wa_tokyo_desu() {
    // 日本の首都は東京です → [日本, の, 首(?), 都(?), は, 東京, です]
    // 首都 (capital city) is NOT in the starter dict; the two
    // ideographs fall through as single-char segments. This
    // exercises the unknown-word path in the middle of a
    // dictionary-heavy sentence.
    let out = segments(
        "\u{65E5}\u{672C}\u{306E}\u{9996}\u{90FD}\u{306F}\u{6771}\u{4EAC}\u{3067}\u{3059}",
        "ja",
    );
    assert!(out.contains(&"\u{65E5}\u{672C}".to_string()));
    assert!(out.contains(&"\u{6771}\u{4EAC}".to_string()));
    assert!(out.contains(&"\u{3067}\u{3059}".to_string()));
    // 首 and 都 unknown → each stands alone.
    assert!(out.contains(&"\u{9996}".to_string()));
    assert!(out.contains(&"\u{90FD}".to_string()));
}

#[test]
fn otoosan_okaasan_family_words() {
    // 母と父 → [母, と, 父]
    assert_eq!(
        segments("\u{6BCD}\u{3068}\u{7236}", "ja"),
        alloc::vec!["\u{6BCD}", "\u{3068}", "\u{7236}"]
    );
}

#[test]
fn pack_size_bounded() {
    // Small-starter-dict sanity: the shipped SCUD blob stays
    // under 16 KiB. Full IPADIC integration would be a data-only
    // follow-up and would land under its own size budget.
    assert!(
        word_dict_data::WORD_DICT_JA_SCUD.len() < 16 * 1024,
        "word-dict-ja.scud grew unexpectedly: {} bytes",
        word_dict_data::WORD_DICT_JA_SCUD.len()
    );
}

extern crate alloc;
