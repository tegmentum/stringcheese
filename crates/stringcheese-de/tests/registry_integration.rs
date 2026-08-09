//! Verifies that depending on `stringcheese-de` is enough to make
//! the German pack visible through
//! [`stringcheese_lang::registry`]. The `register_language!` macro
//! in `stringcheese-de/src/lib.rs` is the sole opt-in — if it stops
//! firing (for instance, if a linker strips the registration
//! `static`), this test flips red.
//!
//! Gated off `target_family = "wasm"` — the `stringcheese_lang::registry`
//! module is itself only compiled on non-wasm targets (linkme's
//! `distributed_slice` has no wasm branch), so a wasm test build
//! would fail to resolve the import.
#![cfg(not(target_family = "wasm"))]

#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_de` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker
// walks, hiding the fact that `register_language!` fired. Naming
// the pack's singleton constant here keeps its object file (and
// thus its `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_DE: &stringcheese_de::German = &stringcheese_de::GERMAN;

#[test]
fn german_pack_is_registered() {
    let lang = registry::language("de").expect("German pack must be registered");
    assert_eq!(lang.code(), "de");
    assert_eq!(lang.name(), "German");
}

#[test]
fn german_pack_registration_is_case_insensitive() {
    for probe in ["DE", "De", "dE"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to German"
        );
    }
}

#[test]
fn german_pack_functions_through_registry() {
    let lang = registry::language("de").expect("German pack must be registered");
    assert!(lang.is_stopword("und"));
    // Snowball German stems `Häuser` -> `haus`; see
    // stringcheese-de's own reference tests for the full suite.
    assert_eq!(lang.stem("Häuser"), "haus");
}
