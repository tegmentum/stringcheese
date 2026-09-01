//! Runtime bridge between the embedded whisper `tokenizer.json`
//! and the WIT boundary.
//!
//! Two code paths:
//!
//! * **Real vocab (`cfg(stringcheese_whisper_real_vocab)`)** — parses
//!   the embedded JSON via
//!   [`stringcheese_tokenizer_hf::hf::parse_tokenizer_json`] +
//!   [`stringcheese_tokenizer_hf::hf::to_bpe_tokenizer`], caches
//!   the [`BpeTokenizer`] in a [`OnceLock`] so the parse cost is
//!   paid once per process.
//! * **Stub (default)** — every operation returns
//!   [`WhisperTokenizerError::Other`] naming the `parity-real-vocab`
//!   feature.  The capabilities record still reports the crate's
//!   identity (`variant-id = "openai-whisper"`) so a caller doing
//!   backend dispatch sees the right value even in stub mode.

use alloc::string::String;
use alloc::vec::Vec;

/// Mirror of the WIT `encoding` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperEncoding {
    pub ids: Vec<u32>,
    pub offsets: Vec<(u32, u32)>,
    pub special_mask: Vec<bool>,
    pub type_ids: Vec<u32>,
    pub attention_mask: Vec<bool>,
}

/// Mirror of the WIT `capabilities` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperCapabilities {
    /// Always `"bpe"` — whisper's tokenizer.json declares this
    /// under `model.type`.
    pub model_type: &'static str,
    /// Always `"openai-whisper"`.  Distinct from `variant_id`
    /// choices for other stringcheese variants
    /// (`"cl100k_base"`, `"o200k_base"`).
    pub variant_id: &'static str,
    /// Semver of this crate.
    pub version: &'static str,
    /// Total number of ids in the vocabulary.  `0` under stub;
    /// under real vocab, `50257` for whisper-tiny/base/small/medium
    /// (English + multilingual share the same GPT-2-derived
    /// vocab) and `51864` for whisper-large-v3 (adds Cantonese
    /// and a few extras — the exact count is read from the live
    /// tokenizer, so a future re-baseline reflects immediately).
    pub vocab_size: u32,
    /// `false`.  Whisper's BPE covers every byte via its 256-byte
    /// byte-level alphabet at the front of the vocabulary; no
    /// `<0xXX>` fallback tokens.
    pub has_byte_fallback: bool,
    /// `true`.  The vocab registers whisper's `<|endoftext|>`,
    /// `<|startoftranscript|>`, per-language `<|xx|>` tokens,
    /// `<|transcribe|>` / `<|translate|>`, and per-timestamp
    /// `<|0.00|>...<|30.00|>`.  A caller `encode`-ing a literal
    /// `"<|endoftext|>"` string gets the special id back;
    /// `decode` on those ids returns the empty string (special
    /// tokens are skipped).
    pub has_special_tokens: bool,
}

/// Mirror of the WIT `tokenizer-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhisperTokenizerError {
    InvalidUtf8,
    UnknownToken(String),
    DisallowedSpecialToken(String),
    VocabularyMismatch,
    Other(String),
}

/// `true` iff this build has whisper's real tokenizer.json
/// embedded.  A downstream caller doing capability discovery can
/// check this before invoking `encode`.
#[must_use]
pub const fn is_real_vocab() -> bool {
    cfg!(stringcheese_whisper_real_vocab)
}

#[must_use]
pub fn get_capabilities() -> WhisperCapabilities {
    WhisperCapabilities {
        model_type: "bpe",
        variant_id: "openai-whisper",
        version: env!("CARGO_PKG_VERSION"),
        vocab_size: real_vocab_size(),
        has_byte_fallback: false,
        has_special_tokens: cfg!(stringcheese_whisper_real_vocab),
    }
}

pub fn encode(text: &str) -> Result<WhisperEncoding, WhisperTokenizerError> {
    real::encode_impl(text)
}

pub fn decode(ids: &[u32]) -> Result<String, WhisperTokenizerError> {
    real::decode_impl(ids)
}

pub fn count(text: &str) -> Result<u32, WhisperTokenizerError> {
    real::count_impl(text)
}

#[cfg(stringcheese_whisper_real_vocab)]
fn real_vocab_size() -> u32 {
    real::with_tokenizer(|tok| u32::try_from(tok.vocab().len()).unwrap_or(u32::MAX)).unwrap_or(0)
}

#[cfg(not(stringcheese_whisper_real_vocab))]
fn real_vocab_size() -> u32 {
    0
}

// ---------------------------------------------------------------------
// Real-vocab path.  Compiles when build.rs staged tokenizer.json
// and emitted --cfg=stringcheese_whisper_real_vocab.
// ---------------------------------------------------------------------

#[cfg(stringcheese_whisper_real_vocab)]
mod real {
    use alloc::string::String;
    use core::str;
    use std::sync::OnceLock;

    use stringcheese_tokenizer::Tokenizer;
    use stringcheese_tokenizer_hf::BpeTokenizer;
    use stringcheese_tokenizer_hf::hf;

    use super::{WhisperEncoding, WhisperTokenizerError};

    /// The embedded tokenizer.json.  build.rs writes real bytes to
    /// `$OUT_DIR/tokenizer.json` when the resolution walk finds a
    /// non-empty file; under this cfg the bytes are guaranteed
    /// non-empty.
    const TOKENIZER_JSON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tokenizer.json"));

    static TOKENIZER: OnceLock<Result<BpeTokenizer, String>> = OnceLock::new();

    fn get() -> Result<&'static BpeTokenizer, WhisperTokenizerError> {
        let entry = TOKENIZER.get_or_init(build);
        match entry {
            Ok(tok) => Ok(tok),
            Err(msg) => Err(WhisperTokenizerError::Other(msg.clone())),
        }
    }

    fn build() -> Result<BpeTokenizer, String> {
        let json = str::from_utf8(TOKENIZER_JSON)
            .map_err(|e| alloc::format!("whisper tokenizer.json is not valid UTF-8: {e}"))?;
        let cfg = hf::parse_tokenizer_json(json)
            .map_err(|e| alloc::format!("parse tokenizer.json: {e:?}"))?;
        hf::to_bpe_tokenizer(&cfg)
            .map_err(|e| alloc::format!("materialize BpeTokenizer: {e:?}"))
    }

    pub(super) fn with_tokenizer<T>(f: impl FnOnce(&BpeTokenizer) -> T) -> Option<T> {
        get().ok().map(f)
    }

    pub(super) fn encode_impl(text: &str) -> Result<WhisperEncoding, WhisperTokenizerError> {
        let tok = get()?;
        let enc = tok.encode(text).map_err(WhisperTokenizerError::from)?;
        let offsets = enc
            .offsets
            .into_iter()
            .map(|r| {
                (
                    u32::try_from(r.start).unwrap_or(u32::MAX),
                    u32::try_from(r.end).unwrap_or(u32::MAX),
                )
            })
            .collect();
        Ok(WhisperEncoding {
            ids: enc.ids,
            offsets,
            special_mask: enc.special_mask,
            type_ids: enc.type_ids,
            attention_mask: enc.attention_mask,
        })
    }

    pub(super) fn decode_impl(ids: &[u32]) -> Result<String, WhisperTokenizerError> {
        let tok = get()?;
        tok.decode(ids).map_err(WhisperTokenizerError::from)
    }

    pub(super) fn count_impl(text: &str) -> Result<u32, WhisperTokenizerError> {
        let tok = get()?;
        let n = tok.count(text).map_err(WhisperTokenizerError::from)?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }
}

// ---------------------------------------------------------------------
// Stub path.  Compiles when the crate was built without a valid
// tokenizer.json.  Every operation returns the same diagnostic.
// ---------------------------------------------------------------------

#[cfg(not(stringcheese_whisper_real_vocab))]
mod real {
    use super::{WhisperEncoding, WhisperTokenizerError};

    fn stub_error() -> WhisperTokenizerError {
        WhisperTokenizerError::Other(alloc::string::String::from(
            "stringcheese-tokenizer-component-whisper built without real vocab: enable the \
             `parity-real-vocab` feature and populate the whisper tokenizer.json cache.  See \
             the crate's build.rs for the resolution order.",
        ))
    }

    pub(super) fn encode_impl(_text: &str) -> Result<WhisperEncoding, WhisperTokenizerError> {
        Err(stub_error())
    }

    pub(super) fn decode_impl(_ids: &[u32]) -> Result<alloc::string::String, WhisperTokenizerError> {
        Err(stub_error())
    }

    pub(super) fn count_impl(_text: &str) -> Result<u32, WhisperTokenizerError> {
        Err(stub_error())
    }
}

impl From<stringcheese_tokenizer::TokenizerError> for WhisperTokenizerError {
    fn from(e: stringcheese_tokenizer::TokenizerError) -> Self {
        use stringcheese_tokenizer::TokenizerError as E;
        match e {
            E::InvalidUtf8 => Self::InvalidUtf8,
            E::UnknownToken(s) => Self::UnknownToken(s),
            E::DisallowedSpecialToken(s) => Self::DisallowedSpecialToken(s),
            E::VocabularyMismatch => Self::VocabularyMismatch,
            E::AllocationFailed => Self::Other(String::from("allocation failed")),
            E::Other(msg) => Self::Other(String::from(msg)),
            other => Self::Other(alloc::format!("{other}")),
        }
    }
}
