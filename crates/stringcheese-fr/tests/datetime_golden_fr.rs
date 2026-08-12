//! Golden date/time-formatting vectors for the French pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_fr::datetime_data::datetime_pack;
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_slashes() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22/09/2024"),
        ("2024-01-01", "01/01/2024"),
        ("2024-12-31", "31/12/2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "fr", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_short_month() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 sept. 2024"),
        ("2024-03-01", "1 mars 2024"),
        ("2024-04-15", "15 avr. 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "fr", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_uses_full_month() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 septembre 2024"),
        ("2024-02-29", "29 f\u{00E9}vrier 2024"),
        ("2024-08-01", "1 ao\u{00FB}t 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "fr", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_uses_lowercase_weekday() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "fr", DateTimeLength::Full)
            .unwrap(),
        "dimanche 22 septembre 2024"
    );
    assert_eq!(
        e.format_date("2024-01-01", "fr", DateTimeLength::Full)
            .unwrap(),
        "lundi 1 janvier 2024"
    );
}

#[test]
fn time_short_24h() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("00:00:00", "00:00"),
        ("13:00:00", "13:00"),
        ("17:03:04", "17:03"),
        ("23:59:00", "23:59"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_time(input, "fr", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "fr", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
}

#[test]
fn combined_datetime() {
    let e = engine();
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04Z",
            "fr",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22/09/2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "fr",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 septembre 2024 17:03:04"
    );
}

#[test]
fn fr_ca_falls_back_to_fr() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "fr-CA", DateTimeLength::Medium)
            .unwrap(),
        "22 sept. 2024"
    );
}
