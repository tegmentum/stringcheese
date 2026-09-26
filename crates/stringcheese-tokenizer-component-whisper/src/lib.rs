//! # Real-vocab OpenAI Whisper `tegmentum:tokenizer@0.1.0` component
//!
//! Sibling of
//! [`stringcheese_tokenizer_component_cl100k`](https://docs.rs/stringcheese-tokenizer-component-cl100k)
//! that layers OpenAI Whisper's real `tokenizer.json` into the same
//! WIT `tokenizer-provider` shape.
//!
//! ## Status (2026-09-25)
//!
//! **The cognition workspace no longer consumes this crate.**
//! `whisper-transcribe-component` migrated to the runtime-bytes
//! `tegmentum:tokenizer/tokenizer-source@0.1.0` interface satisfied
//! by [`stringcheese-tokenizer-component-hf-source`] on 2026-09-25
//! after this crate's compose-time vocab bake silently drifted from
//! the model bundle at runtime.  See the cognition-side memory
//! `whisper_tokenizer_vocab_mismatch.md` for the failure mode.
//!
//! [`stringcheese-tokenizer-component-hf-source`]: ../stringcheese_tokenizer_component_hf_source/index.html
//!
//! New consumers should plug the runtime-bytes satisfier instead;
//! the guest reads `tokenizer.json` from the model bundle's
//! metadata surface at call time, so vocabulary and model always
//! match by construction.  This crate remains available for
//! non-cognition callers who explicitly want a bake-time vocab,
//! but bake-time vs runtime mismatch has bitten twice — see the
//! caveat below.
//!
//! ## Build-time contract
//!
//! * The `tokenizer.json` bytes are **never** committed.  `build.rs`
//!   locates them at build time from `$STRINGCHEESE_WHISPER_TOKENIZER_JSON`
//!   or the standard cache path.  Fetch from HF Hub matching your
//!   target model family:
//!
//!   ```text
//!   mkdir -p ~/.cache/stringcheese-tokenizer-whisper && \
//!   curl -L -o ~/.cache/stringcheese-tokenizer-whisper/tokenizer.json \
//!       https://huggingface.co/openai/whisper-tiny.en/resolve/main/tokenizer.json
//!   # (or openai/whisper-tiny for multilingual models — but see the
//!   #  vocab-family caveat below)
//!   ```
//!
//! ## Vocab-family caveat
//!
//! Contrary to an earlier claim in this doc, **multilingual and .en
//! whisper checkpoints do NOT share a tokenizer**:
//!
//! | Token ID | `openai/whisper-tiny` (multilingual) | `openai/whisper-tiny.en` |
//! |----------|--------------------------------------|--------------------------|
//! | 843      | `'ody'`                              | `'ĠAnd'`                 |
//! | 5891     | `'Ġviel'` (German)                   | `'Ġfellow'`              |
//! | 50256    | `''`                                 | `'<|endoftext|>'`        |
//! | 50257    | `'<|endoftext|>'`                    | `'<|startoftranscript|>'`|
//! | vocab    | 50258                                | 50257                    |
//!
//! Pointing this build at the wrong family produces a composed
//! artifact that argmaxes correct token IDs and detokenizes them
//! as scrambled real-vocab words in the wrong language.  Compose
//! validation still passes (the WIT shape is correct); only the
//! transcript is garbage.
//!
//! Within a family (`.en` variants share, multilingual variants
//! share), any checkpoint's tokenizer.json works — `tiny.en`
//! matches `small.en` matches `medium.en`.  Across the family
//! boundary they don't.
//!
//! The runtime-bytes route via
//! `stringcheese-tokenizer-component-hf-source` avoids this
//! entirely — the vocab travels with the weights.
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
//! # Fetch the tokenizer.json once (choose the URL matching your
//! # target model family — see the vocab-family caveat above):
//! mkdir -p ~/.cache/stringcheese-tokenizer-whisper
//! curl -L -o ~/.cache/stringcheese-tokenizer-whisper/tokenizer.json \
//!     https://huggingface.co/openai/whisper-tiny.en/resolve/main/tokenizer.json
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
