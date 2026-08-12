//! Golden number-formatting vectors for the Japanese pack.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_icu_number::{FormattingOptions, NumberEngine};
use stringcheese_ja::number_data::number_pack;

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

#[test]
fn decimal_group_separators_use_comma() {
    let e = engine();
    let cases = [
        (0.0, "0"),
        (1.0, "1"),
        (999.0, "999"),
        (1_000.0, "1,000"),
        (12_345.0, "12,345"),
        (1_000_000.0, "1,000,000"),
        (1_234_567_890.0, "1,234,567,890"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            e.format_decimal(input, "ja", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn decimal_with_dot_separator() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "ja", FormattingOptions::default())
            .unwrap(),
        "1,234.5"
    );
    assert_eq!(
        e.format_decimal(0.5, "ja", FormattingOptions::default())
            .unwrap(),
        "0.5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "ja", FormattingOptions::default())
            .unwrap(),
        "-1,234.5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "ja", FormattingOptions::default())
            .unwrap(),
        "0.001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "ja", FormattingOptions::default())
            .unwrap(),
        "1,234,567,890.123"
    );
}

#[test]
fn currency_jpy_before_value_no_space() {
    // The Phase 3 engine currency path defaults to 2 fraction digits
    // regardless of the currency. JPY is culturally 0-fraction; the
    // pack's decimal pattern max-fraction is 3 which lets the caller
    // override to 0 explicitly. This test asserts the default (2)
    // behaviour so a downstream that changes it rings a bell.
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "JPY", "ja", FormattingOptions::default())
            .unwrap(),
        "\u{00A5}1,234.56"
    );
}

#[test]
fn currency_jpy_zero_fraction_override() {
    // The culturally-correct way to render a yen amount is 0
    // fraction digits — the caller opts in explicitly.
    let e = engine();
    let out = e
        .format_currency(
            1234.0,
            "JPY",
            "ja",
            FormattingOptions {
                min_fraction: Some(0),
                max_fraction: Some(0),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "\u{00A5}1,234");
    let out = e
        .format_currency(
            1_000_000.0,
            "JPY",
            "ja",
            FormattingOptions {
                min_fraction: Some(0),
                max_fraction: Some(0),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "\u{00A5}1,000,000");
}

#[test]
fn currency_usd_eur_cny() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "ja", FormattingOptions::default())
            .unwrap(),
        "US$100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "ja", FormattingOptions::default())
            .unwrap(),
        "\u{20AC}100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "CNY", "ja", FormattingOptions::default())
            .unwrap(),
        "CN\u{00A5}100.00"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "JPY", "ja", FormattingOptions::default())
            .unwrap(),
        "-\u{00A5}1,234.56"
    );
}

#[test]
fn percent_has_no_space_before_symbol() {
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
            e.format_percent(input, "ja", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn ja_jp_falls_back_to_ja() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "ja-JP", FormattingOptions::default())
            .unwrap(),
        "1,234.5"
    );
    assert_eq!(
        e.format_currency(100.0, "JPY", "ja-JP", FormattingOptions::default())
            .unwrap(),
        "\u{00A5}100.00"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "ja",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "1,234.57");
}
