//! Golden date/time-formatting vectors for the Arabic pack.
//!
//! The pattern strings ship with U+200F RTL MARK between numeric
//! fields and slashes so bidi-aware renderers produce the correct
//! visual right-to-left order. The formatter emits the marks
//! verbatim; the expected strings here embed the same marks.

#![cfg(all(feature = "datetime-scud", not(target_family = "wasm")))]

use stringcheese_ar::datetime_data::datetime_pack;
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn engine() -> DateTimeEngine<'static> {
    DateTimeEngine::new(vec![datetime_pack().unwrap()])
}

#[test]
fn date_short_uses_rtl_marks() {
    let e = engine();
    // Pattern: `d‏/M‏/y` with U+200F between fields and slashes.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22\u{200F}/9\u{200F}/2024"),
        ("2024-01-01", "1\u{200F}/1\u{200F}/2024"),
        ("2024-12-31", "31\u{200F}/12\u{200F}/2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ar", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_medium_uses_dd_mm_pattern() {
    let e = engine();
    // Pattern: `dd‏/MM‏/y` — zero-padded numerics.
    let cases: &[(&str, &str)] = &[
        ("2024-09-22", "22\u{200F}/09\u{200F}/2024"),
        ("2024-01-01", "01\u{200F}/01\u{200F}/2024"),
        ("2024-12-31", "31\u{200F}/12\u{200F}/2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ar", DateTimeLength::Medium).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_long_uses_arabic_month_name() {
    let e = engine();
    // Pattern: `d MMMM y` — no RTL marks, full Arabic month name.
    let cases: &[(&str, &str)] = &[
        (
            "2024-09-22",
            "22 \u{0633}\u{0628}\u{062A}\u{0645}\u{0628}\u{0631} 2024",
        ),
        (
            "2024-01-01",
            "1 \u{064A}\u{0646}\u{0627}\u{064A}\u{0631} 2024",
        ),
        ("2024-05-15", "15 \u{0645}\u{0627}\u{064A}\u{0648} 2024"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_date(input, "ar", DateTimeLength::Long).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn date_full_uses_arabic_comma_and_weekday() {
    let e = engine();
    // Pattern: `EEEE، d MMMM y` — Arabic comma U+060C.
    // 2024-09-22 is Sunday (الأحد).
    assert_eq!(
        e.format_date("2024-09-22", "ar", DateTimeLength::Full)
            .unwrap(),
        "\u{0627}\u{0644}\u{0623}\u{062D}\u{062F}\u{060C} 22 \u{0633}\u{0628}\u{062A}\u{0645}\u{0628}\u{0631} 2024"
    );
    // 2024-01-01 is Monday (الاثنين).
    assert_eq!(
        e.format_date("2024-01-01", "ar", DateTimeLength::Full)
            .unwrap(),
        "\u{0627}\u{0644}\u{0627}\u{062B}\u{0646}\u{064A}\u{0646}\u{060C} 1 \u{064A}\u{0646}\u{0627}\u{064A}\u{0631} 2024"
    );
}

#[test]
fn time_short_12h_with_arabic_am_marker() {
    let e = engine();
    // AM = "ص", PM = "م". 12-hour clock: `h:mm a`.
    let cases: &[(&str, &str)] = &[
        ("00:00:00", "12:00 \u{0635}"),
        ("09:30:00", "9:30 \u{0635}"),
        ("12:00:00", "12:00 \u{0645}"),
        ("13:00:00", "1:00 \u{0645}"),
        ("17:03:04", "5:03 \u{0645}"),
        ("23:59:00", "11:59 \u{0645}"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            &e.format_time(input, "ar", DateTimeLength::Short).unwrap(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn time_medium_includes_seconds() {
    let e = engine();
    assert_eq!(
        e.format_time("17:03:04", "ar", DateTimeLength::Medium)
            .unwrap(),
        "5:03:04 \u{0645}"
    );
    assert_eq!(
        e.format_time("00:00:00", "ar", DateTimeLength::Medium)
            .unwrap(),
        "12:00:00 \u{0635}"
    );
}

#[test]
fn combined_datetime_composes_date_and_time() {
    let e = engine();
    assert_eq!(
        e.format_datetime(
            "2024-09-22T17:03:04Z",
            "ar",
            DateTimeLength::Short,
            DateTimeLength::Short
        )
        .unwrap(),
        "22\u{200F}/9\u{200F}/2024 5:03 \u{0645}"
    );
}

#[test]
fn ar_sa_falls_back_to_ar() {
    let e = engine();
    assert_eq!(
        e.format_date("2024-09-22", "ar-SA", DateTimeLength::Short)
            .unwrap(),
        "22\u{200F}/9\u{200F}/2024"
    );
}

#[test]
fn era_marker_renders_arabic() {
    let pack = datetime_pack().unwrap();
    assert_eq!(pack.data().era_bc(), Some("\u{0642}.\u{0645}")); // ق.م
    assert_eq!(pack.data().era_ad(), Some("\u{0645}")); // م
}
