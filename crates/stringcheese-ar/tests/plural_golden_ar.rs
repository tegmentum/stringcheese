//! Golden plural-classification vectors for the Arabic pack.
//!
//! Arabic is the **maximum-category-count** CLDR locale: it uses
//! every one of the six plural categories (`zero`, `one`, `two`,
//! `few`, `many`, `other`). This suite exercises each — the
//! 30-vector total per the task's minimum for 6-bucket locales.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_ar::plural_data::plural_pack;
use stringcheese_icu_plural::{PluralCategory, PluralEngine};

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_zero_is_zero() {
    let e = engine();
    assert_eq!(e.plural_cardinal(0.0, "ar"), PluralCategory::Zero);
}

#[test]
fn cardinal_one_is_one() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "ar"), PluralCategory::One);
}

#[test]
fn cardinal_two_is_two() {
    let e = engine();
    assert_eq!(e.plural_cardinal(2.0, "ar"), PluralCategory::Two);
}

#[test]
fn cardinal_few_covers_3_through_10_of_every_hundred() {
    let e = engine();
    for n in [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Few, "n={n}");
    }
    // The mod-100 wrap: 103..=110 all count as `few`.
    for n in [103.0, 104.0, 105.0, 108.0, 110.0, 210.0, 1003.0] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Few, "n={n}");
    }
}

#[test]
fn cardinal_many_covers_11_through_99_of_every_hundred() {
    let e = engine();
    for n in [11.0, 12.0, 25.0, 50.0, 75.0, 98.0, 99.0] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Many, "n={n}");
    }
    // Mod-100 wrap: 111..=199 land in `many`.
    for n in [111.0, 150.0, 199.0, 211.0, 299.0, 1050.0] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_other_covers_multiples_of_100() {
    let e = engine();
    // 100, 200, 300, … and n%100 in {0..2} for n > 2 → other.
    for n in [
        100.0, 101.0, 102.0, 200.0, 201.0, 202.0, 300.0, 1000.0, 10_000.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    // Every non-zero fractional value has v != 0 or n != 0/1/2,
    // and n%100 in {3..10, 11..99} requires v = 0 → so fractions
    // like 3.5 fall to `other` because ArFew requires v = 0.
    for n in [0.5, 1.5, 2.5, 3.5, 11.5, 50.5] {
        assert_eq!(e.plural_cardinal(n, "ar"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_uses_absolute_value() {
    let e = engine();
    assert_eq!(e.plural_cardinal(-1.0, "ar"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-2.0, "ar"), PluralCategory::Two);
    assert_eq!(e.plural_cardinal(-3.0, "ar"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(-11.0, "ar"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(-100.0, "ar"), PluralCategory::Other);
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [0.0, 1.0, 2.0, 3.0, 5.0, 11.0, 100.0] {
        assert_eq!(e.plural_ordinal(n, "ar"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ar_eg_falls_back_to_ar() {
    let e = engine();
    assert_eq!(e.plural_cardinal(0.0, "ar-EG"), PluralCategory::Zero);
    assert_eq!(e.plural_cardinal(1.0, "ar-SA"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "ar-AE"), PluralCategory::Two);
    assert_eq!(e.plural_cardinal(5.0, "ar-MA"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(50.0, "ar-JO"), PluralCategory::Many);
}

#[test]
fn cardinal_boundary_sweep_around_100_and_200() {
    // Boundary sweep — the transitions 10→11 (Few→Many), 99→100
    // (Many→Other), 102→103 (Other→Few), 110→111 (Few→Many),
    // 199→200 (Many→Other) all matter.
    let e = engine();
    assert_eq!(e.plural_cardinal(10.0, "ar"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(11.0, "ar"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(99.0, "ar"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(100.0, "ar"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(101.0, "ar"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(102.0, "ar"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(103.0, "ar"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(110.0, "ar"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(111.0, "ar"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(199.0, "ar"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(200.0, "ar"), PluralCategory::Other);
}
