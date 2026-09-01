//! # Real-vocab OpenAI Whisper `tegmentum:tokenizer@0.1.0` component
//!
//! Sibling of
//! [`stringcheese_tokenizer_component_cl100k`](https://docs.rs/stringcheese-tokenizer-component-cl100k)
//! that layers OpenAI Whisper's real `tokenizer.json` into the same
//! WIT `tokenizer-provider` shape.  The cognition-side
//! `whisper-transcribe-component` (`~/git/cognition/crates/whisper-transcribe-component/`)
//! imports `tegmentum:tokenizer/tokenizer@0.1.0`; composing that
//! import against the artifact built here produces a fully-satisfied
//! `dist/whisper-ort.wasm` that decodes greedy-decoder token IDs into
//! text.
//!
//! ## Build-time contract
//!
//! * The `tokenizer.json` bytes are **never** committed.  `build.rs`
//!   locates them at build time from `$STRINGCHEESE_WHISPER_TOKENIZER_JSON`
//!   or the standard cache path.  Fetch from HF Hub:
//!
//!   ```text
//!   mkdir -p ~/.cache/stringcheese-tokenizer-whisper && \
//!   curl -L -o ~/.cache/stringcheese-tokenizer-whisper/tokenizer.json \
//!       https://huggingface.co/openai/whisper-tiny/resolve/main/tokenizer.json
//!   ```
//!
//!   Every whisper-* checkpoint ships the same tokenizer, so
//!   `whisper-tiny` works for `whisper-large-v3` too.  A future
//!   revision could split per-variant if a checkpoint diverges.
//!
//! * Without `parity-real-vocab` the crate compiles into a stub;
//!   every operation returns `TokenizerError::Other` naming the
//!   feature.  Keeps `cargo check` cheap offline and gives the
//!   parent component a stable WIT shape to compose against.
//!
//! * `wit-component` depends on `parity-real-vocab` — a stub
//!   component wraps a stub tokenizer that errors on every call,
//!   which is not a useful artifact to compose against.
//!
//! ## Position in the tokenizer subsystem
//!
//! Third real-vocab variant in the stringcheese tokenizer-provider
//! family, joining `stringcheese-tokenizer-component` (demo vocab)
//! and `stringcheese-tokenizer-component-cl100k` (real cl100k).  The
//! main structural difference from cl100k is the source format:
//! whisper's tokenizer is HuggingFace `tokenizer.json` (parsed via
//! `stringcheese_tokenizer_hf::hf`), not tiktoken plaintext.
//!
//! ## Build recipe
//!
//! ```text
//! # Fetch the tokenizer.json once:
//! mkdir -p ~/.cache/stringcheese-tokenizer-whisper
//! curl -L -o ~/.cache/stringcheese-tokenizer-whisper/tokenizer.json \
//!     https://huggingface.co/openai/whisper-tiny/resolve/main/tokenizer.json
//!
//! # Standalone WIT component build (wasm32-wasip1):
//! cargo build \
//!     --manifest-path crates/stringcheese-tokenizer-component-whisper/Cargo.toml \
//!     --target wasm32-wasip1 \
//!     --features wit-component,parity-real-vocab \
//!     --release
//!
//! # Host-side smoke test (native, no wasm toolchain needed):
//! cargo test \
//!     --manifest-path crates/stringcheese-tokenizer-component-whisper/Cargo.toml \
//!     --features parity-real-vocab
//! ```

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
mod bindings;
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

mod runtime;

pub use runtime::{
    WhisperCapabilities, WhisperEncoding, WhisperTokenizerError, count, decode, encode,
    get_capabilities, is_real_vocab,
};

#[cfg(test)]
mod tests {
    /// The WIT source that ships with the repo, embedded so a
    /// regression to either the WIT source or the parser version
    /// surfaces on `cargo test` before the component build ever
    /// runs.
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
    #[cfg(not(stringcheese_whisper_real_vocab))]
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

    #[test]
    fn capabilities_report_reasonable_shape() {
        let caps = super::get_capabilities();
        assert_eq!(caps.model_type, "bpe");
        assert_eq!(caps.variant_id, "openai-whisper");
    }
}
