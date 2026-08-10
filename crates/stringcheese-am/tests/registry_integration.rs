//! Verifies that depending on `stringcheese-am` is enough to make the
//! Amharic pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-am/src/lib.rs` is the
//! sole opt-in — if it stops firing (for instance, if a linker strips
//! the registration `static`), this test flips red.
//!
//! Gated off wasm — `stringcheese_lang::registry` is itself gated on
//! `not(target_family = "wasm")` because `linkme::distributed_slice`
//! has no wasm branch. The registration on this pack is likewise
//! wasm-gated; on wasm targets this whole test file compiles to
//! nothing.
//!
//! The three `#[test] fn`s and the pack-anchoring `KEEP` const are
//! emitted by the shared `stringcheese_lang::pack_registry_smoke_test!`
//! macro — see its docs for the field-level contract and the
//! rationale-of-shape. This crate is the proof-of-concept for the
//! macro; see `docs/design/language-pack-dry.md` for the design
//! discussion.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_am::AMHARIC,
    pack_ty: stringcheese_am::Amharic,
    code: "am",
    name: "Amharic",
    smoke: |lang| {
        assert!(lang.is_stopword("እና"));
        // Light stemmer strips the plural marker -ኦች.
        assert_eq!(lang.stem("ልጅኦች"), "ልጅ");
    },
}
