//! # Reference `tegmentum:tokenizer@0.1.0` component wrapper
//!
//! This crate is the reference WebAssembly Component-Model packaging of
//! the StringCheese tokenizer subsystem. It wraps a
//! [`stringcheese_tokenizer_hf::BpeTokenizer`] behind the shared
//! `tegmentum:tokenizer@0.1.0` WIT contract (see
//! [`component/wit/tokenizer/stringcheese-tokenizer.wit`](../../../component/wit/tokenizer/stringcheese-tokenizer.wit))
//! so that any component-model-capable host — `Wasmtime`, `jco`,
//! `WasmCloud`, `Spin` — can invoke `encode` / `decode` / `count` /
//! `get-capabilities` without linking Rust.
//!
//! ## Position in the tokenizer subsystem
//!
//! Phase 7 of the tokenizer subsystem design
//! (`docs/design/tokenizers.md` § 11). The design commits to a
//! `component/wit/tokenizer/stringcheese-tokenizer.wit` that parses cleanly under
//! `wit-parser`, `wit-bindgen` producing a clean Rust host binding, and
//! a **reference** `tokenizer-provider` component that echoes correct
//! encodings across the boundary under `wasmtime`. This crate is that
//! reference — the pre-configured `cl100k_base` / `o200k_base` /
//! `HuggingFace` variants remain their own component crates when they
//! land, each mirroring the shape here.
//!
//! ## Why a hand-crafted vocabulary
//!
//! The reference tokenizer here is built from a small hand-crafted
//! character-BPE vocab embedded in this crate. Deliberate choices:
//!
//! * **No real `OpenAI` / `HuggingFace` vocab bytes.** The workspace's
//!   session-standing constraint (see `CLAUDE.md`) forbids committing
//!   real tiktoken / HF vocab bytes into the tree. Real vocabs are
//!   fetched at test time by the
//!   [`stringcheese-tokenizer-tiktoken-conformance`] harness; nothing
//!   ships in-tree.
//! * **A minimal 260-token vocab is enough for the reference.** The
//!   goal of this crate is to demonstrate that the WIT boundary works
//!   end-to-end — one component built, loaded under wasmtime, echoing
//!   a correct encoding round-trip. A one-word merge and a small
//!   special-token map exercise every code path (byte-level seeding,
//!   merge loop, special-token bypass, offset tracking, decode).
//! * **Deterministic and testable.** The vocab is a compile-time
//!   constant; the same encode input produces the same id sequence on
//!   host and wasm targets, so the wasmtime smoke test in `tests/`
//!   compares against a known-good vector.
//!
//! [`stringcheese-tokenizer-tiktoken-conformance`]: https://docs.rs/stringcheese-tokenizer-tiktoken-conformance
//!
//! ## Feature-gated WIT export
//!
//! The WIT `Guest` impl in `crate::wit` and the `crate::bindings`
//! module are gated behind the `wit-component` cargo feature — see
//! the feature docs in `Cargo.toml`. This matches the pattern
//! established by `stringcheese-detect-{script,whatlang,lingua}`:
//! without the gate, an umbrella crate that links multiple WIT
//! components as plain `rlib`s would emit duplicate `export!` symbols
//! and fail to link. The gate is `cfg(all(target_family = "wasm",
//! feature = "wit-component"))` because the WIT export machinery only
//! materialises on wasm — a host `cargo build` never needs it.
//!
//! ## Build recipe
//!
//! ```text
//! # Standalone component build (wasm32-wasip1, feature-gated):
//! cargo build \
//!     -p stringcheese-tokenizer-component \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//!
//! # Verify the produced .wasm exports the expected WIT world:
//! wasm-tools component wit \
//!     target/wasm32-wasip1/release/stringcheese_tokenizer_component.wasm
//!
//! # Run the smoke test under wasmtime:
//! cargo test -p stringcheese-tokenizer-component --features wit-component
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
// The bindings module contains wit-bindgen-generated `unsafe extern`
// blocks; forbidding unsafe would fail its build. The rest of the
// crate keeps the workspace's default `unsafe_op_in_unsafe_fn = deny`
// posture.
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

// WIT bindings + Guest wiring only compile on wasm targets with the
// `wit-component` feature — native builds stay pure-Rust and don't
// drag wit-bindgen-rt into `cargo test` or downstream Rust hosts, and
// non-component wasm consumers (e.g. an umbrella crate that links
// this crate as an `rlib` alongside other tokenizer backends) also
// skip it. Without the feature-gate, two backends `export!`-ing the
// same interface into one binary collide at link time with duplicate
// exported symbols. See the detect-* crates for the same pattern.
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

#[cfg(feature = "alloc")]
mod reference;

#[cfg(feature = "alloc")]
pub use reference::{
    REFERENCE_VOCAB_SIZE, ReferenceCapabilities, ReferenceEncoding, ReferenceTokenizerError, count,
    decode, encode, get_capabilities, reference_tokenizer,
};

#[cfg(test)]
mod tests {
    /// The WIT source that ships with the repo, embedded so the test
    /// asserts the on-disk file parses cleanly under `wit-parser`.
    /// Phase 7's acceptance criterion ("`component/wit/tokenizer/stringcheese-tokenizer.wit`
    /// parses under `wit-parser`") lands as a plain host unit test —
    /// no wasm toolchain needed to run the gate.
    const WIT_SOURCE: &str =
        include_str!("../../../component/wit/tokenizer/stringcheese-tokenizer.wit");

    #[test]
    fn wit_file_parses_under_wit_parser() {
        let mut resolve = wit_parser::Resolve::new();
        let pkg = resolve
            .push_str(
                std::path::Path::new("stringcheese-tokenizer.wit"),
                WIT_SOURCE,
            )
            .expect(
                "component/wit/tokenizer/stringcheese-tokenizer.wit must parse under wit-parser",
            );
        // The parser returns a `PackageId`. Assert the parsed package
        // carries the name we expect — a rename in the WIT file
        // without updating the loader downstream would silently
        // regress here.
        let pkg_name = &resolve.packages[pkg].name;
        assert_eq!(pkg_name.namespace, "tegmentum");
        assert_eq!(pkg_name.name, "tokenizer");
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
    fn wit_file_declares_tokenizer_provider_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-tokenizer.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        // Find the `tokenizer-provider` world. A regression that
        // dropped or renamed it would surface here — the CI wasm
        // component build depends on that world existing.
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "tokenizer-provider"),
            "WIT must export the `tokenizer-provider` world"
        );
    }
}
