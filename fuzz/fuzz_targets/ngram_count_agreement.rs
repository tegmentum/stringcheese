//! Property fuzz target: `count_grams` closed-form vs iterator agreement.
//!
//! `count_grams(input_len, n, padding)` is the closed-form preallocation
//! helper that consumers use to size a backing store before iterating a
//! generator. The iterator itself is the source of truth for how many grams
//! are actually produced. The two must agree for every combination of input
//! length, arity `n`, and padding policy — a mismatch would either
//! under-allocate (heap re-grow / slow) or over-allocate (waste), and would
//! silently break `size_hint`-based consumers.
//!
//! The fuzz sweep covers both `PaddingPolicy::None` and
//! `PaddingPolicy::Boundary`; each `n` from 1 through 5 is exercised. The
//! `Custom` variant is out of scope here — its prefix/suffix are
//! caller-supplied `Vec`s, which the closed form counts directly, and no
//! non-trivial arithmetic sits between the two paths for it.
//!
//! `CharacterGramSlices::count` is also spot-checked for the unpadded case,
//! since the two closed-form counters (`count_grams(_, n, None)` and
//! `CharacterGramSlices::count`) must agree.

#![no_main]

use stringcheese_ngram::{CharacterGramSlices, CharacterGrams, NGramGenerator, PaddingPolicy, count_grams};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    // Cap the input; ngram iteration is linear but there's no reason to feed
    // the closed-form checker huge slabs when a small one exercises the same
    // arithmetic.
    let input = &data[..data.len().min(common::MAX_SIDE)];

    for n in 1..=5usize {
        // --- PaddingPolicy::None -------------------------------------------------
        let none_policy: PaddingPolicy<u8> = PaddingPolicy::None;
        let expected_none = count_grams(input.len(), n, &none_policy);
        let generator_none = CharacterGrams::new(n, none_policy);
        let actual_none = generator_none.grams(input).count();
        assert_eq!(
            expected_none, actual_none,
            "count_grams(len={}, n={}, None) = {} but iterator yielded {}",
            input.len(),
            n,
            expected_none,
            actual_none,
        );

        // Fast-path counter must also agree for the unpadded case.
        let slices = CharacterGramSlices::new(n);
        assert_eq!(
            slices.count(input.len()),
            expected_none,
            "CharacterGramSlices::count disagreed with count_grams for len={}, n={}",
            input.len(),
            n,
        );
        assert_eq!(
            slices.grams(input).count(),
            expected_none,
            "CharacterGramSlices iterator disagreed with count for len={}, n={}",
            input.len(),
            n,
        );

        // --- PaddingPolicy::Boundary --------------------------------------------
        let boundary_policy = PaddingPolicy::Boundary {
            start: 0u8,
            end: 255u8,
        };
        let expected_boundary = count_grams(input.len(), n, &boundary_policy);
        let generator_boundary = CharacterGrams::new(n, boundary_policy);
        let actual_boundary = generator_boundary.grams(input).count();
        assert_eq!(
            expected_boundary, actual_boundary,
            "count_grams(len={}, n={}, Boundary) = {} but iterator yielded {}",
            input.len(),
            n,
            expected_boundary,
            actual_boundary,
        );
    }
});
