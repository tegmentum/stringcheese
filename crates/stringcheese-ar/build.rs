//! Build-time codegen for the Arabic pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-ar.scud` — CLDR 44 Arabic plural rules. Arabic is the
//!    maximum-category-count locale in the CLDR corpus: `zero`,
//!    `one`, `two`, `few`, `many`, `other`.
//! 2. `number-ar.scud` — CLDR 44 Arabic number-formatting patterns
//!    against the `latn` (Western digits) numbering system. RTL bidi
//!    handling and Arabic-Indic (`arab` / `arabext`) digit shapes
//!    are deferred.
//!
//! Both are gated behind the `plural-scud` / `number-scud` Cargo
//! features on the runtime side, but the build.rs unconditionally
//! writes them so `cargo build --features plural-scud` finds the
//! file.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{arabic_cardinals, arabic_ordinals};
use stringcheese_scud::{
    CAP_DATETIME, CAP_NUMBER, CAP_PLURAL, DateTimeLength, DateTimeSectionBuilder,
    NumberSectionBuilder, PluralSectionBuilder, SECT_AM_PM, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DATE_PATTERNS, SECT_DECIMAL_PATTERN, SECT_ERA_NAMES, SECT_MONTH_ABBR,
    SECT_MONTH_NAMES, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_TIME_PATTERNS,
    SECT_WEEKDAY_ABBR, SECT_WEEKDAY_NAMES, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let plural_path = out_dir.join("plural-ar.scud");
    let plural_bytes = build_plural_ar_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-ar.scud");
    let number_bytes = build_number_ar_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-ar.scud");
    let datetime_bytes = build_datetime_ar_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the datetime-ar SCUD pack in memory.
///
/// Arabic date/time formatting (CLDR 44.1, `gregorian.json`):
///
/// * Date patterns (with U+200F RTL MARK between numeric fields
///   and slashes so a bidi-aware renderer produces right-to-left
///   visual order):
///   * short — `d‏/M‏/y`
///   * medium — `dd‏/MM‏/y`
///   * long — `d MMMM y`
///   * full — `EEEE، d MMMM y` (Arabic comma U+060C after the
///     weekday)
/// * Time patterns:
///   * short — `h:mm a` (12-hour with AM/PM)
///   * medium/long/full — `h:mm:ss a`
/// * Month names in Arabic (`يناير`, `فبراير`, …). CLDR ships the
///   same strings for full and abbreviated forms.
/// * Weekday names — `الأحد` (Sunday-first) through `السبت`.
/// * AM/PM markers — `ص` (ante meridiem) / `م` (post meridiem).
/// * Era names — `ق.م` (BC), `م` (AD).
///
/// **Deferred: RTL bidi rendering.** The pattern strings include
/// the RTL marks CLDR ships; stringcheese emits them into the
/// output verbatim so a downstream bidi-aware shaper produces the
/// culturally-correct visual result. stringcheese does not itself
/// perform bidi shaping.
fn build_datetime_ar_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    // U+200F = RIGHT-TO-LEFT MARK. Falls through the pattern
    // interpreter as a literal since it is not ASCII alphabetic.
    d.set_date_pattern(DateTimeLength::Short, "d\u{200F}/M\u{200F}/y");
    d.set_date_pattern(DateTimeLength::Medium, "dd\u{200F}/MM\u{200F}/y");
    d.set_date_pattern(DateTimeLength::Long, "d MMMM y");
    // U+060C = ARABIC COMMA.
    d.set_date_pattern(DateTimeLength::Full, "EEEE\u{060C} d MMMM y");
    d.set_time_pattern(DateTimeLength::Short, "h:mm a");
    d.set_time_pattern(DateTimeLength::Medium, "h:mm:ss a");
    d.set_time_pattern(DateTimeLength::Long, "h:mm:ss a");
    d.set_time_pattern(DateTimeLength::Full, "h:mm:ss a");
    d.set_month_names([
        "\u{064A}\u{0646}\u{0627}\u{064A}\u{0631}", // يناير
        "\u{0641}\u{0628}\u{0631}\u{0627}\u{064A}\u{0631}", // فبراير
        "\u{0645}\u{0627}\u{0631}\u{0633}",         // مارس
        "\u{0623}\u{0628}\u{0631}\u{064A}\u{0644}", // أبريل
        "\u{0645}\u{0627}\u{064A}\u{0648}",         // مايو
        "\u{064A}\u{0648}\u{0646}\u{064A}\u{0648}", // يونيو
        "\u{064A}\u{0648}\u{0644}\u{064A}\u{0648}", // يوليو
        "\u{0623}\u{063A}\u{0633}\u{0637}\u{0633}", // أغسطس
        "\u{0633}\u{0628}\u{062A}\u{0645}\u{0628}\u{0631}", // سبتمبر
        "\u{0623}\u{0643}\u{062A}\u{0648}\u{0628}\u{0631}", // أكتوبر
        "\u{0646}\u{0648}\u{0641}\u{0645}\u{0628}\u{0631}", // نوفمبر
        "\u{062F}\u{064A}\u{0633}\u{0645}\u{0628}\u{0631}", // ديسمبر
    ]);
    // CLDR ships the same strings for full and abbreviated forms.
    d.set_month_abbreviations([
        "\u{064A}\u{0646}\u{0627}\u{064A}\u{0631}",
        "\u{0641}\u{0628}\u{0631}\u{0627}\u{064A}\u{0631}",
        "\u{0645}\u{0627}\u{0631}\u{0633}",
        "\u{0623}\u{0628}\u{0631}\u{064A}\u{0644}",
        "\u{0645}\u{0627}\u{064A}\u{0648}",
        "\u{064A}\u{0648}\u{0646}\u{064A}\u{0648}",
        "\u{064A}\u{0648}\u{0644}\u{064A}\u{0648}",
        "\u{0623}\u{063A}\u{0633}\u{0637}\u{0633}",
        "\u{0633}\u{0628}\u{062A}\u{0645}\u{0628}\u{0631}",
        "\u{0623}\u{0643}\u{062A}\u{0648}\u{0628}\u{0631}",
        "\u{0646}\u{0648}\u{0641}\u{0645}\u{0628}\u{0631}",
        "\u{062F}\u{064A}\u{0633}\u{0645}\u{0628}\u{0631}",
    ]);
    d.set_weekday_names([
        "\u{0627}\u{0644}\u{0623}\u{062D}\u{062F}", // الأحد
        "\u{0627}\u{0644}\u{0627}\u{062B}\u{0646}\u{064A}\u{0646}", // الاثنين
        "\u{0627}\u{0644}\u{062B}\u{0644}\u{0627}\u{062B}\u{0627}\u{0621}", // الثلاثاء
        "\u{0627}\u{0644}\u{0623}\u{0631}\u{0628}\u{0639}\u{0627}\u{0621}", // الأربعاء
        "\u{0627}\u{0644}\u{062E}\u{0645}\u{064A}\u{0633}", // الخميس
        "\u{0627}\u{0644}\u{062C}\u{0645}\u{0639}\u{0629}", // الجمعة
        "\u{0627}\u{0644}\u{0633}\u{0628}\u{062A}", // السبت
    ]);
    d.set_weekday_abbreviations([
        "\u{0627}\u{0644}\u{0623}\u{062D}\u{062F}",
        "\u{0627}\u{0644}\u{0627}\u{062B}\u{0646}\u{064A}\u{0646}",
        "\u{0627}\u{0644}\u{062B}\u{0644}\u{0627}\u{062B}\u{0627}\u{0621}",
        "\u{0627}\u{0644}\u{0623}\u{0631}\u{0628}\u{0639}\u{0627}\u{0621}",
        "\u{0627}\u{0644}\u{062E}\u{0645}\u{064A}\u{0633}",
        "\u{0627}\u{0644}\u{062C}\u{0645}\u{0639}\u{0629}",
        "\u{0627}\u{0644}\u{0633}\u{0628}\u{062A}",
    ]);
    // AM = "ص" (sabāh, "morning"), PM = "م" (masā', "evening").
    d.set_am_pm("\u{0635}", "\u{0645}");
    // BC = "ق.م" (qabl al-milad), AD = "م" (milad).
    d.set_eras("\u{0642}.\u{0645}", "\u{0645}");
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("ar"));
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

/// Build the plural-ar SCUD pack in memory.
///
/// Arabic plural rules (CLDR 44 `plurals.xml`, `<pluralRules
/// locales="ar">`) — the six-category maximum:
///
/// * Cardinal `zero` when `n = 0`.
/// * Cardinal `one` when `n = 1`.
/// * Cardinal `two` when `n = 2`.
/// * Cardinal `few` when `n % 100 in 3..10` (3-10, 103-110, …).
/// * Cardinal `many` when `n % 100 in 11..99` (11-99, 111-199, …).
/// * Cardinal `other` otherwise (100, 101, 102, 200, …).
/// * Ordinal `other` for every value (CLDR 44 ships no distinct
///   ordinal buckets for Arabic).
fn build_plural_ar_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    arabic_cardinals(&mut b);
    arabic_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("ar"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-ar SCUD pack in memory.
///
/// Arabic number formatting (CLDR 44 `ar.xml` — `latn` numbering
/// system default; the native `arab` / `arabext` shapes are deferred
/// to a follow-up):
///
/// * Group separator: `,` (comma) under `latn`.
/// * Decimal separator: `.` (dot) under `latn`.
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with no space (`50%`,
///   pattern `#,##0%`). Locale-specific percent sign U+066A is a
///   deferred follow-up.
/// * Currency: SAR `ر.س.` (Saudi riyal), USD `US$`, EUR `€`,
///   AED `د.إ.` all placed before the value with a space
///   (CLDR pattern `¤ #,##0.00`).
///
/// **RTL bidi handling is deferred.** The output string carries the
/// digits and symbol in logical order; a downstream shaper is
/// responsible for the visual reversal Arabic contexts expect.
fn build_number_ar_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
    // Currency: symbol before value with a space. `symbol_after =
    // false`, `symbol_spaced = true`.
    n.push_currency("SAR", "\u{0631}.\u{0633}.\u{200F}", false, true);
    n.push_currency("USD", "US$", false, true);
    n.push_currency("EUR", "\u{20AC}", false, true);
    n.push_currency("AED", "\u{062F}.\u{0625}.\u{200F}", false, true);
    // Percent has no space in Arabic per CLDR 44 (pattern `#,##0%`).
    n.set_percent("%", true, false);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("ar"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
