//! Property tests for the English language pack.
//!
//! Gated on `feature = "std"` and off wasm — same gating pattern as
//! every other property-test module in the workspace.

use core::cmp::Ordering;

use proptest::prelude::*;
use stringcheese_lang::{Collator, Language};

use crate::collator::EnglishCollator;
use crate::contraction::ContractionTokenizer;
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

/// Strategy for a mixed-case ASCII string that may contain spaces,
/// digits, and apostrophes — the kinds of things the collator has to
/// order.
fn collatable() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ']{0,30}").expect("static regex is valid")
}

/// Strategy for text a contraction tokenizer might see: letters,
/// spaces, apostrophes, and the odd digit / punctuation mark. Kept
/// short so the property runs at a reasonable pace.
fn contractionable_text() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 '.,!?]{0,60}").expect("static regex is valid")
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

    // ---- EnglishCollator contract ----------------------------------

    /// The dictionary-order collator is *total*: every call returns
    /// one of `Less`, `Equal`, or `Greater`. (`Ordering`'s enum shape
    /// makes this true by construction; the assertion witnesses that
    /// the collator returns a valid `Ordering` and never panics.)
    #[test]
    fn collator_is_total(a in collatable(), b in collatable()) {
        let c = EnglishCollator::DICTIONARY;
        let ord = c.compare(&a, &b);
        prop_assert!(matches!(ord, Ordering::Less | Ordering::Equal | Ordering::Greater));
    }

    /// The dictionary-order collator is *antisymmetric*:
    /// `compare(a, b) == compare(b, a).reverse()`.
    #[test]
    fn collator_is_antisymmetric(a in collatable(), b in collatable()) {
        let c = EnglishCollator::DICTIONARY;
        prop_assert_eq!(c.compare(&a, &b), c.compare(&b, &a).reverse());
    }

    /// The dictionary-order collator is *reflexive*:
    /// `compare(x, x) == Equal`.
    #[test]
    fn collator_is_reflexive(a in collatable()) {
        let c = EnglishCollator::DICTIONARY;
        prop_assert_eq!(c.compare(&a, &a), Ordering::Equal);
    }

    /// The dictionary-order collator is *transitive*: if `a <= b` and
    /// `b <= c` then `a <= c`. Sampled on triples of small strings.
    #[test]
    fn collator_is_transitive(
        a in collatable(),
        b in collatable(),
        c in collatable(),
    ) {
        let coll = EnglishCollator::DICTIONARY;
        let ab = coll.compare(&a, &b);
        let bc = coll.compare(&b, &c);
        let ac = coll.compare(&a, &c);
        // If a <= b and b <= c, then a <= c.
        if ab != Ordering::Greater && bc != Ordering::Greater {
            prop_assert_ne!(ac, Ordering::Greater);
        }
        // Dually: if a >= b and b >= c, then a >= c.
        if ab != Ordering::Less && bc != Ordering::Less {
            prop_assert_ne!(ac, Ordering::Less);
        }
    }

    /// The ASCII-preset collator agrees with raw `str::cmp` — a
    /// witness for the "all-rules-off is a no-op" claim in the module
    /// docs.
    #[test]
    fn collator_ascii_preset_matches_str_cmp(a in collatable(), b in collatable()) {
        let c = EnglishCollator::ASCII;
        prop_assert_eq!(c.compare(&a, &b), a.cmp(&b));
    }

    // ---- ContractionTokenizer contract -----------------------------

    /// The [`STANDARD`] contraction tokenizer is *idempotent* when
    /// joined with spaces: re-tokenizing the joined output yields the
    /// same tokens.
    #[test]
    fn contraction_standard_is_idempotent(text in contractionable_text()) {
        let cfg = ContractionTokenizer::STANDARD;
        let toks1: alloc::vec::Vec<alloc::string::String> = cfg.tokenize(&text).collect();
        let joined = toks1.join(" ");
        let toks2: alloc::vec::Vec<alloc::string::String> = cfg.tokenize(&joined).collect();
        prop_assert_eq!(toks1, toks2);
    }

    /// Empty input yields zero tokens for both presets and any
    /// mixture of normalization flags.
    #[test]
    fn contraction_empty_input_yields_no_tokens(
        nll in any::<bool>(),
        nve in any::<bool>(),
        nre in any::<bool>(),
        nd in any::<bool>(),
        nnt in any::<bool>(),
        sf in any::<bool>(),
    ) {
        let cfg = ContractionTokenizer::STANDARD
            .with_normalize_ll(nll)
            .with_normalize_ve(nve)
            .with_normalize_re(nre)
            .with_normalize_d(nd)
            .with_normalize_nt(nnt)
            .with_special_forms(sf);
        let toks: alloc::vec::Vec<alloc::string::String> = cfg.tokenize("").collect();
        prop_assert!(toks.is_empty());
    }

    /// The borrowed and owned tokenize APIs produce the same tokens.
    #[test]
    fn contraction_tokenize_apis_agree(text in contractionable_text()) {
        let cfg = ContractionTokenizer::STANDARD;
        let borrowed: alloc::vec::Vec<&str> = cfg.tokenize_borrowed(&text).collect();
        let owned: alloc::vec::Vec<alloc::string::String> = cfg.tokenize(&text).collect();
        prop_assert_eq!(borrowed.len(), owned.len());
        for (b, o) in borrowed.iter().zip(owned.iter()) {
            prop_assert_eq!(*b, o.as_str());
        }
    }

    /// NORMALIZED tokenization yields the same token *count* as
    /// STANDARD on the same input — the two modes differ only in what
    /// each fragment expands to, not in whether a fragment is emitted.
    #[test]
    fn contraction_standard_and_normalized_same_length(text in contractionable_text()) {
        let s: alloc::vec::Vec<alloc::string::String> =
            ContractionTokenizer::STANDARD.tokenize(&text).collect();
        let n: alloc::vec::Vec<alloc::string::String> =
            ContractionTokenizer::NORMALIZED.tokenize(&text).collect();
        prop_assert_eq!(s.len(), n.len());
    }
}
