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
    CAP_NUMBER, CAP_PLURAL, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN,
    ScudWriter,
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

    println!("cargo:rerun-if-changed=build.rs");
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
