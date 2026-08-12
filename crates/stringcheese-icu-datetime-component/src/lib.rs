//! # Reference `tegmentum:i18n-datetime@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model
//! packaging of the StringCheese ICU-alternative date/time
//! capability. It wraps
//! [`stringcheese_icu_datetime::DateTimeEngine`] behind the shared
//! `tegmentum:i18n-datetime@0.1.0` WIT contract (see
//! [`component/wit/datetime/stringcheese-icu-datetime.wit`](../../../component/wit/datetime/stringcheese-icu-datetime.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `format-date` /
//! `format-time` / `format-datetime` / `supported-locales` /
//! `supports` without linking Rust.
//!
//! ## Position in the WIT-i18n subsystem
//!
//! WIT-i18n Phase 4 (`docs/design/wit-i18n.md` § 8.4). Phase 4
//! ships the WIT file, the `DateTimeEngine` algorithm side, and
//! this component wrapper in a single wave — mirroring the
//! standalone-component-in-line pattern established by
//! Phase 3's plural / number capabilities' post-close
//! landings. The shipped `.wasm` embeds the [`stringcheese-en`],
//! [`stringcheese-de`], and [`stringcheese-fr`] SCUD packs so
//! the smoke test in `tests/component_smoke.rs` — which drives
//! the componentised wasm end-to-end under `wasmtime` — has
//! meaningful data to exercise CLDR patterns against for all
//! three initial locales.
//!
//! [`stringcheese-en`]: https://docs.rs/stringcheese-en
//! [`stringcheese-de`]: https://docs.rs/stringcheese-de
//! [`stringcheese-fr`]: https://docs.rs/stringcheese-fr
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
//!     -p stringcheese-icu-datetime-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_icu_datetime_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-icu-datetime-component --features wit-component
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

use alloc::vec;

use stringcheese_icu_datetime::{DateTimeEngine, DateTimePack};

/// Build a fresh `DateTimeEngine` backed by the bundled English,
/// German, and French SCUD packs.
///
/// # Panics
///
/// Panics if any bundled SCUD pack fails to parse — a packaging
/// bug, not a runtime condition.
#[must_use]
pub fn reference_engine() -> DateTimeEngine<'static> {
    let en = stringcheese_en::datetime_data::datetime_pack()
        .expect("bundled datetime-en.scud must parse (packaging bug otherwise)");
    let de = stringcheese_de::datetime_data::datetime_pack()
        .expect("bundled datetime-de.scud must parse (packaging bug otherwise)");
    let fr = stringcheese_fr::datetime_data::datetime_pack()
        .expect("bundled datetime-fr.scud must parse (packaging bug otherwise)");
    DateTimeEngine::new(vec![en, de, fr])
}

/// The list of BCP 47 locale tags the reference engine's bundled
/// packs cover. Returned in a stable order for host-side snapshot
/// tests.
#[must_use]
pub fn reference_supported_locales() -> alloc::vec::Vec<alloc::string::String> {
    use alloc::string::ToString as _;
    vec!["en".to_string(), "de".to_string(), "fr".to_string()]
}

/// The reusable pack list — the `DateTimePack<'static>` values
/// this crate exports so downstream native tests can build their
/// own `DateTimeEngine` variants without redoing the bundled-pack
/// parse.
///
/// # Panics
///
/// See [`reference_engine`].
#[must_use]
pub fn reference_packs() -> alloc::vec::Vec<DateTimePack<'static>> {
    let en = stringcheese_en::datetime_data::datetime_pack()
        .expect("bundled datetime-en.scud must parse (packaging bug otherwise)");
    let de = stringcheese_de::datetime_data::datetime_pack()
        .expect("bundled datetime-de.scud must parse (packaging bug otherwise)");
    let fr = stringcheese_fr::datetime_data::datetime_pack()
        .expect("bundled datetime-fr.scud must parse (packaging bug otherwise)");
    vec![en, de, fr]
}

/// The CLDR version the bundled packs were generated from.
pub const REFERENCE_CLDR_VERSION: &str = "44.1";

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::DateTimeLength;

    /// The WIT source that ships with the repo, embedded so the
    /// test asserts the on-disk file parses cleanly under
    /// `wit-parser`.
    const WIT_SOURCE: &str =
        include_str!("../../../component/wit/datetime/stringcheese-icu-datetime.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-datetime.wit"),
                WIT_SOURCE,
            )
            .expect(
                "component/wit/datetime/stringcheese-icu-datetime.wit must parse under wit-parser",
            );
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "tegmentum");
        assert_eq!(pkg_name.name, "i18n-datetime");
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
    fn wit_file_declares_datetime_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-icu-datetime.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "datetime-world"),
            "WIT must export the `datetime-world` world",
        );
    }

    #[test]
    fn reference_engine_exercises_bundled_packs() {
        let engine = reference_engine();
        assert_eq!(
            engine
                .format_date("2024-09-22", "en", DateTimeLength::Medium)
                .unwrap(),
            "Sep 22, 2024"
        );
        assert_eq!(
            engine
                .format_date("2024-09-22", "de", DateTimeLength::Short)
                .unwrap(),
            "22.09.2024"
        );
        assert_eq!(
            engine
                .format_date("2024-09-22", "fr", DateTimeLength::Medium)
                .unwrap(),
            "22 sept. 2024"
        );
    }

    #[test]
    fn reference_supported_locales_lists_bundled_tags() {
        let locales = reference_supported_locales();
        assert_eq!(locales, vec!["en", "de", "fr"]);
    }

    #[test]
    fn reference_packs_carry_expected_cldr_version() {
        let packs = reference_packs();
        for pack in &packs {
            assert_eq!(pack.cldr_version(), REFERENCE_CLDR_VERSION);
        }
    }
}
