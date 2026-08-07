//! Shared error taxonomy for the tokenizer subsystem.
//!
//! Every tokenizer and segmenter in this crate — and every algorithm crate
//! that ships a [`Tokenizer`](crate::Tokenizer) implementation on top of
//! it — surfaces the same [`TokenizerError`] variants. The variants are
//! narrow rather than open: a caller reading the enum can decide, at
//! source level, how to react to each failure mode without depending on
//! the specific tokenizer that produced it.
//!
//! # Design
//!
//! * Errors are returned as [`Result`] rather than panicking. `no_std` +
//!   `alloc` builds have no unwind-safe panic surface, and callers who
//!   embed StringCheese into a Wasm component need every failure to be
//!   representable as a value.
//! * A single shared error type keeps the trait signature narrow —
//!   `encode` and `decode` return `Result<_, TokenizerError>` rather than
//!   two associated types. The design doc's Phase 1 charter (see
//!   `docs/design/tokenizers.md` § 2.2) accepts this simplification for
//!   the initial delivery; splitting into per-direction error types is a
//!   future-work item if a real implementation ever needs to.

use core::fmt;

#[cfg(feature = "alloc")]
use alloc::string::String;

/// Errors returned from tokenizer and segmenter operations.
///
/// Every built-in tokenizer and every downstream algorithm implementation
/// surfaces failures through this shared variant. The variants are
/// mutually exclusive: a given failure is exactly one of the named
/// causes, plus [`Other`](TokenizerError::Other) as an explicit catchall
/// for domain-specific failures that don't fit the fixed taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenizerError {
    /// Input contained a byte sequence that is not valid UTF-8, or
    /// decoding produced bytes that no longer form valid UTF-8.
    ///
    /// The tokenizer surface accepts `&str`, so the input path can only
    /// hit this on synthetic paths that construct token IDs out of band
    /// (loader failures, mismatched vocabulary). On the decode side this
    /// fires when concatenated per-token byte strings do not reassemble
    /// into valid UTF-8 (a corrupted BPE vocabulary, for instance).
    InvalidUtf8,

    /// The tokenizer encountered a token — during encoding, a surface
    /// string that is not in the vocabulary and no fallback is
    /// configured; during decoding, a token ID that is not present in
    /// the vocabulary.
    ///
    /// The payload identifies the offending token when the crate feature
    /// `alloc` is available; a bare marker is carried otherwise.
    #[cfg(feature = "alloc")]
    UnknownToken(String),
    /// The tokenizer encountered a token not in the vocabulary. The
    /// no-alloc form carries no context; enable `alloc` to receive the
    /// offending surface string or token ID.
    #[cfg(not(feature = "alloc"))]
    UnknownToken,

    /// A merge table, vocabulary, or special-token map is internally
    /// inconsistent. For BPE this fires when a merge references a token
    /// that is not in the vocabulary; for pack loaders it fires when the
    /// declared token count does not match the stored count.
    VocabularyMismatch,

    /// Heap allocation failed. This variant is present for `no_std`
    /// builds targeting fallible-allocation runtimes; the standard
    /// `alloc` crate today panics on OOM rather than surfacing an error,
    /// so this variant is currently unreachable on stable Rust but
    /// preserved for API stability against the `Allocator` trait
    /// stabilising.
    AllocationFailed,

    /// A domain-specific failure that does not fit the fixed variants
    /// above. A tokenizer that needs to surface configuration errors,
    /// unsupported pre-tokenizer patterns, or loader failures uses this
    /// variant with a short static description.
    ///
    /// The message is a `&'static str` so the variant remains available
    /// in no-alloc builds. Downstream tokenizers that need richer
    /// diagnostics can wrap this crate's error with their own.
    Other(&'static str),
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => f.write_str("input or decoded output is not valid UTF-8"),
            #[cfg(feature = "alloc")]
            Self::UnknownToken(s) => write!(f, "unknown token: {s}"),
            #[cfg(not(feature = "alloc"))]
            Self::UnknownToken => f.write_str("unknown token"),
            Self::VocabularyMismatch => {
                f.write_str("vocabulary and merge table are internally inconsistent")
            }
            Self::AllocationFailed => f.write_str("heap allocation failed"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TokenizerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    use alloc::format;

    #[test]
    fn display_covers_every_variant() {
        assert!(!format!("{}", TokenizerError::InvalidUtf8).is_empty());
        assert!(!format!("{}", TokenizerError::VocabularyMismatch).is_empty());
        assert!(!format!("{}", TokenizerError::AllocationFailed).is_empty());
        assert!(!format!("{}", TokenizerError::Other("nope")).is_empty());
        #[cfg(feature = "alloc")]
        {
            let e = TokenizerError::UnknownToken(String::from("<oops>"));
            assert!(format!("{e}").contains("<oops>"));
        }
    }

    #[test]
    fn equality_holds_by_variant() {
        assert_eq!(TokenizerError::InvalidUtf8, TokenizerError::InvalidUtf8);
        assert_ne!(
            TokenizerError::InvalidUtf8,
            TokenizerError::VocabularyMismatch
        );
        assert_eq!(TokenizerError::Other("a"), TokenizerError::Other("a"));
        assert_ne!(TokenizerError::Other("a"), TokenizerError::Other("b"));
    }
}
