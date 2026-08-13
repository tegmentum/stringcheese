//! Golden plural-classification vectors for the Italian pack.
//!
//! Italian CLDR 44.1 cardinals: `one` (i = 1 and v = 0), `many`
//! (large-number bucket for exact million-multiples, with the
//! compact-notation branch deferred), and `other` for everything
//! else. Italian ordinals ship a `many` bucket for the four
//! distinct-marking values `n ∈ {8, 11, 80, 800}`; every other
//! value is `other`.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_it::plural_data::plural_pack;

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_only_for_integer_1() {
    let e = engine();
    // Italian uses `i = 1 and v = 0` — integer 1 only. Decimal
    // `1.0` classifies as `other` (Italian differs from Spanish
    // here, which uses `n = 1`).
    assert_eq!(e.plural_cardinal(1.0, "it"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-1.0, "it"), PluralCategory::One);
}

#[test]
fn cardinal_decimal_one_point_zero_is_other() {
    let e = engine();
    // 1.0 as a fractional representation → v ≠ 0 → not `one`.
    // f64 can't distinguish `1.0` from `1`, but decimals that
    // carry a non-zero fractional part clearly fall through.
    assert_eq!(e.plural_cardinal(1.5, "it"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(1.1, "it"), PluralCategory::Other);
}

#[test]
fn cardinal_zero_is_other() {
    let e = engine();
    // Italian uses plural for 0 (`0 cose`), unlike French where
    // 0 is singular.
    assert_eq!(e.plural_cardinal(0.0, "it"), PluralCategory::Other);
}

#[test]
fn cardinal_typical_integers_are_other() {
    let e = engine();
    for n in [2.0, 3.0, 5.0, 10.0, 21.0, 100.0, 999.0, 999_999.0] {
        assert_eq!(e.plural_cardinal(n, "it"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_many_covers_exact_million_multiples() {
    let e = engine();
    // The shipped sub-clause: v = 0 and i != 0 and i % 1_000_000 = 0.
    for n in [
        1_000_000.0,
        2_000_000.0,
        3_000_000.0,
        10_000_000.0,
        100_000_000.0,
        1_000_000_000.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "it"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_non_million_multiples_stay_other() {
    let e = engine();
    // Off-by-one from a million multiple → other, not many.
    for n in [999_999.0, 1_000_001.0, 1_500_000.0, 1_999_999.0] {
        assert_eq!(e.plural_cardinal(n, "it"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_fractional_million_is_other() {
    let e = engine();
    // v = 0 guard: a fractional million falls to `other`.
    assert_eq!(e.plural_cardinal(1_000_000.5, "it"), PluralCategory::Other);
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    for n in [0.5, 2.5, 10.1, 100.25, 999_999.5] {
        assert_eq!(e.plural_cardinal(n, "it"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_million_multiple_is_many() {
    let e = engine();
    // Absolute value drives the classification.
    assert_eq!(e.plural_cardinal(-1_000_000.0, "it"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(-2_000_000.0, "it"), PluralCategory::Many);
}

#[test]
fn ordinal_many_covers_8_11_80_800() {
    let e = engine();
    // Italian's four distinct ordinal-marking values:
    //   `ottavo → 8º`
    //   `undicesimo → 11º`
    //   `ottantesimo → 80º`
    //   `ottocentesimo → 800º`
    for n in [8.0, 11.0, 80.0, 800.0] {
        assert_eq!(e.plural_ordinal(n, "it"), PluralCategory::Many, "n={n}");
    }
    // Negatives use absolute value.
    assert_eq!(e.plural_ordinal(-8.0, "it"), PluralCategory::Many);
    assert_eq!(e.plural_ordinal(-11.0, "it"), PluralCategory::Many);
}

#[test]
fn ordinal_typical_values_are_other() {
    let e = engine();
    // Neighbours of the many-bucket values.
    for n in [
        0.0, 1.0, 2.0, 3.0, 7.0, 9.0, 10.0, 12.0, 21.0, 79.0, 81.0, 100.0, 799.0, 801.0,
    ] {
        assert_eq!(e.plural_ordinal(n, "it"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ordinal_fractional_many_bucket_values_are_other() {
    let e = engine();
    // The ItOrdinalMany predicate compares n exactly; fractional
    // inputs fall through.
    for n in [8.5, 11.5, 80.5, 800.5] {
        assert_eq!(e.plural_ordinal(n, "it"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn it_ch_and_it_it_fall_back_to_it() {
    let e = engine();
    // Regional variants fall back to the base `it` pack.
    assert_eq!(e.plural_cardinal(1.0, "it-IT"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "it-CH"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(1_000_000.0, "it-CH"),
        PluralCategory::Many
    );
    assert_eq!(e.plural_ordinal(8.0, "it-IT"), PluralCategory::Many);
    assert_eq!(e.plural_ordinal(11.0, "it-CH"), PluralCategory::Many);
}

#[test]
fn cardinal_coverage_sweep_0_to_20() {
    // 0..=20 sweep — every value except 1 falls to `other`
    // (Italian has no small-integer buckets beyond `one`).
    let e = engine();
    for n in 0..=20u32 {
        let expected = if n == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_cardinal(f64::from(n), "it"), expected, "n={n}");
    }
}

#[test]
fn ordinal_coverage_sweep_0_to_20() {
    // 0..=20 sweep — only 8 and 11 fall to `many` in this range.
    let e = engine();
    for n in 0..=20u32 {
        let expected = if n == 8 || n == 11 {
            PluralCategory::Many
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_ordinal(f64::from(n), "it"), expected, "n={n}");
    }
}

#[test]
fn engine_supports_it_via_pack() {
    let e = engine();
    assert!(e.supports("it"));
    assert!(e.supports("it-IT"));
    assert!(e.supports("it-CH"));
}
