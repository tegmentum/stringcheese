//! Golden number-formatting vectors for the French pack.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_fr::number_data::number_pack;
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

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
            &e.format_decimal(*input, "fr", FormattingOptions::default())
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
        e.format_decimal(1234.5, "fr", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "fr", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn currency_eur_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "EUR", "fr", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(0.01, "EUR", "fr", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "EUR", "fr", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}000\u{00A0}000,00 \u{20AC}"
    );
}

#[test]
fn currency_usd_gbp_cad() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "fr", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "fr", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
    assert_eq!(
        e.format_currency(100.0, "CAD", "fr", FormattingOptions::default())
            .unwrap(),
        "100,00 $CA"
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
            e.format_percent(input, "fr", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn fr_ca_falls_back_to_fr() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "fr-CA", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
}

#[test]
fn negative_decimals_use_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "fr", FormattingOptions::default())
            .unwrap(),
        "-1\u{00A0}234,5"
    );
}
