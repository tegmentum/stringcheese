//! Verifies that depending on `stringcheese-fr` is enough to make
//! the French pack visible through
//! [`stringcheese_lang::registry`]. The `register_language!` macro
//! in `stringcheese-fr/src/lib.rs` is the sole opt-in — if it stops
//! firing (for instance, if a linker strips the registration
//! `static`), this test flips red.

use stringcheese_lang::registry;

// Force the `stringcheese_fr` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker
// walks, hiding the fact that `register_language!` fired. Naming
// the pack's singleton constant here keeps its object file (and
// thus its `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_FR: &stringcheese_fr::French = &stringcheese_fr::FRENCH;

#[test]
fn french_pack_is_registered() {
    let lang = registry::language("fr").expect("French pack must be registered");
    assert_eq!(lang.code(), "fr");
    assert_eq!(lang.name(), "French");
}

#[test]
fn french_pack_registration_is_case_insensitive() {
    for probe in ["FR", "Fr", "fR"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to French"
        );
    }
}

#[test]
fn french_pack_functions_through_registry() {
    let lang = registry::language("fr").expect("French pack must be registered");
    assert!(lang.is_stopword("le"));
    assert_eq!(lang.stem("continuer"), "continu");
}
