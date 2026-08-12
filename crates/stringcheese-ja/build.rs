//! Build-time codegen for the Japanese pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-ja.scud` — CLDR 44 Japanese plural rules. Japanese
//!    lacks grammatical number so the CLDR rule set is `other` only;
//!    the pack contains no plural predicates and every query falls
//!    through to [`PluralCategory::Other`](
//!    stringcheese_icu_plural::PluralCategory::Other).
//! 2. `number-ja.scud` — CLDR 44 Japanese number-formatting
//!    patterns.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{japanese_cardinals, japanese_ordinals};
use stringcheese_scud::{
    CAP_NUMBER, CAP_PLURAL, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN,
    ScudWriter,
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

    println!("cargo:rerun-if-changed=build.rs");
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
