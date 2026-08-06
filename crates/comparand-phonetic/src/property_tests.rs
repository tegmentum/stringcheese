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
        let po = DoubleMetaphone::primary_only();
        prop_assert_eq!(po.encode(&w), po.encode(&w));
        let full = DoubleMetaphone::full();
        prop_assert_eq!(full.encode(&w), full.encode(&w));
    }

    #[test]
    fn double_metaphone_case_insensitive(w in arb_ascii_word()) {
        let upper = w.to_uppercase();
        let lower = w.to_lowercase();
        let po = DoubleMetaphone::primary_only();
        prop_assert_eq!(po.encode(&w), po.encode(&upper));
        prop_assert_eq!(po.encode(&w), po.encode(&lower));
        let full = DoubleMetaphone::full();
        prop_assert_eq!(full.encode(&w), full.encode(&upper));
        prop_assert_eq!(full.encode(&w), full.encode(&lower));
    }

    #[test]
    fn double_metaphone_primary_length_bound(w in arb_ascii_word()) {
        let out = DoubleMetaphone::primary_only().encode(&w);
        prop_assert!(out.primary.len() <= crate::double_metaphone::MAX_KEY_LEN,
            "DM({:?}).primary = {:?} exceeded max len", w, out.primary);
    }

    #[test]
    fn double_metaphone_alternate_always_none_primary_only(w in arb_ascii_word()) {
        // Primary-only variant contract.
        let out = DoubleMetaphone::primary_only().encode(&w);
        prop_assert!(out.alternate.is_none(),
            "DM({:?}).alternate should be None in primary-only variant, got {:?}",
            w, out.alternate);
    }

    /// Backwards-compat guarantee: adding the alternate branch does NOT
    /// change the primary key.
    #[test]
    fn double_metaphone_full_primary_matches_primary_only(w in arb_ascii_word()) {
        let po = DoubleMetaphone::primary_only().encode(&w);
        let f = DoubleMetaphone::full().encode(&w);
        prop_assert_eq!(&po.primary, &f.primary,
            "primary key differs for {:?}: primary_only={:?} vs full={:?}",
            w, po.primary, f.primary);
    }

    /// The full variant's alternate is either `None` or a non-empty string —
    /// never `Some("")`.
    #[test]
    fn double_metaphone_full_alternate_is_none_or_nonempty(w in arb_ascii_word()) {
        let out = DoubleMetaphone::full().encode(&w);
        if let Some(alt) = &out.alternate {
            prop_assert!(!alt.is_empty(),
                "DM.full({:?}).alternate is Some(\"\"), should be None instead", w);
        }
    }

    /// The full variant's alternate, when present, respects the same
    /// four-character length cap as the primary.
    #[test]
    fn double_metaphone_full_alternate_length_bound(w in arb_ascii_word()) {
        let out = DoubleMetaphone::full().encode(&w);
        if let Some(alt) = &out.alternate {
            prop_assert!(alt.len() <= crate::double_metaphone::MAX_KEY_LEN,
                "DM.full({:?}).alternate = {:?} exceeded max len", w, alt);
        }
    }

    /// The full variant's alternate, when present, is never equal to the
    /// primary — we return `None` instead in that case.
    #[test]
    fn double_metaphone_full_alternate_differs_from_primary(w in arb_ascii_word()) {
        let out = DoubleMetaphone::full().encode(&w);
        if let Some(alt) = &out.alternate {
            prop_assert_ne!(alt, &out.primary,
                "DM.full({:?}) reported Some(alternate) equal to primary", w);
        }
    }

    #[test]
    fn double_metaphone_no_panic(w in arb_arbitrary_short_string()) {
        let _ = DoubleMetaphone::primary_only().encode(&w);
        let _ = DoubleMetaphone::full().encode(&w);
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
        let po = DoubleMetaphone::primary_only();
        let m = crate::PhoneticMatcher::new(po)
            .with_mode(crate::MatchMode::PrimaryOnly);
        let matched = m.matches_double_metaphone(&a, &b);
        let pa = po.encode(&a).primary;
        let pb = po.encode(&b).primary;
        prop_assert_eq!(matched, pa == pb);
    }

    /// Full-variant matcher in `AnyPair` mode is at least as permissive as
    /// primary-only matching. If two inputs share a primary key, they must
    /// match under Full-AnyPair too (their alternate branches cannot
    /// override the primary=primary agreement).
    #[test]
    fn dm_full_matcher_any_pair_supersets_primary_equality(
        a in arb_ascii_word(),
        b in arb_ascii_word(),
    ) {
        let full = DoubleMetaphone::full();
        let m = crate::PhoneticMatcher::new(full);
        let matched = m.matches_double_metaphone(&a, &b);
        let pa = full.encode(&a).primary;
        let pb = full.encode(&b).primary;
        if pa == pb {
            prop_assert!(matched,
                "Full-AnyPair rejected {:?} vs {:?} despite equal primaries \
                 ({:?})", a, b, pa);
        }
    }
}
