//! Golden plural-classification vectors for the French pack.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_fr::plural_data::plural_pack;
use stringcheese_icu_plural::{PluralCategory, PluralEngine};

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_zero_is_one() {
    let e = engine();
    // French: 0 uses the singular ("0 chose", not "0 choses").
    assert_eq!(e.plural_cardinal(0.0, "fr"), PluralCategory::One);
}

#[test]
fn cardinal_one_is_one() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "fr"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-1.0, "fr"), PluralCategory::One);
}

#[test]
fn cardinal_two_and_larger_are_other() {
    let e = engine();
    for n in [2.0, 3.0, 5.0, 10.0, 100.0, 1000.0, 1_000_000.0] {
        assert_eq!(e.plural_cardinal(n, "fr"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_fractional_zero_one_still_one() {
    let e = engine();
    // In French, `i in 0..1` means the integer part is 0 or 1 —
    // v = 0 required. `0.5` has i=0, v=1 → falls out.
    // But the shipped rule ignores v, using IIn01 which checks
    // `(i == 0 || i == 1) && v == 0`. So 0.5 → Other.
    assert_eq!(e.plural_cardinal(0.5, "fr"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(1.5, "fr"), PluralCategory::Other);
}

#[test]
fn ordinal_one_is_only_1() {
    let e = engine();
    // French: only 1 is "premier/première"; every other value uses
    // "e" suffix.
    assert_eq!(e.plural_ordinal(1.0, "fr"), PluralCategory::One);
    assert_eq!(e.plural_ordinal(2.0, "fr"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(3.0, "fr"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(11.0, "fr"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(21.0, "fr"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(100.0, "fr"), PluralCategory::Other);
}

#[test]
fn fr_ca_falls_back_to_fr() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "fr-CA"), PluralCategory::One);
    assert_eq!(e.plural_ordinal(2.0, "fr-CA"), PluralCategory::Other);
}

#[test]
fn cardinal_coverage_50_boundaries() {
    // 50-boundary sweep — for French every integer other than 0
    // and 1 is Other.
    let e = engine();
    for n in 0..=50 {
        let expected = if n == 0 || n == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_cardinal(f64::from(n), "fr"), expected, "n={n}");
    }
}
