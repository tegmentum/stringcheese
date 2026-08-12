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
    CAP_NUMBER, CAP_PLURAL, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN,
    ScudWriter,
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

    println!("cargo:rerun-if-changed=build.rs");
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
