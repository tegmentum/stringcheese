//! Verifies that depending on `stringcheese-ro` is enough to make the
//! Romanian pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-ro/src/lib.rs` is the
//! sole opt-in — if it stops firing (for instance, if a linker strips
//! the registration `static`), this test flips red.
//!
//! Wasm-gated at the file level: `stringcheese_lang::registry` is
//! compiled out on wasm targets (linkme's `#[distributed_slice]`
//! expansion has no wasm branch, per `stringcheese-lang/src/lib.rs`).
//! The pack itself still builds and links on wasm; only the registry
//! lookup is absent.
//!
//! The three canonical `#[test] fn`s and the pack-anchoring `KEEP`
//! const are emitted by the shared
//! `stringcheese_lang::pack_registry_smoke_test!` macro — see its docs
//! for the field-level contract. The BCP-47 region-fallback check
//! below is preserved as a pack-specific tack-on.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_ro::ROMANIAN,
    pack_ty: stringcheese_ro::Romanian,
    code: "ro",
    name: "Romanian",
    smoke: |lang| {
        assert!(lang.is_stopword("și"));
        assert_eq!(lang.stem("omul"), "om");
    },
}

#[test]
fn romanian_pack_falls_back_from_region_subtag() {
    // BCP-47 region fallback: ro-RO strips to ro.
    let lang = stringcheese_lang::registry::language("ro-RO").expect("ro-RO falls back to ro");
    assert_eq!(lang.code(), "ro");
    // Moldovan-Latin uses the ro-MD subtag under BCP-47 convention.
    let lang = stringcheese_lang::registry::language("ro-MD").expect("ro-MD falls back to ro");
    assert_eq!(lang.code(), "ro");
}
