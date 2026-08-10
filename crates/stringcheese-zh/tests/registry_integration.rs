//! Verifies that depending on `stringcheese-zh` is enough to make the
//! Chinese pack visible through [`stringcheese_lang::registry`]. The
//! `register_language!` macro in `stringcheese-zh/src/lib.rs` is the
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
//! for the field-level contract. The BCP-47 fallback checks below are
//! preserved as pack-specific tack-ons.
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_zh::CHINESE,
    pack_ty: stringcheese_zh::Chinese,
    code: "zh",
    name: "Chinese",
    smoke: |lang| {
        assert!(lang.is_stopword("的"));
        // Identity stemmer.
        assert_eq!(lang.stem("中国"), "中国");
        // Character-level tokenization.
        let toks: Vec<&str> = lang.tokenize("你好").collect();
        assert_eq!(toks, ["你", "好"]);
    },
}

#[test]
fn chinese_pack_bcp47_fallback_from_zh_cn() {
    // `zh-CN` (Simplified Chinese, mainland) should fall back to
    // `zh` via the registry's BCP-47 subtag-strip walk.
    let lang = stringcheese_lang::registry::language("zh-CN")
        .expect("zh-CN should fall back to zh via BCP-47 subtag strip");
    assert_eq!(lang.code(), "zh");
}

#[test]
fn chinese_pack_bcp47_fallback_from_zh_hans_cn() {
    // `zh-Hans-CN` should fall back to `zh` after two strips.
    let lang = stringcheese_lang::registry::language("zh-Hans-CN")
        .expect("zh-Hans-CN should fall back to zh via BCP-47 subtag strip");
    assert_eq!(lang.code(), "zh");
}
