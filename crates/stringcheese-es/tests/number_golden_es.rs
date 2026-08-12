//! Golden number-formatting vectors for the Spanish pack.
//!
//! Spain conventions (`es-ES`): group `.`, decimal `,`, percent
//! `#,##0 %`, currency `#,##0.00 ¤` with symbol after value and a
//! space. Currency symbols shipped: EUR (Spain), USD, GBP, MXN
//! (Mexico — included for Latin-American relevance despite the
//! pack matching Spain conventions elsewhere).

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_es::number_data::number_pack;
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

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
            &e.format_decimal(*input, "es", FormattingOptions::default())
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
        e.format_decimal(1234.5, "es", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "es", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "es", FormattingOptions::default())
            .unwrap(),
        "-1.234,5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "es", FormattingOptions::default())
            .unwrap(),
        "0,001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "es", FormattingOptions::default())
            .unwrap(),
        "1.234.567.890,123"
    );
}

#[test]
fn currency_eur_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "EUR", "es", FormattingOptions::default())
            .unwrap(),
        "1.234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(0.01, "EUR", "es", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "EUR", "es", FormattingOptions::default())
            .unwrap(),
        "1.000.000,00 \u{20AC}"
    );
}

#[test]
fn currency_usd_gbp_mxn() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "es", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "es", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
    assert_eq!(
        e.format_currency(100.0, "MXN", "es", FormattingOptions::default())
            .unwrap(),
        "100,00 MX$"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "EUR", "es", FormattingOptions::default())
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
            e.format_percent(input, "es", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn es_mx_falls_back_to_es() {
    let e = engine();
    // es-MX would prefer group `,` / decimal `.` and MXN as MX$; the
    // shipped Spain pack fallback still returns Spain conventions
    // (documented deferral for regional variants).
    assert_eq!(
        e.format_decimal(1234.5, "es-MX", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "MXN", "es-MX", FormattingOptions::default())
            .unwrap(),
        "100,00 MX$"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "es",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    // Half-even rounding matches CLDR reference.
    assert_eq!(out, "1.234,57");
}
