//! Property fuzz target: Jaro similarity range and symmetry.
//!
//! Jaro is a bounded similarity in the closed interval `[0.0, 1.0]`. Two
//! invariants must hold on every input:
//!
//! * **Range.** `0.0 <= sim(a, b) <= 1.0`. The kernel constructs a
//!   `NormalizedSimilarity` internally via `new_unchecked`, so an
//!   out-of-range result would silently produce nonsensical downstream
//!   behavior instead of a panic; the fuzz assertion is the safety net.
//! * **Bit-exact symmetry.** Jaro's matching-window formulation is
//!   order-independent — every operation on `(a, b)` has a mirror on
//!   `(b, a)` — so the floating-point result must be *bit-exact* equal, not
//!   merely close-in-tolerance.
//! * **Identity.** `sim(a, a) == 1.0` (bit-exact), the identity of
//!   indiscernibles for a bounded similarity.
//!
//! `NaN` values are explicitly disallowed. `Similarity<f64>` wraps a bare
//! `f64` and does not itself reject NaN; a NaN would propagate silently
//! through `NormalizedSimilarity::new_unchecked`, breaking every consumer
//! that assumes `[0.0, 1.0]`.

#![no_main]

use stringcheese_core::SimilarityMetric;
use stringcheese_compare::jaro::Jaro;
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let (a, b) = common::split2(data);
    let jaro = Jaro;

    let s_ab = jaro.similarity(a, b).into_inner();
    let s_ba = jaro.similarity(b, a).into_inner();

    // Range: every Jaro output must be a finite value in [0.0, 1.0].
    assert!(
        s_ab.is_finite() && (0.0..=1.0).contains(&s_ab),
        "Jaro produced out-of-range value {s_ab} on ({a:?}, {b:?})",
    );
    assert!(
        s_ba.is_finite() && (0.0..=1.0).contains(&s_ba),
        "Jaro produced out-of-range value {s_ba} on reversed ({b:?}, {a:?})",
    );

    // Bit-exact symmetry: Jaro is order-independent.
    assert_eq!(
        s_ab.to_bits(),
        s_ba.to_bits(),
        "Jaro symmetry violated (bit-exact): sim(a,b)={s_ab}, sim(b,a)={s_ba} on ({a:?}, {b:?})",
    );

    // Identity of indiscernibles: sim(a, a) == 1.0 bit-exactly for any input.
    let s_aa = jaro.similarity(a, a).into_inner();
    assert_eq!(
        s_aa.to_bits(),
        1.0_f64.to_bits(),
        "Jaro violated identity of indiscernibles: sim(a,a)={s_aa} on {a:?}",
    );
});
