//! Verifies that depending on `stringcheese-pa` is enough to make
//! the Punjabi pack visible through
//! [`stringcheese_lang::registry`]. The `register_language!` macro
//! in `stringcheese-pa/src/lib.rs` is the sole opt-in — if it stops
//! firing (for instance, if a linker strips the registration
//! `static`), this test flips red.
//!
//! Gated off wasm — `stringcheese_lang::registry` is itself gated on
//! `not(target_family = "wasm")` because `linkme::distributed_slice`
//! has no wasm branch. The registration on this pack is likewise
//! wasm-gated; on wasm targets this whole test file compiles to
//! nothing.
#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_pa` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker
// walks, hiding the fact that `register_language!` fired. Naming
// the pack's singleton constant here keeps its object file (and
// thus its `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_PA: &stringcheese_pa::Punjabi = &stringcheese_pa::PUNJABI;

#[test]
fn punjabi_pack_is_registered() {
    let lang = registry::language("pa").expect("Punjabi pack must be registered");
    assert_eq!(lang.code(), "pa");
    assert_eq!(lang.name(), "Punjabi");
}

#[test]
fn punjabi_pack_registration_is_case_insensitive() {
    for probe in ["PA", "Pa", "pA"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Punjabi"
        );
    }
}

#[test]
fn punjabi_pack_functions_through_registry() {
    let lang = registry::language("pa").expect("Punjabi pack must be registered");
    assert!(lang.is_stopword("ਅਤੇ"));
    assert_eq!(lang.stem("ਘਰਾਂ"), "ਘਰ");
}
