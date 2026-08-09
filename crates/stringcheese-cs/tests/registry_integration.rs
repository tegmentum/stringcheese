//! Verifies that depending on `stringcheese-cs` is enough to make the
//! Czech pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-cs/src/lib.rs` is the
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

// Force the `stringcheese_cs` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker walks,
// hiding the fact that `register_language!` fired. Naming the pack's
// singleton constant here keeps its object file (and thus its
// `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_CS: &stringcheese_cs::Czech = &stringcheese_cs::CZECH;

#[test]
fn czech_pack_is_registered() {
    let lang = registry::language("cs").expect("Czech pack must be registered");
    assert_eq!(lang.code(), "cs");
    assert_eq!(lang.name(), "Czech");
}

#[test]
fn czech_pack_registration_is_case_insensitive() {
    for probe in ["CS", "Cs", "cS"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Czech"
        );
    }
}

#[test]
fn czech_pack_functions_through_registry() {
    let lang = registry::language("cs").expect("Czech pack must be registered");
    assert!(lang.is_stopword("a"));
    assert_eq!(lang.stem("krásný"), "krásn");
}
