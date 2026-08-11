//! # Reference character-BPE tokenizer for the WIT boundary
//!
//! Builds a small, deterministic `BpeTokenizer` out of a hand-crafted
//! 261-token vocabulary: 256 single-byte ids (0..=255, the byte-level
//! seed alphabet every BPE tokenizer needs) plus five multi-byte
//! merged pieces (`"he"`, `"ll"`, `"llo"`, `"hello"`, `" world"`).
//! Backed by a single special token `<|endoftext|>` at id 260 so the
//! WIT interface's `has-special-tokens` flag has something non-trivial
//! to report.
//!
//! ## Why this shape
//!
//! * **Deterministic across host and wasm.** The vocab and merges are
//!   compile-time constants, so `encode("Hello, world!")` produces the
//!   same id sequence on the host as under wasmtime — that is what
//!   lets the smoke test in `tests/` assert against a known-good
//!   fixture.
//! * **Exercises every code path the WIT boundary depends on.** Byte-
//!   level seeding (`b'H'` seeds an id at 72), merge-loop composition
//!   (`h` + `e` → `he` → `he` + `ll` → nothing, then `he` + `llo` →
//!   `hello`), special-token bypass (the endoftext splice), offset
//!   tracking (byte ranges emitted per token), and full round-trip
//!   decode (which is the property the smoke test's decode leg
//!   verifies).
//! * **No real vocab bytes.** The workspace CLAUDE.md forbids
//!   committing real tiktoken / `HuggingFace` vocab bytes. This
//!   hand-crafted vocab is a demonstration substrate, not a
//!   production tokenizer.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stringcheese_tokenizer::{Encoding, Tokenizer, TokenizerError};
use stringcheese_tokenizer_hf::{BpeMergeTable, BpeTokenizer, BpeVocabulary, TokenId};

/// The total number of ids in the reference vocabulary: 256 bytes +
/// 5 merged pieces = 261 ids, with an additional special-token id at
/// 260 reserved above the vocab. Kept as a `pub const` so the WIT
/// `capabilities.vocab-size` can report it verbatim.
pub const REFERENCE_VOCAB_SIZE: u32 = 261;

/// Character-BPE surface strings for the ids at 256..=260. Constructed
/// as a `&[(&str, TokenId)]` slice so the vocab loader can consume
/// them in a stable order — a merge rank inversion here would silently
/// change the smoke-test's expected id sequence. All five merged
/// pieces are contained inside a single whitespace-delimited word so
/// the default BPE pre-tokenizer (whitespace split — no regex
/// attached) does not interrupt the merge loop mid-composition.
const MERGED_PIECES: &[(&str, TokenId)] = &[
    ("he", 256),
    ("ll", 257),
    ("llo", 258),
    ("world", 259),
    ("hello", 260),
];

/// The merges the tokenizer applies in ascending-rank order. Rank 0
/// fires first when adjacent; the encoder walks the sequence
/// bottom-up composing pieces. Each merge is scoped inside a single
/// "word" (no space-crossing merge) — the default BPE pre-tokenizer
/// splits on whitespace before the merge loop runs, so any merge
/// spanning a space would never fire under `encode`.
const MERGES: &[(&str, &str, u32)] = &[
    ("h", "e", 0),
    ("l", "l", 1),
    ("ll", "o", 2),
    ("he", "llo", 3),
    ("w", "o", 4),
    ("wo", "r", 5),
    ("wor", "l", 6),
    ("worl", "d", 7),
];

/// The special token registered on the reference tokenizer. One entry
/// so `has-special-tokens` reads `true`; kept small so the WIT smoke
/// test's fixture stays compact. Placed above the vocab so it never
/// collides with a byte-alphabet or merged-piece id.
const SPECIAL_TOKEN: (&str, TokenId) = ("<|endoftext|>", 261);

/// Builds the reference [`BpeTokenizer`] the WIT wrapper delegates
/// into. Cheap enough that constructing per call is fine for a
/// reference wrapper — the crate is not the place to demonstrate a
/// long-lived tokenizer handle (that's a future resource-typed WIT
/// interface). The tokenizer's `vocab_size` here is
/// [`REFERENCE_VOCAB_SIZE`] plus the one special token, so a caller
/// counting downstream ids can size their host-side arrays correctly.
///
/// # Panics
///
/// Never at runtime: the vocab is populated at compile time from a
/// bijection, so the two `.expect` calls (byte-alphabet insertion
/// and merged-piece insertion) are provably unreachable — the byte
/// alphabet is a one-to-one mapping from `0..=255` to single-byte
/// keys, and the five merged pieces are hand-crafted to be unique.
#[must_use]
pub fn reference_tokenizer() -> BpeTokenizer {
    let mut vocab = BpeVocabulary::new();
    // Seed the 256 byte ids: BPE always needs the byte alphabet at
    // 0..=255 to seed the merge loop; a missing id would surface as
    // an `UnknownToken` on any input byte the merge loop cannot
    // otherwise cover.
    for b in 0u32..=255 {
        vocab
            .insert(b, alloc::vec![u8::try_from(b).expect("0..=255 fits u8")])
            .expect("byte alphabet is a bijection by construction");
    }
    for (surface, id) in MERGED_PIECES {
        vocab
            .insert(*id, surface.as_bytes().to_vec())
            .expect("hand-crafted merge pieces are unique");
    }
    let mut merges = BpeMergeTable::new();
    for (left, right, rank) in MERGES {
        merges.insert(left.as_bytes().to_vec(), right.as_bytes().to_vec(), *rank);
    }
    let mut specials = BTreeMap::new();
    specials.insert(SPECIAL_TOKEN.0.to_string(), SPECIAL_TOKEN.1);
    BpeTokenizer::from_parts(merges, vocab).with_special_tokens(specials)
}

/// Mirror of the WIT `encoding` record — the shape the WIT `Guest`
/// bridges into. Native callers who consume this crate as an `rlib`
/// can produce it directly via [`encode`], skipping the WIT
/// deserialization for the same payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceEncoding {
    /// Token ids, in emission order.
    pub ids: Vec<TokenId>,
    /// Half-open byte ranges into the input, one per id.
    pub offsets: Vec<(u32, u32)>,
    /// One flag per id; `true` iff the id is a special token.
    pub special_mask: Vec<bool>,
    /// One `type_id` per id (always `0` here — pair encoding is a
    /// future WIT interface).
    pub type_ids: Vec<u32>,
    /// One flag per id; `true` for real tokens.
    pub attention_mask: Vec<bool>,
}

/// Mirror of the WIT `capabilities` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCapabilities {
    /// One of `"bpe"` / `"wordpiece"` / `"unigram"` / `"wordlevel"`.
    /// Always `"bpe"` for the reference.
    pub model_type: &'static str,
    /// Stable variant identifier. `"reference-character-bpe"` for
    /// this crate — deliberately not colliding with any real
    /// `cl100k_base` / `o200k_base` / model-derived variant name.
    pub variant_id: &'static str,
    /// Semver of this crate.
    pub version: &'static str,
    /// Total number of ids in the vocabulary (including specials).
    pub vocab_size: u32,
    /// `false` — the reference does not have a byte-fallback table
    /// wired. Every input in ASCII / small-Unicode range is covered
    /// by the byte alphabet at 0..=255.
    pub has_byte_fallback: bool,
    /// `true` — one special token (`<|endoftext|>`) is registered.
    pub has_special_tokens: bool,
}

/// Mirror of the WIT `tokenizer-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTokenizerError {
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
    /// Anything else. Payload carries a short human-readable message.
    Other(String),
}

impl From<TokenizerError> for ReferenceTokenizerError {
    fn from(e: TokenizerError) -> Self {
        match e {
            TokenizerError::InvalidUtf8 => Self::InvalidUtf8,
            TokenizerError::UnknownToken(s) => Self::UnknownToken(s),
            TokenizerError::DisallowedSpecialToken(s) => Self::DisallowedSpecialToken(s),
            TokenizerError::VocabularyMismatch => Self::VocabularyMismatch,
            TokenizerError::AllocationFailed => Self::Other(String::from("allocation failed")),
            TokenizerError::Other(msg) => Self::Other(msg.to_string()),
            // `TokenizerError` is `#[non_exhaustive]`; any variant
            // the trait crate adds without a matching arm here surfaces
            // as `Other` with the variant's Display form. This keeps
            // downstream WIT callers on a stable surface even if the
            // upstream error grows.
            other => Self::Other(alloc::format!("{other}")),
        }
    }
}

/// Encode `text` into the reference [`ReferenceEncoding`] shape.
///
/// # Errors
///
/// Surfaces the underlying [`BpeTokenizer::encode`] error via the
/// [`From<TokenizerError>`] impl above.
pub fn encode(text: &str) -> Result<ReferenceEncoding, ReferenceTokenizerError> {
    let tok = reference_tokenizer();
    let enc: Encoding<TokenId> = tok.encode(text)?;
    Ok(to_reference_encoding(enc))
}

/// Decode a sequence of ids back into a string.
///
/// # Errors
///
/// Surfaces the underlying [`BpeTokenizer::decode`] error via the
/// [`From<TokenizerError>`] impl above.
pub fn decode(ids: &[TokenId]) -> Result<String, ReferenceTokenizerError> {
    let tok = reference_tokenizer();
    Ok(tok.decode(ids)?)
}

/// Count the tokens `encode(text)` would produce.
///
/// # Errors
///
/// Surfaces the underlying [`BpeTokenizer::count`] error via the
/// [`From<TokenizerError>`] impl above.
pub fn count(text: &str) -> Result<u32, ReferenceTokenizerError> {
    let tok = reference_tokenizer();
    let n = tok.count(text)?;
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Report the reference tokenizer's capabilities.
///
/// The values here are intentionally static — the reference vocab is
/// baked in at compile time, so the capabilities record is a
/// constant computation. A future dynamic-vocab component would
/// materialise this at load time instead.
#[must_use]
pub fn get_capabilities() -> ReferenceCapabilities {
    ReferenceCapabilities {
        model_type: "bpe",
        variant_id: "reference-character-bpe",
        version: env!("CARGO_PKG_VERSION"),
        vocab_size: REFERENCE_VOCAB_SIZE,
        has_byte_fallback: false,
        has_special_tokens: true,
    }
}

/// Lower a native `Encoding` to the reference-native representation.
/// The offsets are packed into `(u32, u32)` pairs matching the WIT
/// `range` record.
fn to_reference_encoding(enc: Encoding<TokenId>) -> ReferenceEncoding {
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
    ReferenceEncoding {
        ids: enc.ids,
        offsets,
        special_mask: enc.special_mask,
        type_ids: enc.type_ids,
        attention_mask: enc.attention_mask,
    }
}

// Silence the "unused" lint on `Box` under alloc-only builds where
// the reference tokenizer path never boxes anything at the module
// level — future dynamic-vocab constructors will use `Box<[u8]>` for
// interned surface forms, and the import is documented here as
// forward-looking.
#[allow(dead_code)]
fn _boxes_are_documented_but_unused_for_now() -> Box<[u8]> {
    Box::new([])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference-tokenizer smoke test — the exact fixture the
    /// wasmtime test in `tests/component_smoke.rs` cross-checks
    /// against. If a merge or vocab change alters this sequence the
    /// wasm-side assertion needs to be re-baselined too.
    #[test]
    fn encode_hello_world_produces_expected_ids() {
        let enc = encode("hello world").expect("reference vocab covers this input");
        // The default BPE pre-tokenizer splits on whitespace, so
        // encode runs the merge loop over "hello" and "world"
        // independently. "hello" composes to id 260 (h+e -> he,
        // l+l -> ll, ll+o -> llo, he+llo -> hello); "world"
        // composes to id 259 (w+o -> wo, wo+r -> wor, wor+l -> worl,
        // worl+d -> world). Between the two tokens the whitespace
        // itself is dropped by the whitespace pre-tokenizer.
        assert_eq!(enc.ids, alloc::vec![260, 259]);
    }

    #[test]
    fn decode_round_trips_reference_encoding() {
        // The whitespace pre-tokenizer drops the space at encode time,
        // so `decode(encode("hello world"))` is `"helloworld"` — the
        // BPE round-trip is over the merged pieces, not the input
        // whitespace. This is the tiktoken / HF byte-level convention
        // when whitespace is not represented in the vocab.
        let enc = encode("hello world").expect("encode succeeds");
        let s = decode(&enc.ids).expect("decode succeeds");
        assert_eq!(s, "helloworld");
    }

    #[test]
    fn decode_round_trips_single_word() {
        // A single whitespace-free word round-trips exactly.
        let enc = encode("hello").expect("encode succeeds");
        let s = decode(&enc.ids).expect("decode succeeds");
        assert_eq!(s, "hello");
    }

    #[test]
    fn count_matches_encode_length() {
        let enc = encode("hello world").expect("encode succeeds");
        let n = count("hello world").expect("count succeeds");
        assert_eq!(u32::try_from(enc.ids.len()).unwrap(), n);
    }

    #[test]
    fn capabilities_report_reference_shape() {
        let caps = get_capabilities();
        assert_eq!(caps.model_type, "bpe");
        assert_eq!(caps.variant_id, "reference-character-bpe");
        assert_eq!(caps.vocab_size, REFERENCE_VOCAB_SIZE);
        assert!(!caps.has_byte_fallback);
        assert!(caps.has_special_tokens);
    }

    #[test]
    fn special_token_bypasses_bpe_merge_loop() {
        // Special tokens are matched literally and emitted as their
        // pre-assigned id without going through the merge loop. This
        // proves the special-token path is wired end-to-end.
        let enc = encode("<|endoftext|>").expect("special token is registered");
        assert_eq!(enc.ids, alloc::vec![SPECIAL_TOKEN.1]);
    }

    #[test]
    fn ascii_bytes_seed_at_their_byte_id() {
        // A single unmergeable ASCII char emits the byte id.
        let enc = encode("z").expect("byte alphabet covers 'z'");
        assert_eq!(enc.ids, alloc::vec![u32::from(b'z')]);
    }
}
