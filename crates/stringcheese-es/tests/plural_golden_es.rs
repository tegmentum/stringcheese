//! Golden plural-classification vectors for the Spanish pack.
//!
//! Spanish CLDR 44 cardinals: `one` (n = 1), `many` (large-number
//! bucket for exact million-multiples, with the compact-notation
//! branch deferred), and `other` for everything else. Ordinals are
//! `other`-only.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_es::plural_data::plural_pack;
use stringcheese_icu_plural::{PluralCategory, PluralEngine};

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_matches_n_equals_1() {
    let e = engine();
    // n = 1: Spanish uses value-equality on n, so both integer 1
    // and decimal 1.0 land in `one`.
    assert_eq!(e.plural_cardinal(1.0, "es"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-1.0, "es"), PluralCategory::One);
}

#[test]
fn cardinal_zero_is_other() {
    let e = engine();
    // Unlike French, Spanish uses plural for 0 (`0 cosas`).
    assert_eq!(e.plural_cardinal(0.0, "es"), PluralCategory::Other);
}

#[test]
fn cardinal_typical_integers_are_other() {
    let e = engine();
    for n in [2.0, 3.0, 5.0, 10.0, 21.0, 100.0, 999.0, 999_999.0] {
        assert_eq!(e.plural_cardinal(n, "es"), PluralCategory::Other, "n={n}");
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
        assert_eq!(e.plural_cardinal(n, "es"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_non_million_multiples_stay_other() {
    let e = engine();
    // Off-by-one from a million multiple → other, not many.
    for n in [999_999.0, 1_000_001.0, 1_500_000.0, 1_999_999.0] {
        assert_eq!(e.plural_cardinal(n, "es"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_fractional_million_is_other() {
    let e = engine();
    // v = 0 guard: a fractional million falls to `other`.
    assert_eq!(e.plural_cardinal(1_000_000.5, "es"), PluralCategory::Other);
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    for n in [0.5, 1.5, 2.5, 10.1, 100.25] {
        assert_eq!(e.plural_cardinal(n, "es"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_million_multiple_is_many() {
    let e = engine();
    // Absolute value drives the classification.
    assert_eq!(e.plural_cardinal(-1_000_000.0, "es"), PluralCategory::Many);
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [0.0, 1.0, 2.0, 3.0, 8.0, 11.0, 21.0, 100.0, 1_000_000.0] {
        assert_eq!(e.plural_ordinal(n, "es"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn es_mx_and_es_ar_fall_back_to_es() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "es-MX"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "es-AR"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(1_000_000.0, "es-419"),
        PluralCategory::Many
    );
}

#[test]
fn cardinal_coverage_sweep_0_to_20() {
    // 0..=20 sweep — every value except 1 falls to `other`
    // (Spanish has no small-integer buckets beyond `one`).
    let e = engine();
    for n in 0..=20u32 {
        let expected = if n == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_cardinal(f64::from(n), "es"), expected, "n={n}");
    }
}
