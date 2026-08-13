//! Build-time codegen for the Portuguese pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-pt.scud` — CLDR 44 Portuguese plural rules (Phase 3
//!    of the WIT-i18n subsystem, pt-PT default rules — see the pack
//!    docs for the pt-PT vs pt-BR trade-off).
//! 2. `number-pt.scud` — CLDR 44 Portuguese number-formatting
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

use stringcheese_icu_plural::builder::{portuguese_cardinals, portuguese_ordinals};
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

    let plural_path = out_dir.join("plural-pt.scud");
    let plural_bytes = build_plural_pt_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-pt.scud");
    let number_bytes = build_number_pt_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the plural-pt SCUD pack in memory.
///
/// Portuguese plural rules — the shipped pack matches CLDR 44
/// `<pluralRules locales="pt_PT">`:
///
/// * Cardinal `one` when `n = 1 and v = 0` (equivalent to
///   `i = 1 and v = 0` under the operand tuple; integer 1 only,
///   decimal `1.0` → `other`).
/// * Cardinal `many` when `v = 0 and i != 0 and i % 1000000 = 0`
///   (large-number bucket: `1_000_000`, `2_000_000`, …). CLDR 42+
///   added this category; Phase 3 evaluates only the non-`e`
///   sub-clause, so compact-notation inputs like `1.5c6 →
///   1_500_000` see `other` instead of `many` (documented
///   deferral).
/// * Cardinal `other` otherwise.
/// * Ordinal `other` for every value (CLDR 44 ships no distinct
///   ordinal buckets for Portuguese).
///
/// The pack labels itself `"pt"` so the fallback chain resolves
/// `pt-BR → pt`, `pt-PT → pt`, and `pt` all to this pack. pt-BR
/// uses the default `pt` rule `i = 0..1` (both 0 and 1 → `one`),
/// which differs from the shipped pt-PT rule — the delta is
/// documented as a follow-up in the crate docs.
fn build_plural_pt_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    portuguese_cardinals(&mut b);
    portuguese_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("pt"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-pt SCUD pack in memory.
///
/// Portuguese number formatting (CLDR 44 `pt.xml`, matching both
/// pt-PT and pt-BR defaults which agree on separators and pattern):
///
/// * Group separator: `.` (period).
/// * Decimal separator: `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with a space (`50 %`).
///   Pattern `#,##0 %`.
/// * Currency: EUR `€` (pt-PT), BRL `R$` (pt-BR), USD `US$`, GBP
///   `£` all placed after the value with a space (`1.234,56 €`).
///   Pattern `#,##0.00 ¤`. Both EUR and BRL included so a caller
///   servicing either variety has the primary regional currency
///   present out of the box.
fn build_number_pt_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(".", ",", 0, 3, 3, 3);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("BRL", "R$", true, true);
    n.push_currency("USD", "US$", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("pt"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
