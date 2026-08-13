//! Build-time codegen for the Spanish pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-es.scud` — CLDR 44 Spanish plural rules (Phase 3 of the
//!    WIT-i18n subsystem).
//! 2. `number-es.scud` — CLDR 44 Spanish number-formatting patterns
//!    (Spain conventions).
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

use stringcheese_icu_plural::builder::{spanish_cardinals, spanish_ordinals};
use stringcheese_scud::{
    CAP_NUMBER, CAP_PLURAL, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN,
    ScudWriter,
};

/// CLDR version the shipped tables were compiled against. Bumping
/// this value is a coordinated release action — the SCUD file
/// header carries this string so downstream can trace data
/// provenance.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let plural_path = out_dir.join("plural-es.scud");
    let plural_bytes = build_plural_es_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-es.scud");
    let number_bytes = build_number_es_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the plural-es SCUD pack in memory.
///
/// Spanish plural rules (CLDR 44 `plurals.xml`,
/// `<pluralRules locales="es">`):
///
/// * Cardinal `one` when `n = 1` (integer 1 and decimal 1.0 both
///   classify as `one` — Spanish uses value-equality on `n`).
/// * Cardinal `many` when `v = 0 and i != 0 and i % 1000000 = 0`
///   (large-number bucket: `1_000_000`, `2_000_000`, …). CLDR 42+
///   added this category primarily for compact notation; Phase 3
///   evaluates only the non-`e` sub-clause, so compact-notation
///   inputs like `1.5c6 → 1_500_000` see `other` instead of `many`
///   (documented deferral).
/// * Cardinal `other` otherwise.
/// * Ordinal `other` for every value (CLDR 44 ships no distinct
///   ordinal buckets for Spanish).
fn build_plural_es_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    spanish_cardinals(&mut b);
    spanish_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("es"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-es SCUD pack in memory.
///
/// Spanish (Spain, `es-ES`) number formatting (CLDR 44 `es.xml`):
///
/// * Group separator: `.` (period) — Spain convention. Latin
///   American variants differ (`es-MX` uses `,`); the shipped pack
///   matches Spain defaults, following the same "ship one pack per
///   base locale" pattern the German / French packs use. Regional
///   `es-MX` / `es-AR` variants are documented follow-ups.
/// * Decimal separator: `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with a space (`50 %`).
///   Pattern `#,##0 %`.
/// * Currency: EUR `€` (Spain), USD `$`, GBP `£`, MXN `MX$` all
///   placed after the value with a space (`1.234,56 €`). Pattern
///   `#,##0.00 ¤`. MXN included for Latin-American relevance
///   despite the pack matching Spain conventions elsewhere.
fn build_number_es_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(".", ",", 0, 3, 3, 3);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.push_currency("MXN", "MX$", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("es"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
