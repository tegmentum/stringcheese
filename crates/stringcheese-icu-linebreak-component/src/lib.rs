//! # Reference `tegmentum:i18n-linebreak@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model
//! packaging of the StringCheese ICU-alternative UAX #14 line-break
//! capability. It wraps
//! [`stringcheese_icu_linebreak::LineBreakEngine`] behind the shared
//! `tegmentum:i18n-linebreak@0.1.0` WIT contract (see
//! [`component/wit/linebreak/stringcheese-icu-linebreak.wit`](../../../component/wit/linebreak/stringcheese-icu-linebreak.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `find-breaks` /
//! `find-breaks-with-strictness` / `supported-locales` / `supports`
//! without linking Rust.
//!
//! ## Position in the WIT-i18n subsystem
//!
//! WIT-i18n Phase 5 follow-up (`docs/design/wit-i18n.md` § 8.7).
//! Split out of the UAX #29 segment capability because the UAX #14
//! rule set is much larger than the UAX #29 grapheme / word /
//! sentence rules; keeping the two crates separate lets the
//! line-break subsystem evolve (dictionaries for CJK, tailorings
//! for CSS `line-break` / `word-break`, bidi-aware line breaking,
//! …) without polluting the segment surface.
//!
//! ## Why no bundled SCUD packs
//!
//! Phase 5's algorithm crate ships a locale-neutral default: the
//! UAX #14 classification tables and rule engine live in
//! `stringcheese-icu-linebreak::classes`, and a fresh
//! `LineBreakEngine::new()` runs the default rules end-to-end
//! without consulting any external pack. Locale-specific
//! tailorings (CJK dictionary-based line breaking, CSS
//! `line-break: strict` / `loose` beyond the built-in strictness
//! tag) are deferred to a follow-up.
//!
//! ## Feature-gated WIT export
//!
//! The WIT `Guest` impls in the `wit` module and the `bindings`
//! module are gated behind the `wit-component` cargo feature —
//! see the feature docs in `Cargo.toml`. Without the gate, an
//! umbrella crate that links multiple WIT components as plain
//! `rlib`s would emit duplicate `export!` symbols and fail to
//! link.
//!
//! ## Build recipe
//!
//! ```text
//! # Standalone component build (wasm32-wasip1, feature-gated):
//! cargo build \
//!     -p stringcheese-icu-linebreak-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_icu_linebreak_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-icu-linebreak-component --features wit-component
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
// The bindings module contains wit-bindgen-generated `unsafe extern`
// blocks; forbidding unsafe would fail its build.
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

// WIT bindings + Guest wiring only compile on wasm targets with the
// `wit-component` feature — native builds stay pure-Rust and don't
// drag `wit-bindgen-rt` into `cargo test` or downstream Rust hosts.
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

use stringcheese_icu_linebreak::LineBreakEngine;

/// Build a fresh `LineBreakEngine` backed by the algorithm crate's
/// built-in UAX #14 default classification tables + rules.
///
/// Phase 5's follow-up does not embed any SCUD pack — the algorithm
/// crate's `LineBreakEngine::new()` runs pure-algorithm-driven UAX
/// #14 behaviour without needing external pack data. A future phase
/// that ships locale-specific tailorings (CJK dictionaries) will
/// grow this constructor to load per-locale packs analogous to the
/// datetime component's `reference_engine`.
#[must_use]
pub const fn reference_engine() -> LineBreakEngine<'static> {
    LineBreakEngine::new()
}

/// The list of BCP 47 locale tags the reference engine covers.
/// Phase 5's follow-up returns the root-locale marker `""` and
/// nothing else (see the crate-level docs for why); a future phase
/// populating per-locale tailorings will grow this list.
#[must_use]
pub fn reference_supported_locales() -> alloc::vec::Vec<alloc::string::String> {
    alloc::vec![alloc::string::String::new()]
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The WIT source that ships with the repo, embedded so the
    /// test asserts the on-disk file parses cleanly under
    /// `wit-parser`.
    const WIT_SOURCE: &str =
        include_str!("../../../component/wit/linebreak/stringcheese-icu-linebreak.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-linebreak.wit"),
                WIT_SOURCE,
            )
            .expect(
                "component/wit/linebreak/stringcheese-icu-linebreak.wit must parse under wit-parser",
            );
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "tegmentum");
        assert_eq!(pkg_name.name, "i18n-linebreak");
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
    fn wit_file_declares_linebreak_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-linebreak.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "linebreak-world"),
            "WIT must export the `linebreak-world` world",
        );
    }

    #[test]
    fn reference_engine_finds_breaks_on_simple_input() {
        let engine = reference_engine();
        let bs = engine.find_breaks("hello world");
        // Should have at least a break after the space and at eot.
        assert!(!bs.is_empty());
        assert_eq!(
            bs.last().map(|b| b.offset),
            Some(u32::try_from("hello world".len()).unwrap())
        );
    }

    #[test]
    fn reference_engine_hard_break_at_lf() {
        let engine = reference_engine();
        let bs = engine.find_breaks("a\nb");
        // At least one mandatory break (after LF).
        assert!(
            bs.iter()
                .any(|b| b.kind == stringcheese_icu_linebreak::BreakKind::Mandatory)
        );
    }

    #[test]
    fn reference_supported_locales_lists_root_only() {
        let locales = reference_supported_locales();
        assert_eq!(locales, vec![""]);
    }
}
