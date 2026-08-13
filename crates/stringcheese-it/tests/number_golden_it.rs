//! Golden number-formatting vectors for the Italian pack.
//!
//! Italy conventions (`it-IT`): group `.`, decimal `,`, percent
//! `#,##0 %`, currency `#,##0.00 ¤` with symbol after value and a
//! space. Currency symbols shipped: EUR (Italy), USD, GBP, CHF
//! (Switzerland — included for `it-CH` Italian-Swiss relevance
//! despite the pack matching Italy conventions elsewhere).

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_icu_number::{FormattingOptions, NumberEngine};
use stringcheese_it::number_data::number_pack;

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
            &e.format_decimal(*input, "it", FormattingOptions::default())
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
        e.format_decimal(1234.5, "it", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_decimal(0.5, "it", FormattingOptions::default())
            .unwrap(),
        "0,5"
    );
    assert_eq!(
        e.format_decimal(1.25, "it", FormattingOptions::default())
            .unwrap(),
        "1,25"
    );
}

#[test]
fn decimal_negative_uses_leading_minus() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1234.5, "it", FormattingOptions::default())
            .unwrap(),
        "-1.234,5"
    );
    assert_eq!(
        e.format_decimal(-0.5, "it", FormattingOptions::default())
            .unwrap(),
        "-0,5"
    );
}

#[test]
fn decimal_small_and_large_values() {
    let e = engine();
    assert_eq!(
        e.format_decimal(0.001, "it", FormattingOptions::default())
            .unwrap(),
        "0,001"
    );
    assert_eq!(
        e.format_decimal(1_234_567_890.123, "it", FormattingOptions::default())
            .unwrap(),
        "1.234.567.890,123"
    );
}

#[test]
fn currency_eur_after_value_with_space() {
    let e = engine();
    assert_eq!(
        e.format_currency(1234.56, "EUR", "it", FormattingOptions::default())
            .unwrap(),
        "1.234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(0.01, "EUR", "it", FormattingOptions::default())
            .unwrap(),
        "0,01 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(1_000_000.0, "EUR", "it", FormattingOptions::default())
            .unwrap(),
        "1.000.000,00 \u{20AC}"
    );
}

#[test]
fn currency_usd_gbp_chf() {
    let e = engine();
    assert_eq!(
        e.format_currency(100.0, "USD", "it", FormattingOptions::default())
            .unwrap(),
        "100,00 $"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "it", FormattingOptions::default())
            .unwrap(),
        "100,00 \u{00A3}"
    );
    assert_eq!(
        e.format_currency(100.0, "CHF", "it", FormattingOptions::default())
            .unwrap(),
        "100,00 CHF"
    );
}

#[test]
fn currency_negative_puts_minus_outside() {
    let e = engine();
    assert_eq!(
        e.format_currency(-1234.56, "EUR", "it", FormattingOptions::default())
            .unwrap(),
        "-1.234,56 \u{20AC}"
    );
    assert_eq!(
        e.format_currency(-100.0, "CHF", "it", FormattingOptions::default())
            .unwrap(),
        "-100,00 CHF"
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
            e.format_percent(input, "it", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn it_ch_falls_back_to_it() {
    let e = engine();
    // A dedicated it-CH pack would use Swiss separators (group
    // `'` in some CLDR revisions, decimal `.`); the shipped base
    // `it` pack keeps Italy conventions and CHF is present in the
    // currency table so a fallback works end-to-end.
    assert_eq!(
        e.format_decimal(1234.5, "it-CH", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_currency(100.0, "CHF", "it-CH", FormattingOptions::default())
            .unwrap(),
        "100,00 CHF"
    );
}

#[test]
fn it_it_uses_the_base_pack() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1234.5, "it-IT", FormattingOptions::default())
            .unwrap(),
        "1.234,5"
    );
    assert_eq!(
        e.format_currency(1234.56, "EUR", "it-IT", FormattingOptions::default())
            .unwrap(),
        "1.234,56 \u{20AC}"
    );
}

#[test]
fn fraction_override_max_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.5678,
            "it",
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

#[test]
fn fraction_override_min_2() {
    let e = engine();
    let out = e
        .format_decimal(
            1234.0,
            "it",
            FormattingOptions {
                min_fraction: Some(2),
                max_fraction: None,
                use_grouping: None,
            },
        )
        .unwrap();
    assert_eq!(out, "1.234,00");
}

#[test]
fn currency_with_high_precision_input_rounds_to_two_fraction_digits() {
    let e = engine();
    // Currency defaults to 2 fraction digits regardless of input
    // precision. Half-even rounding: 1.005 → 1.00 (ties to even).
    assert_eq!(
        e.format_currency(1234.567, "EUR", "it", FormattingOptions::default())
            .unwrap(),
        "1.234,57 \u{20AC}"
    );
}

#[test]
fn engine_supports_it_and_variants() {
    let e = engine();
    assert!(e.supports("it"));
    assert!(e.supports("it-IT"));
    assert!(e.supports("it-CH"));
}
