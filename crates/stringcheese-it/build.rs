//! Build-time codegen for the Italian pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-it.scud` — CLDR 44.1 Italian plural rules (Phase 3
//!    of the WIT-i18n subsystem).
//! 2. `number-it.scud` — CLDR 44.1 Italian number-formatting
//!    patterns (Italy conventions).
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

use stringcheese_icu_plural::builder::{italian_cardinals, italian_ordinals};
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

    let plural_path = out_dir.join("plural-it.scud");
    let plural_bytes = build_plural_it_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-it.scud");
    let number_bytes = build_number_it_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the plural-it SCUD pack in memory.
///
/// Italian plural rules (CLDR 44.1 `plurals.xml`, Italian shares
/// the multi-locale `<pluralRules locales="ast ca de en et fi fy
/// gl ia io it ji lij nl sc scn sv sw ur yi">` block for cardinals
/// and the `<pluralRules locales="it sc scn">` block for
/// ordinals):
///
/// * Cardinal `one` when `i = 1 and v = 0` (integer 1 only;
///   decimal `1.0` classifies as `other` — Italian differs from
///   Spanish here, which uses `n = 1`).
/// * Cardinal `many` when `v = 0 and i != 0 and i % 1000000 = 0`
///   (large-number bucket: `1_000_000`, `2_000_000`, …). CLDR 42+
///   added this category primarily for compact notation; Phase 3
///   evaluates only the non-`e` sub-clause, so compact-notation
///   inputs like `1.5c6 → 1_500_000` see `other` instead of `many`
///   (documented deferral, same as `es` and `pt`).
/// * Cardinal `other` otherwise.
/// * Ordinal `many` when `n ∈ {8, 11, 80, 800}` — the four
///   distinct Italian ordinal-marking values (`ottavo → 8º`,
///   `undicesimo → 11º`, `ottantesimo → 80º`, `ottocentesimo →
///   800º`).
/// * Ordinal `other` otherwise.
fn build_plural_it_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    italian_cardinals(&mut b);
    italian_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("it"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-it SCUD pack in memory.
///
/// Italian (Italy, `it-IT`) number formatting (CLDR 44.1
/// `it.xml`):
///
/// * Group separator: `.` (period) — Italy convention. Matches
///   the German / Spanish / Portuguese `.` pattern; not the
///   French NBSP or the English `,`.
/// * Decimal separator: `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with a space (`50 %`).
///   Pattern `#,##0 %`.
/// * Currency: EUR `€` (Italy), USD `$`, GBP `£`, CHF `CHF` all
///   placed after the value with a space (`1.234,56 €`). Pattern
///   `#,##0.00 ¤`. CHF included for Italian-Switzerland (`it-CH`)
///   relevance despite the pack matching Italy conventions
///   elsewhere; a dedicated `it-CH` pack with Swiss group /
///   decimal separators is a documented follow-up.
fn build_number_it_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(".", ",", 0, 3, 3, 3);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.push_currency("CHF", "CHF", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("it"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
