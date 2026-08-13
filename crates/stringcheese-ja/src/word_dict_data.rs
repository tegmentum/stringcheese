//! WIT-i18n CJK word-break dictionary SCUD pack for Japanese.
//!
//! Exposes the compiled `word-dict-ja.scud` blob
//! ([`WORD_DICT_JA_SCUD`]) plus [`break_pack`], a helper that
//! wraps it as a [`stringcheese_icu_segment::BreakPack`].
//!
//! The SCUD blob is generated in `build.rs` from a ~500-entry
//! hand-curated Japanese starter word list and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.5 for the
//! Phase 5 CJK-dictionary follow-up notes.
//!
//! # Coverage
//!
//! * **Pronouns / particles / copula** — 私, あなた, は, が, を,
//!   に, で, と, から, まで, より, の, も, です, ます, だ, …
//! * **Common verbs (dictionary + polite / past forms)** — する,
//!   します, した, している, 行く, 来る, 見る, 食べる, 飲む, 話す, 読む,
//!   書く, 買う, 勉強する, 勉強しています, …
//! * **Common nouns** — 時間, 学校, 学生, 先生, 大学, 東京, 京都,
//!   大阪, 日本, 中国, 世界, 政治, 経済, 会議, 会社, …
//! * **Common adjectives** — 良い, 悪い, 大きい, 小さい, 新しい,
//!   古い, 高い, 安い, 楽しい, 面白い, 難しい, …
//! * **Numbers** — 一, 二, 三, 四, 五, 六, 七, 八, 九, 十, 百, 千, 万, 億.
//! * **~100 common katakana loanwords** — コンピュータ, インターネット,
//!   テレビ, ラジオ, ホテル, レストラン, タクシー, バス, スマホ, メール,
//!   ゲーム, コーヒー, サッカー, テニス, カメラ, …
//!
//! # Deferrals
//!
//! * **Full IPADIC / `JMdict` integration** — 100k+ entries, multi-MB.
//!   Deferred to a follow-up SCUD pack; the starter shipped here
//!   proves the FMM segmenter shape end-to-end.
//! * **Viterbi / CRF-based segmenter** — MeCab-quality output
//!   requires morpheme-level ML models (conditional random fields
//!   over IPADIC's part-of-speech tag set). Out of scope for the
//!   wasm-first offline pack; the FMM segmenter is O(n) and
//!   correct-enough for common vocabulary.

use stringcheese_icu_segment::{BreakPack, ScudError};

/// The compiled CJK word-break dictionary SCUD pack for Japanese.
pub const WORD_DICT_JA_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/word-dict-ja.scud"));

/// Wrap [`WORD_DICT_JA_SCUD`] as a [`BreakPack`].
///
/// The returned pack advertises the `"ja"` locale — the segment
/// engine engages its forward-maximum-match segmenter only when the
/// caller's requested locale primary subtag matches `"ja"` (or
/// `"zh"` for the sibling Chinese pack).
///
/// # Errors
///
/// See [`BreakPack::from_scud_bytes`].
pub fn break_pack() -> Result<BreakPack<'static>, ScudError> {
    BreakPack::from_scud_bytes(WORD_DICT_JA_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ja";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_segment::BreakEngine;

    #[test]
    fn pack_loads() {
        let pack = break_pack().unwrap();
        assert_eq!(pack.locale(), "ja");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn dict_is_populated() {
        let pack = break_pack().unwrap();
        let dict = pack.data().word_dict().expect("dict present");
        assert!(
            dict.len() >= 200,
            "starter dict entry count {} unexpectedly low",
            dict.len()
        );
    }

    #[test]
    fn engine_segments_common_sentence() {
        // "私は学生です" → [私, は, 学生, です]
        let pack = break_pack().unwrap();
        let e = BreakEngine::with_pack(pack);
        let out = e.segment_words("\u{79C1}\u{306F}\u{5B66}\u{751F}\u{3067}\u{3059}", "ja");
        // 4 segments with the byte-length shape of 3/3/6/6.
        assert_eq!(out.len(), 4);
        let lens: Vec<u32> = out.iter().map(|s| s.end - s.start).collect();
        assert_eq!(lens, alloc::vec![3, 3, 6, 6]);
    }
}
