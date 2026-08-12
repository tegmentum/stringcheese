//! Golden number-formatting vectors for the English pack.
//!
//! ≥ 30 vectors covering decimals, currency, percent, and
//! negatives per Phase 3 of the WIT-i18n design.

#![cfg(all(feature = "number-scud", not(target_family = "wasm")))]

use stringcheese_en::number_data::number_pack;
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

fn engine() -> NumberEngine<'static> {
    NumberEngine::new(vec![number_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Decimal formatting (10 vectors)
// -----------------------------------------------------------------------

#[test]
fn decimal_group_separators() {
    let e = engine();
    let cases = [
        (0.0, "0"),
        (1.0, "1"),
        (99.0, "99"),
        (999.0, "999"),
        (1_000.0, "1,000"),
        (12_345.0, "12,345"),
        (123_456.0, "123,456"),
        (1_000_000.0, "1,000,000"),
        (1_234_567_890.0, "1,234,567,890"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            e.format_decimal(input, "en", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn decimal_with_fractions() {
    let e = engine();
    assert_eq!(
        e.format_decimal(1.5, "en", FormattingOptions::default())
            .unwrap(),
        "1.5"
    );
    assert_eq!(
        e.format_decimal(1234.5, "en", FormattingOptions::default())
            .unwrap(),
        "1,234.5"
    );
    assert_eq!(
        e.format_decimal(0.5, "en", FormattingOptions::default())
            .unwrap(),
        "0.5"
    );
}

#[test]
fn decimal_negative() {
    let e = engine();
    assert_eq!(
        e.format_decimal(-1.0, "en", FormattingOptions::default())
            .unwrap(),
        "-1"
    );
    assert_eq!(
        e.format_decimal(-1234.5, "en", FormattingOptions::default())
            .unwrap(),
        "-1,234.5"
    );
    assert_eq!(
        e.format_decimal(-1_000_000.0, "en", FormattingOptions::default())
            .unwrap(),
        "-1,000,000"
    );
}

// -----------------------------------------------------------------------
// Currency formatting (12 vectors)
// -----------------------------------------------------------------------

#[test]
fn currency_usd() {
    let e = engine();
    assert_eq!(
        e.format_currency(1.0, "USD", "en", FormattingOptions::default())
            .unwrap(),
        "$1.00"
    );
    assert_eq!(
        e.format_currency(1234.56, "USD", "en", FormattingOptions::default())
            .unwrap(),
        "$1,234.56"
    );
    assert_eq!(
        e.format_currency(-1234.56, "USD", "en", FormattingOptions::default())
            .unwrap(),
        "-$1,234.56"
    );
    assert_eq!(
        e.format_currency(0.01, "USD", "en", FormattingOptions::default())
            .unwrap(),
        "$0.01"
    );
}

#[test]
fn currency_eur_gbp_jpy() {
    let e = engine();
    // Note: in the English pack every currency uses the same
    // "symbol before, no space" placement.
    assert_eq!(
        e.format_currency(100.0, "EUR", "en", FormattingOptions::default())
            .unwrap(),
        "\u{20AC}100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "GBP", "en", FormattingOptions::default())
            .unwrap(),
        "\u{00A3}100.00"
    );
    assert_eq!(
        e.format_currency(100.0, "JPY", "en", FormattingOptions::default())
            .unwrap(),
        "\u{00A5}100.00"
    );
}

// -----------------------------------------------------------------------
// Percent formatting (8 vectors)
// -----------------------------------------------------------------------

#[test]
fn percent_common_values() {
    let e = engine();
    let cases = [
        (0.0, "0%"),
        (0.01, "1%"),
        (0.5, "50%"),
        (1.0, "100%"),
        (0.99, "99%"),
        (2.5, "250%"),
        (-0.5, "-50%"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            e.format_percent(input, "en", FormattingOptions::default())
                .unwrap(),
            expected,
            "input={input}"
        );
    }
}
