//! Runtime bridge between the embedded cl100k plaintext blob and the
//! WIT boundary.
//!
//! Two code paths:
//!
//! * **Real vocab (`cfg(stringcheese_cl100k_real_vocab)`)** — parses
//!   the embedded plaintext via
//!   [`stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken`],
//!   constructs a [`BpeTokenizer`] with the canonical cl100k
//!   pre-tokenizer regex, and caches it in a [`OnceLock`] so the
//!   parse cost is paid once per process.
//! * **Stub (default)** — every operation returns
//!   [`Cl100kTokenizerError::Other`] with a message pointing the
//!   caller at the `parity-real-vocab` feature. The capabilities
//!   record still reports the crate's identity so a caller doing
//!   backend dispatch sees `variant-id = "cl100k_base"` even in
//!   stub mode.

use alloc::string::String;
use alloc::vec::Vec;

/// Mirror of the WIT `encoding` record — the shape the WIT `Guest`
/// bridges into. Native callers who consume this crate as an `rlib`
/// can produce it directly via [`encode`], skipping the WIT
/// deserialization for the same payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cl100kEncoding {
    /// Token ids in emission order.
    pub ids: Vec<u32>,
    /// Half-open byte ranges into the input, one per id. Empty
    /// under stub mode.
    pub offsets: Vec<(u32, u32)>,
    /// One flag per id; `true` iff the id is a special token. Empty
    /// under stub mode (also empty in real-vocab mode — no special
    /// tokens are registered, matching tiktoken's `encode_ordinary`).
    pub special_mask: Vec<bool>,
    /// One `type_id` per id (always `0` here — pair encoding is a
    /// future WIT interface).
    pub type_ids: Vec<u32>,
    /// One flag per id; `true` for real tokens.
    pub attention_mask: Vec<bool>,
}

/// Mirror of the WIT `capabilities` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cl100kCapabilities {
    /// Always `"bpe"`.
    pub model_type: &'static str,
    /// Always `"cl100k_base"` — the tiktoken canonical variant name.
    pub variant_id: &'static str,
    /// Semver of this crate.
    pub version: &'static str,
    /// Total number of ids in the vocabulary. `0` under stub mode;
    /// `100_277` under real vocab (cl100k's published vocab size,
    /// modulo tiktoken's four `<|…|>` special tokens which this
    /// crate deliberately does not register — see the module docs).
    pub vocab_size: u32,
    /// `false`. cl100k covers every input via the 256-byte seed
    /// alphabet at the start of its vocabulary; the tokenizer never
    /// needs the `<0xXX>` fallback shape.
    pub has_byte_fallback: bool,
    /// `false`. Matches tiktoken's `encode_ordinary` behaviour: the
    /// four `<|endoftext|>` / `<|fim_*|>` special tokens exist in
    /// the vocab file but this crate does not register them, so a
    /// literal `"<|endoftext|>"` in the input is encoded as ordinary
    /// bytes rather than as its special id.
    pub has_special_tokens: bool,
}

/// Mirror of the WIT `tokenizer-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cl100kTokenizerError {
    /// Bytes did not form valid UTF-8.
    InvalidUtf8,
    /// A surface string (encode) or id (decode) was OOV. Payload is
    /// the offending string.
    UnknownToken(String),
    /// The input contained a special token the caller marked
    /// disallowed. Payload is the offending string.
    DisallowedSpecialToken(String),
    /// The vocabulary / merge table is internally inconsistent.
    VocabularyMismatch,
    /// Anything else. Payload carries a short human-readable
    /// message. Used for the stub-mode `parity-real-vocab` diagnostic.
    Other(String),
}

/// The published cl100k pre-tokenizer regex. Verbatim from
/// `openai/tiktoken`'s `openai_public.py`; re-exposed via
/// [`stringcheese_tokenizer_hf::TIKTOKEN_CANONICAL_PATTERN`].
#[cfg(stringcheese_cl100k_real_vocab)]
const CL100K_PATTERN: &str = stringcheese_tokenizer_hf::TIKTOKEN_CANONICAL_PATTERN;

/// `true` iff this build has the real cl100k vocab embedded (i.e.
/// `build.rs` successfully staged and SHA-256-verified a plaintext
/// blob). A downstream caller doing capability discovery can check
/// this before invoking `encode` — a `false` result means every
/// `encode` / `decode` / `count` call will surface the stub-mode
/// diagnostic.
#[must_use]
pub const fn is_real_vocab() -> bool {
    cfg!(stringcheese_cl100k_real_vocab)
}

/// Report the crate's capabilities. Safe to call in every mode; the
/// stub path returns the same identity fields with `vocab_size = 0`
/// so a caller doing dispatch on `variant_id` still sees the right
/// value.
#[must_use]
pub fn get_capabilities() -> Cl100kCapabilities {
    Cl100kCapabilities {
        model_type: "bpe",
        variant_id: "cl100k_base",
        version: env!("CARGO_PKG_VERSION"),
        vocab_size: real_vocab_size(),
        has_byte_fallback: false,
        has_special_tokens: false,
    }
}

/// Encode `text` into the [`Cl100kEncoding`] shape.
///
/// # Errors
///
/// * Under stub mode: always [`Cl100kTokenizerError::Other`] with a
///   message naming the `parity-real-vocab` feature.
/// * Under real vocab: surfaces the underlying
///   `stringcheese_tokenizer_hf::BpeTokenizer::encode` error via the
///   [`From<stringcheese_tokenizer::TokenizerError>`] impl below.
pub fn encode(text: &str) -> Result<Cl100kEncoding, Cl100kTokenizerError> {
    real::encode_impl(text)
}

/// Decode a sequence of ids back into a string.
///
/// # Errors
///
/// Same taxonomy as [`encode`].
pub fn decode(ids: &[u32]) -> Result<String, Cl100kTokenizerError> {
    real::decode_impl(ids)
}

/// Count the tokens `encode(text)` would produce.
///
/// # Errors
///
/// Same taxonomy as [`encode`].
pub fn count(text: &str) -> Result<u32, Cl100kTokenizerError> {
    real::count_impl(text)
}

/// Vocab size reported to the WIT capabilities record.
#[cfg(stringcheese_cl100k_real_vocab)]
fn real_vocab_size() -> u32 {
    // Query the live tokenizer so a future re-baseline of cl100k
    // (should OpenAI ever republish it) reflects immediately here.
    real::with_tokenizer(|tok| u32::try_from(tok.vocab().len()).unwrap_or(u32::MAX)).unwrap_or(0)
}

#[cfg(not(stringcheese_cl100k_real_vocab))]
fn real_vocab_size() -> u32 {
    0
}

// ---------------------------------------------------------------------
// Real-vocab path. Only compiles when `build.rs` staged a valid
// plaintext blob and emitted `--cfg=stringcheese_cl100k_real_vocab`.
// ---------------------------------------------------------------------

#[cfg(stringcheese_cl100k_real_vocab)]
mod real {
    use alloc::string::String;
    use std::sync::OnceLock;

    use stringcheese_tokenizer::Tokenizer;
    use stringcheese_tokenizer_hf::{
        BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, RegexPreTokenizer,
    };
    use stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken;

    use super::{CL100K_PATTERN, Cl100kEncoding, Cl100kTokenizerError};

    /// The embedded plaintext blob. `build.rs` writes real bytes to
    /// `$OUT_DIR/cl100k_base.tiktoken` when the resolution walk
    /// finds a SHA-256-verified copy; the empty stub blob is only
    /// written when the real-vocab cfg is off, so under this
    /// `#[cfg(stringcheese_cl100k_real_vocab)]` branch the bytes are
    /// guaranteed non-empty and hash-verified.
    const PLAINTEXT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cl100k_base.tiktoken"));

    static TOKENIZER: OnceLock<Result<BpeTokenizer, String>> = OnceLock::new();

    /// Materialise the tokenizer on first call, cache the result.
    fn get() -> Result<&'static BpeTokenizer, Cl100kTokenizerError> {
        let entry = TOKENIZER.get_or_init(build);
        match entry {
            Ok(tok) => Ok(tok),
            Err(msg) => Err(Cl100kTokenizerError::Other(msg.clone())),
        }
    }

    fn build() -> Result<BpeTokenizer, String> {
        let (vocab_entries, merge_entries) = build_scud_from_tiktoken(PLAINTEXT)?;

        let mut vocab = BpeVocabulary::new();
        let mut sorted = vocab_entries;
        sorted.sort_by_key(|e| e.id);
        for entry in sorted {
            vocab
                .insert(entry.id, entry.bytes)
                .map_err(|e| alloc::format!("vocabulary insert failed: {e:?}"))?;
        }

        let mut merges = BpeMergeTable::new();
        for m in merge_entries {
            let left_bytes = vocab
                .bytes(m.left_id)
                .ok_or_else(|| alloc::format!("merge references unknown left id {}", m.left_id))?
                .to_vec();
            let right_bytes = vocab
                .bytes(m.right_id)
                .ok_or_else(|| alloc::format!("merge references unknown right id {}", m.right_id))?
                .to_vec();
            merges.insert(left_bytes, right_bytes, m.rank);
        }

        let pre = RegexPreTokenizer::new(CL100K_PATTERN)
            .map_err(|e| alloc::format!("failed to compile cl100k pre-tokenizer regex: {e}"))?;

        Ok(BpeTokenizer::from_parts(merges, vocab)
            .with_pre_tokenizer(PreTokenizerRegex::regex(pre)))
    }

    /// Callback wrapper — the `OnceLock` stores a `Result`, so a
    /// closure keeps the error mapping in one place.
    pub(super) fn with_tokenizer<T>(f: impl FnOnce(&BpeTokenizer) -> T) -> Option<T> {
        get().ok().map(f)
    }

    pub(super) fn encode_impl(text: &str) -> Result<Cl100kEncoding, Cl100kTokenizerError> {
        let tok = get()?;
        let enc = tok.encode(text).map_err(Cl100kTokenizerError::from)?;
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
        Ok(Cl100kEncoding {
            ids: enc.ids,
            offsets,
            special_mask: enc.special_mask,
            type_ids: enc.type_ids,
            attention_mask: enc.attention_mask,
        })
    }

    pub(super) fn decode_impl(ids: &[u32]) -> Result<String, Cl100kTokenizerError> {
        let tok = get()?;
        tok.decode(ids).map_err(Cl100kTokenizerError::from)
    }

    pub(super) fn count_impl(text: &str) -> Result<u32, Cl100kTokenizerError> {
        let tok = get()?;
        let n = tok.count(text).map_err(Cl100kTokenizerError::from)?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }
}

// ---------------------------------------------------------------------
// Stub path. Compiles when the crate was built without a valid
// plaintext blob. Every operation returns the same diagnostic.
// ---------------------------------------------------------------------

#[cfg(not(stringcheese_cl100k_real_vocab))]
mod real {
    use super::{Cl100kEncoding, Cl100kTokenizerError};

    /// The stub diagnostic every fallible operation returns. Written
    /// once here so a caller grepping for the message across all
    /// callsites finds one authoritative source.
    fn stub_error() -> Cl100kTokenizerError {
        Cl100kTokenizerError::Other(alloc::string::String::from(
            "stringcheese-tokenizer-component-cl100k built without real vocab: enable the \
             `parity-real-vocab` feature and populate the cl100k_base.tiktoken cache. See \
             the crate's build.rs for the resolution order.",
        ))
    }

    pub(super) fn encode_impl(_text: &str) -> Result<Cl100kEncoding, Cl100kTokenizerError> {
        Err(stub_error())
    }

    pub(super) fn decode_impl(_ids: &[u32]) -> Result<alloc::string::String, Cl100kTokenizerError> {
        Err(stub_error())
    }

    pub(super) fn count_impl(_text: &str) -> Result<u32, Cl100kTokenizerError> {
        Err(stub_error())
    }
}

/// Convert a native tokenizer error into the runtime enum. Kept
/// out of both `mod real` branches so both modes surface the same
/// shape to the WIT bridge.
impl From<stringcheese_tokenizer::TokenizerError> for Cl100kTokenizerError {
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
