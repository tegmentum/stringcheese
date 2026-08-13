//! FMM word-segmentation golden vectors for the Simplified Chinese
//! pack.
//!
//! Runs the [`WORD_DICT_ZH_SCUD`](
//! stringcheese_zh::word_dict_data::WORD_DICT_ZH_SCUD) starter
//! dictionary through the [`stringcheese_icu_segment::BreakEngine`]
//! FMM segmenter and asserts that the expected token sequence
//! appears on a hand-curated set of common Simplified Chinese
//! sentences.
//!
//! Coverage: pronouns + copula, longest-match tie-breaking on
//! multi-char nouns, unknown-word single-char fallback, common
//! adjectives, HSK-vocabulary bigrams, mixed CJK / Latin,
//! locale-variant subtag matching.

#![cfg(feature = "break-scud")]

use stringcheese_icu_segment::BreakEngine;
use stringcheese_zh::word_dict_data;

/// Convenience: segment `text` under `locale` with the shipped
/// Chinese dictionary pack and return each segment as a `String`.
fn segments(text: &str, locale: &str) -> Vec<String> {
    let pack = word_dict_data::break_pack().unwrap();
    let e = BreakEngine::with_pack(pack);
    e.segment_words(text, locale)
        .into_iter()
        .map(|s| text[s.start as usize..s.end as usize].to_string())
        .collect()
}

#[test]
fn wo_shi_xuesheng() {
    // 我是学生 → [我, 是, 学生]
    assert_eq!(
        segments("\u{6211}\u{662F}\u{5B66}\u{751F}", "zh"),
        alloc::vec!["\u{6211}", "\u{662F}", "\u{5B66}\u{751F}"]
    );
}

#[test]
fn ni_hao() {
    // 你好 → [你好] (single dict entry as a greeting).
    assert_eq!(
        segments("\u{4F60}\u{597D}", "zh"),
        alloc::vec!["\u{4F60}\u{597D}"]
    );
}

#[test]
fn beijing_daxue() {
    // 北京大学 → [北京大学] (dict has both 北京 and 北京大学; FMM
    // prefers the longest).
    assert_eq!(
        segments("\u{5317}\u{4EAC}\u{5927}\u{5B66}", "zh"),
        alloc::vec!["\u{5317}\u{4EAC}\u{5927}\u{5B66}"]
    );
}

#[test]
fn beijing_alone_when_no_daxue_follows() {
    // 北京很大 → [北京, 很, 大]
    assert_eq!(
        segments("\u{5317}\u{4EAC}\u{5F88}\u{5927}", "zh"),
        alloc::vec!["\u{5317}\u{4EAC}", "\u{5F88}", "\u{5927}"]
    );
}

#[test]
fn zhongguo_shi_daguo() {
    // 中国是大国 (China is a great country) →
    // [中国, 是, 大, 国] (大国 not in dict; 国 splits alone)
    let out = segments("\u{4E2D}\u{56FD}\u{662F}\u{5927}\u{56FD}", "zh");
    assert_eq!(out[0], "\u{4E2D}\u{56FD}");
    assert_eq!(out[1], "\u{662F}");
    assert_eq!(out[2], "\u{5927}");
    assert_eq!(out[3], "\u{56FD}");
}

#[test]
fn common_verbs_chinese() {
    // 我看电影 → [我, 看, 电影]
    assert_eq!(
        segments("\u{6211}\u{770B}\u{7535}\u{5F71}", "zh"),
        alloc::vec!["\u{6211}", "\u{770B}", "\u{7535}\u{5F71}"]
    );
}

#[test]
fn women_shi_pengyou() {
    // 我们是朋友 → [我们, 是, 朋友]
    assert_eq!(
        segments("\u{6211}\u{4EEC}\u{662F}\u{670B}\u{53CB}", "zh"),
        alloc::vec!["\u{6211}\u{4EEC}", "\u{662F}", "\u{670B}\u{53CB}"]
    );
}

#[test]
fn xuexi_hanyu() {
    // 我学习中文 → [我, 学习, 中, 文]
    // 中文 not in dict → falls through as single chars.
    let out = segments("\u{6211}\u{5B66}\u{4E60}\u{4E2D}\u{6587}", "zh");
    assert_eq!(out[0], "\u{6211}");
    assert_eq!(out[1], "\u{5B66}\u{4E60}");
    assert_eq!(out[2], "\u{4E2D}");
    assert_eq!(out[3], "\u{6587}");
}

#[test]
fn time_bigrams() {
    // 今天明天昨天 → [今天, 明天, 昨天]
    assert_eq!(
        segments("\u{4ECA}\u{5929}\u{660E}\u{5929}\u{6628}\u{5929}", "zh"),
        alloc::vec!["\u{4ECA}\u{5929}", "\u{660E}\u{5929}", "\u{6628}\u{5929}"]
    );
}

#[test]
fn common_adjectives_zh() {
    // 好坏大小 → 4 single-char segments (each is in the dict).
    assert_eq!(
        segments("\u{597D}\u{574F}\u{5927}\u{5C0F}", "zh"),
        alloc::vec!["\u{597D}", "\u{574F}", "\u{5927}", "\u{5C0F}"]
    );
}

#[test]
fn numbers_hanzi() {
    // 一二三四五 → 5 single-char segments
    let out = segments("\u{4E00}\u{4E8C}\u{4E09}\u{56DB}\u{4E94}", "zh");
    assert_eq!(out.len(), 5);
}

#[test]
fn unknown_hanzi_falls_through() {
    // 龘飝 → 2 single-char segments (rare ideographs not in dict).
    let out = segments("\u{9F98}\u{98DD}", "zh");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], "\u{9F98}");
    assert_eq!(out[1], "\u{98DD}");
}

#[test]
fn greeting_thanks() {
    // 谢谢 → [谢谢] (single dict entry)
    assert_eq!(
        segments("\u{8C22}\u{8C22}", "zh"),
        alloc::vec!["\u{8C22}\u{8C22}"]
    );
}

#[test]
fn duibuqi_meiguanxi() {
    // 对不起没关系 → [对不起, 没关系]
    assert_eq!(
        segments("\u{5BF9}\u{4E0D}\u{8D77}\u{6CA1}\u{5173}\u{7CFB}", "zh"),
        alloc::vec!["\u{5BF9}\u{4E0D}\u{8D77}", "\u{6CA1}\u{5173}\u{7CFB}"]
    );
}

#[test]
fn mixed_cjk_latin_zh() {
    // 我用ABC → [我, 用, ABC]
    let out = segments("\u{6211}\u{7528}ABC", "zh");
    assert!(out.contains(&"\u{6211}".to_string()));
    assert!(out.contains(&"\u{7528}".to_string()));
    assert!(out.contains(&"ABC".to_string()));
}

#[test]
fn locale_variant_zh_cn_engages_dict() {
    // "zh-Hans-CN" primary subtag matches, so FMM engages.
    let out = segments("\u{6211}\u{662F}\u{5B66}\u{751F}", "zh-Hans-CN");
    assert!(out.contains(&"\u{5B66}\u{751F}".to_string()));
}

#[test]
fn locale_non_cjk_bypasses_dict_zh() {
    // "en" — FMM does NOT engage; UAX #29 leaves each ideograph
    // as its own token, so 我 / 是 / 学 / 生 (4 segments) — not
    // the FMM-preferred 3.
    let out = segments("\u{6211}\u{662F}\u{5B66}\u{751F}", "en");
    assert_ne!(out, alloc::vec!["\u{6211}", "\u{662F}", "\u{5B66}\u{751F}"]);
}

#[test]
fn contiguous_coverage_invariant_zh() {
    let text = "\u{6211}\u{4EEC}\u{5728}\u{5317}\u{4EAC}\u{5927}\u{5B66}\u{5B66}\u{4E60}";
    let out = segments(text, "zh");
    let joined: String = out.concat();
    assert_eq!(joined, text);
    assert!(!out.is_empty());
}

#[test]
fn shanghai_place_name() {
    // 上海很大 → [上海, 很, 大]
    assert_eq!(
        segments("\u{4E0A}\u{6D77}\u{5F88}\u{5927}", "zh"),
        alloc::vec!["\u{4E0A}\u{6D77}", "\u{5F88}", "\u{5927}"]
    );
}

#[test]
fn de_particle_between_nouns() {
    // 我的朋友 → [我, 的, 朋友]
    assert_eq!(
        segments("\u{6211}\u{7684}\u{670B}\u{53CB}", "zh"),
        alloc::vec!["\u{6211}", "\u{7684}", "\u{670B}\u{53CB}"]
    );
}

#[test]
fn laoshi_shi_haoren() {
    // 老师是好人 → [老师, 是, 好, 人]
    // 好人 not in dict as a bigram; 好 + 人 each single.
    let out = segments("\u{8001}\u{5E08}\u{662F}\u{597D}\u{4EBA}", "zh");
    assert_eq!(out[0], "\u{8001}\u{5E08}");
    assert_eq!(out[1], "\u{662F}");
}

#[test]
fn pack_size_bounded_zh() {
    // Small-starter-dict sanity: the shipped SCUD blob stays
    // under 16 KiB.
    assert!(
        word_dict_data::WORD_DICT_ZH_SCUD.len() < 16 * 1024,
        "word-dict-zh.scud grew unexpectedly: {} bytes",
        word_dict_data::WORD_DICT_ZH_SCUD.len()
    );
}

extern crate alloc;
