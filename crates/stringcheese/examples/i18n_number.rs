//! # WIT-i18n number formatting across locales
//!
//! Format the same decimal value under four locales — English,
//! German, French, Arabic — to show CLDR group / decimal separator
//! and currency-placement differences:
//!
//! * **en** — group `,`, decimal `.`, currency prefixed (`$1,234.56`).
//! * **de** — group `.`, decimal `,`, currency suffixed (`1.234,56 \u{20AC}`).
//! * **fr** — group NBSP, decimal `,`, currency suffixed
//!   (`1\u{00A0}234,56 \u{20AC}`).
//! * **ar** — CLDR default `latn` digits with `,` group and `.`
//!   decimal, currency prefixed.
//!
//! SCUD packs loaded: `number-en`, `number-de`, `number-fr`,
//! `number-ar` (from each language crate's `number_data` module).
//!
//! Run: `cargo run --example i18n_number -p stringcheese --all-features`
use stringcheese_icu_number::{FormattingOptions, NumberEngine};

fn main() {
    let engine = NumberEngine::new(vec![
        stringcheese_en::number_data::number_pack().expect("number-en pack loads"),
        stringcheese_de::number_data::number_pack().expect("number-de pack loads"),
        stringcheese_fr::number_data::number_pack().expect("number-fr pack loads"),
        stringcheese_ar::number_data::number_pack().expect("number-ar pack loads"),
    ]);

    let value = 1_234_567.89_f64;
    let opts = FormattingOptions::default();

    println!("Value: {value}");
    println!();
    println!(
        "{:<8}  {:<20}  {:<20}  percent (0.5)",
        "locale", "decimal", "currency"
    );
    println!(
        "{:<8}  {:<20}  {:<20}  -------------",
        "------", "-------", "--------"
    );
    for (locale, currency) in [("en", "USD"), ("de", "EUR"), ("fr", "EUR"), ("ar", "USD")] {
        let dec = engine
            .format_decimal(value, locale, opts)
            .unwrap_or_else(|e| format!("<error: {e:?}>"));
        let cur = engine
            .format_currency(value, currency, locale, opts)
            .unwrap_or_else(|e| format!("<error: {e:?}>"));
        let pct = engine
            .format_percent(0.5, locale, opts)
            .unwrap_or_else(|e| format!("<error: {e:?}>"));
        println!("{locale:<8}  {dec:<20}  {cur:<20}  {pct}");
    }
}
