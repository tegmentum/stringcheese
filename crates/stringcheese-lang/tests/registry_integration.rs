// Gated off wasm because `stringcheese_lang::registry` itself is —
// linkme's `#[distributed_slice]` expansion has no wasm branch, so
// the registry module isn't compiled on wasm targets. Downstream
// callers on wasm can still depend on the pack crates for their
// stemming / stopword / tokenizer surfaces; only the registry is
// dark. On wasm this file becomes an empty crate; suppress the
// `missing_docs` lint that fires on the resulting doc-less
// integration-test binary.
#![cfg_attr(target_family = "wasm", allow(missing_docs))]
#![cfg(not(target_family = "wasm"))]
#![cfg(not(target_family = "wasm"))]

//! Multi-language integration test for the [`registry`] module.
//!
//! Depends on all three shipped language packs as dev-dependencies
//! and proves that:
//!
//! 1. Every pack's `register_language!` invocation actually fired —
//!    i.e., linkme collected the per-pack registration `static`s
//!    into `LANGUAGES` at link time.
//! 2. The registry iterates over at least those three packs and
//!    yields each pack's `code()` correctly.
//! 3. The registry's ASCII-case-insensitive lookup finds every pack
//!    under any casing of its code.
//! 4. Objects returned through the registry behave like `Language`
//!    implementations — `stem`, `is_stopword`, and `code` all work.
//!
//! Running this test with only one of the three packs as a dep would
//! still pass its own sub-assertion; the point of the full-three test
//! is to catch bugs where linkme's coalescing of registration
//! sections drops entries under a specific linker or platform.
//!
//! [`registry`]: stringcheese_lang::registry

use stringcheese_lang::Language;
use stringcheese_lang::registry;

/// The BCP-47 codes we expect the three shipped packs to advertise.
const EXPECTED_CODES: &[&str] = &["en", "de", "fr"];

// Force each pack's registration `static` into the final link — a
// mere `use stringcheese_en;` isn't enough (the crate's rlib is
// pulled in but dead-code-elimination drops the registration static
// since nothing else references it). Naming a real item per pack
// keeps the crate's object file alive, which drags its
// `#[linkme::distributed_slice(...)]` section along.
//
// The touched items are intentionally trivial (the singleton pack
// constants) — they are not otherwise used in this test file.
#[allow(dead_code)]
const KEEP_EN: &stringcheese_en::English = &stringcheese_en::ENGLISH;
#[allow(dead_code)]
const KEEP_DE: &stringcheese_de::German = &stringcheese_de::GERMAN;
#[allow(dead_code)]
const KEEP_FR: &stringcheese_fr::French = &stringcheese_fr::FRENCH;

#[test]
fn all_three_shipped_packs_are_registered() {
    let codes: std::collections::BTreeSet<&str> =
        registry::languages().map(Language::code).collect();
    for expected in EXPECTED_CODES {
        assert!(
            codes.contains(expected),
            "{expected:?} is not registered; saw {codes:?}"
        );
    }
}

#[test]
fn registry_yields_at_least_three_languages() {
    let count = registry::languages().count();
    assert!(
        count >= EXPECTED_CODES.len(),
        "expected >= {} registered languages, saw {count}",
        EXPECTED_CODES.len()
    );
}

#[test]
fn each_expected_code_resolves_via_lookup() {
    for code in EXPECTED_CODES {
        let hit = registry::language(code).unwrap_or_else(|| panic!("{code:?} did not resolve"));
        assert_eq!(hit.code(), *code);
    }
}

#[test]
fn lookup_is_case_insensitive_for_every_shipped_pack() {
    for code in EXPECTED_CODES {
        for probe in [code.to_ascii_uppercase(), code.to_ascii_lowercase()] {
            assert!(
                registry::language(&probe).is_some(),
                "case-flipped {probe:?} (for {code:?}) did not resolve"
            );
        }
    }
}

#[test]
fn resolved_packs_expose_working_trait_methods() {
    // English: Porter stems "caresses" -> "caress"; "the" is a stopword.
    let en = registry::language("en").expect("en registered");
    assert_eq!(en.stem("caresses"), "caress");
    assert!(en.is_stopword("the"));

    // German: Snowball stems "Häuser" -> "haus"; "und" is a stopword.
    let de = registry::language("de").expect("de registered");
    assert_eq!(de.stem("Häuser"), "haus");
    assert!(de.is_stopword("und"));

    // French: Snowball stems "continuer" -> "continu"; "le" is a stopword.
    let fr = registry::language("fr").expect("fr registered");
    assert_eq!(fr.stem("continuer"), "continu");
    assert!(fr.is_stopword("le"));
}

#[test]
fn languages_iter_and_distributed_slice_agree() {
    let via_iter: Vec<&str> = registry::languages().map(Language::code).collect();
    let via_slice: Vec<&str> = registry::LANGUAGES.iter().map(|l| l.code()).collect();
    assert_eq!(via_iter, via_slice);
}
