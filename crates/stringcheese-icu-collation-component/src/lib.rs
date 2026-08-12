//! # Reference `stringcheese:icu-collation@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model
//! packaging of the StringCheese ICU-alternative collation
//! capability. It wraps
//! [`stringcheese_icu_collation::CollationEngine`] behind the
//! shared `stringcheese:icu-collation@0.1.0` WIT contract (see
//! [`component/wit/collation/stringcheese-icu-collation.wit`](../../../component/wit/collation/stringcheese-icu-collation.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `compare` / `sort-key` /
//! `get-capabilities` / `supports` without linking Rust.
//!
//! ## Position in the WIT-i18n subsystem
//!
//! WIT-i18n Phase 2's deferred bit
//! (`docs/design/wit-i18n.md` § 8.2). Phase 2 landed the WIT file
//! plus the `CollationEngine` algorithm side of the interface
//! but left the standalone `--target wasm32-wasip1 --features
//! wit-component` recipe for a follow-up. This crate is that
//! follow-up: the reference component that carries the
//! `collation-world` world across the wasm boundary, mirroring
//! the [`stringcheese-icu-case-component`] template's shape at
//! every non-load-bearing seam so an operator switching between
//! the two sees the same feature gates, the same dev-dep set, the
//! same smoke-test structure, and the same CI job layout.
//!
//! ## Why a bundled pack set
//!
//! The shipped `.wasm` embeds the [`stringcheese-en`] and
//! [`stringcheese-de`] SCUD packs so that the smoke test in
//! `tests/component_smoke.rs` — which drives the componentised
//! wasm end-to-end under `wasmtime` — has meaningful data to
//! exercise the DIN 5007-2 (phonebook) tailoring against. The
//! design commits to a future world where a caller composes a
//! bare `stringcheese-icu-collation` component with per-locale
//! pack components via `wasm-tools compose`; this reference
//! component is the "smoke-test everything in one binary"
//! variant that matches the case-component and
//! tokenizer-component precedents.
//!
//! The bundled packs are the same 126 + 194 = 320 byte total
//! exposed by `stringcheese-en::collation_data::COLLATION_EN_SCUD`
//! and `stringcheese-de::collation_data::COLLATION_DE_SCUD` —
//! reused verbatim, never duplicated, so a fix to either pack
//! lands in one place.
//!
//! [`stringcheese-icu-case-component`]: https://docs.rs/stringcheese-icu-case-component
//! [`stringcheese-en`]: https://docs.rs/stringcheese-en
//! [`stringcheese-de`]: https://docs.rs/stringcheese-de
//!
//! ## Feature-gated WIT export
//!
//! The WIT `Guest` impls in the `wit` module and the `bindings`
//! module are gated behind the `wit-component` cargo feature —
//! see the feature docs in `Cargo.toml`. This matches the pattern
//! used by `stringcheese-icu-case-component`,
//! `stringcheese-tokenizer-component`, and the
//! `stringcheese-detect-{script,whatlang,lingua}` crates: without
//! the gate, an umbrella crate that links multiple WIT components
//! as plain `rlib`s would emit duplicate `export!` symbols and
//! fail to link. The gate is
//! `cfg(all(target_family = "wasm", feature = "wit-component"))`
//! because the WIT export machinery only materialises on wasm — a
//! host `cargo build` never needs it.
//!
//! ## Build recipe
//!
//! ```text
//! # Standalone component build (wasm32-wasip1, feature-gated):
//! cargo build \
//!     -p stringcheese-icu-collation-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_icu_collation_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-icu-collation-component --features wit-component
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
// docs and `stringcheese-icu-case-component` for the same pattern.
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

use stringcheese_icu_collation::{CollationEngine, CollationPack};

/// Build a fresh `CollationEngine` backed by the bundled English +
/// German SCUD packs.
///
/// The engine is constructed anew on every call — each
/// `CollationPack` borrows from the `'static` SCUD byte slice, so
/// the returned engine is `CollationEngine<'static>` and cheap to
/// construct (the SCUD headers are re-parsed, but no data is
/// copied). The wasm guest caches one engine in an
/// `AtomicPtr`-backed singleton and hands out borrows from there;
/// native callers (unit tests) build fresh.
///
/// # Panics
///
/// Panics if either bundled SCUD pack fails to parse. Both packs
/// are compile-time constants generated by their crates' `build.rs`
/// against a hand-authored fixture; a parse failure here is a
/// packaging bug, not a runtime condition.
#[must_use]
pub fn reference_engine() -> CollationEngine<'static> {
    let en = stringcheese_en::collation_data::collation_pack()
        .expect("bundled collation-en.scud must parse (packaging bug otherwise)");
    let de = stringcheese_de::collation_data::collation_pack()
        .expect("bundled collation-de.scud must parse (packaging bug otherwise)");
    CollationEngine::new(vec![en, de])
}

/// The list of BCP 47 locale tags the reference engine's bundled
/// packs cover. Returned in a stable order for host-side snapshot
/// tests.
#[must_use]
pub fn reference_supported_locales() -> alloc::vec::Vec<alloc::string::String> {
    use alloc::string::ToString as _;
    // Two entries, ordered to match `reference_engine`'s pack
    // insertion order so the wasm smoke test can assert an exact
    // vector without depending on internal storage order of the
    // `CollationEngine`.
    vec!["en".to_string(), "de".to_string()]
}

/// A single reusable pack pair — the `CollationPack<'static>`
/// values this crate exports so downstream native tests can build
/// their own `CollationEngine` variants without redoing the
/// bundled-pack parse.
///
/// # Panics
///
/// See [`reference_engine`].
#[must_use]
pub fn reference_packs() -> alloc::vec::Vec<CollationPack<'static>> {
    let en = stringcheese_en::collation_data::collation_pack()
        .expect("bundled collation-en.scud must parse (packaging bug otherwise)");
    let de = stringcheese_de::collation_data::collation_pack()
        .expect("bundled collation-de.scud must parse (packaging bug otherwise)");
    vec![en, de]
}

/// The CLDR version the bundled packs were generated from.
///
/// Both packs are generated from the same CLDR release; this
/// constant is what the WIT `capabilities-record.cldr-version`
/// field reports back to callers.
pub const REFERENCE_CLDR_VERSION: &str = "44.1";

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::CollationStrength;

    /// The WIT source that ships with the repo, embedded so the
    /// test asserts the on-disk file parses cleanly under
    /// `wit-parser`. Mirrors the smoke test that already lives in
    /// `stringcheese-icu-collation/tests/wit_parse.rs`; landing
    /// the same gate here surfaces a break in this crate's log
    /// even when the sibling test is not requested.
    const WIT_SOURCE: &str =
        include_str!("../../../component/wit/collation/stringcheese-icu-collation.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-collation.wit"),
                WIT_SOURCE,
            )
            .expect(
                "component/wit/collation/stringcheese-icu-collation.wit must parse under wit-parser",
            );
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "stringcheese");
        assert_eq!(pkg_name.name, "icu-collation");
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
    fn wit_file_declares_collation_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-collation.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "collation-world"),
            "WIT must export the `collation-world` world",
        );
    }

    #[test]
    fn reference_engine_exercises_bundled_packs() {
        let engine = reference_engine();
        // English primary: "apple" < "banana".
        assert_eq!(
            engine.compare("apple", "banana", "en", CollationStrength::Primary),
            Ordering::Less,
        );
        // Primary strips ASCII case: APPLE == apple.
        assert_eq!(
            engine.compare("APPLE", "apple", "en", CollationStrength::Primary),
            Ordering::Equal,
        );
        // German phonebook tertiary: "Bär" (ä→ae under DE pack)
        // compares equal to "Baer" — the canonical DIN 5007-2
        // equivalence the pack ships. (Primary would still hold
        // under a `ä → a` rule; the shipped pack instead spells the
        // expansion out as `ae`, which is why phonebook equivalence
        // surfaces at tertiary rather than at primary.)
        assert_eq!(
            engine.compare("Bär", "Baer", "de", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn reference_supported_locales_lists_bundled_tags() {
        let locales = reference_supported_locales();
        assert_eq!(locales, vec!["en", "de"]);
    }

    #[test]
    fn reference_packs_carry_expected_cldr_version() {
        let packs = reference_packs();
        for pack in &packs {
            assert_eq!(pack.cldr_version(), REFERENCE_CLDR_VERSION);
        }
    }
}
