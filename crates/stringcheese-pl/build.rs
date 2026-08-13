//! Build-time codegen for the Polish pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-pl.scud` — CLDR 44 Polish plural rules (Phase 3 of the
//!    WIT-i18n subsystem).
//! 2. `number-pl.scud` — CLDR 44 Polish number-formatting patterns.
//!
//! Both are gated behind the `plural-scud` / `number-scud` Cargo
//! features on the runtime side, but the build.rs unconditionally
//! writes them so `cargo build --features plural-scud` finds the
//! file.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{polish_cardinals, polish_ordinals};
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

    let plural_path = out_dir.join("plural-pl.scud");
    let plural_bytes = build_plural_pl_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-pl.scud");
    let number_bytes = build_number_pl_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-pl.scud");
    let datetime_bytes = build_datetime_pl_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the datetime-pl SCUD pack in memory.
///
/// Polish date/time formatting (CLDR 44.1, `gregorian.json`):
///
/// * Date patterns:
///   * short — `dd.MM.y` (`22.09.2024`)
///   * medium — `d MMM y` (`22 wrz 2024`)
///   * long — `d MMMM y` (`22 września 2024`)
///   * full — `EEEE, d MMMM y` (`niedziela, 22 września 2024`)
/// * Time patterns (Polish uses 24-hour throughout):
///   * short — `HH:mm`
///   * medium/long/full — `HH:mm:ss`
/// * Month names in the CLDR `format` (genitive) context:
///   `stycznia`, `lutego`, …; `stand-alone` (nominative) is a
///   documented follow-up.
/// * Era names — `p.n.e.` (BC), `n.e.` (AD).
fn build_datetime_pl_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    d.set_date_pattern(DateTimeLength::Short, "dd.MM.y");
    d.set_date_pattern(DateTimeLength::Medium, "d MMM y");
    d.set_date_pattern(DateTimeLength::Long, "d MMMM y");
    d.set_date_pattern(DateTimeLength::Full, "EEEE, d MMMM y");
    d.set_time_pattern(DateTimeLength::Short, "HH:mm");
    d.set_time_pattern(DateTimeLength::Medium, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Long, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Full, "HH:mm:ss");
    d.set_month_names([
        "stycznia",
        "lutego",
        "marca",
        "kwietnia",
        "maja",
        "czerwca",
        "lipca",
        "sierpnia",
        "wrze\u{015B}nia",
        "pa\u{017A}dziernika",
        "listopada",
        "grudnia",
    ]);
    d.set_month_abbreviations([
        "sty",
        "lut",
        "mar",
        "kwi",
        "maj",
        "cze",
        "lip",
        "sie",
        "wrz",
        "pa\u{017A}",
        "lis",
        "gru",
    ]);
    d.set_weekday_names([
        "niedziela",
        "poniedzia\u{0142}ek",
        "wtorek",
        "\u{015B}roda",
        "czwartek",
        "pi\u{0105}tek",
        "sobota",
    ]);
    d.set_weekday_abbreviations(["niedz.", "pon.", "wt.", "\u{015B}r.", "czw.", "pt.", "sob."]);
    d.set_am_pm("AM", "PM");
    d.set_eras("p.n.e.", "n.e.");
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("pl"));
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

/// Build the plural-pl SCUD pack in memory.
///
/// Polish plural rules (CLDR 44 `plurals.xml`, `<pluralRules
/// locales="pl">`):
///
/// * Cardinal `one` when `i = 1 and v = 0` (1 exactly).
/// * Cardinal `few` when `v = 0 and i % 10 in 2..4 and i % 100 not
///   in 12..14` (2-4, 22-24, …). Shares the `SlavFew` predicate with
///   Russian.
/// * Cardinal `many` when `v = 0 and ((i != 1 and i % 10 in 0..1)
///   or i % 10 in 5..9 or i % 100 in 12..14)` (0, 5-19, 25-29, …).
/// * Cardinal `other` for every fractional input.
/// * Ordinal `other` for every value (CLDR 44 ships no distinct
///   ordinal buckets for Polish).
fn build_plural_pl_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    polish_cardinals(&mut b);
    polish_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("pl"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-pl SCUD pack in memory.
///
/// Polish number formatting (CLDR 44 `pl.xml`):
///
/// * Group separator: U+00A0 (NO-BREAK SPACE).
/// * Decimal separator: `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with **no space** (`50%`).
///   Pattern `#,##0%` — Polish is the only shipped Phase 3 locale
///   whose percent lacks a space before the symbol.
/// * Currency: PLN `zł`, USD `$`, EUR `€`, GBP `£` all placed after
///   the value with a space (`1 234,56 zł`). Pattern `#,##0.00 ¤`.
fn build_number_pl_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern("\u{00A0}", ",", 0, 3, 3, 3);
    n.push_currency("PLN", "z\u{0142}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    // Polish percent has no space before `%` — the pattern is
    // `#,##0%` per CLDR 44 `pl.xml`.
    n.set_percent("%", true, false);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("pl"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
