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
//!
//! The three canonical `#[test] fn`s and the pack-anchoring `KEEP`
//! const are emitted by the shared
//! `stringcheese_lang::pack_registry_smoke_test!` macro — see its docs
//! for the field-level contract. The macrolanguage-absence check below
//! is preserved as a pack-specific tack-on.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_no::NORWEGIAN,
    pack_ty: stringcheese_no::Norwegian,
    code: "nb",
    name: "Norwegian Bokmål",
    smoke: |lang| {
        assert!(lang.is_stopword("og"));
        assert_eq!(lang.stem("bilene"), "bil");
    },
}

#[test]
fn macrolanguage_no_is_not_registered_by_this_pack() {
    // The pack registers `"nb"` (Bokmål specifically), not the
    // macrolanguage `"no"`. The sibling `stringcheese-nn` registers
    // `"nn"`; neither pack takes over `"no"`. Looking up `"no"` should
    // therefore return None (until a caller wires up a
    // macrolanguage-fallback layer of their own).
    assert!(
        stringcheese_lang::registry::language("no").is_none(),
        "the macrolanguage code \"no\" should not be registered by the Bokmål pack"
    );
}
