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
    CAP_CASE, CAP_COLLATION, CAP_NUMBER, CAP_PLURAL, CaseSectionBuilder,
    CollationSectionBuilder, NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES,
    SECT_COLLATION_OPTIONS, SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_EXPANSIONS,
    SECT_FULL_FOLD, SECT_FULL_UPPER, SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_SIMPLE_FOLD,
    SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let case_path = out_dir.join("case-zh.scud");
    let case_bytes = build_case_zh_scud();
    fs::write(&case_path, &case_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", case_path.display()));

    let coll_path = out_dir.join("collation-zh.scud");
    let coll_bytes = build_collation_zh_scud();
    fs::write(&coll_path, &coll_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", coll_path.display()));

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

/// Build the case-zh SCUD pack in memory.
///
/// Chinese Han characters (CJK Unified Ideographs, U+4E00..=U+9FFF
/// and extensions) have no case — upper, lower, and titlecase are
/// all identity operations on Han. The `char::to_uppercase` /
/// `char::to_lowercase` fallback already returns the input
/// unchanged for every Han scalar.
///
/// The pack ships:
///
/// * **ASCII a-z ↔ A-Z** — 52 pairs plus 26 folds. Chinese text
///   commonly interleaves Latin (English loanwords, product names,
///   brand identifiers); the pack-hit path gives deterministic
///   behaviour on those.
/// * **German ß / ẞ expansions** — uniform composed-engine
///   behaviour.
///
/// Han characters are deliberately **not** listed in the pack —
/// no simple upper/lower entries for U+4E00..=U+9FFF. The engine
/// falls through to `char::to_lowercase` for Han queries, which
/// returns identity (verified in `case_golden_zh.rs`).
fn build_case_zh_scud() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();

    // ASCII a-z ↔ A-Z.
    for ch in 'a'..='z' {
        let up = ch.to_ascii_uppercase();
        c.push_simple_lower(up as u32, ch as u32);
        c.push_simple_upper(ch as u32, up as u32);
        c.push_simple_fold(up as u32, ch as u32);
    }

    // German ß / ẞ — belt-and-braces for composed-engine behaviour.
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]);
    c.push_full_fold(0x00DF, &[0x0073, 0x0073]);
    c.push_full_fold(0x1E9E, &[0x0073, 0x0073]);
    c.push_simple_lower(0x1E9E, 0x00DF);

    let mut w = ScudWriter::new(CAP_CASE, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
    w.finish()
}

/// Build the collation-zh SCUD pack in memory.
///
/// # Phase 2 deferrals
///
/// CLDR ships multiple `zh` collations — `standard` (stroke-based,
/// numeric stroke order), `pinyin` (Latin pinyin transliteration),
/// `stroke`, and `zhuyin`. All four require large Han-to-order or
/// Han-to-pinyin lookup tables plus algorithm support:
///
/// * **Stroke-based** — a ~90k-entry `(codepoint → stroke count)`
///   table, plus a secondary key for tied stroke counts.
/// * **Pinyin** — a ~40k-entry `(codepoint → pinyin string)` table,
///   plus tone handling.
///
/// Neither table ships in Phase 2. The pack uses feruca's
/// DUCET-root ordering, which sorts CJK Han by codepoint order
/// — deterministic but not linguistically meaningful. See
/// `docs/design/wit-i18n.md` § 8.2 for the deferral rationale
/// and the follow-up plan.
///
/// The pack ships:
///
/// * **German ß / ẞ expansions** — uniform composed-engine
///   behaviour.
/// * **Default strength tertiary.**
fn build_collation_zh_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // German ß expansion — uniform composed-engine behaviour.
    c.push_expansion(0x00DF, &[0x0073, 0x0073]);
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]);

    c.set_default_strength(2);
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("zh"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.finish()
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
