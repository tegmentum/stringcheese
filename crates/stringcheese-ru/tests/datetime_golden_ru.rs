//! Golden date/time-formatting vectors for the Russian pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_ru::datetime_data::datetime_pack;

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
            &e.format_date(input, "ru", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_abbrev_month_with_g_suffix() {
    let e = engine();
    // Russian CLDR medium: `d MMM y г.` — abbreviated month + " г."
    // suffix (short for `года`, "of the year").
    let cases: &[(&str, &str)] = &[
        (
            "2024-09-22",
            "22 \u{0441}\u{0435}\u{043D}\u{0442}. 2024 \u{0433}.",
        ),
        ("2024-01-01", "1 \u{044F}\u{043D}\u{0432}. 2024 \u{0433}."),
        ("2024-05-15", "15 \u{043C}\u{0430}\u{044F} 2024 \u{0433}."),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ru", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_uses_genitive_month() {
    let e = engine();
    // Russian CLDR long: `d MMMM y г.` — full genitive month name.
    let cases: &[(&str, &str)] = &[
        (
            "2024-09-22",
            "22 \u{0441}\u{0435}\u{043D}\u{0442}\u{044F}\u{0431}\u{0440}\u{044F} 2024 \u{0433}.",
        ),
        (
            "2024-01-01",
            "1 \u{044F}\u{043D}\u{0432}\u{0430}\u{0440}\u{044F} 2024 \u{0433}.",
        ),
        (
            "2024-03-15",
            "15 \u{043C}\u{0430}\u{0440}\u{0442}\u{0430} 2024 \u{0433}.",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ru", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_includes_weekday() {
    let e = engine();
    // 2024-09-22 is Sunday (воскресенье).
    assert_eq!(
        e.format_date("2024-09-22", "ru", DateTimeLength::Full)
            .unwrap(),
        "\u{0432}\u{043E}\u{0441}\u{043A}\u{0440}\u{0435}\u{0441}\u{0435}\u{043D}\u{044C}\u{0435}, 22 \u{0441}\u{0435}\u{043D}\u{0442}\u{044F}\u{0431}\u{0440}\u{044F} 2024 \u{0433}."
    );
    // 2024-01-01 is Monday (понедельник).
    assert_eq!(
        e.format_date("2024-01-01", "ru", DateTimeLength::Full)
            .unwrap(),
        "\u{043F}\u{043E}\u{043D}\u{0435}\u{0434}\u{0435}\u{043B}\u{044C}\u{043D}\u{0438}\u{043A}, 1 \u{044F}\u{043D}\u{0432}\u{0430}\u{0440}\u{044F} 2024 \u{0433}."
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
            &e.format_time(input, "ru", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "ru", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
    assert_eq!(
        e.format_time("00:00:00", "ru", DateTimeLength::Medium)
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
            "ru",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22.09.2024 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "ru",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "22 \u{0441}\u{0435}\u{043D}\u{0442}\u{044F}\u{0431}\u{0440}\u{044F} 2024 \u{0433}. 17:03:04"
    );
}

#[test]
fn ru_ua_falls_back_to_ru() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "ru-UA", DateTimeLength::Short)
            .unwrap(),
        "22.09.2024"
    );
}

#[test]
fn era_marker_renders_cyrillic() {
    // The shipped patterns don't emit the `G` token, but the pack
    // ships the era strings for downstream callers building custom
    // patterns.
    let pack = datetime_pack().unwrap();
    assert_eq!(
        pack.data().era_bc(),
        Some("\u{0434}\u{043E} \u{043D}. \u{044D}.")
    );
    assert_eq!(pack.data().era_ad(), Some("\u{043D}. \u{044D}."));
}
