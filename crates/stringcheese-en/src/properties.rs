//! Property tests for the English language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::porter::Porter;
use crate::porter2::Porter2;
use crate::{ENGLISH, STOPWORDS};

/// Strategy for ASCII lowercase words 1..=20 chars.
fn ascii_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,20}").expect("static regex is valid")
}

/// Strategy for a mixed-case ASCII word 1..=20 chars.
fn mixed_case_word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z]{1,20}").expect("static regex is valid")
}

proptest! {
    /// Porter is not universally idempotent on arbitrary strings
    /// (`agreed -> agre -> agr` is a well-known counter-example: step
    /// 5a strips the trailing `e` when its condition holds, and the
    /// resulting `agre` still satisfies that condition), but any input
    /// converges to a fixed point in at most a handful of iterations.
    /// We verify convergence in `<= 5` iterations.
    #[test]
    fn porter_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = Porter.stem(&w).into_owned();
        for _ in 0..5 {
            let next = Porter.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Porter did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// Porter output is lowercase ASCII (its input is coerced to
    /// lowercase and the algorithm never introduces non-ASCII).
    #[test]
    fn porter_output_is_lowercase_ascii(w in mixed_case_word()) {
        let out = Porter.stem(&w);
        prop_assert!(
            out.bytes().all(|b| b.is_ascii_lowercase()),
            "non-lowercase byte in Porter({:?}) = {:?}",
            w,
            out
        );
    }

    /// Porter stem is never longer than the input (all rules are
    /// suffix strip/replace with replacement no longer than the suffix
    /// dropped).
    #[test]
    fn porter_stem_is_no_longer_than_input(w in ascii_word()) {
        let out = Porter.stem(&w).into_owned();
        prop_assert!(
            out.len() <= w.len(),
            "Porter({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.len(),
            out.len()
        );
    }

    /// Porter2 also converges to a fixed point in at most a handful
    /// of iterations. Same rationale as the Porter property above:
    /// individual steps may fire more than once when re-applied to a
    /// stem, but the algorithm bottoms out quickly.
    #[test]
    fn porter2_converges_to_a_fixed_point(w in ascii_word()) {
        let mut cur = Porter2.stem(&w).into_owned();
        for _ in 0..5 {
            let next = Porter2.stem(&cur).into_owned();
            if next == cur {
                return Ok(());
            }
            cur = next;
        }
        prop_assert!(
            false,
            "Porter2 did not converge in 5 iterations starting from {:?}",
            w
        );
    }

    /// Porter2 output is lowercase ASCII. The prelude may temporarily
    /// uppercase `y` to `Y` for its consonant-treatment step, but the
    /// postlude lowercases everything back before returning.
    #[test]
    fn porter2_output_is_lowercase_ascii(w in mixed_case_word()) {
        let out = Porter2.stem(&w);
        prop_assert!(
            out.bytes().all(|b| b.is_ascii_lowercase()),
            "non-lowercase byte in Porter2({:?}) = {:?}",
            w,
            out
        );
    }

    /// Porter2 stem length is bounded by input length. Individual
    /// rules can grow the buffer transiently — Step 1a's `ies`→`ie`
    /// keeps length constant when preceded by 1 letter, and Step 1b's
    /// short-word rule appends an `e` — but the *net* effect of all
    /// steps together is a stem no longer than the input.
    #[test]
    fn porter2_stem_is_no_longer_than_input(w in ascii_word()) {
        let out = Porter2.stem(&w).into_owned();
        prop_assert!(
            out.len() <= w.len(),
            "Porter2({:?}) = {:?} grew ({}→{})",
            w,
            out,
            w.len(),
            out.len()
        );
    }

    /// Porter2 respects the R1/R2 region invariants: R1 <= R2, and
    /// both markers point at valid byte offsets in the (post-prelude)
    /// buffer. The prelude may only insert or transform bytes
    /// (leading-apostrophe strip, y->Y), so the length after prelude
    /// is bounded by the input length.
    #[test]
    fn porter2_region_markers_are_ordered(w in ascii_word()) {
        let mut bytes: alloc::vec::Vec<u8> = w.as_bytes().to_vec();
        crate::porter2::prelude(&mut bytes);
        let (p1, p2) = crate::porter2::mark_regions(&bytes);
        prop_assert!(p1 <= p2, "R1({}) > R2({}) for {:?}", p1, p2, w);
        prop_assert!(p2 <= bytes.len(), "R2({}) > len({}) for {:?}", p2, bytes.len(), w);
    }

    /// `is_stopword` is ASCII-case-invariant on the shipped stopword
    /// list.
    #[test]
    fn is_stopword_case_invariant(w in ascii_word()) {
        let hit_lower = ENGLISH.is_stopword(&w.to_ascii_lowercase());
        let hit_upper = ENGLISH.is_stopword(&w.to_ascii_uppercase());
        prop_assert_eq!(hit_lower, hit_upper);
    }

    /// Every stopword in the list is recognized (and any variant
    /// casing thereof).
    #[test]
    fn every_stopword_is_recognized(i in 0usize..STOPWORDS.len()) {
        let w = STOPWORDS[i];
        prop_assert!(ENGLISH.is_stopword(w));
        prop_assert!(ENGLISH.is_stopword(&w.to_ascii_uppercase()));
    }
}
