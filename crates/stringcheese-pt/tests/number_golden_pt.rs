//! Golden number-formatting vectors for the Portuguese pack.
//!
//! CLDR 44 `pt.xml`: group `.`, decimal `,`, percent `#,##0 %`,
//! currency `#,##0.00 ¤` with symbol after value and a space. Both
//! pt-PT and pt-BR share these separator conventions. Currency
//! symbols shipped: EUR (pt-PT), BRL (pt-BR), USD, GBP.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_icu_number::{FormattingOptions, NumberEngine};
use stringcheese_pt::number_data::number_pack;

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

#[test]
fn decimal_group_separators_use_period() {
    let e = engine();
    let cases: &[(f64, &str)] = &[
        (0.0, "0"),
        (1.0, "1"),
        (999.0, "999"),
        (1_000.0, "1.000"),
        (12_345.0, "12.345"),
        (1_000_000.0, "1.000.000"),
        (1_234_567_890.0, "1.234.567.890"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_decimal(*input, "pt", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn decimal_uses_comma_separator() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "pt", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "pt", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "pt", FormattingOptions::default())
            .unwrap(),
        "-1.234,5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "pt", FormattingOptions::default())
            .unwrap(),
        "0,001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "pt", FormattingOptions::default())
            .unwrap(),
        "1.234.567.890,123"
    );
}

#[test]
fn currency_eur_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "EUR", "pt", FormattingOptions::default())
            .unwrap(),
        "1.234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(0.01, "EUR", "pt", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "EUR", "pt", FormattingOptions::default())
            .unwrap(),
        "1.000.000,00 \u{20AC}"
    );
}

#[test]
fn currency_brl_for_pt_br_use_cases() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "BRL", "pt", FormattingOptions::default())
            .unwrap(),
        "1.234,56 R$"
    );
    // pt-BR fallback also gets BRL.
    assert_eq!(
        e.format_currency(1234.56, "BRL", "pt-BR", FormattingOptions::default())
            .unwrap(),
        "1.234,56 R$"
    );
}

#[test]
fn currency_usd_gbp() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "pt", FormattingOptions::default())
            .unwrap(),
        "100,00 US$"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "pt", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "EUR", "pt", FormattingOptions::default())
            .unwrap(),
        "-1.234,56 \u{20AC}"
    );
}

#[test]
fn percent_has_space_before_symbol() {
    let e = engine();
    let cases = [
        (0.0, "0 %"),
        (0.5, "50 %"),
        (1.0, "100 %"),
        (0.01, "1 %"),
        (-0.5, "-50 %"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            e.format_percent(input, "pt", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn pt_pt_and_pt_br_fall_back_to_pt() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "pt-PT", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_decimal(1234.5, "pt-BR", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "pt-PT", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(100.0, "BRL", "pt-BR", FormattingOptions::default())
            .unwrap(),
        "100,00 R$"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "pt",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "1.234,57");
}
