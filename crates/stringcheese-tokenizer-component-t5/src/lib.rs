//! # Real-vocab Google T5 `tegmentum:tokenizer@0.1.0` component
//!
//! Sibling of
//! [`stringcheese_tokenizer_component_whisper`](https://docs.rs/stringcheese-tokenizer-component-whisper)
//! that layers Google T5's real `tokenizer.json` (SentencePiece
//! Unigram) into the same WIT `tokenizer-provider` shape.  The
//! cognition-side `cognition-t5-component-ort`
//! (`~/git/cognition/crates/cognition-t5-component-ort/`) imports
//! `tegmentum:tokenizer/tokenizer@0.1.0`; composing that import
//! against the artifact built here produces a fully-satisfied
//! `dist/text-generation.t5.wasm` that encodes English prompts +
//! decodes T5's generated ids into text.
//!
//! ## Build-time contract
//!
//! * The `tokenizer.json` bytes are **never** committed.
//!   `build.rs` locates them at build time from
//!   `$STRINGCHEESE_T5_TOKENIZER_JSON` or the standard cache path.
//!   Fetch from HF Hub (Xenova mirror ships the ONNX-compatible
//!   tokenizer.json):
//!
//!   ```text
//!   mkdir -p ~/.cache/stringcheese-tokenizer-t5 && \
//!   curl -L -o ~/.cache/stringcheese-tokenizer-t5/tokenizer.json \
//!       https://huggingface.co/Xenova/t5-small/resolve/main/tokenizer.json
//!   ```
//!
//!   Every T5 checkpoint in the base line (T5-small / T5-base /
//!   T5-large) ships the same 32100-token SentencePiece vocab, so
//!   `t5-small` works for `t5-large` too.  T5-1.1 / mT5 use larger
//!   vocabs — those would need their own pack (or an env-var
//!   override to point at a different tokenizer.json).
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
//! Fourth real-vocab variant in the stringcheese tokenizer-provider
//! family, joining the demo `stringcheese-tokenizer-component`,
//! `stringcheese-tokenizer-component-cl100k` (real cl100k),
//! and `stringcheese-tokenizer-component-whisper` (real whisper
//! BPE).  Structural difference from the others: T5's model is
//! Unigram (SentencePiece), not BPE, so the runtime reaches for
//! `stringcheese_tokenizer_hf::hf::to_unigram_tokenizer` +
//! `hf::UnigramTokenizer` instead of the BPE materializer.
//!
//! ## Build recipe
//!
//! ```text
//! # Fetch the tokenizer.json once:
//! mkdir -p ~/.cache/stringcheese-tokenizer-t5
//! curl -L -o ~/.cache/stringcheese-tokenizer-t5/tokenizer.json \
//!     https://huggingface.co/Xenova/t5-small/resolve/main/tokenizer.json
//!
//! # Standalone WIT component build (wasm32-wasip1):
//! cargo build \
//!     --manifest-path crates/stringcheese-tokenizer-component-t5/Cargo.toml \
//!     --target wasm32-wasip1 \
//!     --features wit-component,parity-real-vocab \
//!     --release
//!
//! # Host-side smoke test (native, no wasm toolchain needed):
//! cargo test \
//!     --manifest-path crates/stringcheese-tokenizer-component-t5/Cargo.toml \
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
    T5Capabilities, T5Encoding, T5TokenizerError, count, decode, encode,
    get_capabilities, is_real_vocab,
};

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

    /// The stub path — verifiable without the real vocab
    /// feature.  Every call must surface a diagnostic that
    /// names the feature gate so a caller who forgot to enable
    /// it sees the message immediately.
    #[cfg(not(stringcheese_t5_real_vocab))]
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
        assert_eq!(caps.model_type, "unigram");
        assert_eq!(caps.variant_id, "google-t5");
    }

    /// Real-vocab paths — only exercised when the crate was
    /// built with the T5 tokenizer.json embedded.  Under the
    /// stub build these tests are cfg'd out entirely.
    #[cfg(stringcheese_t5_real_vocab)]
    mod real_vocab {
        #[test]
        fn capabilities_report_populated_vocab_size() {
            let caps = super::super::get_capabilities();
            assert!(super::super::is_real_vocab());
            assert!(caps.has_special_tokens, "T5 vocab includes </s>, <pad>, <unk> + extra_ids");
            // T5-small ships a 32100-token vocab.  The check
            // stays inclusive so a bigger T5 variant (T5-1.1,
            // mT5) plugged via env var still passes.
            assert!(
                caps.vocab_size >= 32000,
                "unexpectedly small T5 vocab: {}",
                caps.vocab_size
            );
        }

        /// Round-trip a small English phrase.  T5's Unigram
        /// tokenizer normalizes internally (strip leading
        /// whitespace, insert SentencePiece word-marker
        /// spaces), so the decoded string may differ from the
        /// original by whitespace layout — we just check the
        /// content survives.
        #[test]
        fn encode_then_decode_preserves_content() {
            let enc = super::super::encode("hello world")
                .expect("encode succeeds under real vocab");
            assert!(!enc.ids.is_empty(), "expected at least one token id");
            let decoded = super::super::decode(&enc.ids)
                .expect("decode succeeds under real vocab");
            let lower = decoded.to_lowercase();
            assert!(lower.contains("hello"), "decoded {decoded:?} lost 'hello'");
            assert!(lower.contains("world"), "decoded {decoded:?} lost 'world'");
        }

        /// `count(text)` must match `encode(text).ids.len()`.
        #[test]
        fn count_matches_encode_len() {
            let text = "translate English to German: the weather is nice today.";
            let n = super::super::count(text).expect("count succeeds");
            let enc = super::super::encode(text).expect("encode succeeds");
            assert_eq!(n as usize, enc.ids.len());
        }
    }
}
