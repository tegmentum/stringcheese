//! Golden date/time-formatting vectors for the Spanish pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_es::datetime_data::datetime_pack;
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_slashes_no_zero_pad() {
    let e = engine();
    // Pattern: `d/M/y` — no zero padding.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22/9/2024"),
        ("2024-01-01", "1/1/2024"),
        ("2024-12-31", "31/12/2024"),
        ("2000-02-29", "29/2/2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "es", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_abbrev_month() {
    let e = engine();
    // Pattern: `d MMM y` — post-CLDR-42 September = "sept".
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 sept 2024"),
        ("2024-01-01", "1 ene 2024"),
        ("2024-05-15", "15 may 2024"),
        ("2024-08-01", "1 ago 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "es", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_wraps_month_with_de_literal() {
    let e = engine();
    // Pattern: `d 'de' MMMM 'de' y` — `de` inside CLDR quoted
    // literals. Verifies the quoted-literal handling.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 de septiembre de 2024"),
        ("2024-02-29", "29 de febrero de 2024"),
        ("2024-01-01", "1 de enero de 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "es", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_prepends_weekday() {
    let e = engine();
    // 2024-09-22 is Sunday (domingo).
    assert_eq!(
        e.format_date("2024-09-22", "es", DateTimeLength::Full)
            .unwrap(),
        "domingo, 22 de septiembre de 2024"
    );
    // 2024-01-01 is Monday (lunes).
    assert_eq!(
        e.format_date("2024-01-01", "es", DateTimeLength::Full)
            .unwrap(),
        "lunes, 1 de enero de 2024"
    );
    // 2024-01-03 is Wednesday (miércoles).
    assert_eq!(
        e.format_date("2024-01-03", "es", DateTimeLength::Full)
            .unwrap(),
        "mi\u{00E9}rcoles, 3 de enero de 2024"
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
            &e.format_time(input, "es", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "es", DateTimeLength::Medium)
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
            "es",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22/9/2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "es",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 de septiembre de 2024 17:03:04"
    );
}

#[test]
fn es_mx_falls_back_to_es() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "es-MX", DateTimeLength::Long)
            .unwrap(),
        "22 de septiembre de 2024"
    );
    assert_eq!(
        e.format_date("2024-09-22", "es-AR", DateTimeLength::Short)
            .unwrap(),
        "22/9/2024"
    );
}

#[test]
fn era_marker_renders_spanish() {
    let pack = datetime_pack().unwrap();
    assert_eq!(pack.data().era_bc(), Some("a. C."));
    assert_eq!(pack.data().era_ad(), Some("d. C."));
}
