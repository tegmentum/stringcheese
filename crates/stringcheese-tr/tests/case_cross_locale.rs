//! Cross-locale case-mapping composition test.
//!
//! Phase 1 of the WIT-i18n design (`docs/design/wit-i18n.md` § 8)
//! commits to "cross-locale composition is exercised" — the same
//! input surface should yield different output when the Turkish
//! pack overrides English behaviour. This test wires an
//! `[en_pack, tr_pack]` engine and asserts the divergence at the
//! query surface.
//!
//! Test lives in `stringcheese-tr` (rather than a new fixtures
//! crate) because it needs both packs and `stringcheese-tr` already
//! depends on `stringcheese-icu-case` for its own `case_data`
//! module; the English pack rides in as a `[dev-dependencies]` addition.

#![cfg(all(feature = "case-scud", not(target_family = "wasm")))]

use stringcheese_en::case_data::case_pack as en_pack;
use stringcheese_icu_case::CaseEngine;
use stringcheese_tr::case_data::case_pack as tr_pack;

fn engine_en_only() -> CaseEngine<'static> {
    CaseEngine::new(vec![en_pack().unwrap()])
}

fn engine_tr_only() -> CaseEngine<'static> {
    CaseEngine::new(vec![tr_pack().unwrap()])
}

fn engine_en_and_tr() -> CaseEngine<'static> {
    // English pack first, Turkish pack second — the engine looks up
    // packs by locale, so order does not matter for correctness,
    // but the test uses a deterministic order for reproducibility.
    CaseEngine::new(vec![en_pack().unwrap(), tr_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Divergence on `i` under different locales — the crux of Phase 1
// -----------------------------------------------------------------------

#[test]
fn same_input_different_output_by_locale() {
    let engine = engine_en_and_tr();

    // English locale: standard Unicode behaviour.
    assert_eq!(engine.to_lower("ISTANBUL", "en"), "istanbul");
    assert_eq!(engine.to_upper("istanbul", "en"), "ISTANBUL");

    // Turkish locale: Turkish tailoring wins.
    assert_eq!(engine.to_lower("ISTANBUL", "tr"), "ıstanbul");
    assert_eq!(engine.to_upper("istanbul", "tr"), "İSTANBUL");
}

#[test]
fn english_engine_never_produces_turkish_dotless_i() {
    let engine = engine_en_only();
    // No `ı` (U+0131) can be produced from ASCII input under `en`.
    let out = engine.to_lower("HELLO WORLD I AM HERE", "en");
    assert!(!out.contains('\u{0131}'), "got {out:?}");
}

#[test]
fn turkish_engine_never_produces_dot_i_from_capital_i() {
    let engine = engine_tr_only();
    let out = engine.to_lower("MERHABA IŞIK", "tr");
    // Every `I` must have folded to `ı` (U+0131), not to `i`.
    assert!(!out.contains('i'), "got {out:?}");
    assert!(out.contains('\u{0131}'), "got {out:?}");
}

// -----------------------------------------------------------------------
// Fallback chain: `tr-CY` falls back to `tr`
// -----------------------------------------------------------------------

#[test]
fn tr_cy_falls_back_to_tr_pack() {
    let engine = engine_en_and_tr();
    // Turkish Cyprus: no `tr-CY` pack shipped; the engine walks
    // `tr-CY → tr → ""` and picks up the `tr` pack.
    assert_eq!(engine.to_lower("IZMIR", "tr-CY"), "ızmır");
}

// -----------------------------------------------------------------------
// Fallback chain: unknown locale → default Unicode
// -----------------------------------------------------------------------

#[test]
fn unknown_locale_uses_default_unicode() {
    let engine = engine_en_and_tr();
    // Neither `xx-YY` nor `xx` matches any pack; queries fall
    // through to `char::to_lowercase` / `char::to_uppercase`.
    assert_eq!(engine.to_lower("HELLO", "xx-YY"), "hello");
    assert_eq!(engine.to_upper("hello", "xx-YY"), "HELLO");
    // `I` folds to `i` under default Unicode (no Turkish tailoring).
    assert_eq!(engine.to_lower("I", "xx-YY"), "i");
}

// -----------------------------------------------------------------------
// supports() and supported_locales() introspection
// -----------------------------------------------------------------------

#[test]
fn engine_reports_supported_locales() {
    let engine = engine_en_and_tr();
    let mut locales = engine.supported_locales();
    locales.sort_unstable();
    assert_eq!(locales, vec!["en", "tr"]);
}

#[test]
fn supports_checks_fallback_chain() {
    let engine = engine_en_and_tr();
    assert!(engine.supports("en"));
    assert!(engine.supports("en-US")); // via `en-US → en`
    assert!(engine.supports("tr"));
    assert!(engine.supports("tr-CY")); // via `tr-CY → tr`
    assert!(!engine.supports("de"));
    assert!(!engine.supports("ja"));
}

// -----------------------------------------------------------------------
// Composed pack sizes — informational, printed to stdout so CI's
// wasm-i18n-case job can trend it over time.
// -----------------------------------------------------------------------

#[test]
fn print_composed_pack_sizes() {
    let en = en_pack().unwrap();
    let tr = tr_pack().unwrap();
    println!("case-en.scud size: {} bytes", en.scud_bytes_len());
    println!("case-tr.scud size: {} bytes", tr.scud_bytes_len());
    println!(
        "composed engine total data size: {} bytes",
        en.scud_bytes_len() + tr.scud_bytes_len(),
    );
}
