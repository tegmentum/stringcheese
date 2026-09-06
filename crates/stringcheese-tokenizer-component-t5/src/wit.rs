//! WIT `Guest` implementation for the real-vocab T5 tokenizer.
//!
//! Bridges the crate's native API ([`crate::encode`],
//! [`crate::decode`], [`crate::count`],
//! [`crate::get_capabilities`]) to the shared
//! `tegmentum:tokenizer@0.1.0` interface so this crate can ship
//! as a standalone WebAssembly component with T5's real vocab
//! embedded.
//!
//! Only compiled on wasm targets with the `wit-component` feature
//! (which itself requires `parity-real-vocab`).  Mirrors the
//! whisper sibling's structure verbatim aside from the identifier
//! renames (BpeTokenizer → UnigramTokenizer, T5 vs Whisper types).

use crate::bindings::exports::tegmentum::tokenizer::tokenizer::{
    Capabilities, Encoding, Guest, Range, TokenId, TokenizerError,
};
use crate::{T5Encoding, T5TokenizerError};

pub struct Component;

impl Guest for Component {
    fn get_capabilities() -> Capabilities {
        let native = crate::get_capabilities();
        Capabilities {
            model_type: native.model_type.into(),
            variant_id: native.variant_id.into(),
            version: native.version.into(),
            vocab_size: native.vocab_size,
            has_byte_fallback: native.has_byte_fallback,
            has_special_tokens: native.has_special_tokens,
        }
    }

    fn encode(text: alloc::string::String) -> Result<Encoding, TokenizerError> {
        crate::encode(&text)
            .map(to_wit_encoding)
            .map_err(to_wit_error)
    }

    fn decode(ids: alloc::vec::Vec<TokenId>) -> Result<alloc::string::String, TokenizerError> {
        crate::decode(&ids).map_err(to_wit_error)
    }

    fn count(text: alloc::string::String) -> Result<u32, TokenizerError> {
        crate::count(&text).map_err(to_wit_error)
    }
}

fn to_wit_encoding(enc: T5Encoding) -> Encoding {
    Encoding {
        ids: enc.ids,
        offsets: enc
            .offsets
            .into_iter()
            .map(|(start, end)| Range { start, end })
            .collect(),
        special_mask: enc.special_mask,
        type_ids: enc.type_ids,
        attention_mask: enc.attention_mask,
    }
}

fn to_wit_error(e: T5TokenizerError) -> TokenizerError {
    match e {
        T5TokenizerError::InvalidUtf8 => TokenizerError::InvalidUtf8,
        T5TokenizerError::UnknownToken(s) => TokenizerError::UnknownToken(s),
        T5TokenizerError::DisallowedSpecialToken(s) => {
            TokenizerError::DisallowedSpecialToken(s)
        }
        T5TokenizerError::VocabularyMismatch => TokenizerError::VocabularyMismatch,
        T5TokenizerError::Other(s) => TokenizerError::Other(s),
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
