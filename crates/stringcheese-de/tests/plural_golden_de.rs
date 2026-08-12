//! Golden plural-classification vectors for the German pack.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_de::plural_data::plural_pack;
use stringcheese_icu_plural::{PluralCategory, PluralEngine};

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_is_singular_integer() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "de"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-1.0, "de"), PluralCategory::One);
}

#[test]
fn cardinal_other_covers_zero_two_and_larger() {
    let e = engine();
    for n in [0.0, 2.0, 3.0, 5.0, 10.0, 42.0, 100.0, 1000.0, 1_000_000.0] {
        assert_eq!(e.plural_cardinal(n, "de"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_other_covers_fractional() {
    let e = engine();
    for n in [0.5, 1.5, 2.7, 10.1] {
        assert_eq!(e.plural_cardinal(n, "de"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [
        0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 11.0, 21.0, 100.0, 101.0, 1000.0,
    ] {
        assert_eq!(e.plural_ordinal(n, "de"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn de_de_falls_back_to_de() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "de-DE"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "de-AT"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(1.0, "de-CH"), PluralCategory::Other);
}

#[test]
fn cardinal_coverage_50_boundaries() {
    // 50-boundary sweep — for German every integer except 1 (and
    // -1) is Other; every fractional is Other. Enumerate a wide
    // range to catch any accidental rule-id divergence.
    let e = engine();
    for n in 0..=50 {
        let expected = if n == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_cardinal(f64::from(n), "de"), expected, "n={n}");
    }
}
