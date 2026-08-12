//! Golden number-formatting vectors for the Russian pack.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_icu_number::{FormattingOptions, NumberEngine};
use stringcheese_ru::number_data::number_pack;

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
            &e.format_decimal(*input, "ru", FormattingOptions::default())
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
        e.format_decimal(1234.5, "ru", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "ru", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "ru", FormattingOptions::default())
            .unwrap(),
        "-1\u{00A0}234,5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "ru", FormattingOptions::default())
            .unwrap(),
        "0,001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "ru", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234\u{00A0}567\u{00A0}890,123"
    );
}

#[test]
fn currency_rub_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "RUB", "ru", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,56 \u{20BD}"
    );
    assert_eq!(
        e.format_currency(0.01, "RUB", "ru", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20BD}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "RUB", "ru", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}000\u{00A0}000,00 \u{20BD}"
    );
}

#[test]
fn currency_usd_eur_gbp() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "ru", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "ru", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "ru", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "RUB", "ru", FormattingOptions::default())
            .unwrap(),
        "-1\u{00A0}234,56 \u{20BD}"
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
            e.format_percent(input, "ru", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn ru_ru_falls_back_to_ru() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "ru-RU", FormattingOptions::default())
            .unwrap(),
        "1\u{00A0}234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "RUB", "ru-BY", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{20BD}"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "ru",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    // 1234.5678 rounds half-even to 2 fraction digits.
    assert_eq!(out, "1\u{00A0}234,57");
}
