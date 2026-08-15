//! # WIT-i18n datetime formatting across locales and pattern lengths
//!
//! Render an ISO-8601 date at the four CLDR pattern lengths (short,
//! medium, long, full) in three locales — English, German, Japanese
//! — to show pattern + month-name + weekday-name variations.
//!
//! * `en` — `M/d/y`, `MMM d, y`, `MMMM d, y`, `EEEE, MMMM d, y`.
//! * `de` — day-first CLDR patterns.
//! * `ja` — numeric `y/MM/dd` short/medium, kanji-suffixed
//!   `y\u{5E74}M\u{6708}d\u{65E5}` long/full.
//!
//! SCUD packs loaded: `datetime-en`, `datetime-de`, `datetime-ja`
//! (from each language crate's `datetime_data` module).
//!
//! Run: `cargo run --example i18n_datetime -p stringcheese --all-features`
use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

fn main() {
    let engine = DateTimeEngine::new(vec![
        stringcheese_en::datetime_data::datetime_pack().expect("datetime-en pack loads"),
        stringcheese_de::datetime_data::datetime_pack().expect("datetime-de pack loads"),
        stringcheese_ja::datetime_data::datetime_pack().expect("datetime-ja pack loads"),
    ]);

    let iso_date = "2026-08-15";
    let lengths = [
        (DateTimeLength::Short, "short"),
        (DateTimeLength::Medium, "medium"),
        (DateTimeLength::Long, "long"),
        (DateTimeLength::Full, "full"),
    ];

    println!("Input date (ISO-8601): {iso_date}");
    println!();
    println!("{:<8}  {:<8}  formatted", "locale", "length");
    println!("{:<8}  {:<8}  ---------", "------", "------");
    for locale in ["en", "de", "ja"] {
        for (length, label) in lengths {
            let formatted = engine
                .format_date(iso_date, locale, length)
                .unwrap_or_else(|e| format!("<error: {e:?}>"));
            println!("{locale:<8}  {label:<8}  {formatted}");
        }
    }
}
