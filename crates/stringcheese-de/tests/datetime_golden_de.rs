//! Golden date/time-formatting vectors for the German pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_de::datetime_data::datetime_pack;
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_dots() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22.09.2024"),
        ("2024-01-01", "01.01.2024"),
        ("2024-12-31", "31.12.2024"),
        ("2000-02-29", "29.02.2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "de", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_matches_short() {
    let e = engine();
    // German CLDR ships identical short and medium.
    assert_eq!(
        e.format_date("2024-09-22", "de", DateTimeLength::Medium)
            .unwrap(),
        "22.09.2024"
    );
}

#[test]
fn date_long_uses_full_month_name() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22. September 2024"),
        ("2024-03-15", "15. M\u{00E4}rz 2024"),
        ("2024-01-01", "1. Januar 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "de", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_includes_weekday() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "de", DateTimeLength::Full)
            .unwrap(),
        "Sonntag, 22. September 2024"
    );
    assert_eq!(
        e.format_date("2024-01-01", "de", DateTimeLength::Full)
            .unwrap(),
        "Montag, 1. Januar 2024"
    );
}

#[test]
fn time_short_24h() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("00:00:00", "00:00"),
        ("09:30:00", "09:30"),
        ("13:00:00", "13:00"),
        ("17:03:04", "17:03"),
        ("23:59:00", "23:59"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_time(input, "de", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "de", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
    assert_eq!(
        e.format_time("00:00:00", "de", DateTimeLength::Medium)
            .unwrap(),
        "00:00:00"
    );
}

#[test]
fn combined_datetime() {
    let e = engine();
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04Z",
            "de",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22.09.2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "de",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22. September 2024 17:03:04"
    );
}

#[test]
fn de_at_falls_back_to_de() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "de-AT", DateTimeLength::Short)
            .unwrap(),
        "22.09.2024"
    );
}
