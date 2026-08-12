//! Golden plural-classification vectors for the Russian pack.
//!
//! Russian is one of the classic Slavic three-way plurals (`one` /
//! `few` / `many`), plus an implicit `other` for fractional inputs.
//! Every rule requires `v = 0`, so any visible fraction digit
//! sends the value to `other` regardless of its integer part.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_ru::plural_data::plural_pack;

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_covers_x1_except_x11() {
    let e = engine();
    for n in [
        1.0, 21.0, 31.0, 41.0, 51.0, 61.0, 71.0, 81.0, 91.0, 101.0, 121.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::One, "n={n}");
    }
    // The 111 teens exception — 111 % 100 = 11, so `many`.
    assert_eq!(e.plural_cardinal(111.0, "ru"), PluralCategory::Many);
}

#[test]
fn cardinal_few_covers_x2_x3_x4_except_teens() {
    let e = engine();
    for n in [
        2.0, 3.0, 4.0, 22.0, 23.0, 24.0, 32.0, 33.0, 34.0, 102.0, 103.0, 104.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::Few, "n={n}");
    }
}

#[test]
fn cardinal_many_covers_0_teens_and_x5_x9() {
    let e = engine();
    // 0 and 5-20 are `many`.
    for n in [
        0.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 20.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::Many, "n={n}");
    }
    // The 25-30 tail — decades except x1-x4.
    for n in [25.0, 26.0, 27.0, 28.0, 29.0, 30.0] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::Many, "n={n}");
    }
    // 11-14 are the teens exception even inside larger numbers.
    for n in [111.0, 112.0, 113.0, 114.0] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    // Every rule requires v = 0; visible fractions → other.
    for n in [0.5, 1.5, 2.5, 5.5, 10.1, 100.25] {
        assert_eq!(e.plural_cardinal(n, "ru"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_uses_absolute_value() {
    let e = engine();
    assert_eq!(e.plural_cardinal(-1.0, "ru"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-2.0, "ru"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(-5.0, "ru"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(-11.0, "ru"), PluralCategory::Many);
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [0.0, 1.0, 2.0, 3.0, 5.0, 11.0, 21.0, 100.0, 101.0] {
        assert_eq!(e.plural_ordinal(n, "ru"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ru_ru_falls_back_to_ru() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "ru-RU"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "ru-BY"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(5.0, "ru-KZ"), PluralCategory::Many);
    assert_eq!(e.plural_ordinal(1.0, "ru-UA"), PluralCategory::Other);
}

#[test]
fn cardinal_coverage_sweep_0_to_30() {
    // Sweep 0..=30 with the CLDR-declared expectations. Any drift
    // in the SlavFew / RuMany / RuOne predicate rings a bell.
    let e = engine();
    let expected = [
        PluralCategory::Many, // 0
        PluralCategory::One,  // 1
        PluralCategory::Few,  // 2
        PluralCategory::Few,  // 3
        PluralCategory::Few,  // 4
        PluralCategory::Many, // 5
        PluralCategory::Many, // 6
        PluralCategory::Many, // 7
        PluralCategory::Many, // 8
        PluralCategory::Many, // 9
        PluralCategory::Many, // 10
        PluralCategory::Many, // 11
        PluralCategory::Many, // 12
        PluralCategory::Many, // 13
        PluralCategory::Many, // 14
        PluralCategory::Many, // 15
        PluralCategory::Many, // 16
        PluralCategory::Many, // 17
        PluralCategory::Many, // 18
        PluralCategory::Many, // 19
        PluralCategory::Many, // 20
        PluralCategory::One,  // 21
        PluralCategory::Few,  // 22
        PluralCategory::Few,  // 23
        PluralCategory::Few,  // 24
        PluralCategory::Many, // 25
        PluralCategory::Many, // 26
        PluralCategory::Many, // 27
        PluralCategory::Many, // 28
        PluralCategory::Many, // 29
        PluralCategory::Many, // 30
    ];
    for (n, want) in expected.iter().enumerate() {
        let n_u32 = u32::try_from(n).expect("sweep index fits in u32");
        assert_eq!(e.plural_cardinal(f64::from(n_u32), "ru"), *want, "n={n}");
    }
}
