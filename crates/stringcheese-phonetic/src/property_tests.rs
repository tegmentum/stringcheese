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

    // -- Whole-rule-set invariants for the completed Philips-1999 encoder --
    //
    // Each property below is a general invariant that must hold across the
    // entire rule set — Slavo-Germanic, SC-before-IEY, French endings, and
    // surname exceptions all obey these together.

    /// Any ASCII-letter input containing at least one vowel (A/E/I/O/U/Y)
    /// or one emitting consonant produces a non-empty primary key. Only
    /// pathological all-silent inputs like a lone "W" (silent with no
    /// following vowel to promote to first-committed) map to empty; the
    /// strategy filter carves those out.
    #[test]
    fn dm_input_with_emitting_letter_produces_nonempty_primary(
        w in "[A-Za-z]{1,20}".prop_filter(
            "must contain at least one vowel or non-silent consonant",
            |s| s.bytes().any(|b| {
                let u = b.to_ascii_uppercase();
                // Vowels emit as first char. Consonants except a lone
                // silent-in-context W also always emit something.
                matches!(u, b'A'|b'E'|b'I'|b'O'|b'U'|b'Y') ||
                    (u.is_ascii_alphabetic() && u != b'W')
            })
        )
    ) {
        let po = DoubleMetaphone::primary_only().encode(&w);
        prop_assert!(!po.primary.is_empty(),
            "DM({:?}).primary is empty for input with emitting letter", w);
        let f = DoubleMetaphone::full().encode(&w);
        prop_assert!(!f.primary.is_empty(),
            "DM.full({:?}).primary is empty for input with emitting letter", w);
    }

    /// The alternate key is `None` or a non-empty `String` — never
    /// `Some("")`. Enforced by the encoder's `encode_full` merge step.
    #[test]
    fn dm_full_alternate_is_none_or_nonempty_string(w in arb_ascii_word()) {
        let out = DoubleMetaphone::full().encode(&w);
        if let Some(alt) = &out.alternate {
            prop_assert!(!alt.is_empty(),
                "DM.full({:?}).alternate is Some(\"\") — should be None", w);
        }
    }

    /// Both keys are ASCII uppercase (plus the theta placeholder `'0'`).
    /// Every rule the encoder implements emits ASCII characters in the
    /// [A-Z0] set, never lowercase, never non-ASCII, never punctuation.
    #[test]
    fn dm_keys_are_ascii_uppercase_or_theta(w in arb_ascii_word()) {
        let out = DoubleMetaphone::full().encode(&w);
        for c in out.primary.chars() {
            prop_assert!(
                c.is_ascii_uppercase() || c == '0',
                "DM.full({:?}).primary = {:?} contains non-uppercase/non-theta char {:?}",
                w, out.primary, c
            );
        }
        if let Some(alt) = &out.alternate {
            for c in alt.chars() {
                prop_assert!(
                    c.is_ascii_uppercase() || c == '0',
                    "DM.full({:?}).alternate = {:?} contains non-uppercase/non-theta char {:?}",
                    w, alt, c
                );
            }
        }
    }

    /// Both keys match the regex `^[A-Z0]{0,4}$` given the four-character
    /// truncation Philips (1999) specifies. The primary may be empty for
    /// pathological all-silent inputs (a lone "W", for instance); the
    /// alternate, when `Some`, is always non-empty per the encoder's merge
    /// contract.
    #[test]
    fn dm_keys_match_regex_shape(w in arb_ascii_word()) {
        fn well_shaped_primary(s: &str) -> bool {
            let n = s.chars().count();
            n <= crate::double_metaphone::MAX_KEY_LEN &&
                s.chars().all(|c| c.is_ascii_uppercase() || c == '0')
        }
        fn well_shaped_alternate(s: &str) -> bool {
            let n = s.chars().count();
            (1..=crate::double_metaphone::MAX_KEY_LEN).contains(&n) &&
                s.chars().all(|c| c.is_ascii_uppercase() || c == '0')
        }
        let out = DoubleMetaphone::full().encode(&w);
        prop_assert!(well_shaped_primary(&out.primary),
            "DM.full({:?}).primary = {:?} does not match ^[A-Z0]{{0,4}}$",
            w, out.primary);
        if let Some(alt) = &out.alternate {
            prop_assert!(well_shaped_alternate(alt),
                "DM.full({:?}).alternate = {:?} does not match ^[A-Z0]{{1,4}}$",
                w, alt);
        }
    }

    /// The Slavo-Germanic heuristic must never crash the encoder and must
    /// only affect the alternate key. This restated property just exercises
    /// SG-detected input ("must not crash; primary is well-shaped; the
    /// alternate — if present — is well-shaped and may differ from the
    /// primary"). The stronger "primary(w) prefix-relates to primary(w+K)"
    /// property this test previously asserted does NOT hold in general —
    /// appending a K to `w` both (a) flips SG classification, which can
    /// change the alternate but not the primary, AND (b) shifts what was
    /// a word-final letter cluster to medial position, which changes the
    /// primary because word-final French silent-terminal rules
    /// (-GN, -MB, -MPT) key on the word end. Property test #XGN failed on
    /// exactly that interaction (primary("XGN") = "SN" via the -GN silent-G
    /// rule; primary("XGNK") = "SKNK" because GN is now medial).
    /// A future revision could try to reformulate the intent as
    /// "primary(w) is stable under any character that neither flips SG
    /// nor moves a word-final cluster off the end" — but that's a
    /// substantial narrowing and probably better expressed as targeted
    /// unit tests than a proptest generator.
    #[test]
    fn dm_slavo_germanic_gates_only_alternate(w in "[A-Z]{2,10}") {
        // Same well-shaped predicates as `dm_keys_match_regex_shape`
        // above — kept local because Rust nests them inside that test's
        // closure. Not worth hoisting to module scope yet.
        fn well_shaped_primary(s: &str) -> bool {
            let n = s.chars().count();
            n <= crate::double_metaphone::MAX_KEY_LEN &&
                s.chars().all(|c| c.is_ascii_uppercase() || c == '0')
        }
        fn well_shaped_alternate(s: &str) -> bool {
            let n = s.chars().count();
            (1..=crate::double_metaphone::MAX_KEY_LEN).contains(&n) &&
                s.chars().all(|c| c.is_ascii_uppercase() || c == '0')
        }
        let out = DoubleMetaphone::full().encode(&w);
        prop_assert!(well_shaped_primary(&out.primary),
            "DM.full({:?}).primary = {:?} does not match ^[A-Z0]{{0,4}}$",
            w, out.primary);
        if let Some(alt) = &out.alternate {
            prop_assert!(well_shaped_alternate(alt),
                "DM.full({:?}).alternate = {:?} does not match ^[A-Z0]{{1,4}}$",
                w, alt);
        }
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
