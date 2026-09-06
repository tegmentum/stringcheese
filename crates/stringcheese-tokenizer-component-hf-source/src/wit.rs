//! WIT `Guest` implementation for the runtime-bytes hf-source
//! tokenizer.
//!
//! Bridges [`crate::HfSourceHandle`] to the `tegmentum:tokenizer/
//! tokenizer-source@0.1.0` resource shape exported by
//! `world tokenizer-source-provider`.  Only compiled on wasm
//! targets with `wit-component` (the `cargo component build`
//! flag) — the crate's native API works fine on hosts without any
//! WIT plumbing loaded, which is what the runtime unit tests
//! exercise.

use crate::bindings::exports::tegmentum::tokenizer::tokenizer_source::{
    Capabilities, Encoding, Guest, GuestTokenizerHandle, Range, TokenId, TokenizerError,
    TokenizerHandle,
};
use crate::runtime::{HfSourceError, HfSourceHandle};

/// The world's only export: a guest-owned tokenizer-handle
/// resource.  cargo-component wires `TokenizerHandle` back to
/// [`Self`] via the associated type below, so the resource id
/// the guest hands the caller is a `Box<Self>` internally.
pub struct Component;

impl Guest for Component {
    type TokenizerHandle = HfSourceHandle;
}

impl GuestTokenizerHandle for HfSourceHandle {
    fn from_json(json: alloc::string::String) -> Result<TokenizerHandle, TokenizerError> {
        HfSourceHandle::from_json(&json)
            .map(TokenizerHandle::new)
            .map_err(to_wit_error)
    }

    fn from_bytes(bytes: alloc::vec::Vec<u8>) -> Result<TokenizerHandle, TokenizerError> {
        HfSourceHandle::from_bytes(&bytes)
            .map(TokenizerHandle::new)
            .map_err(to_wit_error)
    }

    fn get_capabilities(&self) -> Capabilities {
        Capabilities {
            model_type: self.model_type().into(),
            variant_id: "runtime-source".into(),
            version: "0.1.0".into(),
            vocab_size: self.vocab_size(),
            has_byte_fallback: false,
            has_special_tokens: true,
        }
    }

    fn encode(&self, text: alloc::string::String) -> Result<Encoding, TokenizerError> {
        HfSourceHandle::encode(self, &text)
            .map(to_wit_encoding)
            .map_err(to_wit_error)
    }

    fn decode(
        &self,
        ids: alloc::vec::Vec<TokenId>,
    ) -> Result<alloc::string::String, TokenizerError> {
        HfSourceHandle::decode(self, &ids).map_err(to_wit_error)
    }

    fn count(&self, text: alloc::string::String) -> Result<u32, TokenizerError> {
        HfSourceHandle::count(self, &text).map_err(to_wit_error)
    }
}

fn to_wit_encoding(enc: stringcheese_tokenizer::Encoding<u32>) -> Encoding {
    // The runtime encoding stores offsets as `(usize, usize)`
    // tuples per token; the WIT wants `range { u32; u32 }`.
    Encoding {
        ids: enc.ids,
        offsets: enc
            .offsets
            .into_iter()
            .map(|r| Range {
                start: u32::try_from(r.start).unwrap_or(u32::MAX),
                end: u32::try_from(r.end).unwrap_or(u32::MAX),
            })
            .collect(),
        special_mask: enc.special_mask,
        type_ids: enc.type_ids,
        attention_mask: enc.attention_mask,
    }
}

fn to_wit_error(e: HfSourceError) -> TokenizerError {
    match e {
        HfSourceError::InvalidUtf8 => TokenizerError::InvalidUtf8,
        HfSourceError::InvalidTokenizerJson(_) => TokenizerError::VocabularyMismatch,
        HfSourceError::EncodeFailed(s) => TokenizerError::Other(s),
        HfSourceError::DecodeFailed(s) => TokenizerError::Other(s),
        HfSourceError::Other(s) => TokenizerError::Other(s),
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
