//! Build-time codegen for the Russian pack.
//!
//! Two SCUD artifacts emitted into `$OUT_DIR`:
//!
//! 1. `plural-ru.scud` — CLDR 44 Russian plural rules (Phase 3 of the
//!    WIT-i18n subsystem).
//! 2. `number-ru.scud` — CLDR 44 Russian number-formatting patterns.
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

use stringcheese_icu_plural::builder::{russian_cardinals, russian_ordinals};
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

    let plural_path = out_dir.join("plural-ru.scud");
    let plural_bytes = build_plural_ru_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-ru.scud");
    let number_bytes = build_number_ru_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the plural-ru SCUD pack in memory.
///
/// Russian plural rules (CLDR 44 `plurals.xml`, `<pluralRules
/// locales="ru">`):
///
/// * Cardinal `one` when `v = 0 and i % 10 = 1 and i % 100 != 11`
///   (1, 21, 31, … but not 11).
/// * Cardinal `few` when `v = 0 and i % 10 in 2..4 and i % 100 not
///   in 12..14` (2, 3, 4, 22, 23, 24, …).
/// * Cardinal `many` when `v = 0 and (i % 10 = 0 or i % 10 in 5..9
///   or i % 100 in 11..14)` (0, 5-20, 25-30, …).
/// * Cardinal `other` otherwise (every fractional value; there is no
///   integer Russian input that lands in `other` under CLDR 44).
/// * Ordinal `other` for every value (CLDR 44 ships no distinct
///   ordinal buckets for Russian).
fn build_plural_ru_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    russian_cardinals(&mut b);
    russian_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("ru"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-ru SCUD pack in memory.
///
/// Russian number formatting (CLDR 44 `ru.xml`):
///
/// * Group separator: U+00A0 (NO-BREAK SPACE).
/// * Decimal separator: `,` (comma).
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value with a space (`50 %`).
///   Pattern `#,##0 %`.
/// * Currency: RUB `₽`, USD `$`, EUR `€`, GBP `£` all placed after
///   the value with a space (`1 234,56 ₽`). Pattern
///   `#,##0.00 ¤`.
fn build_number_ru_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    // U+00A0 NO-BREAK SPACE per CLDR 44 `<group>`. Some Russian
    // renderers use plain ASCII space historically; we ship NBSP
    // because that is the CLDR-authoritative separator.
    n.set_decimal_pattern("\u{00A0}", ",", 0, 3, 3, 3);
    n.push_currency("RUB", "\u{20BD}", true, true);
    n.push_currency("USD", "$", true, true);
    n.push_currency("EUR", "\u{20AC}", true, true);
    n.push_currency("GBP", "\u{00A3}", true, true);
    n.set_percent("%", true, true);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("ru"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}
