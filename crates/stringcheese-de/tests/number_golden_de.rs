//! Golden number-formatting vectors for the German pack.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_de::number_data::number_pack;
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

#[test]
fn decimal_group_separators_use_dot() {
    let e = engine();
    let cases = [
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
            e.format_decimal(input, "de", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn decimal_with_comma_separator() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "de", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "de", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "de", FormattingOptions::default())
            .unwrap(),
        "-1.234,5"
    );
}

#[test]
fn currency_eur_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "EUR", "de", FormattingOptions::default())
            .unwrap(),
        "1.234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(0.01, "EUR", "de", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "EUR", "de", FormattingOptions::default())
            .unwrap(),
        "1.000.000,00 \u{20AC}"
    );
}

#[test]
fn currency_usd_gbp_chf() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "de", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "de", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
    assert_eq!(
        e.format_currency(100.0, "CHF", "de", FormattingOptions::default())
            .unwrap(),
        "100,00 CHF"
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
            e.format_percent(input, "de", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn de_de_falls_back_to_de() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "de-DE", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "de-AT", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{20AC}"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "de",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    // 1234.5678 rounds to 2 fraction digits.
    assert_eq!(out, "1.234,57");
}
