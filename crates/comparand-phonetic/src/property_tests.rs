//! Property-based tests for the three phonetic encoders.
//!
//! Each encoder gets four property groups:
//!
//! * [determinism][soundex_determinism] — `encode(x)` returns the same key
//!   every time.
//! * [case insensitivity][soundex_case_insensitive] — `encode(x) ==
//!   encode(x.to_uppercase()) == encode(x.to_lowercase())`.
//! * [length bounds][soundex_length_bound] — the returned key length lies
//!   within the algorithm's declared spec bound.
//! * [no-panic on arbitrary input][soundex_no_panic] — the encoder handles
//!   any UTF-8 string without panicking or overflowing.

use proptest::prelude::*;

use crate::{DoubleMetaphone, Nysiis, Soundex};

/// An ASCII-letter strategy with mixed case and a small length cap.
///
/// The three-word alphabet is deliberately narrow: it produces a healthy
/// density of collapses, digraphs, and vowel/consonant boundaries in short
/// inputs, which is where the encoders' rules interact.
fn arb_ascii_word() -> impl Strategy<Value = std::string::String> {
    proptest::string::string_regex("[A-Za-z]{1,20}").unwrap()
}

/// A more permissive strategy including non-ASCII characters. Used only for
/// the no-panic properties; the encoders are not defined to produce
/// meaningful output for these inputs.
fn arb_arbitrary_short_string() -> impl Strategy<Value = std::string::String> {
    proptest::string::string_regex(".{0,30}").unwrap()
}

// ---------------------------------------------------------------------------
// Soundex
// ---------------------------------------------------------------------------

proptest! {
    /// Determinism: `encode(x)` returns the same key every call.
    #[test]
    fn soundex_determinism(w in arb_ascii_word()) {
        prop_assert_eq!(Soundex::encode(&w), Soundex::encode(&w));
    }

    /// Case insensitivity: Soundex ignores letter case.
    #[test]
    fn soundex_case_insensitive(w in arb_ascii_word()) {
        let upper = w.to_uppercase();
        let lower = w.to_lowercase();
        prop_assert_eq!(Soundex::encode(&w), Soundex::encode(&upper));
        prop_assert_eq!(Soundex::encode(&w), Soundex::encode(&lower));
    }

    /// Length bound: Soundex output is either exactly 4 characters (for any
    /// input containing an ASCII letter) or empty (for input without one).
    #[test]
    fn soundex_length_bound(w in arb_ascii_word()) {
        let out = Soundex::encode(&w);
        let has_letter = w.bytes().any(|b| b.is_ascii_alphabetic());
        if has_letter {
            prop_assert_eq!(out.len(), 4, "Soundex({:?}) = {:?}", w, out);
        } else {
            prop_assert!(out.is_empty());
        }
    }

    /// Structure: the first character of the key is the uppercase of the
    /// input's first ASCII letter, and the trailing three characters are
    /// each in `'0'..='6'`.
    #[test]
    fn soundex_structure(w in arb_ascii_word()) {
        let out = Soundex::encode(&w);
        if out.is_empty() {
            return Ok(());
        }
        let mut chars = out.chars();
        let first = chars.next().unwrap();
        prop_assert!(first.is_ascii_uppercase());
        for c in chars {
            prop_assert!(('0'..='6').contains(&c), "invalid digit: {c}");
        }
    }

    /// No-panic on arbitrary input: including non-ASCII, control characters,
    /// and long strings.
    #[test]
    fn soundex_no_panic(w in arb_arbitrary_short_string()) {
        // A pass without panic is the whole test.
        let _ = Soundex::encode(&w);
    }
}

// ---------------------------------------------------------------------------
// NYSIIS
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn nysiis_determinism(w in arb_ascii_word()) {
        prop_assert_eq!(Nysiis::encode(&w), Nysiis::encode(&w));
    }

    #[test]
    fn nysiis_case_insensitive(w in arb_ascii_word()) {
        let upper = w.to_uppercase();
        let lower = w.to_lowercase();
        prop_assert_eq!(Nysiis::encode(&w), Nysiis::encode(&upper));
        prop_assert_eq!(Nysiis::encode(&w), Nysiis::encode(&lower));
    }

    #[test]
    fn nysiis_length_bound(w in arb_ascii_word()) {
        let out = Nysiis::encode(&w);
        // Taft's classical output is at most 6 characters.
        prop_assert!(out.len() <= crate::nysiis::MAX_KEY_LEN,
            "NYSIIS({:?}) = {:?} exceeded max len", w, out);
    }

    #[test]
    fn nysiis_only_ascii_uppercase(w in arb_ascii_word()) {
        let out = Nysiis::encode(&w);
        for c in out.chars() {
            prop_assert!(c.is_ascii_uppercase(),
                "NYSIIS({:?}) = {:?} contains non-uppercase char {:?}",
                w, out, c);
        }
    }

    #[test]
    fn nysiis_no_panic(w in arb_arbitrary_short_string()) {
        let _ = Nysiis::encode(&w);
    }
}

// ---------------------------------------------------------------------------
// Double Metaphone
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn double_metaphone_determinism(w in arb_ascii_word()) {
        prop_assert_eq!(DoubleMetaphone::encode(&w), DoubleMetaphone::encode(&w));
    }

    #[test]
    fn double_metaphone_case_insensitive(w in arb_ascii_word()) {
        let upper = w.to_uppercase();
        let lower = w.to_lowercase();
        prop_assert_eq!(DoubleMetaphone::encode(&w), DoubleMetaphone::encode(&upper));
        prop_assert_eq!(DoubleMetaphone::encode(&w), DoubleMetaphone::encode(&lower));
    }

    #[test]
    fn double_metaphone_primary_length_bound(w in arb_ascii_word()) {
        let out = DoubleMetaphone::encode(&w);
        prop_assert!(out.primary.len() <= crate::double_metaphone::MAX_KEY_LEN,
            "DM({:?}).primary = {:?} exceeded max len", w, out.primary);
    }

    #[test]
    fn double_metaphone_alternate_always_none(w in arb_ascii_word()) {
        // Primary-only variant contract.
        let out = DoubleMetaphone::encode(&w);
        prop_assert!(out.alternate.is_none(),
            "DM({:?}).alternate should be None in primary-only variant, got {:?}",
            w, out.alternate);
    }

    #[test]
    fn double_metaphone_no_panic(w in arb_arbitrary_short_string()) {
        let _ = DoubleMetaphone::encode(&w);
    }
}

// ---------------------------------------------------------------------------
// Cross-encoder properties
// ---------------------------------------------------------------------------

proptest! {
    /// The matcher agrees with encoder equality on the same input pair.
    #[test]
    fn matcher_agrees_with_soundex_equality(a in arb_ascii_word(), b in arb_ascii_word()) {
        let m = crate::PhoneticMatcher::new(Soundex);
        let matched = m.matches(&a, &b);
        let ka = Soundex::encode(&a);
        let kb = Soundex::encode(&b);
        prop_assert_eq!(matched, ka == kb);
    }

    /// The matcher agrees with encoder equality for NYSIIS too.
    #[test]
    fn matcher_agrees_with_nysiis_equality(a in arb_ascii_word(), b in arb_ascii_word()) {
        let m = crate::PhoneticMatcher::new(Nysiis);
        let matched = m.matches(&a, &b);
        let ka = Nysiis::encode(&a);
        let kb = Nysiis::encode(&b);
        prop_assert_eq!(matched, ka == kb);
    }

    /// Double Metaphone matcher in `PrimaryOnly` mode reduces to primary
    /// equality.
    #[test]
    fn dm_matcher_primary_only_reduces_to_primary_equality(
        a in arb_ascii_word(),
        b in arb_ascii_word(),
    ) {
        let m = crate::PhoneticMatcher::new(DoubleMetaphone)
            .with_mode(crate::MatchMode::PrimaryOnly);
        let matched = m.matches_double_metaphone(&a, &b);
        let pa = DoubleMetaphone::encode(&a).primary;
        let pb = DoubleMetaphone::encode(&b).primary;
        prop_assert_eq!(matched, pa == pb);
    }
}
