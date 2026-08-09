//! Verifies that depending on `stringcheese-th` is enough to make
//! the Thai pack visible through [`stringcheese_lang::registry`].
//! The `register_language!` macro in `stringcheese-th/src/lib.rs` is
//! the sole opt-in — if it stops firing (for instance, if a linker
//! strips the registration `static`), this test flips red.
//!
//! Gated off wasm — `stringcheese_lang::registry` is itself gated on
//! `not(target_family = "wasm")` because `linkme::distributed_slice`
//! has no wasm branch. The registration on this pack is likewise
//! wasm-gated; on wasm targets this whole test file compiles to
//! nothing.
#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_th` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker walks,
// hiding the fact that `register_language!` fired. Naming the pack's
// singleton constant here keeps its object file (and thus its
// `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_TH: &stringcheese_th::Thai = &stringcheese_th::THAI;

#[test]
fn thai_pack_is_registered() {
    let lang = registry::language("th").expect("Thai pack must be registered");
    assert_eq!(lang.code(), "th");
    assert_eq!(lang.name(), "Thai");
}

#[test]
fn thai_pack_registration_is_case_insensitive() {
    for probe in ["TH", "Th", "tH"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Thai"
        );
    }
}

#[test]
fn thai_pack_functions_through_registry() {
    let lang = registry::language("th").expect("Thai pack must be registered");
    assert!(lang.is_stopword("และ"));
    // Nominalizer strip.
    assert_eq!(lang.stem("การเรียน"), "เรียน");
    // Syllable-cluster tokenization.
    let toks: Vec<&str> = lang.tokenize("ไทย").collect();
    assert_eq!(toks, ["ไท", "ย"]);
}
