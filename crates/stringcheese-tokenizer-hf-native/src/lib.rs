//! Native-only adapter that delegates to Hugging Face's own
//! [`tokenizers`](https://docs.rs/tokenizers) crate for byte-identical
//! parity with upstream `tokenizer.json` semantics.
//!
//! # Why this crate exists
//!
//! The wasm-friendly runtime in
//! [`stringcheese-tokenizer-hf`](https://docs.rs/stringcheese-tokenizer-hf)
//! is a from-scratch reimplementation of the Hugging Face tokenizer
//! spec — no C dependencies, no `onig`, no `esaxx-rs`, no host-only
//! side channels; every subword pipeline shape (BPE, `WordPiece`,
//! Unigram, `WordLevel`) plus the normalizer / pre-tokenizer /
//! post-processor / decoder composition are all implemented directly
//! against the JSON on-disk shape. That is the right dependency
//! profile for a wasm-first, offline-first, StringCheese-surface-
//! integrated toolkit — but there is always a *long tail* of edge
//! cases (Unicode-property regex classes at pre-tokenizer edges,
//! `Precompiled` charsmap byte-for-byte parity on new scripts, subtle
//! decoder interactions) where a caller may prefer byte-identical
//! agreement with HF's reference tool over the in-process runtime.
//! This crate is that escape hatch.
//!
//! # Native-only, by construction
//!
//! `tokenizers-rs` pulls in `onig` (a C library — PCRE-shaped regex
//! engine) and `esaxx-rs` (a C++ header-only suffix automaton for
//! Unigram training) on its default feature set. Both are host-only
//! C/C++ code that does not cross-compile to
//! `wasm32-unknown-unknown` without a bespoke toolchain and does not
//! belong on the wasm hot path in the first place. Consequently this
//! crate carries a `#![cfg(not(target_family = "wasm"))]` gate at
//! the top of the file — on wasm targets the whole crate compiles to
//! an empty module. That is also why the workspace's wasm CI jobs
//! `--exclude` this crate.
//!
//! # Design shape
//!
//! * [`HfNativeTokenizer`] wraps an owned [`tokenizers::Tokenizer`]
//!   and implements the toolkit's own
//!   [`stringcheese_tokenizer::Tokenizer`] trait, delegating every
//!   method to the underlying upstream call. The wrap is one field
//!   plus a policy flag; the pipeline shape (normalizer,
//!   pre-tokenizer, model, post-processor, decoder) lives inside the
//!   upstream tokenizer verbatim.
//! * Constructors mirror the upstream API surface:
//!   [`HfNativeTokenizer::from_file`] wraps
//!   `tokenizers::Tokenizer::from_file` and
//!   [`HfNativeTokenizer::from_bytes`] wraps
//!   `tokenizers::Tokenizer::from_bytes`. Both accept an
//!   `add_special_tokens` policy which the trait's `encode` /
//!   `encode_batch` / `encode_pair` methods honour when delegating
//!   to upstream.
//! * The [`Encoding`] shape
//!   mismatch — HF's own [`tokenizers::Encoding`] carries more
//!   parallel arrays than the toolkit's compact shape does — is
//!   handled at the encode-side boundary: `ids`, `offsets`,
//!   `special_tokens_mask`, `type_ids`, and `attention_mask` all
//!   map straight across; HF's extra arrays (`tokens`, `word_ids`,
//!   `overflowing`, etc.) are dropped. Callers who need those extra
//!   arrays should reach into the wrapped tokenizer directly via
//!   [`HfNativeTokenizer::inner`].
//!
//! # Trade-off
//!
//! What you get: byte-identical parity with `python -m tokenizers`
//! and `transformers.AutoTokenizer` on every `tokenizer.json` in the
//! wild, without waiting on the wasm-friendly runtime to catch up on
//! the long tail.
//!
//! What you give up: a native-only build (no wasm at all), a much
//! larger dependency graph (tokenizers-rs plus its C dependencies
//! and their build tooling), and no in-process portability. If your
//! deployment shape is a native, online, host-privileged binary,
//! that trade-off is often the right one — this crate makes it a
//! one-line dependency addition.

#![cfg(not(target_family = "wasm"))]
#![forbid(unsafe_code)]

use std::path::Path;

use stringcheese_tokenizer::{Encoding, Tokenizer, TokenizerError};
use tokenizers::{EncodeInput, Encoding as HfEncoding, Tokenizer as HfLibTokenizer};

/// Convert an upstream `tokenizers::Encoding` into the toolkit's
/// `Encoding<u32>` shape.
///
/// The five arrays the toolkit's encoding carries map straight across
/// from HF's own encoding:
///
/// * `ids` ← `Encoding::get_ids` (verbatim `Vec<u32>` copy).
/// * `offsets` ← `Encoding::get_offsets` converted from HF's
///   `(usize, usize)` tuple shape to our half-open `Range<usize>`
///   shape. Note that HF reports offsets in the *pre-tokenizer's*
///   output-space bytes (i.e. bytes into the *normalized* input, not
///   the caller's raw input); when the tokenizer's normalizer is
///   identity the two coincide, but a NFKC / lowercase / Precompiled
///   normalizer breaks the equivalence. This mirror stays literal —
///   the reason to reach for this adapter over
///   `stringcheese-tokenizer-hf` is *byte-identical parity with
///   HF*, and remapping offsets would defeat the point.
/// * `special_mask` ← `Encoding::get_special_tokens_mask`
///   converted from HF's `u32`-per-token flag (0 or 1) to a `bool`.
/// * `type_ids` ← `Encoding::get_type_ids` verbatim
///   (`0` for the `A` slot of a pair, `1` for the `B` slot; `0` for
///   single-input encodes).
/// * `attention_mask` ← `Encoding::get_attention_mask` converted
///   from HF's `u32`-per-token flag (0 or 1) to a `bool` (`true` for
///   real tokens, `false` for pad tokens).
///
/// HF's extra parallel arrays — `tokens` (the surface strings),
/// `word_ids` (the word-index-in-input per token), and `overflowing`
/// (the truncation overflow encodings) — are dropped. Callers who
/// need them should reach into
/// [`HfNativeTokenizer::inner`] and call the upstream API directly.
fn convert_encoding(enc: &HfEncoding) -> Encoding<u32> {
    let ids = enc.get_ids().to_vec();
    let offsets = enc.get_offsets().iter().map(|(s, e)| *s..*e).collect();
    let special_mask = enc
        .get_special_tokens_mask()
        .iter()
        .map(|&b| b != 0)
        .collect();
    let type_ids = enc.get_type_ids().to_vec();
    let attention_mask = enc.get_attention_mask().iter().map(|&b| b != 0).collect();
    Encoding {
        ids,
        offsets,
        special_mask,
        type_ids,
        attention_mask,
    }
}

/// Toolkit-facing wrapper around an upstream
/// [`tokenizers::Tokenizer`].
///
/// See the crate-level documentation for the design rationale and the
/// wasm-target policy.
///
/// # Example
///
/// ```no_run
/// use stringcheese_tokenizer::Tokenizer;
/// use stringcheese_tokenizer_hf_native::HfNativeTokenizer;
///
/// let tok = HfNativeTokenizer::from_file("tokenizer.json").unwrap();
/// let enc = tok.encode("Hello, world!").unwrap();
/// let _ids: &[u32] = &enc.ids;
/// ```
pub struct HfNativeTokenizer {
    inner: HfLibTokenizer,
    add_special_tokens: bool,
}

impl HfNativeTokenizer {
    /// Load a tokenizer from a `tokenizer.json` file on disk.
    ///
    /// Delegates to `tokenizers::Tokenizer::from_file`. Every
    /// pipeline shape the upstream crate supports at its current
    /// release is honoured — this crate does not filter or
    /// re-validate the config.
    ///
    /// # Errors
    ///
    /// Returns [`TokenizerError::Other`] with a static message when
    /// the upstream loader fails (missing file, malformed JSON,
    /// unsupported model type, etc.). The upstream error's own
    /// diagnostic string is lost at the crate boundary because the
    /// shared [`TokenizerError`] variants do not carry an owned
    /// message payload; callers who need the full upstream error
    /// should call `tokenizers::Tokenizer::from_file` directly and
    /// wrap the resulting tokenizer with
    /// [`HfNativeTokenizer::from_inner`].
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, TokenizerError> {
        let inner = HfLibTokenizer::from_file(path)
            .map_err(|_| TokenizerError::Other("hf-native: failed to load tokenizer.json"))?;
        Ok(Self::from_inner(inner))
    }

    /// Load a tokenizer from a `tokenizer.json` byte blob.
    ///
    /// Delegates to `tokenizers::Tokenizer::from_bytes`. Convenient
    /// for embedding a config at compile time via [`include_bytes!`]
    /// or for loading from a caller-managed cache without a
    /// filesystem detour.
    ///
    /// # Errors
    ///
    /// Returns [`TokenizerError::Other`] with a static message when
    /// the upstream loader fails. See
    /// [`HfNativeTokenizer::from_file`] for the caveat on lost
    /// upstream diagnostic strings.
    pub fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, TokenizerError> {
        let inner = HfLibTokenizer::from_bytes(bytes.as_ref()).map_err(|_| {
            TokenizerError::Other("hf-native: failed to parse tokenizer.json bytes")
        })?;
        Ok(Self::from_inner(inner))
    }

    /// Wrap an already-constructed upstream tokenizer.
    ///
    /// Useful when the caller wants to configure the upstream
    /// tokenizer's normalizer / pre-tokenizer / post-processor /
    /// decoder programmatically (via the upstream builder API) rather
    /// than through a `tokenizer.json` blob, and then hand the
    /// finished value to this crate for the trait-shape delegation.
    #[must_use]
    pub fn from_inner(inner: HfLibTokenizer) -> Self {
        Self {
            inner,
            add_special_tokens: true,
        }
    }

    /// Configure whether encoding calls request the upstream
    /// tokenizer's post-processor add its registered special tokens
    /// (BOS / EOS / CLS / SEP / …). Defaults to `true`.
    ///
    /// The value maps onto the `add_special_tokens` argument the
    /// upstream `encode` / `encode_batch` / `encode` methods take.
    /// Set to `false` for parity with tiktoken-shape "raw BPE output
    /// only" callers.
    #[must_use]
    pub fn with_add_special_tokens(mut self, add: bool) -> Self {
        self.add_special_tokens = add;
        self
    }

    /// Whether encoding calls request the upstream post-processor add
    /// its registered special tokens. See
    /// [`HfNativeTokenizer::with_add_special_tokens`] for the
    /// semantics.
    #[must_use]
    pub fn add_special_tokens(&self) -> bool {
        self.add_special_tokens
    }

    /// Borrow the wrapped upstream tokenizer.
    ///
    /// Useful for reaching into the parallel arrays the toolkit's
    /// [`Encoding`] shape drops
    /// (surface `tokens`, `word_ids`, `overflowing`) or for calling
    /// upstream methods this adapter does not surface (adding
    /// tokens, mutating the pipeline, saving back out).
    #[must_use]
    pub fn inner(&self) -> &HfLibTokenizer {
        &self.inner
    }

    /// Consume the wrapper and return the owned upstream tokenizer.
    #[must_use]
    pub fn into_inner(self) -> HfLibTokenizer {
        self.inner
    }
}

impl core::fmt::Debug for HfNativeTokenizer {
    // `tokenizers::Tokenizer` does not implement `Debug`, so print a
    // marker plus the policy flag rather than derive.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HfNativeTokenizer")
            .field("add_special_tokens", &self.add_special_tokens)
            .field("inner", &"<tokenizers::Tokenizer>")
            .finish()
    }
}

impl Tokenizer for HfNativeTokenizer {
    type Token = u32;

    /// Encode `text` through the wrapped upstream tokenizer.
    ///
    /// Delegates to `tokenizers::Tokenizer::encode` with the
    /// configured `add_special_tokens` policy. See the crate-level
    /// docs' "Design shape" section for the encoding-shape map.
    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        let input: EncodeInput<'_> = text.into();
        let enc = self
            .inner
            .encode(input, self.add_special_tokens)
            .map_err(|_| TokenizerError::Other("hf-native: encode failed"))?;
        Ok(convert_encoding(&enc))
    }

    /// Decode a sequence of token ids back into a string through the
    /// wrapped upstream tokenizer.
    ///
    /// Delegates to `tokenizers::Tokenizer::decode` with
    /// `skip_special_tokens = false` so the decoded output is a full
    /// round-trip of the ids (special tokens included). Callers who
    /// want the strip-specials behaviour should reach into
    /// [`HfNativeTokenizer::inner`] and call upstream directly.
    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError> {
        self.inner
            .decode(tokens, false)
            .map_err(|_| TokenizerError::Other("hf-native: decode failed"))
    }

    /// Count the tokens in an encode of `text`, without allocating the
    /// per-token parallel arrays the full [`encode`](Tokenizer::encode)
    /// path materialises.
    ///
    /// The upstream crate does not expose a count-only fast path, so
    /// this override calls `encode` and returns `.get_ids().len()`,
    /// dropping the encoding value before the trait-side conversion
    /// runs. Same behaviour as the default implementation modulo
    /// skipping the encoding-shape conversion copy.
    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        let input: EncodeInput<'_> = text.into();
        let enc = self
            .inner
            .encode(input, self.add_special_tokens)
            .map_err(|_| TokenizerError::Other("hf-native: encode failed"))?;
        Ok(enc.get_ids().len())
    }

    /// Encode a batch of inputs through the upstream parallel batch
    /// path.
    ///
    /// Delegates to `tokenizers::Tokenizer::encode_batch`, which
    /// runs the encodes in parallel through `rayon` on the host
    /// thread pool. Returns encodings in the same order as `inputs`.
    fn encode_batch(&self, inputs: &[&str]) -> Result<Vec<Encoding<Self::Token>>, TokenizerError> {
        let owned: Vec<EncodeInput<'_>> = inputs.iter().map(|s| (*s).into()).collect();
        let encs = self
            .inner
            .encode_batch(owned, self.add_special_tokens)
            .map_err(|_| TokenizerError::Other("hf-native: encode_batch failed"))?;
        Ok(encs.iter().map(convert_encoding).collect())
    }

    /// Encode a `(question, context)` style pair through the upstream
    /// post-processor's pair template.
    ///
    /// Delegates to `tokenizers::Tokenizer::encode` with an
    /// [`EncodeInput`]`::Dual` constructed from `(a, b)`. When the
    /// wrapped tokenizer has a pair template
    /// configured (BERT / `DistilBERT` / `RoBERTa` /
    /// `XLM-RoBERTa` post-processors) the pair template is applied
    /// verbatim upstream; when it does not, upstream falls back to
    /// concatenating the two encodings and tagging the second half
    /// with `type_ids = 1`.
    fn encode_pair(&self, a: &str, b: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        let input: EncodeInput<'_> = (a, b).into();
        let enc = self
            .inner
            .encode(input, self.add_special_tokens)
            .map_err(|_| TokenizerError::Other("hf-native: encode_pair failed"))?;
        Ok(convert_encoding(&enc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal but shape-representative BPE `tokenizer.json` blob:
    /// one tiny vocab, one merge, no pre-tokenizer, no post-processor.
    /// Small enough to embed here (well under any "no real vocab bytes"
    /// policy) and rich enough to exercise every trait method.
    const MINIMAL_BPE_JSON: &str = r#"{
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": [],
        "normalizer": null,
        "pre_tokenizer": null,
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "BPE",
            "dropout": null,
            "unk_token": null,
            "continuing_subword_prefix": null,
            "end_of_word_suffix": null,
            "fuse_unk": false,
            "byte_fallback": false,
            "ignore_merges": false,
            "vocab": {"a": 0, "b": 1, "c": 2, "ab": 3, "abc": 4},
            "merges": [["a", "b"], ["ab", "c"]]
        }
    }"#;

    /// A `WordLevel` + Whitespace pre-tokenizer combination exercises
    /// the pre-tokenizer regex machinery so we know the `onig`-backed
    /// path is actually linked and running.
    const MINIMAL_WORDLEVEL_JSON: &str = r#"{
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": [],
        "normalizer": null,
        "pre_tokenizer": {
            "type": "Whitespace"
        },
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordLevel",
            "vocab": {
                "[UNK]": 0,
                "Hello": 1,
                ",": 2,
                "world": 3,
                "!": 4,
                "the": 5,
                "quick": 6,
                "brown": 7,
                "fox": 8
            },
            "unk_token": "[UNK]"
        }
    }"#;

    #[test]
    fn from_bytes_loads_a_minimal_bpe_config() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes()).unwrap();
        // Every trait method is available; sanity-check `count`.
        assert_eq!(Tokenizer::count(&tok, "abc").unwrap(), 1);
    }

    #[test]
    fn encode_produces_populated_arrays() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);

        // `abc` is in the vocab and both `("a","b")` and `("ab","c")`
        // merges apply, so the merge loop collapses to a single token
        // id 4.
        let enc = Tokenizer::encode(&tok, "abc").unwrap();
        assert_eq!(enc.ids, vec![4]);
        // Offsets always populated for the default pipeline (HF's own
        // encoder tracks byte offsets for every token).
        assert_eq!(enc.offsets.len(), enc.ids.len());
        assert_eq!(enc.offsets[0], 0..3);
        // Special mask / attention mask / type ids all populated in
        // lockstep with `ids`.
        assert_eq!(enc.special_mask.len(), enc.ids.len());
        assert_eq!(enc.type_ids.len(), enc.ids.len());
        assert_eq!(enc.attention_mask.len(), enc.ids.len());
        // Every entry is a real token; nothing padded, nothing special.
        assert!(enc.attention_mask.iter().all(|&b| b));
        assert!(enc.special_mask.iter().all(|&b| !b));
        assert!(enc.type_ids.iter().all(|&t| t == 0));
    }

    #[test]
    fn encode_then_decode_round_trips_when_the_input_collapses_to_a_single_token() {
        // Note: upstream `tokenizers-rs`'s default decode path for a
        // config with `"decoder": null` joins tokens with a space
        // (mirroring HF's own default WordPiece decoder), so
        // multi-token round-trips break for a `MINIMAL_BPE_JSON`
        // that ships no explicit `ByteLevel` / `BPEDecoder` block.
        // Round-tripping is a promise the underlying tokenizer makes
        // (via its configured decoder), not one this adapter can add;
        // the assertion below covers the well-defined single-token
        // case and the parity-vs-upstream tests below cover the full
        // encode contract. See the module-level docs for the
        // adapter's role: byte-identical delegation, not additional
        // round-trip machinery.
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);
        for input in ["a", "b", "c", "ab", "abc"] {
            let enc = Tokenizer::encode(&tok, input).unwrap();
            assert_eq!(enc.ids.len(), 1, "input {input:?} did not collapse");
            let out = Tokenizer::decode(&tok, &enc.ids).unwrap();
            assert_eq!(out, input, "round-trip failed for input {input:?}");
        }
    }

    #[test]
    fn encode_a_wordlevel_hello_world() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_WORDLEVEL_JSON.as_bytes()).unwrap();
        let enc = Tokenizer::encode(&tok, "Hello , world !").unwrap();
        // Whitespace pre-tokenizer splits on runs of whitespace and on
        // ASCII punctuation boundaries; the vocab covers every piece.
        assert_eq!(enc.ids, vec![1, 2, 3, 4]);
        assert_eq!(enc.offsets.len(), 4);
    }

    #[test]
    fn count_matches_encode_len() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_WORDLEVEL_JSON.as_bytes()).unwrap();
        for input in [
            "",
            "Hello",
            "Hello , world !",
            "the quick brown fox",
            "the quick brown fox Hello world",
        ] {
            let n = Tokenizer::count(&tok, input).unwrap();
            let m = Tokenizer::encode(&tok, input).unwrap().ids.len();
            assert_eq!(n, m, "count vs encode disagreement on {input:?}");
        }
    }

    #[test]
    fn encode_batch_returns_one_encoding_per_input_in_order() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);
        let inputs = ["a", "abc", "ababc"];
        let inputs_ref: Vec<&str> = inputs.to_vec();
        let batch = Tokenizer::encode_batch(&tok, &inputs_ref).unwrap();
        assert_eq!(batch.len(), inputs.len());
        for (i, input) in inputs.iter().enumerate() {
            let expected = Tokenizer::encode(&tok, input).unwrap();
            assert_eq!(
                batch[i].ids, expected.ids,
                "batch[{i}] vs sequential encode disagreement"
            );
        }
    }

    #[test]
    fn encode_pair_tags_the_second_half_type_id_1() {
        // No post-processor: HF's own dual-encode path just concatenates
        // the two encodings and tags them with `type_ids = [0.., 1..]`.
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);
        let enc = Tokenizer::encode_pair(&tok, "abc", "ab").unwrap();
        // "abc" collapses to id 4; "ab" collapses to id 3. Concatenated
        // in order.
        assert_eq!(enc.ids, vec![4, 3]);
        assert_eq!(enc.type_ids, vec![0, 1]);
    }

    /// Byte-identical parity vs the upstream `tokenizers-rs` crate's
    /// own `encode` output over a small corpus of diverse inputs — the
    /// load-bearing contract of this crate.
    #[test]
    fn parity_vs_upstream_bpe() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);
        let upstream = HfLibTokenizer::from_bytes(MINIMAL_BPE_JSON.as_bytes()).unwrap();
        for input in [
            "",
            "a",
            "b",
            "c",
            "ab",
            "abc",
            "aab",
            "aabc",
            "ababc",
            "aabbcc",
            "abcabcabc",
            "cba",
            "cab",
            "acb",
            "bac",
            "bca",
            "abababab",
            "cccc",
            "aaaa",
            "bbbb",
        ] {
            let ours = Tokenizer::encode(&tok, input).unwrap().ids;
            let theirs: Vec<u32> = {
                let hf_input: EncodeInput<'_> = input.into();
                upstream.encode(hf_input, false).unwrap().get_ids().to_vec()
            };
            assert_eq!(
                ours, theirs,
                "byte-identical parity broke on input {input:?}"
            );
        }
    }

    /// Byte-identical parity vs upstream on the `WordLevel` + Whitespace
    /// pre-tokenizer path — exercises the `onig`-backed pre-tokenizer
    /// so a link-time drop of the C dep would surface as a test
    /// failure rather than pass silently.
    #[test]
    fn parity_vs_upstream_wordlevel() {
        let tok = HfNativeTokenizer::from_bytes(MINIMAL_WORDLEVEL_JSON.as_bytes()).unwrap();
        let upstream = HfLibTokenizer::from_bytes(MINIMAL_WORDLEVEL_JSON.as_bytes()).unwrap();
        for input in [
            "",
            "Hello",
            "Hello , world !",
            "the quick brown fox",
            "the quick brown fox Hello world",
            "  Hello   ,   world   !  ",
        ] {
            let ours = Tokenizer::encode(&tok, input).unwrap().ids;
            let theirs: Vec<u32> = {
                let hf_input: EncodeInput<'_> = input.into();
                upstream.encode(hf_input, true).unwrap().get_ids().to_vec()
            };
            assert_eq!(
                ours, theirs,
                "byte-identical parity broke on input {input:?}"
            );
        }
    }

    #[test]
    fn from_file_reports_missing_file() {
        let err = HfNativeTokenizer::from_file("/nonexistent/tokenizer.json").unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }

    #[test]
    fn from_bytes_reports_malformed_json() {
        let err = HfNativeTokenizer::from_bytes(b"{ not json").unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }
}
