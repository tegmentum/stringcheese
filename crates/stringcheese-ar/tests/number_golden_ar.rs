//! Golden number-formatting vectors for the Arabic pack.
//!
//! This suite runs the `latn` (Western digits) numbering-system
//! path; the `arab` / `arabext` digit shapes and RTL bidi handling
//! are documented Phase 3 deferrals.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_ar::number_data::number_pack;
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

#[test]
fn decimal_uses_comma_grouping_and_dot_decimal() {
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
            e.format_decimal(input, "ar", FormattingOptions::default())
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
        e.format_decimal(1234.5, "ar", FormattingOptions::default())
            .unwrap(),
        "1,234.5"
    );
    assert_eq!(
        e.format_decimal(0.5, "ar", FormattingOptions::default())
            .unwrap(),
        "0.5"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "ar", FormattingOptions::default())
            .unwrap(),
        "-1,234.5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "ar", FormattingOptions::default())
            .unwrap(),
        "0.001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "ar", FormattingOptions::default())
            .unwrap(),
        "1,234,567,890.123"
    );
}

#[test]
fn currency_sar_before_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "SAR", "ar", FormattingOptions::default())
            .unwrap(),
        "\u{0631}.\u{0633}.\u{200F} 1,234.56"
    );
    assert_eq!(
        e.format_currency(0.01, "SAR", "ar", FormattingOptions::default())
            .unwrap(),
        "\u{0631}.\u{0633}.\u{200F} 0.01"
    );
}

#[test]
fn currency_usd_eur_aed() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "ar", FormattingOptions::default())
            .unwrap(),
        "US$ 100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "EUR", "ar", FormattingOptions::default())
            .unwrap(),
        "\u{20AC} 100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "AED", "ar", FormattingOptions::default())
            .unwrap(),
        "\u{062F}.\u{0625}.\u{200F} 100.00"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    // Negative composition: '-' precedes the symbol+body composite.
    assert_eq!(
        e.format_currency(-1234.56, "USD", "ar", FormattingOptions::default())
            .unwrap(),
        "-US$ 1,234.56"
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
            e.format_percent(input, "ar", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn ar_eg_falls_back_to_ar() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "ar-EG", FormattingOptions::default())
            .unwrap(),
        "1,234.5"
    );
    assert_eq!(
        e.format_currency(100.0, "SAR", "ar-SA", FormattingOptions::default())
            .unwrap(),
        "\u{0631}.\u{0633}.\u{200F} 100.00"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "ar",
            FormattingOptions {
                min_fraction: None,
                max_fraction: Some(2),
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "1,234.57");
}
