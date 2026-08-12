//! Golden plural-classification vectors for the Polish pack.
//!
//! Polish is a three-way Slavic plural (`one` / `few` / `many`) plus
//! `other` for fractional inputs. Every rule requires `v = 0`, so
//! visible fractions always land in `other`.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_pl::plural_data::plural_pack;

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_only_matches_exactly_1() {
    let e = engine();
    // Polish `one` is the strictest of the shipped locales — only
    // integer 1 (not 21, 31, …). 21 is `many` in Polish.
    assert_eq!(e.plural_cardinal(1.0, "pl"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(21.0, "pl"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(31.0, "pl"), PluralCategory::Many);
    assert_eq!(e.plural_cardinal(101.0, "pl"), PluralCategory::Many);
}

#[test]
fn cardinal_few_covers_x2_x3_x4_except_teens() {
    let e = engine();
    for n in [
        2.0, 3.0, 4.0, 22.0, 23.0, 24.0, 32.0, 33.0, 34.0, 102.0, 103.0, 104.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "pl"), PluralCategory::Few, "n={n}");
    }
}

#[test]
fn cardinal_many_covers_0_teens_5_9_and_x1_from_20up() {
    let e = engine();
    // 0 and 5-19 → many.
    for n in [
        0.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 19.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "pl"), PluralCategory::Many, "n={n}");
    }
    // 20, 25-30 → many.
    for n in [20.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0] {
        assert_eq!(e.plural_cardinal(n, "pl"), PluralCategory::Many, "n={n}");
    }
    // 12-14, even inside larger numbers → many (teens exception).
    for n in [112.0, 113.0, 114.0] {
        assert_eq!(e.plural_cardinal(n, "pl"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    for n in [0.5, 1.5, 2.5, 5.5, 100.25] {
        assert_eq!(e.plural_cardinal(n, "pl"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_uses_absolute_value() {
    let e = engine();
    assert_eq!(e.plural_cardinal(-1.0, "pl"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-2.0, "pl"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(-5.0, "pl"), PluralCategory::Many);
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [0.0, 1.0, 2.0, 3.0, 5.0, 11.0, 21.0, 100.0] {
        assert_eq!(e.plural_ordinal(n, "pl"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn pl_pl_falls_back_to_pl() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "pl-PL"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "pl-PL"), PluralCategory::Few);
    assert_eq!(e.plural_cardinal(5.0, "pl-PL"), PluralCategory::Many);
}

#[test]
fn cardinal_coverage_sweep_0_to_25() {
    // Polish table for the [0..=25] range: `one` only at 1;
    // `few` at 2/3/4/22/23/24; `many` everywhere else.
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
        PluralCategory::Many, // 21 — Polish differs from Russian
        PluralCategory::Few,  // 22
        PluralCategory::Few,  // 23
        PluralCategory::Few,  // 24
        PluralCategory::Many, // 25
    ];
    for (n, want) in expected.iter().enumerate() {
        let n_u32 = u32::try_from(n).expect("sweep index fits in u32");
        assert_eq!(e.plural_cardinal(f64::from(n_u32), "pl"), *want, "n={n}");
    }
}
