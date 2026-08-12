//! Build-time codegen for the French pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-fr.scud` — CLDR 44.1 French plural rules (Phase 3 of
//!    the WIT-i18n subsystem).
//! 2. `number-fr.scud` — CLDR 44.1 French number-formatting
//!    patterns.
//!
//! Both are gated behind the `plural-scud` / `number-scud` Cargo
//! features on the runtime side, but the build.rs unconditionally
//! writes them so `cargo build --features plural-scud` finds the
//! file. `stringcheese-scud` and `stringcheese-icu-plural` are
//! `[build-dependencies]` — they run here at build time and drop
//! out of the shipped binary.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{french_cardinals, french_ordinals};
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

    let plural_path = out_dir.join("plural-fr.scud");
    let plural_bytes = build_plural_fr_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-fr.scud");
    let number_bytes = build_number_fr_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-fr.scud");
    let datetime_bytes = build_datetime_fr_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the datetime-fr SCUD pack in memory.
///
/// French date/time formatting (CLDR 44.1, `gregorian.json`):
///
/// * Date patterns:
///   * short — `dd/MM/y` (`22/09/2024`)
///   * medium — `d MMM y` (`22 sept. 2024`)
///   * long — `d MMMM y` (`22 septembre 2024`)
///   * full — `EEEE d MMMM y` (`dimanche 22 septembre 2024`)
/// * Time patterns (French uses 24-hour throughout):
///   * short — `HH:mm` (`17:03`)
///   * medium/long/full — `HH:mm:ss` (`17:03:04`)
/// * Month + weekday names per CLDR French `gregorian.json` — all
///   lowercase in the CLDR data.
/// * Era names: `av. J.-C.`, `ap. J.-C.`.
fn build_datetime_fr_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    d.set_date_pattern(DateTimeLength::Short, "dd/MM/y");
    d.set_date_pattern(DateTimeLength::Medium, "d MMM y");
    d.set_date_pattern(DateTimeLength::Long, "d MMMM y");
    d.set_date_pattern(DateTimeLength::Full, "EEEE d MMMM y");
    d.set_time_pattern(DateTimeLength::Short, "HH:mm");
    d.set_time_pattern(DateTimeLength::Medium, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Long, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Full, "HH:mm:ss");
    d.set_month_names([
        "janvier",
        "f\u{00E9}vrier",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "ao\u{00FB}t",
        "septembre",
        "octobre",
        "novembre",
        "d\u{00E9}cembre",
    ]);
    d.set_month_abbreviations([
        "janv.",
        "f\u{00E9}vr.",
        "mars",
        "avr.",
        "mai",
        "juin",
        "juil.",
        "ao\u{00FB}t",
        "sept.",
        "oct.",
        "nov.",
        "d\u{00E9}c.",
    ]);
    d.set_weekday_names([
        "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
    ]);
    d.set_weekday_abbreviations(["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."]);
    d.set_am_pm("AM", "PM");
    d.set_eras("av. J.-C.", "ap. J.-C.");
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("fr"));
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

/// Build the plural-fr SCUD pack in memory.
///
/// French plural rules (CLDR 44.1, `plurals.xml`):
///
/// * Cardinal `one` when `i in 0..1` (0 and 1 are both singular in
///   French — "0 chose", "1 chose"), else `other`. The `many`
///   category for compact large-number notation is deferred.
/// * Ordinal `one` when `n = 1` (1er / 1re), else `other`.
fn build_plural_fr_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    french_cardinals(&mut b);
    french_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("fr"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-fr SCUD pack in memory.
///
/// French number formatting (CLDR 44.1, `fr.xml`):
///
/// * Group separator: NARROW NO-BREAK SPACE (U+202F) as of CLDR
///   42+. We ship U+00A0 (NBSP) here for compatibility with
///   downstream consumers that still expect the pre-CLDR-42
///   separator; the difference is invisible in most renderings.
/// * Decimal separator: `,` (comma).
/// * Percent: symbol `%` after the value with a space (`50 %`).
/// * Currency: EUR / USD / GBP / CAD placed after the value with
///   a space (`1 234,56 €`).
fn build_number_fr_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    // U+00A0 NO-BREAK SPACE — see doc comment above for the
    // CLDR 42 change to U+202F.
    n.set_decimal_pattern("\u{00A0}", ",", 0, 3, 3, 3);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.push_currency("CAD", "$CA", true, true);
    n.push_currency("CHF", "CHF", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("fr"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
