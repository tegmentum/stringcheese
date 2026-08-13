//! Golden date/time-formatting vectors for the Portuguese pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_pt::datetime_data::datetime_pack;

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_zero_padded_slash() {
    let e = engine();
    // Pattern: `dd/MM/y` — zero-padded.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22/09/2024"),
        ("2024-01-01", "01/01/2024"),
        ("2024-12-31", "31/12/2024"),
        ("2000-02-29", "29/02/2000"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "pt", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_wraps_abbrev_month_with_de_literals() {
    let e = engine();
    // Pattern: `d 'de' MMM 'de' y` — abbreviated month plus
    // literal "de" wrappers.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 de set. de 2024"),
        ("2024-01-01", "1 de jan. de 2024"),
        ("2024-05-15", "15 de mai. de 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "pt", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_wraps_full_month_with_de_literals() {
    let e = engine();
    // Pattern: `d 'de' MMMM 'de' y`.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22 de setembro de 2024"),
        ("2024-03-15", "15 de mar\u{00E7}o de 2024"),
        ("2024-01-01", "1 de janeiro de 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "pt", DateTimeLength::Long).unwrap(),
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
        e.format_date("2024-09-22", "pt", DateTimeLength::Full)
            .unwrap(),
        "domingo, 22 de setembro de 2024"
    );
    // 2024-01-01 is Monday (segunda-feira).
    assert_eq!(
        e.format_date("2024-01-01", "pt", DateTimeLength::Full)
            .unwrap(),
        "segunda-feira, 1 de janeiro de 2024"
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
            &e.format_time(input, "pt", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "pt", DateTimeLength::Medium)
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
            "pt",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22/09/2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "pt",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 de setembro de 2024 17:03:04"
    );
}

#[test]
fn pt_br_falls_back_to_pt() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "pt-BR", DateTimeLength::Long)
            .unwrap(),
        "22 de setembro de 2024"
    );
    assert_eq!(
        e.format_date("2024-09-22", "pt-PT", DateTimeLength::Short)
            .unwrap(),
        "22/09/2024"
    );
}

#[test]
fn era_marker_renders_portuguese() {
    let pack = datetime_pack().unwrap();
    assert_eq!(pack.data().era_bc(), Some("a.C."));
    assert_eq!(pack.data().era_ad(), Some("d.C."));
}
