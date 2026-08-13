//! # Reference `tegmentum:i18n-segment@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model
//! packaging of the StringCheese ICU-alternative break-iteration
//! capability. It wraps
//! [`stringcheese_icu_segment::BreakEngine`] behind the shared
//! `tegmentum:i18n-segment@0.1.0` WIT contract (see
//! [`component/wit/segment/stringcheese-icu-segment.wit`](../../../component/wit/segment/stringcheese-icu-segment.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `segment-graphemes` /
//! `segment-words` / `segment-sentences` / `supported-locales` /
//! `supports` without linking Rust.
//!
//! ## Position in the WIT-i18n subsystem
//!
//! WIT-i18n Phase 5 (`docs/design/wit-i18n.md` § 8.5). Phase 5
//! ships the WIT file, the `BreakEngine` algorithm side, and this
//! component wrapper across a single wave — matching the
//! standalone-component-in-line pattern established by Phase 4's
//! datetime capability.
//!
//! ## Why no bundled SCUD packs
//!
//! Phase 5's algorithm crate ships a locale-neutral default: the
//! UAX #29 classification tables and rule state machines live in
//! `stringcheese-icu-segment::classes`, and a fresh
//! `BreakEngine::new()` runs the default rules end-to-end without
//! consulting any external pack. Locale-specific tailorings
//! (Japanese/Chinese word-break dictionaries, Thai/Lao/Khmer
//! syllable segmentation) are deferred to a follow-up. Compared to
//! the collation / datetime component wrappers this means the
//! shipped `.wasm` embeds *no* bundled `.scud` bytes — the
//! reference engine is just `BreakEngine::new()`, and the
//! `supported-locales` export returns the root-locale marker
//! `""` and nothing else (matching what the algorithm crate's
//! `BreakEngine::supported_locales()` reports).
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
//!     -p stringcheese-icu-segment-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_icu_segment_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-icu-segment-component --features wit-component
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

use stringcheese_icu_segment::BreakEngine;

/// Build a fresh `BreakEngine` backed by the algorithm crate's
/// built-in UAX #29 default classification tables + rules.
///
/// Phase 5 does not embed any SCUD pack — the algorithm crate's
/// `BreakEngine::new()` runs pure-algorithm-driven UAX #29
/// behaviour without needing external pack data. A future phase
/// that ships locale-specific tailorings (Japanese/Chinese
/// word-break dictionaries, Thai syllable segmentation, ...) will
/// grow this constructor to load per-locale packs analogous to the
/// datetime component's `reference_engine`.
#[must_use]
pub const fn reference_engine() -> BreakEngine<'static> {
    BreakEngine::new()
}

/// The list of BCP 47 locale tags the reference engine covers.
/// Phase 5 returns the root-locale marker `""` and nothing else
/// (see the crate-level docs for why); a future phase populating
/// per-locale tailorings will grow this list.
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
        include_str!("../../../component/wit/segment/stringcheese-icu-segment.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-segment.wit"),
                WIT_SOURCE,
            )
            .expect(
                "component/wit/segment/stringcheese-icu-segment.wit must parse under wit-parser",
            );
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "tegmentum");
        assert_eq!(pkg_name.name, "i18n-segment");
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
    fn wit_file_declares_segment_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-segment.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "segment-world"),
            "WIT must export the `segment-world` world",
        );
    }

    #[test]
    fn reference_engine_segments_ascii_word() {
        let engine = reference_engine();
        let bs = engine.segment_graphemes("abc");
        assert_eq!(bs, vec![0, 1, 2, 3]);
    }

    #[test]
    fn reference_engine_segments_family_emoji_as_one_grapheme() {
        let engine = reference_engine();
        // Man + ZWJ + Woman = one grapheme under UAX #29 GB11.
        let s = "\u{1F468}\u{200D}\u{1F469}";
        let bs = engine.segment_graphemes(s);
        assert_eq!(bs, vec![0, u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn reference_engine_segments_two_sentences() {
        let engine = reference_engine();
        let bs = engine.segment_sentences("Hi. Bye.", "");
        assert_eq!(bs.first(), Some(&0));
        assert_eq!(bs.last(), Some(&8));
        assert!(bs.len() >= 3);
    }

    #[test]
    fn reference_supported_locales_lists_root_only() {
        let locales = reference_supported_locales();
        assert_eq!(locales, vec![""]);
    }
}
