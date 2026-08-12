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
    CAP_NUMBER, CAP_PLURAL, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN,
    ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let plural_path = out_dir.join("plural-zh.scud");
    let plural_bytes = build_plural_zh_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-zh.scud");
    let number_bytes = build_number_zh_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
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
