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
//!
//! The three canonical `#[test] fn`s and the pack-anchoring `KEEP`
//! const are emitted by the shared
//! `stringcheese_lang::pack_registry_smoke_test!` macro — see its docs
//! for the field-level contract. The macrolanguage-absence check below
//! is preserved as a pack-specific tack-on.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_nn::NYNORSK,
    pack_ty: stringcheese_nn::Nynorsk,
    code: "nn",
    name: "Norwegian Nynorsk",
    smoke: |lang| {
        assert!(lang.is_stopword("og"));
        assert!(lang.is_stopword("ikkje"));
        assert_eq!(lang.stem("bilane"), "bil");
    },
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
        stringcheese_lang::registry::language("no").is_none(),
        "the macrolanguage code \"no\" should not be registered by the Nynorsk pack"
    );
}
