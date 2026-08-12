//! Golden date/time-formatting vectors for the English pack.
//!
//! ≥ 30 vectors covering short/medium/long/full dates + short/
//! medium times + combined datetime + weekday computation
//! (Zeller's congruence) + leap-year edge cases per Phase 4 of
//! the WIT-i18n design.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_en::datetime_data::datetime_pack;
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Date formatting — 12 vectors
// -----------------------------------------------------------------------

#[test]
fn date_short() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "9/22/2024"),
        ("2024-01-01", "1/1/2024"),
        ("2024-12-31", "12/31/2024"),
        ("2000-02-29", "2/29/2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "en", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "Sep 22, 2024"),
        ("2024-01-15", "Jan 15, 2024"),
        ("2024-07-04", "Jul 4, 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "en", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "September 22, 2024"),
        ("1969-07-20", "July 20, 1969"),
        ("2000-01-01", "January 1, 2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "en", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_includes_weekday() {
    let e = engine();
    // 2024-09-22 was a Sunday.
    assert_eq!(
        e.format_date("2024-09-22", "en", DateTimeLength::Full)
            .unwrap(),
        "Sunday, September 22, 2024"
    );
    // 2024-01-01 was a Monday.
    assert_eq!(
        e.format_date("2024-01-01", "en", DateTimeLength::Full)
            .unwrap(),
        "Monday, January 1, 2024"
    );
    // 1776-07-04 was a Thursday.
    assert_eq!(
        e.format_date("1776-07-04", "en", DateTimeLength::Full)
            .unwrap(),
        "Thursday, July 4, 1776"
    );
}

// -----------------------------------------------------------------------
// Time formatting — 8 vectors
// -----------------------------------------------------------------------

#[test]
fn time_short_am_pm() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("00:00:00", "12:00 AM"),
        ("11:59:00", "11:59 AM"),
        ("12:00:00", "12:00 PM"),
        ("13:00:00", "1:00 PM"),
        ("17:03:04", "5:03 PM"),
        ("23:59:00", "11:59 PM"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_time(input, "en", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "en", DateTimeLength::Medium)
            .unwrap(),
        "5:03:04 PM"
    );
    assert_eq!(
        e.format_time("00:00:00", "en", DateTimeLength::Medium)
            .unwrap(),
        "12:00:00 AM"
    );
}

// -----------------------------------------------------------------------
// Combined datetime — 4 vectors
// -----------------------------------------------------------------------

#[test]
fn combined_datetime() {
    let e = engine();
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04Z",
            "en",
            DateTimeLength::Medium,
            DateTimeLength::Short
        )
        .unwrap(),
        "Sep 22, 2024 5:03 PM"
    );
    assert_eq!(
        e.format_datetime(
            "2024-01-01T00:00:00",
            "en",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "January 1, 2024 12:00:00 AM"
    );
    // Timezone info is parsed and discarded.
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04+02:00",
            "en",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "9/22/2024 5:03 PM"
    );
    // Fractional seconds discarded.
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04.123Z",
            "en",
            DateTimeLength::Short,
            DateTimeLength::Medium
        )
        .unwrap(),
        "9/22/2024 5:03:04 PM"
    );
}

// -----------------------------------------------------------------------
// Locale fallback + errors — 3 vectors
// -----------------------------------------------------------------------

#[test]
fn en_us_falls_back_to_en() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "en-US", DateTimeLength::Medium)
            .unwrap(),
        "Sep 22, 2024"
    );
}

#[test]
fn out_of_range_input_errors() {
    let e = engine();
    assert!(
        e.format_date("2023-02-29", "en", DateTimeLength::Short)
            .is_err()
    );
    assert!(
        e.format_time("25:00:00", "en", DateTimeLength::Short)
            .is_err()
    );
}

#[test]
fn unknown_locale_errors() {
    let e = engine();
    assert!(
        e.format_date("2024-09-22", "xx", DateTimeLength::Short)
            .is_err()
    );
}
