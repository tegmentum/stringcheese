//! # Reference `stringcheese:icu-case@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model
//! packaging of the StringCheese ICU-alternative case-mapping
//! capability. It wraps [`stringcheese_icu_case::CaseEngine`]
//! behind the shared `stringcheese:icu-case@0.1.0` WIT contract
//! (see
//! [`component/wit/icu-case/stringcheese-icu-case.wit`](../../../component/wit/icu-case/stringcheese-icu-case.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `to-lower` / `to-upper` /
//! `to-title` / `fold` / `supported-locales` / `supports` without
//! linking Rust.
//!
//! ## Position in the WIT-i18n subsystem
//!
//! WIT-i18n Phase 1's deferred bit
//! (`docs/design/wit-i18n.md` § 8.1). Phase 1 landed the WIT file
//! plus the `CaseEngine` algorithm side of the interface but left
//! the standalone `--target wasm32-wasip1 --features wit-component`
//! recipe for a follow-up. This crate is that follow-up: the
//! reference component that carries the `case` world across the
//! wasm boundary, mirroring the
//! [`stringcheese-tokenizer-component`] template's shape at every
//! non-load-bearing seam so an operator switching between the two
//! sees the same feature gates, the same dev-dep set, the same
//! smoke-test structure, and the same CI job layout.
//!
//! ## Why a bundled pack set
//!
//! The shipped `.wasm` embeds the [`stringcheese-en`] and
//! [`stringcheese-tr`] SCUD packs so that the smoke test in
//! `tests/component_smoke.rs` — which drives the componentised
//! wasm end-to-end under `wasmtime` — has meaningful data to
//! exercise the Turkish contextual overrides against. The design
//! commits to a future world where a caller composes a bare
//! `stringcheese-icu-case` component with per-locale pack
//! components via `wasm-tools compose`; this reference component
//! is the "smoke-test everything in one binary" variant that
//! matches the tokenizer-component precedent.
//!
//! The bundled packs are the same ~2.5 KiB total exposed by
//! `stringcheese-en::case_data::CASE_EN_SCUD` and
//! `stringcheese-tr::case_data::CASE_TR_SCUD` — reused verbatim,
//! never duplicated, so a fix to either pack lands in one place.
//!
//! [`stringcheese-tokenizer-component`]: https://docs.rs/stringcheese-tokenizer-component
//! [`stringcheese-en`]: https://docs.rs/stringcheese-en
//! [`stringcheese-tr`]: https://docs.rs/stringcheese-tr
//!
//! ## Feature-gated WIT export
//!
//! The WIT `Guest` impls in the `wit` module and the `bindings`
//! module are gated behind the `wit-component` cargo feature — see
//! the feature docs in `Cargo.toml`. This matches the pattern used
//! by `stringcheese-tokenizer-component` and the
//! `stringcheese-detect-{script,whatlang,lingua}` crates: without
//! the gate, an umbrella crate that links multiple WIT components
//! as plain `rlib`s would emit duplicate `export!` symbols and fail
//! to link. The gate is
//! `cfg(all(target_family = "wasm", feature = "wit-component"))`
//! because the WIT export machinery only materialises on wasm — a
//! host `cargo build` never needs it.
//!
//! ## Build recipe
//!
//! ```text
//! # Standalone component build (wasm32-wasip1, feature-gated):
//! cargo build \
//!     -p stringcheese-icu-case-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_icu_case_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-icu-case-component --features wit-component
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
// The bindings module contains wit-bindgen-generated `unsafe extern`
// blocks; forbidding unsafe would fail its build. The rest of the
// crate keeps the workspace's default `unsafe_op_in_unsafe_fn = deny`
// posture.
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

// WIT bindings + Guest wiring only compile on wasm targets with the
// `wit-component` feature — native builds stay pure-Rust and don't
// drag `wit-bindgen-rt` into `cargo test` or downstream Rust hosts,
// and non-component wasm consumers (e.g. an umbrella crate that
// links this crate as an `rlib` alongside other capability
// backends) also skip it. Without the feature gate, two backends
// `export!`-ing the same interface into one binary would collide at
// link time with duplicate exported symbols. See the crate-level
// docs and `stringcheese-tokenizer-component` for the same pattern.
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::restriction
)]
mod bindings;
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

use alloc::vec;

use stringcheese_icu_case::{CaseEngine, CasePack};

/// Build a fresh `CaseEngine` backed by the bundled English +
/// Turkish SCUD packs.
///
/// The engine is constructed anew on every call — each `CasePack`
/// borrows from the `'static` SCUD byte slice, so the returned
/// engine is `CaseEngine<'static>` and cheap to construct
/// (the SCUD headers are re-parsed, but no data is copied). The
/// wasm guest caches one engine in a `OnceLock` and hands out
/// borrows from there; native callers (unit tests) build fresh.
///
/// # Panics
///
/// Panics if either bundled SCUD pack fails to parse. Both packs
/// are compile-time constants generated by their crates' `build.rs`
/// against a hand-authored fixture; a parse failure here is a
/// packaging bug, not a runtime condition.
#[must_use]
pub fn reference_engine() -> CaseEngine<'static> {
    let en = stringcheese_en::case_data::case_pack()
        .expect("bundled case-en.scud must parse (packaging bug otherwise)");
    let tr = stringcheese_tr::case_data::case_pack()
        .expect("bundled case-tr.scud must parse (packaging bug otherwise)");
    CaseEngine::new(vec![en, tr])
}

/// The list of BCP 47 locale tags the reference engine's bundled
/// packs cover. Returned in a stable order for host-side snapshot
/// tests.
#[must_use]
pub fn reference_supported_locales() -> alloc::vec::Vec<alloc::string::String> {
    use alloc::string::ToString as _;
    // Two entries, ordered by BCP 47 tag ASCII value so the wasm
    // smoke test can assert an exact vector without depending on
    // internal storage order of the `CaseEngine`.
    vec!["en".to_string(), "tr".to_string()]
}

/// A single reusable pack pair — the `CasePack<'static>` values
/// this crate exports so downstream native tests can build their
/// own `CaseEngine` variants without redoing the bundled-pack
/// parse.
///
/// # Panics
///
/// See [`reference_engine`].
#[must_use]
pub fn reference_packs() -> alloc::vec::Vec<CasePack<'static>> {
    let en = stringcheese_en::case_data::case_pack()
        .expect("bundled case-en.scud must parse (packaging bug otherwise)");
    let tr = stringcheese_tr::case_data::case_pack()
        .expect("bundled case-tr.scud must parse (packaging bug otherwise)");
    vec![en, tr]
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The WIT source that ships with the repo, embedded so the
    /// test asserts the on-disk file parses cleanly under
    /// `wit-parser`. Mirrors the smoke test that already lives in
    /// `stringcheese-icu-case/tests/wit_parse.rs`; landing the same
    /// gate here surfaces a break in this crate's log even when
    /// the sibling test is not requested.
    const WIT_SOURCE: &str =
        include_str!("../../../component/wit/icu-case/stringcheese-icu-case.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-case.wit"),
                WIT_SOURCE,
            )
            .expect("component/wit/icu-case/stringcheese-icu-case.wit must parse under wit-parser");
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "stringcheese");
        assert_eq!(pkg_name.name, "icu-case");
        assert_eq!(
            pkg_name
                .version
                .as_ref()
                .expect("package must carry a version")
                .to_string(),
            "0.1.0"
        );
    }

    #[test]
    fn wit_file_declares_case_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-case.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve.worlds.iter().any(|(_, world)| world.name == "case"),
            "WIT must export the `case` world",
        );
    }

    #[test]
    fn reference_engine_exercises_bundled_packs() {
        let engine = reference_engine();
        // English default: capital I lowers to i.
        assert_eq!(engine.to_lower("ISTANBUL", "en"), "istanbul");
        // Turkish contextual override: capital I lowers to dotless ı.
        assert_eq!(engine.to_lower("ISTANBUL", "tr"), "ıstanbul");
        // Turkish contextual override: lowercase i uppers to dotted İ.
        assert_eq!(engine.to_upper("istanbul", "tr"), "İSTANBUL");
        // English default: lowercase i uppers to I.
        assert_eq!(engine.to_upper("istanbul", "en"), "ISTANBUL");
    }

    #[test]
    fn reference_supported_locales_lists_bundled_tags() {
        let locales = reference_supported_locales();
        assert_eq!(locales, vec!["en", "tr"]);
    }
}
