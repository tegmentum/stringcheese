//! Build-time codegen for the Chinese (Simplified) pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-zh.scud` — CLDR 44 Chinese plural rules. Chinese lacks
//!    grammatical number so the CLDR rule set is `other` only; the
//!    pack contains no plural predicates and every query falls
//!    through to [`PluralCategory::Other`](
//!    stringcheese_icu_plural::PluralCategory::Other).
//! 2. `number-zh.scud` — CLDR 44 Chinese number-formatting patterns.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{chinese_cardinals, chinese_ordinals};
use stringcheese_scud::{
    CAP_CASE, CAP_COLLATION, CAP_DATETIME, CAP_NUMBER, CAP_PLURAL, CaseSectionBuilder,
    CollationSectionBuilder, DateTimeLength, DateTimeSectionBuilder, NumberSectionBuilder,
    PluralSectionBuilder, SECT_AM_PM, SECT_CARDINAL_RULES, SECT_COLLATION_OPTIONS,
    SECT_CURRENCY_TABLE, SECT_DATE_PATTERNS, SECT_DECIMAL_PATTERN, SECT_ERA_NAMES, SECT_EXPANSIONS,
    SECT_FULL_FOLD, SECT_FULL_UPPER, SECT_MONTH_ABBR, SECT_MONTH_NAMES, SECT_ORDINAL_RULES,
    SECT_PERCENT_PATTERN, SECT_PRIMARY_OVERRIDES, SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER,
    SECT_SIMPLE_UPPER, SECT_TIME_PATTERNS, SECT_WEEKDAY_ABBR, SECT_WEEKDAY_NAMES, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let case_path = out_dir.join("case-zh.scud");
    let case_bytes = build_case_zh_scud();
    fs::write(&case_path, &case_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", case_path.display()));

    let coll_path = out_dir.join("collation-zh.scud");
    let coll_bytes = build_collation_zh_scud();
    fs::write(&coll_path, &coll_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", coll_path.display()));

    let plural_path = out_dir.join("plural-zh.scud");
    let plural_bytes = build_plural_zh_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-zh.scud");
    let number_bytes = build_number_zh_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-zh.scud");
    let datetime_bytes = build_datetime_zh_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the datetime-zh SCUD pack in memory.
///
/// Chinese (Simplified) date/time formatting (CLDR 44.1,
/// `gregorian.json`):
///
/// * Date patterns:
///   * short — `y/M/d` (`2024/9/22`)
///   * medium — `y年M月d日` (`2024年9月22日`)
///   * long — `y年M月d日` (same as medium in CLDR)
///   * full — `y年M月d日EEEE` (`2024年9月22日星期日`)
/// * Time patterns (24-hour default):
///   * short — `HH:mm`
///   * medium/long/full — `HH:mm:ss`
/// * Month names — wide form (`一月`, `二月`, …) and numeric-with-
///   suffix abbreviations (`1月`, `2月`, …).
/// * Weekday names — `星期日` (Sunday-first) through `星期六`;
///   abbreviated as `周日` through `周六`.
/// * AM/PM — `上午` / `下午`. Shipped for completeness; the
///   default 24-hour patterns never emit the `a` token.
/// * Era names — `公元前` (BC), `公元` (AD).
fn build_datetime_zh_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    d.set_date_pattern(DateTimeLength::Short, "y/M/d");
    d.set_date_pattern(DateTimeLength::Medium, "y\u{5E74}M\u{6708}d\u{65E5}");
    d.set_date_pattern(DateTimeLength::Long, "y\u{5E74}M\u{6708}d\u{65E5}");
    d.set_date_pattern(DateTimeLength::Full, "y\u{5E74}M\u{6708}d\u{65E5}EEEE");
    d.set_time_pattern(DateTimeLength::Short, "HH:mm");
    d.set_time_pattern(DateTimeLength::Medium, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Long, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Full, "HH:mm:ss");
    d.set_month_names([
        "\u{4E00}\u{6708}",         // 一月
        "\u{4E8C}\u{6708}",         // 二月
        "\u{4E09}\u{6708}",         // 三月
        "\u{56DB}\u{6708}",         // 四月
        "\u{4E94}\u{6708}",         // 五月
        "\u{516D}\u{6708}",         // 六月
        "\u{4E03}\u{6708}",         // 七月
        "\u{516B}\u{6708}",         // 八月
        "\u{4E5D}\u{6708}",         // 九月
        "\u{5341}\u{6708}",         // 十月
        "\u{5341}\u{4E00}\u{6708}", // 十一月
        "\u{5341}\u{4E8C}\u{6708}", // 十二月
    ]);
    d.set_month_abbreviations([
        "1\u{6708}",
        "2\u{6708}",
        "3\u{6708}",
        "4\u{6708}",
        "5\u{6708}",
        "6\u{6708}",
        "7\u{6708}",
        "8\u{6708}",
        "9\u{6708}",
        "10\u{6708}",
        "11\u{6708}",
        "12\u{6708}",
    ]);
    d.set_weekday_names([
        "\u{661F}\u{671F}\u{65E5}", // 星期日 (Sunday)
        "\u{661F}\u{671F}\u{4E00}", // 星期一 (Monday)
        "\u{661F}\u{671F}\u{4E8C}", // 星期二
        "\u{661F}\u{671F}\u{4E09}", // 星期三
        "\u{661F}\u{671F}\u{56DB}", // 星期四
        "\u{661F}\u{671F}\u{4E94}", // 星期五
        "\u{661F}\u{671F}\u{516D}", // 星期六 (Saturday)
    ]);
    d.set_weekday_abbreviations([
        "\u{5468}\u{65E5}", // 周日
        "\u{5468}\u{4E00}", // 周一
        "\u{5468}\u{4E8C}",
        "\u{5468}\u{4E09}",
        "\u{5468}\u{56DB}",
        "\u{5468}\u{4E94}",
        "\u{5468}\u{516D}",
    ]);
    d.set_am_pm("\u{4E0A}\u{5348}", "\u{4E0B}\u{5348}"); // 上午 / 下午
    d.set_eras("\u{516C}\u{5143}\u{524D}", "\u{516C}\u{5143}"); // 公元前 / 公元
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_DATE_PATTERNS, &d.date_patterns_bytes());
    w.append_section(SECT_TIME_PATTERNS, &d.time_patterns_bytes());
    w.append_section(SECT_MONTH_NAMES, &d.month_names_bytes());
    w.append_section(SECT_MONTH_ABBR, &d.month_abbr_bytes());
    w.append_section(SECT_WEEKDAY_NAMES, &d.weekday_names_bytes());
    w.append_section(SECT_WEEKDAY_ABBR, &d.weekday_abbr_bytes());
    w.append_section(SECT_AM_PM, &d.am_pm_bytes());
    w.append_section(SECT_ERA_NAMES, &d.era_names_bytes());
    w.finish()
}

/// Build the case-zh SCUD pack in memory.
///
/// Chinese Han characters (CJK Unified Ideographs, U+4E00..=U+9FFF
/// and extensions) have no case — upper, lower, and titlecase are
/// all identity operations on Han. The `char::to_uppercase` /
/// `char::to_lowercase` fallback already returns the input
/// unchanged for every Han scalar.
///
/// The pack ships:
///
/// * **ASCII a-z ↔ A-Z** — 52 pairs plus 26 folds. Chinese text
///   commonly interleaves Latin (English loanwords, product names,
///   brand identifiers); the pack-hit path gives deterministic
///   behaviour on those.
/// * **German ß / ẞ expansions** — uniform composed-engine
///   behaviour.
///
/// Han characters are deliberately **not** listed in the pack —
/// no simple upper/lower entries for U+4E00..=U+9FFF. The engine
/// falls through to `char::to_lowercase` for Han queries, which
/// returns identity (verified in `case_golden_zh.rs`).
fn build_case_zh_scud() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();

    // ASCII a-z ↔ A-Z.
    for ch in 'a'..='z' {
        let up = ch.to_ascii_uppercase();
        c.push_simple_lower(up as u32, ch as u32);
        c.push_simple_upper(ch as u32, up as u32);
        c.push_simple_fold(up as u32, ch as u32);
    }

    // German ß / ẞ — belt-and-braces for composed-engine behaviour.
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]);
    c.push_full_fold(0x00DF, &[0x0073, 0x0073]);
    c.push_full_fold(0x1E9E, &[0x0073, 0x0073]);
    c.push_simple_lower(0x1E9E, 0x00DF);

    let mut w = ScudWriter::new(CAP_CASE, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
    w.finish()
}

/// Build the collation-zh SCUD pack in memory.
///
/// # Stroke-based ordering scaffold
///
/// CLDR's `zh` `standard` collation orders CJK Ideographs by
/// **stroke count** (primary), with radical + codepoint tiebreak
/// within same stroke count. The full CLDR table covers ~20 000
/// glyphs; shipping all of them wires the reference pack up to a
/// data-only follow-up.
///
/// This scaffold ships a curated **starter set of the most common
/// CJK characters ordered by stroke count** (see
/// [`STROKE_ORDERED_HAN`] below) via [`SECT_PRIMARY_OVERRIDES`].
/// The engine already knows how to consume that section (the shape
/// landed with the Turkish primary-distinct dotless-ı work); this
/// pack just supplies the data.
///
/// Weight scheme: `primary = 1000 + stroke_count * 100 +
/// within_stroke_index`. Within-stroke index is the character's
/// position among same-stroke-count entries in the shipped list
/// (which is authored in a radical → codepoint order for the small
/// set covered here). Characters outside the shipped table fall
/// through to their codepoint value as an approximation — that
/// keeps unshipped Han sorting deterministic (by codepoint) while
/// letting the shipped subset demonstrate stroke ordering.
///
/// # Phase 2 deferrals
///
/// * **Full stroke dataset** — the remaining ~20 000 glyphs beyond
///   this starter set are a data-only follow-up.
/// * **Pinyin ordering** — CLDR's `pinyin` variant requires a
///   ~40 000-entry Han → pinyin table plus tone handling. Still
///   deferred.
///
/// The pack also ships German ß / ẞ expansions for uniform
/// composed-engine behaviour and defaults to tertiary strength.
fn build_collation_zh_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // German ß expansion — uniform composed-engine behaviour.
    c.push_expansion(0x00DF, &[0x0073, 0x0073]);
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]);

    // Stroke-ordered CJK primary weights — the CLDR `zh` `standard`
    // scaffold. Weights are 1000 + stroke * 100 + index so shipped
    // characters sort by stroke count first, then by their in-list
    // position (radical + codepoint order).
    push_stroke_weighted_han(&mut c);

    c.set_default_strength(2);
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.append_section(SECT_PRIMARY_OVERRIDES, &c.primary_overrides_bytes());
    w.finish()
}

/// Push every entry in [`STROKE_ORDERED_HAN`] as a primary-weight
/// override.
///
/// Weight formula: `1000 + stroke_count * 100 +
/// within_stroke_index`. Within-stroke index is the character's
/// zero-based position among same-stroke-count entries. The formula
/// is monotonic in `(stroke_count, within_stroke_index)` and leaves
/// room for a stroke count up to `u32::MAX / 100 - 10` (plenty for
/// CJK).
fn push_stroke_weighted_han(c: &mut CollationSectionBuilder) {
    assert_stroke_table_well_formed();
    let mut prev_stroke: u8 = 0;
    let mut within: u32 = 0;
    for (cp, stroke) in STROKE_ORDERED_HAN {
        if *stroke != prev_stroke {
            within = 0;
            prev_stroke = *stroke;
        }
        // Every stroke count fits in u32 and index-within-stroke is
        // small; the arithmetic is bounded well below u32::MAX.
        let primary = 1000u32 + u32::from(*stroke) * 100 + within;
        c.push_primary_override(*cp, primary, 0, 0);
        within += 1;
    }
}

/// Panic if [`STROKE_ORDERED_HAN`] violates any of the invariants
/// described on its doc-comment. Called from
/// [`push_stroke_weighted_han`] so a table typo surfaces at
/// `cargo build` time rather than as a mysterious sort disagreement
/// at query time.
fn assert_stroke_table_well_formed() {
    let mut prev_stroke: u8 = 0;
    let mut prev_cp_in_stroke: Option<u32> = None;
    let mut seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for (cp, stroke) in STROKE_ORDERED_HAN {
        assert!(
            (0x4E00..=0x9FFF).contains(cp),
            "STROKE_ORDERED_HAN: cp U+{cp:04X} outside CJK Unified Ideographs range"
        );
        assert!(
            seen.insert(*cp),
            "STROKE_ORDERED_HAN: duplicate codepoint U+{cp:04X}"
        );
        if *stroke != prev_stroke {
            assert!(
                *stroke > prev_stroke,
                "STROKE_ORDERED_HAN: stroke buckets must be non-decreasing (saw {stroke} after {prev_stroke})"
            );
            prev_stroke = *stroke;
            prev_cp_in_stroke = None;
        }
        if let Some(prev_cp) = prev_cp_in_stroke {
            assert!(
                *cp > prev_cp,
                "STROKE_ORDERED_HAN: within-stroke {stroke} entries must be sorted by codepoint (saw U+{cp:04X} after U+{prev_cp:04X})"
            );
        }
        prev_cp_in_stroke = Some(*cp);
    }
}

/// Starter set of common CJK Ideographs paired with their standard
/// stroke count.
///
/// Sorted by `(stroke_count, codepoint)` — an approximation of the
/// CLDR `zh` `standard` collation's within-stroke tiebreak
/// (radical + codepoint). Coverage: ~230 of the most common
/// characters spanning stroke counts 1-15, enough to prove the
/// shape works end-to-end. The full ~20 000-entry CLDR dataset is
/// a documented data-only follow-up (see the doc-comment on
/// [`build_collation_zh_scud`]).
///
/// Stroke counts follow the [Unicode Han Database](https://www.unicode.org/reports/tr38/)
/// `kTotalStrokes` field (which mirrors CLDR's `<primary>` stroke
/// bucket) for Simplified Chinese glyphs.
///
/// Kept intentionally small so `collation-zh.scud` stays under a
/// few kilobytes; a follow-up wave streaming in the remaining glyphs
/// will grow the blob into the ~200 KiB range typical of full
/// ICU-style CJK ordering data.
///
/// **Invariants** verified at build time by
/// [`assert_stroke_table_well_formed`]:
///
/// * No duplicate codepoints.
/// * Entries are grouped by stroke count in non-decreasing order.
/// * Every codepoint sits inside the CJK Unified Ideographs range
///   `U+4E00..=U+9FFF`.
#[rustfmt::skip]
const STROKE_ORDERED_HAN: &[(u32, u8)] = &[
    // 1 stroke
    (0x4E00, 1), // 一
    (0x4E59, 1), // 乙
    // 2 strokes
    (0x4E01, 2), // 丁
    (0x4E03, 2), // 七
    (0x4E43, 2), // 乃
    (0x4E5D, 2), // 九
    (0x4E86, 2), // 了
    (0x4E8C, 2), // 二
    (0x4EBA, 2), // 人
    (0x5165, 2), // 入
    (0x516B, 2), // 八
    (0x51E0, 2), // 几
    (0x529B, 2), // 力
    (0x5341, 2), // 十
    (0x53C8, 2), // 又
    // 3 strokes
    (0x4E07, 3), // 万
    (0x4E09, 3), // 三
    (0x4E0A, 3), // 上
    (0x4E0B, 3), // 下
    (0x4E4B, 3), // 之
    (0x4E5F, 3), // 也
    (0x4EA1, 3), // 亡
    (0x51E1, 3), // 凡
    (0x5203, 3), // 刃
    (0x5343, 3), // 千
    (0x53E3, 3), // 口
    (0x571F, 3), // 土
    (0x58EB, 3), // 士
    (0x5915, 3), // 夕
    (0x5927, 3), // 大
    (0x5973, 3), // 女
    (0x5B50, 3), // 子
    (0x5C0F, 3), // 小
    (0x5C71, 3), // 山
    (0x5DDD, 3), // 川
    (0x5DE5, 3), // 工
    (0x5DF1, 3), // 己
    (0x5E7F, 3), // 广
    (0x5F13, 3), // 弓
    (0x624D, 3), // 才
    // 4 strokes
    (0x4E0D, 4), // 不
    (0x4E11, 4), // 丑
    (0x4E2D, 4), // 中
    (0x4E30, 4), // 丰
    (0x4E88, 4), // 予
    (0x4E91, 4), // 云
    (0x4E92, 4), // 互
    (0x4E94, 4), // 五
    (0x4E95, 4), // 井
    (0x4EC0, 4), // 什
    (0x4EC1, 4), // 仁
    (0x4ECA, 4), // 今
    (0x4ECB, 4), // 介
    (0x4ECE, 4), // 从
    (0x4EE5, 4), // 以
    (0x516C, 4), // 公
    (0x516D, 4), // 六
    (0x5185, 4), // 内
    (0x5206, 4), // 分
    (0x5207, 4), // 切
    (0x5316, 4), // 化
    (0x5347, 4), // 升
    (0x5348, 4), // 午
    (0x53CB, 4), // 友
    (0x53CD, 4), // 反
    (0x5929, 4), // 天
    (0x592A, 4), // 太
    (0x592B, 4), // 夫
    (0x592D, 4), // 夭
    (0x5B54, 4), // 孔
    (0x5C11, 4), // 少
    (0x5C39, 4), // 尹
    (0x5C3A, 4), // 尺
    (0x5F15, 4), // 引
    (0x5FC3, 4), // 心
    (0x6208, 4), // 戈
    (0x6236, 4), // 户
    (0x624B, 4), // 手
    (0x652F, 4), // 支
    (0x6587, 4), // 文
    (0x6597, 4), // 斗
    (0x65A4, 4), // 斤
    (0x65B9, 4), // 方
    (0x65E5, 4), // 日
    (0x66F0, 4), // 曰
    (0x6708, 4), // 月
    (0x6728, 4), // 木
    (0x6B20, 4), // 欠
    (0x6B62, 4), // 止
    (0x6BD4, 4), // 比
    (0x6BDB, 4), // 毛
    (0x6C0F, 4), // 氏
    (0x6C14, 4), // 气
    (0x6C34, 4), // 水
    (0x706B, 4), // 火
    (0x722A, 4), // 爪
    (0x7236, 4), // 父
    (0x7247, 4), // 片
    (0x725B, 4), // 牛
    (0x72AC, 4), // 犬
    (0x738B, 4), // 王
    // 5 strokes
    (0x4E14, 5), // 且
    (0x4E16, 5), // 世
    (0x4E19, 5), // 丙
    (0x4E1C, 5), // 东
    (0x4E1D, 5), // 丝
    (0x4E3B, 5), // 主
    (0x51AC, 5), // 冬
    (0x51FA, 5), // 出
    (0x5361, 5), // 卡
    (0x53E4, 5), // 古
    (0x53E5, 5), // 句
    (0x53EA, 5), // 只
    (0x53EC, 5), // 召
    (0x53EF, 5), // 可
    (0x53F0, 5), // 台
    (0x53F2, 5), // 史
    (0x53F3, 5), // 右
    (0x53F8, 5), // 司
    (0x5DE6, 5), // 左
    (0x5E73, 5), // 平
    (0x5E7C, 5), // 幼
    (0x5F17, 5), // 弗
    (0x672A, 5), // 未
    (0x672B, 5), // 末
    (0x672C, 5), // 本
    (0x6B63, 5), // 正
    (0x6C11, 5), // 民
    (0x6C41, 5), // 汁
    (0x7530, 5), // 田
    (0x7531, 5), // 由
    (0x7532, 5), // 甲
    (0x7533, 5), // 申
    (0x767D, 5), // 白
    (0x76BF, 5), // 皿
    (0x76EE, 5), // 目
    (0x77DB, 5), // 矛
    (0x77E2, 5), // 矢
    (0x77F3, 5), // 石
    (0x793A, 5), // 示
    // 6 strokes
    (0x4E1E, 6), // 丞
    (0x4E32, 6), // 串
    (0x4E9A, 6), // 亚
    (0x4EA4, 6), // 交
    (0x4EBF, 6), // 亿
    (0x4EEC, 6), // 们
    (0x4EF6, 6), // 件
    (0x4EFB, 6), // 任
    (0x4F17, 6), // 众
    (0x4F1A, 6), // 会
    (0x5149, 6), // 光
    (0x5171, 6), // 共
    (0x5173, 6), // 关
    (0x5174, 6), // 兴
    (0x540C, 6), // 同
    (0x540D, 6), // 名
    (0x5411, 6), // 向
    (0x56DE, 6), // 回
    (0x5730, 6), // 地
    (0x5747, 6), // 均
    (0x597D, 6), // 好
    (0x5B57, 6), // 字
    (0x5B58, 6), // 存
    (0x5B87, 6), // 宇
    (0x5B89, 6), // 安
    (0x5DDE, 6), // 州
    (0x5E74, 6), // 年
    (0x5FD9, 6), // 忙
    (0x6210, 6), // 成
    (0x6536, 6), // 收
    (0x65E9, 6), // 早
    (0x66F2, 6), // 曲
    (0x6709, 6), // 有
    (0x6735, 6), // 朵
    (0x6B21, 6), // 次
    (0x81EA, 6), // 自
    (0x884C, 6), // 行
    (0x897F, 6), // 西
    // 7 strokes
    (0x4F53, 7), // 体
    (0x4F5C, 7), // 作
    (0x4F60, 7), // 你
    (0x5175, 7), // 兵
    (0x5229, 7), // 利
    (0x522B, 7), // 别
    (0x529E, 7), // 办
    (0x542B, 7), // 含
    (0x542C, 7), // 听
    (0x542F, 7), // 启
    (0x544A, 7), // 告
    (0x56E0, 7), // 因
    (0x56ED, 7), // 园
    (0x5B8C, 7), // 完
    (0x6211, 7), // 我
    (0x627E, 7), // 找
    (0x6280, 7), // 技
    (0x62BC, 7), // 押
    (0x674E, 7), // 李
    (0x674F, 7), // 杏
    (0x6BCF, 7), // 每
    (0x6C42, 7), // 求
    (0x6C99, 7), // 沙
    (0x82B1, 7), // 花
    // 8 strokes
    (0x4F86, 8), // 來
    (0x4F9B, 8), // 供
    (0x4F9D, 8), // 依
    (0x4FBF, 8), // 便
    (0x5230, 8), // 到
    (0x54C1, 8), // 品
    (0x5B98, 8), // 官
    (0x5B9A, 8), // 定
    (0x5E97, 8), // 店
    (0x5F80, 8), // 往
    (0x660E, 8), // 明
    (0x670B, 8), // 朋
    (0x670D, 8), // 服
    (0x6790, 8), // 析
    (0x6797, 8), // 林
    (0x679C, 8), // 果
    (0x679D, 8), // 枝
    (0x67AA, 8), // 枪
    (0x67CF, 8), // 柏
    (0x67F1, 8), // 柱
    (0x67F3, 8), // 柳
    (0x6807, 8), // 标
    // 9 strokes
    (0x524D, 9), // 前
    (0x5BA2, 9), // 客
    (0x5BA4, 9), // 室
    (0x6625, 9), // 春
    (0x662F, 9), // 是
    (0x67D0, 9), // 某
    (0x6D77, 9), // 海
    (0x9762, 9), // 面
    // 10 strokes
    (0x5BB6, 10), // 家
    (0x5F92, 10), // 徒
    (0x606D, 10), // 恭
    (0x606F, 10), // 息
    (0x6070, 10), // 恰
    (0x6842, 10), // 桂
    (0x6843, 10), // 桃
    (0x685C, 10), // 桜
    (0x9AD8, 10), // 高
    // 11 strokes
    (0x60C5, 11), // 情
    (0x63A2, 11), // 探
    (0x63A5, 11), // 接
    (0x6DF1, 11), // 深
    (0x6E05, 11), // 清
    (0x7ADE, 11), // 竞
    // 12 strokes
    (0x559C, 12), // 喜
    (0x60B2, 12), // 悲
    (0x611F, 12), // 感
    (0x666F, 12), // 景
    (0x68EE, 12), // 森
    (0x6E29, 12), // 温
    (0x6E56, 12), // 湖
    (0x8857, 12), // 街
    // 13 strokes
    (0x60F3, 13), // 想
    (0x610F, 13), // 意
    (0x611B, 13), // 愛
    (0x6570, 13), // 数
    (0x6E90, 13), // 源
    // 14 strokes
    (0x88FD, 14), // 製
    (0x9700, 14), // 需
    // 15 strokes
    (0x7BC7, 15), // 篇
    (0x7BEE, 15), // 篮
];

/// Build the plural-zh SCUD pack in memory.
///
/// Chinese plural rules (CLDR 44 `plurals.xml`, `<pluralRules
/// locales="zh">`):
///
/// * Cardinal `other` for every input — Chinese lacks grammatical
///   number.
/// * Ordinal `other` for every input.
///
/// The pack ships no rule entries; the engine's fall-through-to-
/// `Other` behaviour handles every query.
fn build_plural_zh_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    chinese_cardinals(&mut b);
    chinese_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-zh SCUD pack in memory.
///
/// Chinese (Simplified) number formatting (CLDR 44 `zh.xml`):
///
/// * Group separator: `,` (comma).
/// * Decimal separator: `.` (dot).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with no space (`50%`,
///   CLDR pattern `#,##0%`).
/// * Currency: CNY `¥`, USD `US$`, EUR `€`, HKD `HK$` all placed
///   before the value with **no space** (`¥1,234.56`, CLDR pattern
///   `¤#,##0.00`).
fn build_number_zh_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
    // Currency: symbol before value with **no** space.
    n.push_currency("CNY", "\u{00A5}", false, false);
    n.push_currency("USD", "US$", false, false);
    n.push_currency("EUR", "\u{20AC}", false, false);
    n.push_currency("HKD", "HK$", false, false);
    n.set_percent("%", true, false);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
