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
    CAP_CASE, CAP_COLLATION, CAP_NUMBER, CAP_PLURAL, CaseSectionBuilder, CollationSectionBuilder,
    NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES, SECT_COLLATION_OPTIONS,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_EXPANSIONS, SECT_FULL_FOLD, SECT_FULL_UPPER,
    SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER,
    SECT_SIMPLE_UPPER, ScudWriter,
};

/// CLDR version the shipped tables were compiled against. Bumping
/// this value is a coordinated release action — the SCUD file
/// header carries this string so downstream can trace data
/// provenance.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let case_path = out_dir.join("case-ru.scud");
    let case_bytes = build_case_ru_scud();
    fs::write(&case_path, &case_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", case_path.display()));

    let coll_path = out_dir.join("collation-ru.scud");
    let coll_bytes = build_collation_ru_scud();
    fs::write(&coll_path, &coll_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", coll_path.display()));

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

/// Build the case-ru SCUD pack in memory.
///
/// Russian uses the default Unicode case-mapping rules for the
/// Cyrillic block — no locale-specific tailoring the way Turkish
/// requires for the dotted / dotless-I.
///
/// Coverage:
///
/// * **ASCII a-z ↔ A-Z** — 52 simple pairs plus 26 folds. Same
///   rationale as every other pack: uniform pack-hit ratios and
///   correct behaviour on Latin transliterations mixed into
///   Cyrillic text.
/// * **Modern Russian alphabet (U+0410..=U+042F / U+0430..=U+044F)**
///   — 32 uppercase/lowercase pairs (А..Я / а..я, excluding the
///   irregular Ё/ё at U+0401 / U+0451).
/// * **Ё ↔ ё (U+0401 ↔ U+0451)** — the one Cyrillic letter with a
///   non-adjacent case pair.
/// * **German ß (U+00DF)** — full uppercase to "SS" and full fold
///   to "ss". Belt-and-braces so a composed engine sees the
///   expansion regardless of which pack the query resolves through.
fn build_case_ru_scud() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();

    // ASCII a-z ↔ A-Z.
    for ch in 'a'..='z' {
        let up = ch.to_ascii_uppercase();
        c.push_simple_lower(up as u32, ch as u32);
        c.push_simple_upper(ch as u32, up as u32);
        c.push_simple_fold(up as u32, ch as u32);
    }

    // Modern Cyrillic uppercase A (U+0410) to Я (U+042F) map to
    // lowercase a (U+0430) to я (U+044F) at +0x20 offset.
    for upper in 0x0410u32..=0x042Fu32 {
        let lower = upper + 0x20;
        c.push_simple_lower(upper, lower);
        c.push_simple_upper(lower, upper);
        c.push_simple_fold(upper, lower);
    }

    // Ё (U+0401) ↔ ё (U+0451) — the one Cyrillic letter with an
    // irregular case pair (not part of the U+0410 block).
    c.push_simple_lower(0x0401, 0x0451);
    c.push_simple_upper(0x0451, 0x0401);
    c.push_simple_fold(0x0401, 0x0451);

    // German ß / ẞ — belt-and-braces for composed-engine behaviour.
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]); // ß → SS
    c.push_full_fold(0x00DF, &[0x0073, 0x0073]); // ß → ss
    c.push_full_fold(0x1E9E, &[0x0073, 0x0073]); // ẞ → ss
    c.push_simple_lower(0x1E9E, 0x00DF);

    let mut w = ScudWriter::new(CAP_CASE, CLDR_VERSION, Some("ru"));
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
    w.finish()
}

/// Build the collation-ru SCUD pack in memory.
///
/// CLDR's Russian collation for the modern alphabet is close to
/// DUCET-root ordering — the Cyrillic block sorts by codepoint
/// order (Ё inserted between Е and Ж), and feruca handles this
/// correctly out of the box.
///
/// # Phase 2 deferral
///
/// The Russian **case-second variant** — uppercase-first vs
/// lowercase-first ordering at tertiary strength (CLDR ships two
/// variants for `ru`) — is a documented Phase 2 `CollationEngine`
/// deferral. The shipped pack uses the default lowercase-first
/// direction (matching feruca's DUCET-root). Selecting the
/// uppercase-first variant requires an options-section extension
/// plus algorithm changes to consume it. See
/// `docs/design/wit-i18n.md` § 8.2.
fn build_collation_ru_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // German ß expansion — uniform composed-engine behaviour.
    c.push_expansion(0x00DF, &[0x0073, 0x0073]);
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]);

    c.set_default_strength(2); // Tertiary
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("ru"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.finish()
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
