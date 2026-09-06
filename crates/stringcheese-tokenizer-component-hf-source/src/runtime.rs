//! Native runtime for the hf-source pack.
//!
//! Owns an [`HfTokenizer`] materialised from a tokenizer.json blob
//! at runtime and exposes the same four verbs the baked packs do
//! (encode / decode / count / get-capabilities) plus the two
//! constructors the resource-based WIT adds (from-json /
//! from-bytes).  The [`crate::wit`] module — compiled only on
//! wasm targets with `wit-component` — bridges this native API
//! to the WIT boundary.

use alloc::string::{String, ToString};

use stringcheese_tokenizer::Encoding;
use stringcheese_tokenizer_hf::hf::{
    parse_tokenizer_json, to_tokenizer, HfConversionError, HfParseError, HfTokenizer,
};

/// A live tokenizer instance loaded from a tokenizer.json blob.
///
/// Holds the materialised [`HfTokenizer`] enum + the vocabulary
/// size cached at load time.  Dispatch on the enum is inline in
/// [`Self::encode`] / [`Self::decode`] / [`Self::count`] rather
/// than through a `dyn Trait` — every variant already implements
/// the same [`stringcheese_tokenizer::Tokenizer`] trait, but
/// keeping the concrete type lets the wasm binary avoid a vtable
/// per call.
#[derive(Debug)]
pub struct HfSourceHandle {
    inner: HfTokenizer,
    vocab_size: u32,
    model_type: &'static str,
}

impl HfSourceHandle {
    /// Construct from a UTF-8 tokenizer.json blob.
    ///
    /// # Errors
    ///
    /// Any deserialisation / conversion failure surfaces as
    /// [`HfSourceError::InvalidTokenizerJson`].  The wasm bridge
    /// maps that variant onto the WIT's
    /// `TokenizerError::VocabularyMismatch`.
    pub fn from_json(json: &str) -> Result<Self, HfSourceError> {
        let config =
            parse_tokenizer_json(json).map_err(HfSourceError::parse)?;
        let tokenizer = to_tokenizer(&config).map_err(HfSourceError::conversion)?;
        Ok(Self::from_tokenizer(tokenizer))
    }

    /// Construct from raw tokenizer.json bytes.
    ///
    /// Convenience wrapper around [`Self::from_json`]; validates the
    /// bytes are UTF-8 first so a caller who fetched a binary blob
    /// gets `InvalidUtf8` rather than a confusing parse error.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HfSourceError> {
        let json = core::str::from_utf8(bytes).map_err(|_| HfSourceError::InvalidUtf8)?;
        Self::from_json(json)
    }

    /// Total id count in the tokenizer's vocabulary.
    #[must_use]
    pub const fn vocab_size(&self) -> u32 {
        self.vocab_size
    }

    /// Backend identifier — `"bpe" | "wordpiece" | "unigram" | "wordlevel"`.
    /// Matches the `model.type` field of a HuggingFace tokenizer.json.
    #[must_use]
    pub const fn model_type(&self) -> &'static str {
        self.model_type
    }

    /// Encode `text` through the underlying tokenizer's full
    /// pipeline (normalize + pre-tokenize + model + post-process
    /// + truncate).
    ///
    /// # Errors
    ///
    /// A tokenizer-internal failure (unknown token without byte
    /// fallback, invalid utf-8 in a decoder round-trip, …) maps
    /// to [`HfSourceError::EncodeFailed`].
    pub fn encode(&self, text: &str) -> Result<Encoding<u32>, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::encode(t.as_ref(), text),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::encode(t, text),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::encode(t, text),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::encode(t, text),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::encode)
    }

    /// Decode `ids` back to text.
    ///
    /// # Errors
    ///
    /// A caller-side inconsistency (an id outside the vocabulary,
    /// bytes that no longer form valid utf-8) surfaces as
    /// [`HfSourceError::DecodeFailed`].
    pub fn decode(&self, ids: &[u32]) -> Result<String, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::decode(t.as_ref(), ids),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::decode(t, ids),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::decode(t, ids),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::decode(t, ids),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::decode)
    }

    /// Count the tokens `encode(text)` would produce without
    /// materialising offsets / masks.
    pub fn count(&self, text: &str) -> Result<u32, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        let n = match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::count(t.as_ref(), text),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::count(t, text),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::count(t, text),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::count(t, text),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::encode)?;
        u32::try_from(n).map_err(|_| HfSourceError::Other("count overflowed u32".to_string()))
    }

    fn from_tokenizer(tokenizer: HfTokenizer) -> Self {
        let (vocab_size, model_type) = match &tokenizer {
            HfTokenizer::Bpe(t) => (VocabSize::vocab_size(t.as_ref()), "bpe"),
            HfTokenizer::WordPiece(t) => (VocabSize::vocab_size(t), "wordpiece"),
            HfTokenizer::Unigram(t) => (VocabSize::vocab_size(t), "unigram"),
            HfTokenizer::WordLevel(t) => (VocabSize::vocab_size(t), "wordlevel"),
            _ => (0, "unknown"),
        };
        Self {
            inner: tokenizer,
            vocab_size: u32::try_from(vocab_size).unwrap_or(u32::MAX),
            model_type,
        }
    }
}

/// Failure modes the hf-source runtime surfaces.  Kept narrow so
/// the WIT bridge can map each variant onto a matching
/// `TokenizerError` case without an `Other(...)` catch-all
/// swallowing detail.
#[derive(Debug)]
pub enum HfSourceError {
    /// The input to `from-bytes` was not valid UTF-8.
    InvalidUtf8,
    /// Deserialising or converting the tokenizer.json failed —
    /// malformed JSON, unrecognised `model.type`, incoherent
    /// vocab / merges, …
    InvalidTokenizerJson(String),
    /// A per-call `encode` (or `count`) surfaced an internal error
    /// after the tokenizer.json had loaded cleanly.
    EncodeFailed(String),
    /// A per-call `decode` surfaced an internal error.
    DecodeFailed(String),
    /// Anything else — carries a short human-readable message.
    Other(String),
}

impl HfSourceError {
    fn parse(e: HfParseError) -> Self {
        Self::InvalidTokenizerJson(alloc::format!("{e:?}"))
    }

    fn conversion(e: HfConversionError) -> Self {
        Self::InvalidTokenizerJson(alloc::format!("{e:?}"))
    }

    fn encode(e: stringcheese_tokenizer::TokenizerError) -> Self {
        Self::EncodeFailed(alloc::format!("{e:?}"))
    }

    fn decode(e: stringcheese_tokenizer::TokenizerError) -> Self {
        Self::DecodeFailed(alloc::format!("{e:?}"))
    }
}

/// Every tokenizer variant `HfTokenizer` wraps advertises a vocab
/// size, but the trait only surfaces it as a method on the
/// concrete variant.  Cover the four with one tiny helper that
/// returns a `usize` so the enum dispatch above stays uniform.
trait VocabSize {
    fn vocab_size(&self) -> usize;
}

impl VocabSize for stringcheese_tokenizer_hf::BpeTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::hf::UnigramTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small WordLevel fixture — the minimal shape
    /// `parse_tokenizer_json + to_tokenizer` will accept.  Same
    /// shape the cognition-t5-runner's tokenizer_source unit
    /// tests use, so a WIT / parser drift shows up in both
    /// places.
    const WORDLEVEL_FIXTURE: &str = r###"{
      "version": "1.0",
      "truncation": null,
      "padding": null,
      "added_tokens": [
        {"id": 0, "content": "[UNK]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}
      ],
      "normalizer": null,
      "pre_tokenizer": {"type": "Whitespace"},
      "post_processor": null,
      "decoder": null,
      "model": {
        "type": "WordLevel",
        "vocab": {"[UNK]": 0, "hello": 1, "world": 2},
        "unk_token": "[UNK]"
      }
    }"###;

    #[test]
    fn from_bytes_rejects_non_utf8() {
        let err = HfSourceHandle::from_bytes(&[0xff, 0xfe, 0x00]).unwrap_err();
        assert!(matches!(err, HfSourceError::InvalidUtf8));
    }

    #[test]
    fn from_json_rejects_bad_json() {
        let err = HfSourceHandle::from_json("{this is not tokenizer.json").unwrap_err();
        assert!(matches!(err, HfSourceError::InvalidTokenizerJson(_)));
    }

    #[test]
    fn from_json_loads_wordlevel_fixture() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        assert_eq!(handle.vocab_size(), 3);
        assert_eq!(handle.model_type(), "wordlevel");
    }

    #[test]
    fn encode_wordlevel_fixture_round_trips() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        let enc = handle.encode("hello world").expect("encode");
        assert_eq!(enc.ids, vec![1u32, 2u32]);
        let decoded = handle.decode(&[1, 2]).expect("decode");
        assert!(decoded.contains("hello") && decoded.contains("world"));
    }

    #[test]
    fn count_matches_encode_len() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        let enc = handle.encode("hello world").expect("encode");
        let n = handle.count("hello world").expect("count");
        assert_eq!(n as usize, enc.ids.len());
    }
}
