//! Property tests for the English language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use proptest::prelude::*;
use stringcheese_lang::Language;

use crate::porter::Porter;
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
