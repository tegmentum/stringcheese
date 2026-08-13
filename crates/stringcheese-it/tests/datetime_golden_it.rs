//! Golden date/time-formatting vectors for the Italian pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_it::datetime_data::datetime_pack;

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_zero_padded_slash() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22/09/2024"),
        ("2024-01-01", "01/01/2024"),
        ("2024-12-31", "31/12/2024"),
        ("2000-02-29", "29/02/2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "it", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_abbrev_month() {
    let e = engine();
    // Pattern: `d MMM y` — non-zero-padded day.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 set 2024"),
        ("2024-01-01", "1 gen 2024"),
        ("2024-05-15", "15 mag 2024"),
        ("2024-08-01", "1 ago 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "it", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_uses_full_month_no_de_literal() {
    let e = engine();
    // Pattern: `d MMMM y` — unlike Spanish / Portuguese, Italian
    // has no "de" literal wrapper.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 settembre 2024"),
        ("2024-03-15", "15 marzo 2024"),
        ("2024-01-01", "1 gennaio 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "it", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_prepends_weekday_no_comma() {
    let e = engine();
    // Pattern: `EEEE d MMMM y` — Italian has no comma after
    // the weekday.
    // 2024-09-22 is Sunday (domenica).
    assert_eq!(
        e.format_date("2024-09-22", "it", DateTimeLength::Full)
            .unwrap(),
        "domenica 22 settembre 2024"
    );
    // 2024-01-01 is Monday (lunedì).
    assert_eq!(
        e.format_date("2024-01-01", "it", DateTimeLength::Full)
            .unwrap(),
        "luned\u{00EC} 1 gennaio 2024"
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
            &e.format_time(input, "it", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "it", DateTimeLength::Medium)
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
            "it",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22/09/2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "it",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 settembre 2024 17:03:04"
    );
}

#[test]
fn it_ch_falls_back_to_it() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "it-CH", DateTimeLength::Long)
            .unwrap(),
        "22 settembre 2024"
    );
}

#[test]
fn era_marker_renders_italian() {
    let pack = datetime_pack().unwrap();
    assert_eq!(pack.data().era_bc(), Some("a.C."));
    assert_eq!(pack.data().era_ad(), Some("d.C."));
}
