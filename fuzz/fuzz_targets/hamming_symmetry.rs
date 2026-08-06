//! Property fuzz target: Hamming symmetry and length-mismatch semantics.
//!
//! Two invariants are asserted per iteration:
//!
//! * Symmetry on equal-length inputs. Hamming is a true metric on the space
//!   of equal-length sequences; `d(a, b) == d(b, a)` must hold bit-exactly
//!   for every pair the fuzzer produces.
//! * `try_distance` returns `Ok` iff `left.len() == right.len()`, and returns
//!   `Err(LengthMismatch)` otherwise. Both cases are exercised — the
//!   equal-halves split checks the happy path, and the asymmetric split
//!   checks the length-mismatch guard.
//!
//! No differential oracle is required here: Hamming distance is a single
//! fold with a running counter, and the crate's kernel doubles as its own
//! oracle. The interesting axioms live at the API boundary.

#![no_main]

use stringcheese_hamming::{Hamming, LengthMismatch, hamming_distance};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    // Equal-length case: symmetry.
    let (a, b) = common::equal_halves(data);
    assert_eq!(a.len(), b.len(), "equal_halves must return equal-length sides");

    let d_ab = hamming_distance(a, b).into_inner();
    let d_ba = hamming_distance(b, a).into_inner();
    assert_eq!(
        d_ab, d_ba,
        "Hamming symmetry violated: d(a,b)={d_ab} but d(b,a)={d_ba} on ({a:?}, {b:?})",
    );

    // Identity of indiscernibles: d(a, a) must be zero for any input a.
    assert_eq!(
        hamming_distance(a, a).into_inner(),
        0,
        "Hamming failed identity of indiscernibles on {a:?}",
    );

    // try_distance on equal lengths must agree with the infallible kernel.
    let ok = Hamming.try_distance(a, b).expect("equal-length inputs must succeed");
    assert_eq!(ok.into_inner(), d_ab);

    // Asymmetric split: `try_distance` must report length mismatch when the
    // lengths differ. Use `split2` (which is generally unequal) and check
    // the guard explicitly.
    let (x, y) = common::split2(data);
    let result = Hamming.try_distance(x, y);
    if x.len() == y.len() {
        assert!(
            result.is_ok(),
            "try_distance rejected equal-length inputs ({}, {})",
            x.len(),
            y.len(),
        );
    } else {
        assert_eq!(
            result,
            Err(LengthMismatch {
                left: x.len(),
                right: y.len(),
            }),
            "try_distance did not report the expected LengthMismatch on ({}, {})",
            x.len(),
            y.len(),
        );
    }
});
