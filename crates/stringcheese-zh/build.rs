//! Build-time codegen for the Chinese (Simplified) pack.
//!
//! Six SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-zh.scud` — CLDR 44 Chinese plural rules. Chinese lacks
//!    grammatical number so the CLDR rule set is `other` only; the
//!    pack contains no plural predicates and every query falls
//!    through to [`PluralCategory::Other`](
//!    stringcheese_icu_plural::PluralCategory::Other).
//! 2. `number-zh.scud` — CLDR 44 Chinese number-formatting patterns.
//! 3. `case-zh.scud` — ASCII a-z ↔ A-Z plus German ß expansions.
//! 4. `collation-zh.scud` — DUCET-root + German ß expansions.
//! 5. `datetime-zh.scud` — CLDR 44.1 Gregorian date/time patterns.
//! 6. `word-dict-zh.scud` — ~500-entry starter word dictionary for
//!    the FMM-based CJK word segmenter. Full `CC-CEDICT` integration
//!    is a documented data-only follow-up.

#[path = "build_dict.rs"]
mod build_dict;

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{chinese_cardinals, chinese_ordinals};
use stringcheese_scud::{
    BreakSectionBuilder, CAP_BREAK, CAP_CASE, CAP_COLLATION, CAP_DATETIME, CAP_NUMBER, CAP_PLURAL,
    CaseSectionBuilder, CollationSectionBuilder, DateTimeLength, DateTimeSectionBuilder,
    NumberSectionBuilder, PluralSectionBuilder, SECT_AM_PM, SECT_CARDINAL_RULES,
    SECT_COLLATION_OPTIONS, SECT_CURRENCY_TABLE, SECT_DATE_PATTERNS, SECT_DECIMAL_PATTERN,
    SECT_ERA_NAMES, SECT_EXPANSIONS, SECT_FULL_FOLD, SECT_FULL_UPPER, SECT_GRAPHEME_CLASSES,
    SECT_GRAPHEME_RULES, SECT_MONTH_ABBR, SECT_MONTH_NAMES, SECT_ORDINAL_RULES,
    SECT_PERCENT_PATTERN, SECT_SENTENCE_CLASSES, SECT_SENTENCE_RULES, SECT_SIMPLE_FOLD,
    SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, SECT_TIME_PATTERNS, SECT_WEEKDAY_ABBR,
    SECT_WEEKDAY_NAMES, SECT_WORD_CLASSES, SECT_WORD_DICT, SECT_WORD_RULES, ScudWriter,
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

    let word_dict_path = out_dir.join("word-dict-zh.scud");
    let word_dict_bytes = build_word_dict_zh_scud();
    fs::write(&word_dict_path, &word_dict_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", word_dict_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_dict.rs");
}

/// Build the word-dict-zh SCUD pack in memory.
///
/// Ships a ~500-entry hand-curated starter dictionary drawn from
/// HSK levels 1-4 vocabulary and public-domain Simplified Chinese
/// frequency lists — enough to give the FMM segmenter a working
/// sample on common vocabulary. Ships a `RULES_UAX29_DEFAULT`
/// marker on all three UAX #29 axes for non-CJK runs; the segment
/// engine consults the dictionary only for CJK-script runs and
/// falls through to UAX #29 defaults elsewhere.
///
/// Full `CC-CEDICT` integration (~130k entries, multi-MB) is a
/// documented data-only follow-up SCUD pack.
fn build_word_dict_zh_scud() -> Vec<u8> {
    let mut b = BreakSectionBuilder::new();
    b.set_default_rules();
    for word in build_dict::ZH_WORDS {
        b.push_dict_entry(word);
    }
    let mut w = ScudWriter::new(CAP_BREAK, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
    w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
    w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
    w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
    w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
    w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
    w.append_section(SECT_WORD_DICT, &b.word_dict_bytes());
    w.finish()
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
/// # Phase 2 deferrals
///
/// CLDR ships multiple `zh` collations — `standard` (stroke-based,
/// numeric stroke order), `pinyin` (Latin pinyin transliteration),
/// `stroke`, and `zhuyin`. All four require large Han-to-order or
/// Han-to-pinyin lookup tables plus algorithm support:
///
/// * **Stroke-based** — a ~90k-entry `(codepoint → stroke count)`
///   table, plus a secondary key for tied stroke counts.
/// * **Pinyin** — a ~40k-entry `(codepoint → pinyin string)` table,
///   plus tone handling.
///
/// Neither table ships in Phase 2. The pack uses feruca's
/// DUCET-root ordering, which sorts CJK Han by codepoint order
/// — deterministic but not linguistically meaningful. See
/// `docs/design/wit-i18n.md` § 8.2 for the deferral rationale
/// and the follow-up plan.
///
/// The pack ships:
///
/// * **German ß / ẞ expansions** — uniform composed-engine
///   behaviour.
/// * **Default strength tertiary.**
fn build_collation_zh_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // German ß expansion — uniform composed-engine behaviour.
    c.push_expansion(0x00DF, &[0x0073, 0x0073]);
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]);

    c.set_default_strength(2);
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.finish()
}

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
