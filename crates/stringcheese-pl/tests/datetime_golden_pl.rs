//! Golden date/time-formatting vectors for the Polish pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_pl::datetime_data::datetime_pack;

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
            &e.format_date(input, "pl", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_abbrev_month() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 wrz 2024"),
        ("2024-01-01", "1 sty 2024"),
        ("2024-05-15", "15 maj 2024"),
        ("2024-12-31", "31 gru 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "pl", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_uses_genitive_month() {
    let e = engine();
    // Polish CLDR long: `d MMMM y` — full genitive month name.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 wrze\u{015B}nia 2024"),
        ("2024-01-01", "1 stycznia 2024"),
        ("2024-03-15", "15 marca 2024"),
        ("2024-10-05", "5 pa\u{017A}dziernika 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "pl", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_includes_weekday() {
    let e = engine();
    // 2024-09-22 is Sunday (niedziela).
    assert_eq!(
        e.format_date("2024-09-22", "pl", DateTimeLength::Full)
            .unwrap(),
        "niedziela, 22 wrze\u{015B}nia 2024"
    );
    // 2024-01-01 is Monday (poniedziałek).
    assert_eq!(
        e.format_date("2024-01-01", "pl", DateTimeLength::Full)
            .unwrap(),
        "poniedzia\u{0142}ek, 1 stycznia 2024"
    );
    // 2024-01-03 is Wednesday (środa).
    assert_eq!(
        e.format_date("2024-01-03", "pl", DateTimeLength::Full)
            .unwrap(),
        "\u{015B}roda, 3 stycznia 2024"
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
            &e.format_time(input, "pl", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "pl", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
    assert_eq!(
        e.format_time("00:00:00", "pl", DateTimeLength::Medium)
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
            "pl",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22.09.2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "pl",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 wrze\u{015B}nia 2024 17:03:04"
    );
}

#[test]
fn pl_pl_falls_back_to_pl() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "pl-PL", DateTimeLength::Short)
            .unwrap(),
        "22.09.2024"
    );
}

#[test]
fn era_marker_renders_polish() {
    let pack = datetime_pack().unwrap();
    assert_eq!(pack.data().era_bc(), Some("p.n.e."));
    assert_eq!(pack.data().era_ad(), Some("n.e."));
}
