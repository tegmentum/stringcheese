//! Verifies that depending on `stringcheese-no` is enough to make the
//! Norwegian pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-no/src/lib.rs` is the
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

// Force the `stringcheese_no` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker walks,
// hiding the fact that `register_language!` fired. Naming the pack's
// singleton constant here keeps its object file (and thus its
// `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_NO: &stringcheese_no::Norwegian = &stringcheese_no::NORWEGIAN;

#[test]
fn norwegian_pack_is_registered() {
    let lang = registry::language("nb").expect("Norwegian pack must be registered");
    assert_eq!(lang.code(), "nb");
    assert_eq!(lang.name(), "Norwegian Bokmål");
}

#[test]
fn norwegian_pack_registration_is_case_insensitive() {
    for probe in ["NB", "Nb", "nB"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Norwegian"
        );
    }
}

#[test]
fn norwegian_pack_functions_through_registry() {
    let lang = registry::language("nb").expect("Norwegian pack must be registered");
    assert!(lang.is_stopword("og"));
    assert_eq!(lang.stem("bilene"), "bil");
}

#[test]
fn macrolanguage_no_is_not_registered_by_this_pack() {
    // The pack registers `"nb"` (Bokmål specifically), not the
    // macrolanguage `"no"`. The future `stringcheese-nn` sibling will
    // register `"nn"` in its turn; neither pack takes over `"no"`.
    // Looking up `"no"` should therefore return None (until a caller
    // wires up a macrolanguage-fallback layer of their own).
    assert!(
        registry::language("no").is_none(),
        "the macrolanguage code \"no\" should not be registered by the Bokmål pack"
    );
}
