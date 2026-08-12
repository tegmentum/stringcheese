//! Build-time codegen for the German pack.
//!
//! Two artifacts emitted into `$OUT_DIR`:
//!
//! 1. `generated.rs` — the `stringcheese-lang-gen`-derived
//!    stopword slice and capability record. Read from
//!    `rules/de.toml`.
//! 2. `collation-de.scud` — the CLDR-derived collation-tailoring
//!    SCUD pack shipped by the crate under the `collation-scud`
//!    feature. See `docs/design/wit-i18n.md` § 6 and the crate
//!    root docs for the pack shape.
//!
//! Both `stringcheese-lang-gen` and `stringcheese-scud` are
//! `[build-dependencies]` — they run here at build time and drop
//! out of the shipped binary. A caller pulling
//! `stringcheese-de = "0.1"` pays zero cost for either generator's
//! presence.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{german_cardinals, german_ordinals};
use stringcheese_scud::{
    CAP_COLLATION, CAP_DATETIME, CAP_NUMBER, CAP_PLURAL, CollationSectionBuilder, DateTimeLength,
    DateTimeSectionBuilder, NumberSectionBuilder, PluralSectionBuilder, SECT_AM_PM,
    SECT_CARDINAL_RULES, SECT_COLLATION_OPTIONS, SECT_CURRENCY_TABLE, SECT_DATE_PATTERNS,
    SECT_DECIMAL_PATTERN, SECT_ERA_NAMES, SECT_EXPANSIONS, SECT_MONTH_ABBR, SECT_MONTH_NAMES,
    SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_TIME_PATTERNS, SECT_WEEKDAY_ABBR,
    SECT_WEEKDAY_NAMES, ScudWriter,
};

/// CLDR version the shipped tables were compiled against. Bumping
/// this value is a coordinated release action — the SCUD file
/// header carries this string so downstream can trace data
/// provenance.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let rules = "rules/de.toml";
    let out = out_dir.join("generated.rs");
    stringcheese_lang_gen::generate(rules, &out)
        .unwrap_or_else(|e| panic!("stringcheese-lang-gen failed on {rules}: {e}"));

    let collation_path = out_dir.join("collation-de.scud");
    let collation_bytes = build_collation_de_scud();
    fs::write(&collation_path, &collation_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", collation_path.display()));

    let plural_path = out_dir.join("plural-de.scud");
    let plural_bytes = build_plural_de_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-de.scud");
    let number_bytes = build_number_de_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    let datetime_path = out_dir.join("datetime-de.scud");
    let datetime_bytes = build_datetime_de_scud();
    fs::write(&datetime_path, &datetime_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", datetime_path.display()));

    println!("cargo:rerun-if-changed={rules}");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the datetime-de SCUD pack in memory.
///
/// German date/time formatting (CLDR 44.1, `gregorian.json`):
///
/// * Date patterns:
///   * short — `dd.MM.y` (`22.09.2024`)
///   * medium — `dd.MM.y` (`22.09.2024`; CLDR's German pack ships
///     the same pattern at short and medium)
///   * long — `d. MMMM y` (`22. September 2024`)
///   * full — `EEEE, d. MMMM y` (`Sonntag, 22. September 2024`)
/// * Time patterns (German uses 24-hour throughout, no am/pm):
///   * short — `HH:mm` (`17:03`)
///   * medium/long/full — `HH:mm:ss` (`17:03:04`)
/// * Month + weekday names per CLDR German `gregorian.json`.
/// * AM/PM markers: `AM`, `PM` — shipped for completeness (the
///   patterns never emit the `a` token).
/// * Era names: `v. Chr.`, `n. Chr.` (BC, AD in German CLDR).
fn build_datetime_de_scud() -> Vec<u8> {
    let mut d = DateTimeSectionBuilder::new();
    d.set_date_pattern(DateTimeLength::Short, "dd.MM.y");
    d.set_date_pattern(DateTimeLength::Medium, "dd.MM.y");
    d.set_date_pattern(DateTimeLength::Long, "d. MMMM y");
    d.set_date_pattern(DateTimeLength::Full, "EEEE, d. MMMM y");
    d.set_time_pattern(DateTimeLength::Short, "HH:mm");
    d.set_time_pattern(DateTimeLength::Medium, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Long, "HH:mm:ss");
    d.set_time_pattern(DateTimeLength::Full, "HH:mm:ss");
    d.set_month_names([
        "Januar",
        "Februar",
        "M\u{00E4}rz",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ]);
    d.set_month_abbreviations([
        "Jan.",
        "Feb.",
        "M\u{00E4}rz",
        "Apr.",
        "Mai",
        "Juni",
        "Juli",
        "Aug.",
        "Sept.",
        "Okt.",
        "Nov.",
        "Dez.",
    ]);
    d.set_weekday_names([
        "Sonntag",
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
    ]);
    d.set_weekday_abbreviations(["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."]);
    d.set_am_pm("AM", "PM");
    d.set_eras("v. Chr.", "n. Chr.");
    let mut w = ScudWriter::new(CAP_DATETIME, CLDR_VERSION, Some("de"));
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

/// Build the plural-de SCUD pack in memory.
///
/// German plural rules (CLDR 44.1, `plurals.xml`):
///
/// * Cardinal `one` when `i = 1 and v = 0`, else `other`.
/// * Ordinal: everything is `other` (German does not distinguish
///   ordinal plural forms in CLDR).
fn build_plural_de_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    german_cardinals(&mut b);
    german_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("de"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-de SCUD pack in memory.
///
/// German number formatting (CLDR 44.1, `de.xml`):
///
/// * Group separator `.` (point!), decimal separator `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits.
/// * Percent: symbol `%` after the value with a space (`50 %`).
/// * Currency: EUR `€`, USD `$`, GBP `£` all placed after the
///   value with a space (`1.234,56 €`).
fn build_number_de_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(".", ",", 0, 3, 3, 3);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.push_currency("CHF", "CHF", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("de"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}

/// Build the collation-de SCUD pack in memory.
///
/// German collation follows DIN 5007. Two conventions coexist:
///
/// * **DIN 5007-1 (dictionary / Duden ordering)** — umlauts fold
///   to their base letter (`ä = a`, `ö = o`, `ü = u`); `ß = ss`.
/// * **DIN 5007-2 (phonebook ordering)** — umlauts expand to the
///   digraph they historically abbreviate (`ä = ae`, `ö = oe`,
///   `ü = ue`); `ß = ss`.
///
/// The shipped pack encodes the **DIN 5007-2 (phonebook)**
/// tailoring, which is the more distinctive and the one telephone
/// directories, birth-record indexes, and the German-language
/// Wikipedia's category ordering use. Dictionary ordering is
/// available via the native
/// `stringcheese_de::GermanCollator::DIN_5007_DICTIONARY` preset
/// (which does not consult the SCUD pack); a future SCUD-supplied
/// dictionary variant lives in a follow-up pack under a different
/// locale tag (e.g. `de-x-din5007-1`).
fn build_collation_de_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // ß → ss (both DIN variants agree here).
    c.push_expansion(0x00DF, &[0x0073, 0x0073]);
    // Capital sharp S (ẞ, U+1E9E) → SS.
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]);

    // Umlauts under DIN 5007-2 (phonebook): expand to digraph.
    c.push_expansion(0x00E4, &[0x0061, 0x0065]); // ä → ae
    c.push_expansion(0x00C4, &[0x0041, 0x0045]); // Ä → AE
    c.push_expansion(0x00F6, &[0x006F, 0x0065]); // ö → oe
    c.push_expansion(0x00D6, &[0x004F, 0x0045]); // Ö → OE
    c.push_expansion(0x00FC, &[0x0075, 0x0065]); // ü → ue
    c.push_expansion(0x00DC, &[0x0055, 0x0045]); // Ü → UE

    // Default strength tertiary (case + diacritics + base).
    c.set_default_strength(2);
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("de"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.finish()
}
