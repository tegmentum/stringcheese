//! # WIT-i18n CLDR plural categorisation across locales
//!
//! Classify the integers 0-10 into CLDR cardinal plural categories in
//! three locales with different bucket sets:
//!
//! * **en** — two categories (`one`, `other`).
//! * **ru** — four categories (`one`, `few`, `many`, `other`) with
//!   modulo-based bucketing.
//! * **ar** — the maximum-count locale (`zero`, `one`, `two`, `few`,
//!   `many`, `other`).
//!
//! One `PluralEngine` holds all three packs; each query routes by
//! BCP 47 locale tag.
//!
//! SCUD packs loaded: `plural-en` (`stringcheese-en::plural_data`),
//! `plural-ru` (`stringcheese-ru::plural_data`),
//! `plural-ar` (`stringcheese-ar::plural_data`).
//!
//! Run: `cargo run --example i18n_plural -p stringcheese --all-features`
use stringcheese_icu_plural::PluralEngine;

fn main() {
    let engine = PluralEngine::new(vec![
        stringcheese_en::plural_data::plural_pack().expect("plural-en pack loads"),
        stringcheese_ru::plural_data::plural_pack().expect("plural-ru pack loads"),
        stringcheese_ar::plural_data::plural_pack().expect("plural-ar pack loads"),
    ]);

    let locales = ["en", "ru", "ar"];
    let numbers: [u32; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 21, 100];

    print!("{:<5}", "n");
    for locale in locales {
        print!("  {locale:<7}");
    }
    println!();
    print!("{:<5}", "----");
    for _ in locales {
        print!("  {:<7}", "-------");
    }
    println!();

    for n in numbers {
        print!("{n:<5}");
        for locale in locales {
            let cat = engine.plural_cardinal(f64::from(n), locale);
            print!("  {:<7}", format!("{cat:?}").to_lowercase());
        }
        println!();
    }
}
