//! Verifies that depending on `stringcheese-ro` is enough to make
//! the Romanian pack visible through
//! [`stringcheese_lang::registry`]. The `register_language!` macro
//! in `stringcheese-ro/src/lib.rs` is the sole opt-in — if it stops
//! firing (for instance, if a linker strips the registration
//! `static`), this test flips red.
//!
//! Wasm-gated at the file level: `stringcheese_lang::registry` is
//! compiled out on wasm targets (linkme's `#[distributed_slice]`
//! expansion has no wasm branch, per `stringcheese-lang/src/lib.rs`).
//! The pack itself still builds and links on wasm; only the
//! registry lookup is absent.
#![cfg(not(target_family = "wasm"))]

#![cfg(not(target_family = "wasm"))]

use stringcheese_lang::registry;

// Force the `stringcheese_ro` rlib into the test binary's link — a
// test that only names `stringcheese_lang` items would leave the
// pack's registration `static` outside the closure the linker
// walks, hiding the fact that `register_language!` fired. Naming
// the pack's singleton constant here keeps its object file (and
// thus its `#[linkme::distributed_slice(...)]` section) alive.
#[allow(dead_code)]
const KEEP_RO: &stringcheese_ro::Romanian = &stringcheese_ro::ROMANIAN;

#[test]
fn romanian_pack_is_registered() {
    let lang = registry::language("ro").expect("Romanian pack must be registered");
    assert_eq!(lang.code(), "ro");
    assert_eq!(lang.name(), "Romanian");
}

#[test]
fn romanian_pack_registration_is_case_insensitive() {
    for probe in ["RO", "Ro", "rO"] {
        assert!(
            registry::language(probe).is_some(),
            "{probe:?} did not resolve to Romanian"
        );
    }
}

#[test]
fn romanian_pack_falls_back_from_region_subtag() {
    // BCP-47 region fallback: ro-RO strips to ro.
    let lang = registry::language("ro-RO").expect("ro-RO falls back to ro");
    assert_eq!(lang.code(), "ro");
    // Moldovan-Latin uses the ro-MD subtag under BCP-47 convention.
    let lang = registry::language("ro-MD").expect("ro-MD falls back to ro");
    assert_eq!(lang.code(), "ro");
}

#[test]
fn romanian_pack_functions_through_registry() {
    let lang = registry::language("ro").expect("Romanian pack must be registered");
    assert!(lang.is_stopword("și"));
    assert_eq!(lang.stem("omul"), "om");
}
