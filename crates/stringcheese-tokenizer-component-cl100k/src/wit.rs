//! WIT `Guest` implementation for the real-vocab cl100k tokenizer.
//!
//! Bridges the crate's native API ([`crate::encode`], [`crate::decode`],
//! [`crate::count`], [`crate::get_capabilities`]) to the shared
//! `tegmentum:tokenizer@0.1.0` interface so this crate can ship as a
//! standalone WebAssembly component with real OpenAI cl100k bytes
//! embedded.
//!
//! Only compiled on wasm targets with the `wit-component` feature
//! (which itself requires `parity-real-vocab`, so a stub-mode build
//! never produces a component). See the crate-level docs and the
//! parent `stringcheese-tokenizer-component` crate for the same
//! pattern.

use crate::bindings::exports::tegmentum::tokenizer::tokenizer::{
    Capabilities, Encoding, Guest, Range, TokenId, TokenizerError,
};
use crate::{Cl100kEncoding, Cl100kTokenizerError};

/// The unit struct every WIT `Guest` trait is implemented on. Zero
/// sized; the cached tokenizer lives in the runtime module's
/// `OnceLock`, so no per-Component state is carried here. A future
/// component with a first-class handle would carry it here (or
/// better, expose it via a WIT `resource` type).
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

fn to_wit_encoding(enc: Cl100kEncoding) -> Encoding {
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

fn to_wit_error(e: Cl100kTokenizerError) -> TokenizerError {
    match e {
        Cl100kTokenizerError::InvalidUtf8 => TokenizerError::InvalidUtf8,
        Cl100kTokenizerError::UnknownToken(s) => TokenizerError::UnknownToken(s),
        Cl100kTokenizerError::DisallowedSpecialToken(s) => {
            TokenizerError::DisallowedSpecialToken(s)
        }
        Cl100kTokenizerError::VocabularyMismatch => TokenizerError::VocabularyMismatch,
        Cl100kTokenizerError::Other(s) => TokenizerError::Other(s),
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
