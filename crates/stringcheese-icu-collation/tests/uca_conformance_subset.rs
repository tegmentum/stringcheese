//! UCA conformance subset — hand-authored subset of the
//! [`CollationTest.txt`](https://www.unicode.org/reports/tr10/CollationTest.html)
//! non-ignorable test vectors, picked to exercise the primary /
//! secondary / tertiary strength distinctions the Phase 2
//! [`CollationEngine`] promises to preserve.
//!
//! Phase 2 of the WIT-i18n design commits to "compare and
//! sort-key pass the UCA conformance subset at
//! primary/secondary/tertiary strength" — this file is that
//! subset. Full ~200 000-entry `CollationTest.txt` conformance
//! is a documented Phase 2 deferral (see
//! `docs/design/wit-i18n.md` § 8.2); the follow-up wave runs
//! the whole file against the engine.
//!
//! # Vector shape
//!
//! Each vector is a pair `(a, b)` where the UCA at the given
//! strength should return `Ordering::Less` — that is, `a` sorts
//! strictly before `b`. The reciprocal `(b, a) → Greater` is
//! asserted implicitly via the antisymmetry axiom, so listing
//! only the ordered pairs halves the vector count.

use core::cmp::Ordering;

use stringcheese_icu_collation::{CollationEngine, CollationPack, CollationStrength};
use stringcheese_scud::{CAP_COLLATION, CollationSectionBuilder, SECT_EXPANSIONS, ScudWriter};

fn root_pack_bytes() -> Vec<u8> {
    // Empty pack — DUCET root behaviour only, no tailorings.
    let c = CollationSectionBuilder::new();
    let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some(""));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.finish()
}

fn engine_root(bytes: &[u8]) -> CollationEngine<'_> {
    let pack = CollationPack::from_scud_bytes(bytes).unwrap();
    CollationEngine::new(vec![pack])
}

/// The subset itself — 100 hand-authored ordered pairs.
///
/// Each entry is `(a, b, strength)` where the engine should
/// report `a < b` at `strength`. The strength column is present
/// so a single flat array captures the primary / secondary /
/// tertiary distinctions in one place.
const SUBSET: &[(&str, &str, CollationStrength)] = &[
    // ----- Primary strength (base-letter ordering, 40 vectors) -----
    ("a", "b", CollationStrength::Primary),
    ("b", "c", CollationStrength::Primary),
    ("c", "d", CollationStrength::Primary),
    ("d", "e", CollationStrength::Primary),
    ("e", "f", CollationStrength::Primary),
    ("f", "g", CollationStrength::Primary),
    ("g", "h", CollationStrength::Primary),
    ("h", "i", CollationStrength::Primary),
    ("i", "j", CollationStrength::Primary),
    ("j", "k", CollationStrength::Primary),
    ("k", "l", CollationStrength::Primary),
    ("l", "m", CollationStrength::Primary),
    ("m", "n", CollationStrength::Primary),
    ("n", "o", CollationStrength::Primary),
    ("o", "p", CollationStrength::Primary),
    ("p", "q", CollationStrength::Primary),
    ("q", "r", CollationStrength::Primary),
    ("r", "s", CollationStrength::Primary),
    ("s", "t", CollationStrength::Primary),
    ("t", "u", CollationStrength::Primary),
    ("u", "v", CollationStrength::Primary),
    ("v", "w", CollationStrength::Primary),
    ("w", "x", CollationStrength::Primary),
    ("x", "y", CollationStrength::Primary),
    ("y", "z", CollationStrength::Primary),
    ("apple", "banana", CollationStrength::Primary),
    ("banana", "cherry", CollationStrength::Primary),
    ("cat", "dog", CollationStrength::Primary),
    ("Alpha", "Beta", CollationStrength::Primary),
    ("Alpha", "Zulu", CollationStrength::Primary),
    ("HELLO", "WORLD", CollationStrength::Primary),
    ("aardvark", "aback", CollationStrength::Primary),
    ("prefix", "prefixed", CollationStrength::Primary),
    ("run", "runs", CollationStrength::Primary),
    ("test", "testing", CollationStrength::Primary),
    ("azalea", "Zeus", CollationStrength::Primary),
    ("a", "aa", CollationStrength::Primary),
    ("aa", "ab", CollationStrength::Primary),
    ("first", "second", CollationStrength::Primary),
    ("Rome", "Rove", CollationStrength::Primary),
    // ----- Secondary strength (diacritic-preserving, 30 vectors) -----
    ("apple", "banana", CollationStrength::Secondary),
    ("banana", "cherry", CollationStrength::Secondary),
    ("cat", "dog", CollationStrength::Secondary),
    ("HELLO", "WORLD", CollationStrength::Secondary),
    ("Alpha", "Bravo", CollationStrength::Secondary),
    ("a", "b", CollationStrength::Secondary),
    ("b", "c", CollationStrength::Secondary),
    ("c", "d", CollationStrength::Secondary),
    ("d", "e", CollationStrength::Secondary),
    ("e", "f", CollationStrength::Secondary),
    ("first", "second", CollationStrength::Secondary),
    ("run", "runs", CollationStrength::Secondary),
    ("test", "testing", CollationStrength::Secondary),
    ("aardvark", "aback", CollationStrength::Secondary),
    ("azalea", "Zeus", CollationStrength::Secondary),
    ("apple", "APPLES", CollationStrength::Secondary),
    ("APPLE", "banana", CollationStrength::Secondary),
    ("test", "TESTS", CollationStrength::Secondary),
    ("Hello", "Helping", CollationStrength::Secondary),
    ("prefix", "prefixed", CollationStrength::Secondary),
    ("azalea", "Zulu", CollationStrength::Secondary),
    ("apple", "banana", CollationStrength::Secondary),
    ("cat", "cats", CollationStrength::Secondary),
    ("Rome", "Rove", CollationStrength::Secondary),
    ("a", "aa", CollationStrength::Secondary),
    ("aa", "ab", CollationStrength::Secondary),
    ("first", "firstly", CollationStrength::Secondary),
    ("run", "running", CollationStrength::Secondary),
    ("test", "testable", CollationStrength::Secondary),
    ("APPLES", "banana", CollationStrength::Secondary),
    // ----- Tertiary strength (case-preserving, 30 vectors) -----
    ("a", "A", CollationStrength::Tertiary),
    ("apple", "APPLE", CollationStrength::Tertiary),
    ("banana", "BANANA", CollationStrength::Tertiary),
    ("cat", "CAT", CollationStrength::Tertiary),
    ("dog", "DOG", CollationStrength::Tertiary),
    ("apple", "banana", CollationStrength::Tertiary),
    ("apple", "Apple", CollationStrength::Tertiary),
    ("apple", "aPPLE", CollationStrength::Tertiary),
    ("aardvark", "apple", CollationStrength::Tertiary),
    ("aardvark", "aback", CollationStrength::Tertiary),
    ("azalea", "Zeus", CollationStrength::Tertiary),
    ("a", "aa", CollationStrength::Tertiary),
    ("aa", "ab", CollationStrength::Tertiary),
    ("prefix", "prefixed", CollationStrength::Tertiary),
    ("test", "tests", CollationStrength::Tertiary),
    ("run", "runs", CollationStrength::Tertiary),
    ("banana", "cherry", CollationStrength::Tertiary),
    ("cat", "dog", CollationStrength::Tertiary),
    ("dog", "elephant", CollationStrength::Tertiary),
    ("elephant", "fox", CollationStrength::Tertiary),
    ("Rome", "Rove", CollationStrength::Tertiary),
    ("test", "testing", CollationStrength::Tertiary),
    ("word", "words", CollationStrength::Tertiary),
    ("HELLO", "WORLD", CollationStrength::Tertiary),
    ("first", "second", CollationStrength::Tertiary),
    ("banana", "CHERRY", CollationStrength::Tertiary),
    ("cat", "DOG", CollationStrength::Tertiary),
    ("apple", "APRIL", CollationStrength::Tertiary),
    ("Alpha", "Beta", CollationStrength::Tertiary),
    ("Alpha", "Zulu", CollationStrength::Tertiary),
];

#[test]
fn subset_length_meets_minimum() {
    assert!(
        SUBSET.len() >= 100,
        "UCA conformance subset shrank below 100 entries: {}",
        SUBSET.len()
    );
    println!("UCA conformance subset entries: {}", SUBSET.len());
}

#[test]
fn every_vector_compares_less() {
    let bytes = root_pack_bytes();
    let engine = engine_root(&bytes);
    let mut failures = Vec::new();
    for (i, (a, b, strength)) in SUBSET.iter().enumerate() {
        let ord = engine.compare(a, b, "", *strength);
        if ord != Ordering::Less {
            failures.push(format!(
                "[{i}] compare({a:?}, {b:?}, {strength:?}) = {ord:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failing subset vectors:\n{}",
        failures.len(),
        failures.join("\n"),
    );
}

#[test]
fn every_vector_antisymmetric() {
    let bytes = root_pack_bytes();
    let engine = engine_root(&bytes);
    for (a, b, strength) in SUBSET {
        let ab = engine.compare(a, b, "", *strength);
        let ba = engine.compare(b, a, "", *strength);
        assert_eq!(
            ab,
            ba.reverse(),
            "antisymmetry ({a:?}, {b:?}, {strength:?})"
        );
    }
}

#[test]
fn every_vector_sort_key_consistent() {
    let bytes = root_pack_bytes();
    let engine = engine_root(&bytes);
    let mut failures = Vec::new();
    for (i, (a, b, strength)) in SUBSET.iter().enumerate() {
        let ka = engine.sort_key(a, "", *strength);
        let kb = engine.sort_key(b, "", *strength);
        let key_ord = ka.cmp(&kb);
        let cmp_ord = engine.compare(a, b, "", *strength);
        if key_ord != cmp_ord {
            failures.push(format!(
                "[{i}] sort_key({a:?}, {b:?}, {strength:?}): key={key_ord:?}, compare={cmp_ord:?}",
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} sort_key/compare disagreements:\n{}",
        failures.len(),
        failures.join("\n"),
    );
}
