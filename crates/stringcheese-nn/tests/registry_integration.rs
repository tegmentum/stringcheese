//! Verifies that depending on `stringcheese-nn` is enough to make the
//! Nynorsk pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-nn/src/lib.rs` is the
//! sole opt-in — if it stops firing (for instance, if a linker strips
//! the registration `static`), this test flips red.
//!
//! Wasm-gated at the file level: `stringcheese_lang::registry` is
//! compiled out on wasm targets (linkme's `#[distributed_slice]`
//! expansion has no wasm branch, per `stringcheese-lang/src/lib.rs`).
//! The pack itself still builds and links on wasm; only the registry
//! lookup is absent.
#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_nn` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker walks,
// hiding the fact that `register_language!` fired. Naming the pack's
// singleton constant here keeps its object file (and thus its
// `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_NN: &stringcheese_nn::Nynorsk = &stringcheese_nn::NYNORSK;

#[test]
fn nynorsk_pack_is_registered() {
    let lang = registry::language("nn").expect("Nynorsk pack must be registered");
    assert_eq!(lang.code(), "nn");
    assert_eq!(lang.name(), "Norwegian Nynorsk");
}

#[test]
fn nynorsk_pack_registration_is_case_insensitive() {
    for probe in ["NN", "Nn", "nN"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Nynorsk"
        );
    }
}

#[test]
fn nynorsk_pack_functions_through_registry() {
    let lang = registry::language("nn").expect("Nynorsk pack must be registered");
    assert!(lang.is_stopword("og"));
    assert!(lang.is_stopword("ikkje"));
    assert_eq!(lang.stem("bilane"), "bil");
}

#[test]
fn macrolanguage_no_is_not_registered_by_this_pack() {
    // The pack registers `"nn"` (Nynorsk specifically), not the
    // macrolanguage `"no"`. The Bokmål sibling `stringcheese-no`
    // registers `"nb"`; neither pack takes over `"no"`.
    //
    // Note: this test only asserts that *this* pack does not register
    // `"no"`. If the Bokmål pack is linked into the same binary
    // (e.g. a downstream that depends on both), that pack likewise
    // registers only `"nb"`, so the invariant holds transitively —
    // but this test does not assert that (the Bokmål pack isn't a
    // dependency here).
    assert!(
        registry::language("no").is_none(),
        "the macrolanguage code \"no\" should not be registered by the Nynorsk pack"
    );
}
