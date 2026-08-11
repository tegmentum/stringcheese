//! # Real-vocab `cl100k_base` `tegmentum:tokenizer@0.1.0` component
//!
//! Sibling of [`stringcheese_tokenizer_component`][ref] that layers
//! `OpenAI`'s **real** `cl100k_base` `mergeable_ranks` blob into the
//! same WIT `tokenizer-provider` shape. Where the reference crate
//! ships a hand-crafted 261-token demonstration vocab (deliberately
//! not real bytes — see the workspace `CLAUDE.md` constraint), this
//! crate embeds the actual GPT-3.5 / GPT-4 tokeniser and produces
//! byte-identical output to upstream tiktoken.
//!
//! [ref]: https://docs.rs/stringcheese-tokenizer-component
//!
//! ## Build-time contract
//!
//! * The real `cl100k_base.tiktoken` plaintext bytes are **never**
//!   committed. `build.rs` locates them at build time from
//!   `$STRINGCHEESE_CL100K_TIKTOKEN` or the standard cache path
//!   populated by the sibling
//!   `stringcheese-tokenizer-tiktoken-conformance` harness, verifies
//!   the SHA-256 against the pinned constant, and stages the bytes
//!   under `$OUT_DIR` for embedding.
//! * Without the `parity-real-vocab` feature the crate compiles into
//!   a stub — every `encode` / `decode` / `count` call returns a
//!   `TokenizerError::Other` naming the feature. This keeps the
//!   crate `cargo check`able offline and gives downstream callers a
//!   stable API surface to wire against.
//! * With `parity-real-vocab` on, a missing blob is a hard build
//!   error (see `build.rs`). The `wit-component` feature depends on
//!   `parity-real-vocab` — a stub component wraps a stub tokenizer
//!   that would error on every call, which is not a useful artifact.
//!
//! ## Position in the tokenizer subsystem
//!
//! Phase 7 follow-on of the tokenizer subsystem design
//! (`docs/design/tokenizers.md` § 11). The reference crate proves the
//! WIT boundary echoes correct encodings; this crate proves the WIT
//! boundary continues to echo correct encodings when a real,
//! production-scale vocabulary sits behind it. Future sibling crates
//! for `o200k_base` and per-model `HuggingFace` packs mirror this
//! shape.
//!
//! ## Build recipe
//!
//! ```text
//! # Populate the vocab cache once (fetches via SHA-256-verified HTTP):
//! cargo test --manifest-path \
//!     crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml \
//!     --features parity-real-vocab -- --nocapture
//!
//! # Standalone WIT component build (wasm32-wasip1):
//! cargo build \
//!     --manifest-path crates/stringcheese-tokenizer-component-cl100k/Cargo.toml \
//!     --target wasm32-wasip1 \
//!     --features wit-component,parity-real-vocab \
//!     --release
//!
//! # Host-side smoke test (native, no wasm toolchain needed):
//! cargo test \
//!     --manifest-path crates/stringcheese-tokenizer-component-cl100k/Cargo.toml \
//!     --features parity-real-vocab
//! ```

#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

// WIT bindings + Guest wiring only compile on wasm targets with the
// `wit-component` feature — native builds stay pure-Rust and don't
// drag `wit-bindgen-rt` into `cargo test` or downstream Rust hosts,
// and non-component wasm consumers also skip it. Without the
// feature-gate, two backends `export!`-ing the same interface into
// one binary collide at link time with duplicate exported symbols.
// See the parent `stringcheese-tokenizer-component` crate and the
// detect-* crates for the same pattern.
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

mod runtime;

pub use runtime::{
    Cl100kCapabilities, Cl100kEncoding, Cl100kTokenizerError, count, decode, encode,
    get_capabilities, is_real_vocab,
};

#[cfg(test)]
mod tests {
    /// The WIT source that ships with the repo, embedded so the test
    /// asserts the on-disk file parses cleanly under `wit-parser`.
    /// Mirrors the parent crate's gate — same WIT file, same shape,
    /// so a regression to either the WIT source or the parser
    /// version surfaces on `cargo test` before the component build
    /// ever runs.
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
    fn wit_file_declares_tokenizer_provider_world() {
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
                .any(|(_, world)| world.name == "tokenizer-provider"),
            "WIT must export the `tokenizer-provider` world"
        );
    }

    /// The stub path — verifiable without the real vocab feature.
    /// Every call must surface a diagnostic that names the feature
    /// gate so a caller who forgot to enable it sees the message
    /// immediately.
    #[cfg(not(stringcheese_cl100k_real_vocab))]
    #[test]
    fn stub_mode_reports_missing_feature() {
        assert!(!super::is_real_vocab());
        let err = super::encode("hello").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("parity-real-vocab"),
            "stub error must name the parity-real-vocab feature: {msg}"
        );
    }

    /// Capabilities call is always safe — under stub mode it reports
    /// the stub shape (vocab-size 0, no byte fallback); under real
    /// vocab it reports cl100k's real numbers.
    #[test]
    fn capabilities_report_reasonable_shape() {
        let caps = super::get_capabilities();
        assert_eq!(caps.model_type, "bpe");
        assert_eq!(caps.variant_id, "cl100k_base");
    }
}
