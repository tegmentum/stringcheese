//! Golden number-formatting vectors for the Polish pack.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_icu_number::{FormattingOptions, NumberEngine};
use stringcheese_pl::number_data::number_pack;

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

#[test]
fn decimal_group_separators_use_nbsp() {
    let e = engine();
    let cases: &[(f64, &str)] = &[
        (0.0, "0"),
        (1.0, "1"),
        (999.0, "999"),
        (1_000.0, "1\u{00A0}000"),
        (12_345.0, "12\u{00A0}345"),
        (1_000_000.0, "1\u{00A0}000\u{00A0}000"),
        (1_234_567_890.0, "1\u{00A0}234\u{00A0}567\u{00A0}890"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_decimal(*input, "pl", FormattingOptions::default())
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
        e.format_decimal(1234.5, "pl", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "pl", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "pl", FormattingOptions::default())
            .unwrap(),
        "-1\u{00A0}234,5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "pl", FormattingOptions::default())
            .unwrap(),
        "0,001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "pl", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234\u{00A0}567\u{00A0}890,123"
    );
}

#[test]
fn currency_pln_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "PLN", "pl", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,56 z\u{0142}"
    );
    assert_eq!(
        e.format_currency(0.01, "PLN", "pl", FormattingOptions::default())
            .unwrap(),
        "0,01 z\u{0142}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "PLN", "pl", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}000\u{00A0}000,00 z\u{0142}"
    );
}

#[test]
fn currency_usd_eur_gbp() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "pl", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "pl", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "pl", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "PLN", "pl", FormattingOptions::default())
            .unwrap(),
        "-1\u{00A0}234,56 z\u{0142}"
    );
}

#[test]
fn percent_has_no_space_before_symbol() {
    // Polish is the odd shipped Phase 3 locale: CLDR pattern is
    // `#,##0%` (no space) — unlike de/fr/ru which all use ` %`.
    let e = engine();
    let cases = [
        (0.0, "0%"),
        (0.5, "50%"),
        (1.0, "100%"),
        (0.01, "1%"),
        (-0.5, "-50%"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            e.format_percent(input, "pl", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn pl_pl_falls_back_to_pl() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "pl-PL", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "PLN", "pl-PL", FormattingOptions::default())
            .unwrap(),
        "100,00 z\u{0142}"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "pl",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "1\u{00A0}234,57");
}
