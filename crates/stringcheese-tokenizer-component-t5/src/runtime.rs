//! Runtime bridge between the embedded T5 `tokenizer.json` and
//! the WIT boundary.
//!
//! Two code paths:
//!
//! * **Real vocab (`cfg(stringcheese_t5_real_vocab)`)** — parses
//!   the embedded JSON via
//!   [`stringcheese_tokenizer_hf::hf::parse_tokenizer_json`] +
//!   [`stringcheese_tokenizer_hf::hf::to_unigram_tokenizer`],
//!   caches the [`UnigramTokenizer`] in a [`OnceLock`] so the
//!   parse cost is paid once per process.
//! * **Stub (default)** — every operation returns
//!   [`T5TokenizerError::Other`] naming the `parity-real-vocab`
//!   feature.  The capabilities record still reports the crate's
//!   identity (`variant-id = "google-t5"`) so a caller doing
//!   backend dispatch sees the right value even in stub mode.

use alloc::string::String;
use alloc::vec::Vec;

/// Mirror of the WIT `encoding` record.  All non-id arrays
/// are empty under the Unigram path — `stringcheese-tokenizer-
/// hf::UnigramTokenizer` doesn't track offsets / special-mask
/// / type-ids / attention-mask (per the WIT contract "empty
/// when not tracked").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T5Encoding {
    /// Token IDs in emission order.
    pub ids: Vec<u32>,
    /// Half-open byte ranges in the pre-normalization input,
    /// one per token.  Empty — Unigram doesn't track offsets.
    pub offsets: Vec<(u32, u32)>,
    /// One flag per token; `true` iff the id is one of T5's
    /// registered special tokens.  Empty under Unigram.
    pub special_mask: Vec<bool>,
    /// Segment id per token (single-side encode always emits
    /// zero for every token).  Empty under Unigram.
    pub type_ids: Vec<u32>,
    /// `true` for real tokens, `false` for pad tokens.  Empty
    /// under Unigram — populated caller-side via
    /// `pad-batch`.
    pub attention_mask: Vec<bool>,
}

/// Mirror of the WIT `capabilities` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T5Capabilities {
    /// Always `"unigram"` — T5's tokenizer.json declares this
    /// under `model.type` (SentencePiece Unigram).
    pub model_type: &'static str,
    /// Always `"google-t5"`.  Distinct from `variant_id` choices
    /// for other stringcheese variants (`"cl100k_base"`,
    /// `"openai-whisper"`).
    pub variant_id: &'static str,
    /// Semver of this crate.
    pub version: &'static str,
    /// Total number of ids in the vocabulary.  `0` under stub;
    /// under real vocab, `32100` for T5-small / T5-base / T5-large
    /// (the shared SentencePiece vocab across the base T5 line;
    /// T5-1.1 / mT5 use larger vocabs — the exact count is read
    /// from the live tokenizer, so a future re-baseline reflects
    /// immediately).
    pub vocab_size: u32,
    /// `false`.  T5's Unigram vocab includes an `<unk>` token
    /// (id 2 in the canonical T5 vocab) but does not use the
    /// 256-byte reserved fallback that Llama / Mistral do.  The
    /// SentencePiece byte_fallback flag is disabled in Xenova's
    /// T5 tokenizer.json export.
    pub has_byte_fallback: bool,
    /// `true`.  The vocab registers T5's `<pad>` (id 0), `</s>`
    /// (id 1), `<unk>` (id 2) plus 100 `<extra_id_N>` sentinel
    /// tokens used by the span-corruption pretraining objective.
    pub has_special_tokens: bool,
}

/// Mirror of the WIT `tokenizer-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum T5TokenizerError {
    /// Input carried a byte sequence that isn't valid UTF-8.
    InvalidUtf8,
    /// A surface string or token id was not present in the
    /// vocabulary + no fallback is configured.
    UnknownToken(String),
    /// A special-token surface string appeared in the input
    /// that the caller's policy rejects.
    DisallowedSpecialToken(String),
    /// The vocabulary is internally inconsistent (merge table
    /// / vocab / special-token map mismatch).
    VocabularyMismatch,
    /// Any other domain-specific failure that doesn't fit the
    /// fixed variants.  Payload carries a short message.
    Other(String),
}

/// `true` iff this build has T5's real tokenizer.json embedded.
#[must_use]
pub const fn is_real_vocab() -> bool {
    cfg!(stringcheese_t5_real_vocab)
}

#[must_use]
pub fn get_capabilities() -> T5Capabilities {
    T5Capabilities {
        model_type: "unigram",
        variant_id: "google-t5",
        version: env!("CARGO_PKG_VERSION"),
        vocab_size: real_vocab_size(),
        has_byte_fallback: false,
        has_special_tokens: cfg!(stringcheese_t5_real_vocab),
    }
}

/// Encode `text` into a metadata-carrying [`T5Encoding`].
/// Under stub mode returns [`T5TokenizerError::Other`] naming
/// the `parity-real-vocab` feature.
pub fn encode(text: &str) -> Result<T5Encoding, T5TokenizerError> {
    real::encode_impl(text)
}

/// Decode a sequence of ids back into the SentencePiece
/// surface form.  Under stub mode returns
/// [`T5TokenizerError::Other`] naming `parity-real-vocab`.
pub fn decode(ids: &[u32]) -> Result<String, T5TokenizerError> {
    real::decode_impl(ids)
}

/// Count the tokens `encode(text)` would produce, skipping
/// the offset / mask synthesis.  Under stub mode returns
/// [`T5TokenizerError::Other`] naming `parity-real-vocab`.
pub fn count(text: &str) -> Result<u32, T5TokenizerError> {
    real::count_impl(text)
}

#[cfg(stringcheese_t5_real_vocab)]
fn real_vocab_size() -> u32 {
    real::with_tokenizer(|tok| u32::try_from(tok.vocab().len()).unwrap_or(u32::MAX)).unwrap_or(0)
}

#[cfg(not(stringcheese_t5_real_vocab))]
fn real_vocab_size() -> u32 {
    0
}

// ---------------------------------------------------------------------
// Real-vocab path.  Compiles when build.rs staged tokenizer.json
// and emitted --cfg=stringcheese_t5_real_vocab.
// ---------------------------------------------------------------------

#[cfg(stringcheese_t5_real_vocab)]
mod real {
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::str;
    use std::sync::OnceLock;

    use stringcheese_tokenizer_hf::hf;
    use stringcheese_tokenizer_hf::hf::UnigramTokenizer;

    use super::{T5Encoding, T5TokenizerError};

    /// The embedded tokenizer.json.  build.rs writes real bytes
    /// to `$OUT_DIR/tokenizer.json` when the resolution walk
    /// finds a non-empty file; under this cfg the bytes are
    /// guaranteed non-empty.
    const TOKENIZER_JSON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tokenizer.json"));

    static TOKENIZER: OnceLock<Result<UnigramTokenizer, String>> = OnceLock::new();

    fn get() -> Result<&'static UnigramTokenizer, T5TokenizerError> {
        let entry = TOKENIZER.get_or_init(build);
        match entry {
            Ok(tok) => Ok(tok),
            Err(msg) => Err(T5TokenizerError::Other(msg.clone())),
        }
    }

    fn build() -> Result<UnigramTokenizer, String> {
        let json = str::from_utf8(TOKENIZER_JSON)
            .map_err(|e| alloc::format!("T5 tokenizer.json is not valid UTF-8: {e}"))?;
        let cfg = hf::parse_tokenizer_json(json)
            .map_err(|e| alloc::format!("parse tokenizer.json: {e:?}"))?;
        hf::to_unigram_tokenizer(&cfg)
            .map_err(|e| alloc::format!("materialize UnigramTokenizer: {e:?}"))
    }

    pub(super) fn with_tokenizer<T>(f: impl FnOnce(&UnigramTokenizer) -> T) -> Option<T> {
        get().ok().map(f)
    }

    pub(super) fn encode_impl(text: &str) -> Result<T5Encoding, T5TokenizerError> {
        let tok = get()?;
        // UnigramTokenizer::encode returns Vec<usize> (raw ids).
        // The WIT Encoding record carries per-token metadata
        // arrays (offsets / special_mask / type_ids /
        // attention_mask) — Unigram doesn't track those, so
        // per the WIT contract ("empty when not tracked") each
        // is emitted as an empty vec.
        let raw_ids = tok
            .encode(text)
            .map_err(|e| T5TokenizerError::Other(alloc::format!("unigram encode: {e:?}")))?;
        let ids: Vec<u32> = raw_ids
            .into_iter()
            .map(|id| u32::try_from(id).unwrap_or(u32::MAX))
            .collect();
        Ok(T5Encoding {
            ids,
            offsets: Vec::new(),
            special_mask: Vec::new(),
            type_ids: Vec::new(),
            attention_mask: Vec::new(),
        })
    }

    pub(super) fn decode_impl(ids: &[u32]) -> Result<String, T5TokenizerError> {
        let tok = get()?;
        let ids_usize: Vec<usize> = ids.iter().map(|&id| id as usize).collect();
        tok.decode(&ids_usize)
            .map_err(|e| T5TokenizerError::Other(alloc::format!("unigram decode: {e:?}")))
    }

    pub(super) fn count_impl(text: &str) -> Result<u32, T5TokenizerError> {
        let tok = get()?;
        let raw = tok
            .encode(text)
            .map_err(|e| T5TokenizerError::Other(alloc::format!("unigram count: {e:?}")))?;
        Ok(u32::try_from(raw.len()).unwrap_or(u32::MAX))
    }
}

// ---------------------------------------------------------------------
// Stub path.  Compiles when the crate was built without a valid
// tokenizer.json.  Every operation returns the same diagnostic.
// ---------------------------------------------------------------------

#[cfg(not(stringcheese_t5_real_vocab))]
mod real {
    use super::{T5Encoding, T5TokenizerError};

    fn stub_error() -> T5TokenizerError {
        T5TokenizerError::Other(alloc::string::String::from(
            "stringcheese-tokenizer-component-t5 built without real vocab: enable the \
             `parity-real-vocab` feature and populate the T5 tokenizer.json cache.  See \
             the crate's build.rs for the resolution order.",
        ))
    }

    pub(super) fn encode_impl(_text: &str) -> Result<T5Encoding, T5TokenizerError> {
        Err(stub_error())
    }

    pub(super) fn decode_impl(_ids: &[u32]) -> Result<alloc::string::String, T5TokenizerError> {
        Err(stub_error())
    }

    pub(super) fn count_impl(_text: &str) -> Result<u32, T5TokenizerError> {
        Err(stub_error())
    }
}
