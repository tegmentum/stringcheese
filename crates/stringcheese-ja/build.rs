//! Build-time codegen for the Japanese pack.
//!
//! Four SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-ja.scud` — CLDR 44 Japanese plural rules. Japanese
//!    lacks grammatical number so the CLDR rule set is `other` only;
//!    the pack contains no plural predicates and every query falls
//!    through to [`PluralCategory::Other`](
//!    stringcheese_icu_plural::PluralCategory::Other).
//! 2. `number-ja.scud` — CLDR 44 Japanese number-formatting
//!    patterns.
//! 3. `datetime-ja.scud` — CLDR 44.1 Gregorian date/time patterns.
//! 4. `word-dict-ja.scud` — ~500-entry starter word dictionary for
//!    the FMM-based CJK word segmenter. Full IPADIC / `JMdict`
//!    integration is a documented data-only follow-up.

#[path = "build_dict.rs"]
mod build_dict;

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{japanese_cardinals, japanese_ordinals};
use stringcheese_scud::{
    BreakSectionBuilder, CAP_BREAK, CAP_DATETIME, CAP_NUMBER, CAP_PLURAL, DateTimeLength,
    DateTimeSectionBuilder, NumberSectionBuilder, PluralSectionBuilder, SECT_AM_PM,
    SECT_CARDINAL_RULES, SECT_CURRENCY_TABLE, SECT_DATE_PATTERNS, SECT_DECIMAL_PATTERN,
    SECT_ERA_NAMES, SECT_GRAPHEME_CLASSES, SECT_GRAPHEME_RULES, SECT_MONTH_ABBR, SECT_MONTH_NAMES,
    SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_SENTENCE_CLASSES, SECT_SENTENCE_RULES,
    SECT_TIME_PATTERNS, SECT_WEEKDAY_ABBR, SECT_WEEKDAY_NAMES, SECT_WORD_CLASSES, SECT_WORD_DICT,
    SECT_WORD_RULES, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let plural_path = out_dir.join("plural-ja.scud");
    let plural_bytes = build_plural_ja_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-ja.scud");
    let number_bytes = build_number_ja_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-ja.scud");
    let datetime_bytes = build_datetime_ja_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    let word_dict_path = out_dir.join("word-dict-ja.scud");
    let word_dict_bytes = build_word_dict_ja_scud();
    fs::write(&word_dict_path, &word_dict_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", word_dict_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_dict.rs");
}

/// Build the word-dict-ja SCUD pack in memory.
///
/// Ships a ~500-entry hand-curated starter dictionary drawn from
/// public-domain Japanese-learner frequency lists — enough to give
/// the FMM segmenter a working sample on common vocabulary. Ships
/// a `RULES_UAX29_DEFAULT` marker on all three UAX #29 axes for
/// non-CJK runs; the segment engine consults the dictionary only
/// for CJK-script runs and falls through to UAX #29 defaults
/// elsewhere.
///
/// Full IPADIC / `JMdict` integration (100k+ entries, multi-MB) is a
/// documented data-only follow-up SCUD pack.
fn build_word_dict_ja_scud() -> Vec<u8> {
    let mut b = BreakSectionBuilder::new();
    b.set_default_rules();
    for word in build_dict::JA_WORDS {
        b.push_dict_entry(word);
    }
    let mut w = ScudWriter::new(CAP_BREAK, CLDR_VERSION, Some("ja"));
    w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
    w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
    w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
    w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
    w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
    w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
    w.append_section(SECT_WORD_DICT, &b.word_dict_bytes());
    w.finish()
}

/// Build the datetime-ja SCUD pack in memory.
///
/// Japanese date/time formatting (CLDR 44.1, `gregorian.json`,
/// Gregorian calendar — the Imperial `japanese` calendar is a
/// documented follow-up):
///
/// * Date patterns:
///   * short — `y/MM/dd` (`2024/09/22`)
///   * medium — `y/MM/dd`
///   * long — `y年M月d日` (`2024年9月22日`)
///   * full — `y年M月d日EEEE` (`2024年9月22日日曜日`)
/// * Time patterns (24-hour default):
///   * short — `HH:mm`
///   * medium/long/full — `HH:mm:ss`
/// * Month names — numeric-with-suffix (`1月`..`12月`); Japanese
///   uses the same shape for wide and abbreviated forms.
/// * Weekday names — `日曜日` (Sunday-first) through `土曜日`;
///   abbreviated to single-character `日`..`土`.
/// * AM/PM — `午前` (AM) / `午後` (PM). Shipped for completeness.
/// * Era names — `紀元前` (BC), `西暦` (AD).
fn build_datetime_ja_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    d.set_date_pattern(DateTimeLength::Short, "y/MM/dd");
    d.set_date_pattern(DateTimeLength::Medium, "y/MM/dd");
    d.set_date_pattern(DateTimeLength::Long, "y\u{5E74}M\u{6708}d\u{65E5}");
    d.set_date_pattern(DateTimeLength::Full, "y\u{5E74}M\u{6708}d\u{65E5}EEEE");
    d.set_time_pattern(DateTimeLength::Short, "HH:mm");
    d.set_time_pattern(DateTimeLength::Medium, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Long, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Full, "HH:mm:ss");
    // Wide + abbreviated month names both use numeric + 月 suffix
    // in CLDR Japanese.
    d.set_month_names([
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
        "\u{65E5}\u{66DC}\u{65E5}", // 日曜日 (Sunday)
        "\u{6708}\u{66DC}\u{65E5}", // 月曜日 (Monday)
        "\u{706B}\u{66DC}\u{65E5}", // 火曜日
        "\u{6C34}\u{66DC}\u{65E5}", // 水曜日
        "\u{6728}\u{66DC}\u{65E5}", // 木曜日
        "\u{91D1}\u{66DC}\u{65E5}", // 金曜日
        "\u{571F}\u{66DC}\u{65E5}", // 土曜日
    ]);
    d.set_weekday_abbreviations([
        "\u{65E5}", // 日
        "\u{6708}", // 月
        "\u{706B}", "\u{6C34}", "\u{6728}", "\u{91D1}", "\u{571F}",
    ]);
    d.set_am_pm("\u{5348}\u{524D}", "\u{5348}\u{5F8C}"); // 午前 / 午後
    // Gregorian eras. The Japanese Imperial calendar (Reiwa /
    // Heisei / Shōwa …) is a documented follow-up.
    d.set_eras("\u{7D00}\u{5143}\u{524D}", "\u{897F}\u{66A6}"); // 紀元前 / 西暦
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("ja"));
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

/// Build the plural-ja SCUD pack in memory.
///
/// Japanese plural rules (CLDR 44 `plurals.xml`, `<pluralRules
/// locales="ja">`):
///
/// * Cardinal `other` for every input — Japanese lacks grammatical
///   number.
/// * Ordinal `other` for every input.
///
/// The pack ships no rule entries.
fn build_plural_ja_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    japanese_cardinals(&mut b);
    japanese_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("ja"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-ja SCUD pack in memory.
///
/// Japanese number formatting (CLDR 44 `ja.xml`):
///
/// * Group separator: `,` (comma).
/// * Decimal separator: `.` (dot).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with no space (`50%`,
///   CLDR pattern `#,##0%`).
/// * Currency: JPY `¥`, USD `US$`, EUR `€`, CNY `CN¥` all placed
///   before the value with **no space** (`¥1,234`, CLDR pattern
///   `¤#,##0`). The default currency pattern has zero fraction
///   digits because the yen — Japan's native currency — has no
///   sub-unit; the [`NumberEngine::format_currency`] path forces 2
///   fraction digits by default, so downstream callers who want
///   the culturally-correct `¥1234` (no fraction) must pass
///   [`FormattingOptions`] with `min_fraction = Some(0)` and
///   `max_fraction = Some(0)`.
///
/// [`NumberEngine::format_currency`]:
///     stringcheese_icu_number::NumberEngine::format_currency
/// [`FormattingOptions`]: stringcheese_icu_number::FormattingOptions
fn build_number_ja_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
    n.push_currency("JPY", "\u{00A5}", false, false);
    n.push_currency("USD", "US$", false, false);
    n.push_currency("EUR", "\u{20AC}", false, false);
    n.push_currency("CNY", "CN\u{00A5}", false, false);
    n.set_percent("%", true, false);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("ja"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
