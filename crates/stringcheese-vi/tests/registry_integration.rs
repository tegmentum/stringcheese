//! Verifies that depending on `stringcheese-vi` is enough to make the
//! Vietnamese pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-vi/src/lib.rs` is the
//! sole opt-in — if it stops firing (for instance, if a linker strips
//! the registration `static`), this test flips red.
//!
//! Wasm-gated at the file level: `stringcheese_lang::registry` is
//! compiled out on wasm targets (linkme's `#[distributed_slice]`
//! expansion has no wasm branch, per `stringcheese-lang/src/lib.rs`).
//! The pack itself still builds and links on wasm; only the registry
//! lookup is absent.
//!
//! The three `#[test] fn`s and the pack-anchoring `KEEP` const are
//! emitted by the shared `stringcheese_lang::pack_registry_smoke_test!`
//! macro — see its docs for the field-level contract.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_vi::VIETNAMESE,
    pack_ty: stringcheese_vi::Vietnamese,
    code: "vi",
    name: "Vietnamese",
    smoke: |lang| {
        assert!(lang.is_stopword("và"));
        // The identity-style stemmer returns the NFC canonical form.
        assert_eq!(lang.stem("được"), "được");
    },
}
