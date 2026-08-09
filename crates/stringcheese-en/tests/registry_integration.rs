//! Verifies that depending on `stringcheese-en` is enough to make
//! the English pack visible through
//! [`stringcheese_lang::registry`]. The `register_language!` macro
//! in `stringcheese-en/src/lib.rs` is the sole opt-in — if it stops
//! firing (for instance, if a linker strips the registration
//! `static`), this test flips red.
#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_en` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker
// walks, hiding the fact that `register_language!` fired. Naming
// the pack's singleton constant here keeps its object file (and
// thus its `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_EN: &stringcheese_en::English = &stringcheese_en::ENGLISH;

#[test]
fn english_pack_is_registered() {
    let lang = registry::language("en").expect("English pack must be registered");
    assert_eq!(lang.code(), "en");
    assert_eq!(lang.name(), "English");
}

#[test]
fn english_pack_registration_is_case_insensitive() {
    for probe in ["EN", "En", "eN"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to English"
        );
    }
}

#[test]
fn english_pack_functions_through_registry() {
    let lang = registry::language("en").expect("English pack must be registered");
    // Delegate through the trait — proves the registered object is a
    // real Language, not just a shell.
    assert_eq!(lang.stem("caresses"), "caress");
    assert!(lang.is_stopword("the"));
}
