//! WIT `Guest` implementation for the reference tokenizer.
//!
//! Bridges the crate's native API ([`crate::encode`], [`crate::decode`],
//! [`crate::count`], [`crate::get_capabilities`]) to the shared
//! `tegmentum:tokenizer@0.1.0` interface so this crate can ship as a
//! standalone WebAssembly component that any component-model-capable
//! host loads without linking Rust.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! Native builds skip the bindings module so `cargo test` doesn't
//! pull in `wit-bindgen-rt`, and wasm builds without the feature
//! also skip it — otherwise two backends `export!`-ing the same
//! `tegmentum:tokenizer` interface into one binary would collide at
//! link time with duplicate exported symbols. See the crate-level
//! docs and the detect-* crates for the same pattern.

use crate::bindings::exports::tegmentum::tokenizer::tokenizer::{
    Capabilities, Encoding, Guest, Range, TokenId, TokenizerError,
};
use crate::{ReferenceEncoding, ReferenceTokenizerError};

/// The unit struct every WIT `Guest` trait is implemented on. Zero
/// sized; the reference tokenizer is constructed per call inside
/// [`crate::reference_tokenizer`], so no per-Component state lives
/// here. A future component with a long-lived tokenizer handle would
/// carry it here (or better, expose it via a WIT `resource` type).
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

fn to_wit_encoding(enc: ReferenceEncoding) -> Encoding {
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

fn to_wit_error(e: ReferenceTokenizerError) -> TokenizerError {
    match e {
        ReferenceTokenizerError::InvalidUtf8 => TokenizerError::InvalidUtf8,
        ReferenceTokenizerError::UnknownToken(s) => TokenizerError::UnknownToken(s),
        ReferenceTokenizerError::DisallowedSpecialToken(s) => {
            TokenizerError::DisallowedSpecialToken(s)
        }
        ReferenceTokenizerError::VocabularyMismatch => TokenizerError::VocabularyMismatch,
        ReferenceTokenizerError::Other(s) => TokenizerError::Other(s),
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
