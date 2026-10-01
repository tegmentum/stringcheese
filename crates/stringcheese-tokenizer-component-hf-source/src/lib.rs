//! # Runtime-bytes HuggingFace tokenizer-source component
//!
//! Sibling of the [baked packs][t5] that layer a specific model's
//! tokenizer.json into their compiled artifact.  This crate takes
//! the opposite ownership stance: it exports the resource-based
//! [`tegmentum:tokenizer/tokenizer-source@0.1.0`][wit] interface
//! whose `from-json` / `from-bytes` constructors accept a
//! tokenizer.json blob at runtime and return a
//! [`tokenizer-handle`][resource] the caller drives.
//!
//! One compiled component satisfies every consumer that needs a
//! runtime-supplied HuggingFace vocabulary.  Same underlying
//! [`stringcheese_tokenizer_hf::hf`] machinery every baked pack
//! uses; different composition rules.
//!
//! [t5]: https://docs.rs/stringcheese-tokenizer-component-t5
//! [wit]: ../../component/wit/tokenizer/stringcheese-tokenizer.wit
//! [resource]: https://component-model.bytecodealliance.org/design/wit.html#resources
//!
//! ## Build recipe
//!
//! ```text
//! cargo component build \
//!     --manifest-path crates/stringcheese-tokenizer-component-hf-source/Cargo.toml \
//!     --target wasm32-wasip1 \
//!     --features wit-component \
//!     --release
//! ```
//!
//! `cargo-component` (>= 0.21.1) is required — the crate's
//! `[package.metadata.component]` block points at
//! `component/wit/tokenizer` and targets world
//! `tokenizer-source-provider`, which cargo-component's post-build
//! pass reads to encode the cdylib as a real WIT component.  Plain
//! `cargo build` emits a core module with an empty world and
//! wasmos-compose / wac plug reject it.
//!
//! ## Ownership vs the baked packs
//!
//! The baked packs (`stringcheese-tokenizer-component-{t5,whisper,
//! cl100k}`) embed a specific tokenizer.json at build time and
//! export the free-function `tokenizer` interface.  One pack per
//! model, vocabulary immutable, guest never handles bytes.
//!
//! This pack embeds no bytes and exports the resource-based
//! `tokenizer-source` interface.  One shared satisfier, vocabulary
//! decided by whichever caller instantiates the first `from-json`
//! / `from-bytes`, guest owns the tokenizer.json blob.
//!
//! Both patterns live in one WIT package (`tegmentum:tokenizer@
//! 0.1.0`) so a downstream world can pick whichever shape fits its
//! deployment story.

#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

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
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        world: "tokenizer-source-provider",
        path: "wit",
        generate_all,
    });
}

// Non-wasm or `wit-component` feature off: emit a stub module so the
// rest of this crate's `use crate::bindings::...` paths resolve.  Host
// (`cargo check` / `cargo test`) does not exercise the WIT boundary.
#[cfg(not(all(target_family = "wasm", feature = "wit-component")))]
mod bindings {}

#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

mod runtime;

pub use runtime::{HfSourceError, HfSourceHandle};

#[cfg(test)]
mod tests {
    /// The WIT source that ships with the repo, embedded so a
    /// regression to either the WIT source or the parser
    /// version surfaces on `cargo test` before the component
    /// build ever runs.
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
    fn wit_file_declares_tokenizer_source_provider_world() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-tokenizer.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .worlds
                .iter()
                .any(|(_, world)| world.name == "tokenizer-source-provider"),
            "WIT must export the `tokenizer-source-provider` world"
        );
    }

    #[test]
    fn wit_file_declares_tokenizer_source_interface() {
        let mut resolve = wit_parser::Resolve::new();
        let _ = resolve
            .push_str(
                std::path::Path::new("stringcheese-tokenizer.wit"),
                WIT_SOURCE,
            )
            .expect("WIT parses");
        assert!(
            resolve
                .interfaces
                .iter()
                .any(|(_, iface)| iface.name.as_deref() == Some("tokenizer-source")),
            "WIT must define the `tokenizer-source` interface"
        );
    }
}
