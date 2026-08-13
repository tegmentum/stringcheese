//! WIT-i18n CJK word-break dictionary SCUD pack for Simplified
//! Chinese.
//!
//! Exposes the compiled `word-dict-zh.scud` blob
//! ([`WORD_DICT_ZH_SCUD`]) plus [`break_pack`], a helper that
//! wraps it as a [`stringcheese_icu_segment::BreakPack`].
//!
//! The SCUD blob is generated in `build.rs` from a ~500-entry
//! hand-curated Simplified Chinese starter word list and embedded
//! here via `include_bytes!`. See `docs/design/wit-i18n.md` § 8.5
//! for the Phase 5 CJK-dictionary follow-up notes.
//!
//! # Coverage
//!
//! * **Pronouns** — 我, 你, 他, 她, 我们, 你们, 他们, 自己, 这, 那, 哪,
//!   谁, 什么, 哪里, 这里, 那里, …
//! * **Common verbs** — 是, 有, 说, 看, 想, 去, 来, 做, 吃, 喝, 用, 买,
//!   卖, 学习, 工作, 认识, 知道, 开始, 结束, 喜欢, …
//! * **Common auxiliaries** — 能, 会, 要, 可以, 应该, 必须, 愿意, 希望, …
//! * **Common particles** — 的, 了, 着, 过, 吧, 呢, 吗, 也, 都, 就, 已经,
//!   正在, 和, 或者, 但是, 如果, 因为, 所以, …
//! * **Common nouns** — 人, 时间, 事情, 东西, 地方, 家, 朋友, 老师, 学生,
//!   医生, 学校, 大学, 中学, 小学, 公司, 图书馆, 医院, 火车站, 电影院,
//!   餐厅, 电脑, 手机, 电话, …
//! * **City / country / world** — 北京, 上海, 广州, 深圳, 香港, 澳门, 台湾,
//!   北京大学, 清华大学, 中国, 美国, 日本, 韩国, 世界, 国家, 政府, …
//! * **Common adjectives** — 好, 坏, 大, 小, 新, 旧, 高, 低, 快, 慢, 多,
//!   少, 贵, 便宜, 冷, 热, 漂亮, 干净, 安静, 忙, 累, 开心, 高兴, …
//! * **Numbers** — 一 through 亿; 两, 半, 几, 零.
//! * **Time words** — 今天, 明天, 昨天, 现在, 以前, 以后, 早上, 中午,
//!   下午, 晚上, 今年, 明年, 去年, 小时, 分钟, 秒, …
//! * **Common food / body words** — 米, 饭, 面, 肉, 鱼, 鸡, 苹果, 眼,
//!   耳, 口, 手, 脚, 心.
//! * **Greetings** — 你好, 您好, 谢谢, 对不起, 没关系, 再见.
//!
//! # Deferrals
//!
//! * **Full `CC-CEDICT` integration** — ~130k entries, ~10 MB.
//!   Deferred to a follow-up SCUD pack; the starter shipped here
//!   proves the FMM segmenter shape end-to-end.
//! * **Traditional Chinese variant (`zh-TW` / `zh-HK`)** — the
//!   shipped dictionary is Simplified-only. Traditional characters
//!   fall through as single-char segments via the unknown-word
//!   path. A Traditional Chinese pack is a follow-up.
//! * **Word-frequency-weighted disambiguation** — FMM prefers the
//!   longest match, which is not always the most-frequent one. A
//!   Viterbi / CRF decoder over a lexicon with unigram / bigram
//!   frequencies would improve accuracy on ambiguous inputs. Out
//!   of scope for the wasm-first offline pack.

use stringcheese_icu_segment::{BreakPack, ScudError};

/// The compiled CJK word-break dictionary SCUD pack for Simplified
/// Chinese.
pub const WORD_DICT_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/word-dict-zh.scud"));

/// Wrap [`WORD_DICT_ZH_SCUD`] as a [`BreakPack`].
///
/// The returned pack advertises the `"zh"` locale — the segment
/// engine engages its forward-maximum-match segmenter only when the
/// caller's requested locale primary subtag matches `"zh"` (or
/// `"ja"` for the sibling Japanese pack).
///
/// # Errors
///
/// See [`BreakPack::from_scud_bytes`].
pub fn break_pack() -> Result<BreakPack<'static>, ScudError> {
    BreakPack::from_scud_bytes(WORD_DICT_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_segment::BreakEngine;

    #[test]
    fn pack_loads() {
        let pack = break_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
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
        // "我是学生" → [我, 是, 学生]
        let pack = break_pack().unwrap();
        let e = BreakEngine::with_pack(pack);
        let out = e.segment_words("\u{6211}\u{662F}\u{5B66}\u{751F}", "zh");
        assert_eq!(out.len(), 3);
        let lens: Vec<u32> = out.iter().map(|s| s.end - s.start).collect();
        assert_eq!(lens, alloc::vec![3, 3, 6]);
    }
}
