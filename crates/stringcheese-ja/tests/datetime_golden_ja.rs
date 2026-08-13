//! Golden date/time-formatting vectors for the Japanese pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_ja::datetime_data::datetime_pack;

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_zero_pads() {
    let e = engine();
    // Pattern: `y/MM/dd` — zero-padded month and day.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "2024/09/22"),
        ("2024-01-01", "2024/01/01"),
        ("2024-12-31", "2024/12/31"),
        ("2000-02-29", "2000/02/29"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ja", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_matches_short() {
    let e = engine();
    // Japanese CLDR ships identical short and medium (`y/MM/dd`).
    assert_eq!(
        e.format_date("2024-09-22", "ja", DateTimeLength::Medium)
            .unwrap(),
        "2024/09/22"
    );
    assert_eq!(
        e.format_date("2024-01-01", "ja", DateTimeLength::Medium)
            .unwrap(),
        "2024/01/01"
    );
}

#[test]
fn date_long_uses_han_year_month_day() {
    let e = engine();
    // Pattern: `y年M月d日` — non-zero-padded month/day.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "2024\u{5E74}9\u{6708}22\u{65E5}"),
        ("2024-01-01", "2024\u{5E74}1\u{6708}1\u{65E5}"),
        ("2024-12-31", "2024\u{5E74}12\u{6708}31\u{65E5}"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ja", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_appends_weekday() {
    let e = engine();
    // Pattern: `y年M月d日EEEE`.
    // 2024-09-22 is Sunday (日曜日).
    assert_eq!(
        e.format_date("2024-09-22", "ja", DateTimeLength::Full)
            .unwrap(),
        "2024\u{5E74}9\u{6708}22\u{65E5}\u{65E5}\u{66DC}\u{65E5}"
    );
    // 2024-01-01 is Monday (月曜日).
    assert_eq!(
        e.format_date("2024-01-01", "ja", DateTimeLength::Full)
            .unwrap(),
        "2024\u{5E74}1\u{6708}1\u{65E5}\u{6708}\u{66DC}\u{65E5}"
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
            &e.format_time(input, "ja", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "ja", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
    assert_eq!(
        e.format_time("00:00:00", "ja", DateTimeLength::Medium)
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
            "ja",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "2024/09/22 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "ja",
            DateTimeLength::Long,
            DateTimeLength::Medium
        )
        .unwrap(),
        "2024\u{5E74}9\u{6708}22\u{65E5} 17:03:04"
    );
}

#[test]
fn ja_jp_falls_back_to_ja() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "ja-JP", DateTimeLength::Short)
            .unwrap(),
        "2024/09/22"
    );
}

#[test]
fn era_marker_renders_gregorian_kana_and_han() {
    let pack = datetime_pack().unwrap();
    // BC = 紀元前, AD = 西暦 (Gregorian). The Japanese Imperial
    // calendar (Reiwa/Heisei/…) is a documented Phase 4 follow-up.
    assert_eq!(pack.data().era_bc(), Some("\u{7D00}\u{5143}\u{524D}"));
    assert_eq!(pack.data().era_ad(), Some("\u{897F}\u{66A6}"));
}
