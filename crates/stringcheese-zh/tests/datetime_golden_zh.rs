//! Golden date/time-formatting vectors for the Chinese (Simplified)
//! pack.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};
use stringcheese_zh::datetime_data::datetime_pack;

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_slashes() {
    let e = engine();
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "2024/9/22"),
        ("2024-01-01", "2024/1/1"),
        ("2024-12-31", "2024/12/31"),
        ("2000-02-29", "2000/2/29"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "zh", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_han_year_month_day() {
    let e = engine();
    // Pattern: `y年M月d日`.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "2024\u{5E74}9\u{6708}22\u{65E5}"),
        ("2024-01-01", "2024\u{5E74}1\u{6708}1\u{65E5}"),
        ("2024-12-31", "2024\u{5E74}12\u{6708}31\u{65E5}"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "zh", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_matches_medium() {
    let e = engine();
    // CLDR ships identical long and medium for zh.
    assert_eq!(
        e.format_date("2024-09-22", "zh", DateTimeLength::Long)
            .unwrap(),
        "2024\u{5E74}9\u{6708}22\u{65E5}"
    );
}

#[test]
fn date_full_appends_weekday() {
    let e = engine();
    // Pattern: `y年M月d日EEEE`.
    // 2024-09-22 is Sunday (星期日).
    assert_eq!(
        e.format_date("2024-09-22", "zh", DateTimeLength::Full)
            .unwrap(),
        "2024\u{5E74}9\u{6708}22\u{65E5}\u{661F}\u{671F}\u{65E5}"
    );
    // 2024-01-01 is Monday (星期一).
    assert_eq!(
        e.format_date("2024-01-01", "zh", DateTimeLength::Full)
            .unwrap(),
        "2024\u{5E74}1\u{6708}1\u{65E5}\u{661F}\u{671F}\u{4E00}"
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
            &e.format_time(input, "zh", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "zh", DateTimeLength::Medium)
            .unwrap(),
        "17:03:04"
    );
    assert_eq!(
        e.format_time("00:00:00", "zh", DateTimeLength::Medium)
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
            "zh",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "2024/9/22 17:03"
    );
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04",
            "zh",
            DateTimeLength::Medium,
            DateTimeLength::Medium
        )
        .unwrap(),
        "2024\u{5E74}9\u{6708}22\u{65E5} 17:03:04"
    );
}

#[test]
fn zh_cn_falls_back_to_zh() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "zh-CN", DateTimeLength::Short)
            .unwrap(),
        "2024/9/22"
    );
}

#[test]
fn era_marker_renders_han() {
    let pack = datetime_pack().unwrap();
    assert_eq!(
        pack.data().era_bc(),
        Some("\u{516C}\u{5143}\u{524D}") // 公元前
    );
    assert_eq!(pack.data().era_ad(), Some("\u{516C}\u{5143}")); // 公元
}
