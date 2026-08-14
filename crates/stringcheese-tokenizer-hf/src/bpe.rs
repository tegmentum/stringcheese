//! The BPE core: merge table, vocabulary, and tokenizer types.

use alloc::collections::{BTreeMap, BTreeSet, BinaryHeap};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::ops::Range;

use stringcheese_tokenizer::{Encoding, Tokenizer, TokenizerError};

/// Vocabulary index. `u32` is large enough for every shipped tokenizer
/// (tiktoken's `o200k_base`, the largest planned Wave-1 variant, uses
/// ~200 000 ids) and matches the tiktoken/HF convention.
pub type TokenId = u32;

/// Errors that can arise when building a [`BpeVocabulary`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VocabularyBuilderError {
    /// The same token id was declared twice with different byte strings.
    DuplicateTokenId(TokenId),
    /// The same byte string was declared twice with different token ids.
    DuplicateByteString,
}

/// The merge table for a BPE tokenizer.
///
/// Maps each merge pair `(left, right)` — stored internally as the
/// single *concatenation* `[left, right].concat()` — to a numeric rank.
/// Lower rank means the merge is applied *earlier* — that is, when two
/// adjacent pairs are both in the table, the one with the lower rank
/// wins. This matches the tiktoken / Hugging Face convention.
///
/// # Representation
///
/// Before Wave-14 this table was a `BTreeMap<(Vec<u8>, Vec<u8>), u32>`.
/// Every [`Self::rank`] call materialised the tuple key with two
/// `Vec<u8>` clones, and the map itself did O(log n) byte-slice
/// comparisons on the way to the leaf. The audit called this out as
/// the dominant per-token cost — the merge loop invokes `rank` once
/// per heap-pop validation *and* once per new-adjacency check.
///
/// The current representation matches tiktoken's storage: a
/// [`hashbrown::HashMap`] keyed on the *concatenation* of the two
/// pieces. Lookup allocates one `Vec<u8>` per call (down from two)
/// and hits an O(1) hash lookup afterwards.
///
/// The hasher is pinned to [`rustc_hash::FxBuildHasher`] rather than
/// hashbrown's default `ahash`. Merge keys are almost always 2-8
/// bytes — well below the length where `ahash`'s stronger mixing pays
/// for its per-call setup cost — and the encode hot loop pays one
/// [`Self::rank_slice`] call per candidate merge, so a leaner hash
/// step compounds. `FxHash`'s multiply-xor-shift kernel is what rustc
/// itself uses for its interning maps, and it is a zero-sized
/// `BuildHasher`, so `[BpeMergeTable]` carries no per-instance state
/// beyond the map itself.
#[derive(Debug, Default, Clone)]
pub struct BpeMergeTable {
    ranks: hashbrown::HashMap<Vec<u8>, u32, rustc_hash::FxBuildHasher>,
}

impl BpeMergeTable {
    /// Constructs an empty merge table. A tokenizer with no merges is
    /// well-defined: it emits one token per input byte.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ranks: hashbrown::HashMap::with_hasher(rustc_hash::FxBuildHasher),
        }
    }

    /// Inserts a merge with the given rank. Overwrites any prior entry
    /// for the same pair.
    ///
    /// Both arguments are consumed: `left` is reused as the underlying
    /// key allocation and `right` is drained into it, so a caller who
    /// already owns the two `Vec`s pays no extra copy to build the key.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "public API stability: this signature ships across the workspace and matches every existing caller; changing it to `&[u8]` would be a breaking change"
    )]
    pub fn insert(&mut self, left: Vec<u8>, right: Vec<u8>, rank: u32) {
        // Concatenate into a single key: the internal storage is keyed
        // on the merged bytes, not the pair-tuple. Reuse `left`'s
        // allocation (extend in place) so callers who already own the
        // left `Vec` pay nothing extra to build the key.
        let mut key = left;
        key.extend_from_slice(&right);
        self.ranks.insert(key, rank);
    }

    /// Returns the rank of `(left, right)`, or `None` if the pair is
    /// not in the table.
    ///
    /// This shape allocates one temporary `Vec<u8>` per lookup to build
    /// the concatenated key. Callers on the hot merge-loop path should
    /// prefer [`Self::rank_slice`] with an already-concatenated slice
    /// (typically a range into the enclosing word's byte buffer) which
    /// avoids the temporary entirely.
    #[must_use]
    pub fn rank(&self, left: &[u8], right: &[u8]) -> Option<u32> {
        let mut key = Vec::with_capacity(left.len() + right.len());
        key.extend_from_slice(left);
        key.extend_from_slice(right);
        self.ranks.get(&key).copied()
    }

    /// Returns the rank of the merge whose concatenated bytes equal
    /// `key`, or `None` if no such merge exists.
    ///
    /// This is the hot-path lookup used by the encode loop: the merge
    /// loop keeps pieces as `(start, len)` byte ranges into the current
    /// word, and every candidate merge is the slice
    /// `word_bytes[left_start..right_start + right_len]`. Passing that
    /// slice directly to [`hashbrown::HashMap::get`] via `Vec<u8>:
    /// Borrow<[u8]>` avoids the per-lookup `Vec` allocation the
    /// [`Self::rank`] convenience shape pays.
    #[must_use]
    pub fn rank_slice(&self, key: &[u8]) -> Option<u32> {
        self.ranks.get(key).copied()
    }

    /// Number of merges in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ranks.len()
    }

    /// `true` iff there are no merges.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty()
    }
}

/// The token-id ↔ byte-string vocabulary for a BPE tokenizer.
///
/// The vocabulary is a bijection: every id has exactly one byte-string
/// surface form, and vice versa. Byte strings are used (rather than
/// `String`s) because BPE operates at the byte level and merged pieces
/// may not correspond to complete UTF-8 scalars.
#[derive(Debug, Default, Clone)]
pub struct BpeVocabulary {
    // Forward: token id -> byte string.
    id_to_bytes: BTreeMap<TokenId, Vec<u8>>,
    // Reverse: byte string -> token id.
    bytes_to_id: BTreeMap<Vec<u8>, TokenId>,
}

impl BpeVocabulary {
    /// Constructs an empty vocabulary.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            id_to_bytes: BTreeMap::new(),
            bytes_to_id: BTreeMap::new(),
        }
    }

    /// Inserts a token into the vocabulary. Fails if the id or the
    /// byte string is already registered under a different mapping.
    ///
    /// # Errors
    ///
    /// Returns [`VocabularyBuilderError::DuplicateTokenId`] if `id` is
    /// already mapped to a different byte string;
    /// [`VocabularyBuilderError::DuplicateByteString`] if the byte
    /// string is already mapped to a different id. Re-inserting the
    /// exact same mapping is a no-op.
    pub fn insert(&mut self, id: TokenId, bytes: Vec<u8>) -> Result<(), VocabularyBuilderError> {
        if let Some(existing) = self.id_to_bytes.get(&id) {
            if existing != &bytes {
                return Err(VocabularyBuilderError::DuplicateTokenId(id));
            }
        }
        if let Some(&existing_id) = self.bytes_to_id.get(&bytes) {
            if existing_id != id {
                return Err(VocabularyBuilderError::DuplicateByteString);
            }
        }
        self.bytes_to_id.insert(bytes.clone(), id);
        self.id_to_bytes.insert(id, bytes);
        Ok(())
    }

    /// Looks up a token id by its byte surface form.
    #[must_use]
    pub fn id(&self, bytes: &[u8]) -> Option<TokenId> {
        self.bytes_to_id.get(bytes).copied()
    }

    /// Looks up the byte surface form of a token id.
    #[must_use]
    pub fn bytes(&self, id: TokenId) -> Option<&[u8]> {
        self.id_to_bytes.get(&id).map(Vec::as_slice)
    }

    /// Number of entries in the vocabulary.
    #[must_use]
    pub fn len(&self) -> usize {
        self.id_to_bytes.len()
    }

    /// `true` iff the vocabulary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.id_to_bytes.is_empty()
    }

    /// Adds an entry for every single-byte value 0..=255 that is not
    /// already registered. Convenience for building byte-level BPE
    /// vocabularies incrementally.
    ///
    /// `next_id` is the id to use for the *first* unregistered byte;
    /// subsequent bytes get consecutive ids. The function returns the
    /// next unused id after the operation.
    ///
    /// # Errors
    ///
    /// Propagates the underlying [`insert`](BpeVocabulary::insert)
    /// error if a conflict arises.
    ///
    /// # Panics
    ///
    /// Panics if the `TokenId` space is exhausted while assigning
    /// consecutive ids to the byte alphabet. With `TokenId = u32` this
    /// requires the caller to have started the operation less than 256
    /// ids from the top of the space, which is not a realistic
    /// configuration for any shipped tokenizer.
    pub fn ensure_byte_alphabet(
        &mut self,
        mut next_id: TokenId,
    ) -> Result<TokenId, VocabularyBuilderError> {
        for b in 0u8..=255 {
            let bytes = alloc::vec![b];
            if self.bytes_to_id.contains_key(&bytes) {
                continue;
            }
            self.insert(next_id, bytes)?;
            next_id = next_id
                .checked_add(1)
                .expect("token id space exhausted while filling byte alphabet");
        }
        Ok(next_id)
    }
}

/// Encode-time policy for *disallowed* special tokens, mirroring the
/// second argument of tiktoken's Python `encode(text, allowed_special,
/// disallowed_special)` API.
///
/// Rules, in order, at every position in the input:
///
/// 1. If a registered special-token surface appears in the input and it
///    is listed in `allowed_special` (the other argument to
///    [`BpeTokenizer::encode_with_special_policy`]), it is always
///    emitted as its reserved id. `allowed_special` always wins.
/// 2. Otherwise, if this variant is [`Self::All`], the appearance of
///    *any* registered special not in `allowed_special` triggers
///    [`TokenizerError::DisallowedSpecialToken`].
/// 3. Otherwise, if this variant is [`Self::These`], only surfaces in
///    the wrapped set trigger the error.
/// 4. Otherwise ([`Self::None`], or a surface not covered by 2 or 3),
///    the surface is *not* treated as a special: its bytes flow through
///    the BPE loop as regular text. This matches tiktoken's semantics.
///
/// Callers who want the crate's default "allow every registered
/// special" behaviour should pass their full registered-specials set as
/// `allowed_special` and [`Self::None`] as `disallowed_special` —
/// [`BpeTokenizer::encode`] does exactly that.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum DisallowedSpecials<'a> {
    /// Do not treat any surface as disallowed. Combined with an
    /// `allowed_special` set that lists every registered special, this
    /// is bit-for-bit the crate's pre-policy behaviour.
    None,
    /// Every registered special-token surface *not* in `allowed_special`
    /// is disallowed. This is tiktoken's Python-side default (the
    /// `disallowed_special="all"` sentinel).
    All,
    /// Only the surfaces in the wrapped set are disallowed. Any
    /// registered special not in this set — and not in
    /// `allowed_special` — passes through as regular text.
    These(&'a BTreeSet<&'a str>),
}

/// A pre-tokenizer pattern.
///
/// Three shapes are supported:
///
/// * [`Self::Literal`] — split at every occurrence of a fixed
///   separator string. The separator is discarded; adjacent matches
///   collapse; leading and trailing matches yield no empty word.
///   Available in every build; the fallback for `no_std` /
///   `alloc`-only environments where the full regex backend is not
///   linked.
/// * [`Self::Regex`] — apply a compiled
///   [`RegexPreTokenizer`](crate::pre_tokenizer::RegexPreTokenizer),
///   taking every non-overlapping match as a word. This is the shape
///   needed to reproduce tiktoken / Hugging Face pre-tokenization; only
///   available when the `std` feature is on because the underlying
///   `fancy-regex` backend requires `std::error::Error`.
/// * [`Self::ByteLevel`] — the byte-level pipeline used by GPT-2 and
///   Llama-family byte-level BPE: optionally prefix a leading space,
///   optionally split by a regex first, then map every input byte to
///   a printable Unicode char via
///   [`byte_level::encode_bytes`](crate::byte_level::encode_bytes)
///   before the BPE merge loop runs. See that module's docs for the
///   bijection this leans on. The nested `split` regex is optional so
///   callers can drop it (matching Hugging Face's `use_regex: false`
///   flag on the `ByteLevel` pre-tokenizer). Only available when the
///   `std` feature is on because it composes with
///   [`RegexPreTokenizer`](crate::pre_tokenizer::RegexPreTokenizer).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreTokenizerRegex {
    /// Split at every occurrence of a literal string. Adjacent separator
    /// matches collapse; leading and trailing matches are dropped;
    /// nothing empty is yielded.
    Literal(String),
    /// Split by matching a compiled regex against the input; each
    /// match becomes one word. Regions between matches are silently
    /// dropped (matching tiktoken / HF `re.findall(...)` semantics).
    #[cfg(feature = "std")]
    Regex(crate::pre_tokenizer::RegexPreTokenizer),
    /// Byte-level pipeline (GPT-2 / Llama): optionally prefix a space,
    /// optionally regex-split first, then remap every byte through
    /// [`byte_level::BYTES_TO_CHARS`](crate::byte_level::BYTES_TO_CHARS).
    /// The vocabulary and merges seen by the BPE core must be stored
    /// in *encoded* form (`" hello"` → `"Ġhello"`), which is what
    /// Hugging Face's `tokenizer.json` ships.
    #[cfg(feature = "std")]
    ByteLevel {
        /// If `true`, prepend `' '` (ASCII space) to the input region
        /// before splitting. Default in Hugging Face's `ByteLevel`
        /// config is `true`; the pre-tokenizer then produces
        /// `"Ġhello"` for an input of `"hello"`.
        add_prefix_space: bool,
        /// Optional regex to split the (possibly prefixed) input
        /// before byte-encoding. Hugging Face wraps its own copy of
        /// the GPT-2 regex here when `use_regex: true`; supplying
        /// [`RegexPreTokenizer::gpt2()`](crate::pre_tokenizer::RegexPreTokenizer::gpt2)
        /// reproduces that behaviour. When `None`, the entire input
        /// region is byte-encoded as a single chunk.
        split: Option<crate::pre_tokenizer::RegexPreTokenizer>,
    },
    /// Hugging Face `Punctuation` — split every Unicode-punctuation
    /// character (any Unicode category `P*`). The behaviour argument
    /// governs how the punctuation matches relate to the emitted
    /// pieces — see
    /// [`SplitDelimiterBehavior`](crate::pre_tokenizer::SplitDelimiterBehavior)
    /// for the five modes. Falcon-family checkpoints ship
    /// `Punctuation { behavior: Contiguous }` as the first entry of a
    /// pre-tokenizer sequence; the default is
    /// [`crate::pre_tokenizer::SplitDelimiterBehavior::Isolated`],
    /// matching HF's own default. Gated on `std` (like `Regex` /
    /// `ByteLevel`) because the runtime driver relies on the same
    /// regex backend those variants use.
    #[cfg(feature = "std")]
    Punctuation(crate::pre_tokenizer::SplitDelimiterBehavior),
    /// Hugging Face `Digits` — split runs of decimal digits. When
    /// `individual_digits` is `true`, every digit becomes its own
    /// piece (each digit is a match); when `false`, contiguous digit
    /// runs are single pieces. Both shapes use `Isolated` behaviour —
    /// non-digit regions are kept as pieces alongside the digit
    /// matches. HF's `Digits { individual_digits: false }` is what
    /// Falcon-family checkpoints wire between the `ByteLevel` stage
    /// and a `Split(Regex="[0-9]{3}")` cleanup. Gated on `std` for the
    /// same reason as [`Self::Punctuation`].
    #[cfg(feature = "std")]
    Digits {
        /// If `true`, each digit becomes its own piece; otherwise
        /// runs of digits stay together as single pieces.
        individual_digits: bool,
    },
    /// Hugging Face `Split { pattern: Regex, behavior, invert }` with a
    /// general behaviour argument. The `invert` field is not accepted
    /// here — every real HF checkpoint ships `invert: false`, and the
    /// runtime does not implement inversion. Callers that need HF's
    /// `Isolated` behaviour on a `Split(Regex)` block can also pick
    /// [`Self::Regex`], which is byte-for-byte equivalent to
    /// `Self::Split { regex, behavior: Isolated }` when the match
    /// gaps are ignored (tiktoken semantics); the two split when
    /// non-match regions matter.
    #[cfg(feature = "std")]
    Split {
        /// The compiled pattern to match.
        regex: crate::pre_tokenizer::RegexPreTokenizer,
        /// How the matches relate to the emitted pieces.
        behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
    },
    /// An ordered sequence of pre-tokenizer stages, applied
    /// left-to-right. Each stage receives the previous stage's output
    /// as its input regions and further splits them. HF's
    /// `pre_tokenizer.type == "Sequence"` block materialises into this
    /// variant when it has more than one operative child. Empty
    /// sequences are permitted and behave as the identity. Gated on
    /// `std` — the children usually contain regex-backed variants that
    /// require it.
    #[cfg(feature = "std")]
    Sequence(Vec<PreTokenizerRegex>),
}

/// Per-entry flags for an added-vocabulary token, mirroring Hugging
/// Face's `AddedToken` struct.
///
/// Populated from a `tokenizer.json` `added_tokens[*]` entry by the
/// HF loader via [`BpeTokenizer::with_added_vocab`]. Governs how the
/// entry's surface interacts with the extract-and-normalize pipeline:
///
/// * `normalized == false` (HF's default for `special == true`
///   entries) — the surface is matched against the RAW caller input,
///   before the normalizer runs.
/// * `normalized == true` (HF's default for `special == false`
///   entries) — the surface is matched against each per-region
///   NORMALIZED text, after normalization runs.
/// * `lstrip == true` — any leading whitespace immediately preceding
///   the matched surface is absorbed into the match span.
/// * `rstrip == true` — any trailing whitespace immediately following
///   the matched surface is absorbed into the match span.
/// * `special == true` — the entry counts as a "special" token for
///   downstream consumers (the emitted piece's `special_mask` bit is
///   set); `false` entries still get their assigned id but do not.
///
/// The four flags compose independently. See
/// [`BpeTokenizer::encode`] for the exact match-point semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the four flags mirror Hugging Face's AddedToken struct on the wire byte-for-byte; \
              collapsing them into an enum would break parity with the on-disk shape"
)]
pub struct AddedTokenFlags {
    /// The token id emitted when the surface matches.
    pub id: TokenId,
    /// `true` when the surface is matched post-normalization.
    pub normalized: bool,
    /// `true` when leading whitespace is absorbed into the match span.
    pub lstrip: bool,
    /// `true` when trailing whitespace is absorbed into the match span.
    pub rstrip: bool,
    /// `true` when the token is registered as a "special" for the
    /// downstream `special_mask`.
    pub special: bool,
}

impl AddedTokenFlags {
    /// Convenience: the "special, non-normalized, no strip" defaults
    /// that reproduce the pre-flag behaviour of
    /// [`BpeTokenizer::with_special_tokens`]. Every entry passed
    /// through the legacy `with_special_tokens` builder implicitly
    /// picks up these flags.
    #[must_use]
    pub const fn legacy_special(id: TokenId) -> Self {
        Self {
            id,
            normalized: false,
            lstrip: false,
            rstrip: false,
            special: true,
        }
    }
}

impl PreTokenizerRegex {
    /// Constructs a literal-string pre-tokenizer.
    #[must_use]
    pub fn literal(s: impl Into<String>) -> Self {
        Self::Literal(s.into())
    }

    /// Wraps a compiled [`RegexPreTokenizer`](crate::pre_tokenizer::RegexPreTokenizer)
    /// as a pre-tokenizer pattern.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn regex(regex: crate::pre_tokenizer::RegexPreTokenizer) -> Self {
        Self::Regex(regex)
    }

    /// Builds a byte-level pre-tokenizer.
    ///
    /// Callers who want to reproduce Hugging Face's default `ByteLevel`
    /// (used by GPT-2, `RoBERTa`, and every `ByteLevel`-based Llama
    /// derivative) should pass `add_prefix_space = true` and
    /// `Some(RegexPreTokenizer::gpt2())`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn byte_level(
        add_prefix_space: bool,
        split: Option<crate::pre_tokenizer::RegexPreTokenizer>,
    ) -> Self {
        Self::ByteLevel {
            add_prefix_space,
            split,
        }
    }

    /// Builds a Hugging Face `Punctuation` pre-tokenizer with the
    /// given behaviour. HF's own default (bare `{"type":"Punctuation"}`)
    /// is [`SplitDelimiterBehavior::Isolated`](crate::pre_tokenizer::SplitDelimiterBehavior::Isolated).
    #[cfg(feature = "std")]
    #[must_use]
    pub const fn punctuation(behavior: crate::pre_tokenizer::SplitDelimiterBehavior) -> Self {
        Self::Punctuation(behavior)
    }

    /// Builds a Hugging Face `Digits` pre-tokenizer.
    #[cfg(feature = "std")]
    #[must_use]
    pub const fn digits(individual_digits: bool) -> Self {
        Self::Digits { individual_digits }
    }

    /// Builds a `Split(Regex, behavior)` pre-tokenizer — the general
    /// form of HF's `Split` block.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn split(
        regex: crate::pre_tokenizer::RegexPreTokenizer,
        behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
    ) -> Self {
        Self::Split { regex, behavior }
    }

    /// Wraps a list of stages in a [`Self::Sequence`] pipeline.
    #[cfg(feature = "std")]
    #[must_use]
    pub const fn sequence(stages: Vec<PreTokenizerRegex>) -> Self {
        Self::Sequence(stages)
    }

    /// `true` when this pipeline, or any of its stages, is (or contains)
    /// a `ByteLevel` block.
    ///
    /// Used by the merge-loop seed strategy inside
    /// `BpeTokenizer::encode_word_bpe` (private): byte-level encoding
    /// produces multi-byte chars from a single input byte, and the
    /// merge loop must seed one piece per char (not per byte) so the
    /// encoded chars stay atomic. That property propagates into any
    /// `Sequence` pipeline that contains a `ByteLevel` stage.
    ///
    /// Always `false` on `alloc`-only builds — the `ByteLevel`,
    /// `Sequence`, `Punctuation`, `Digits`, and `Split` variants are
    /// all gated behind `std`, so the only matchable variant is
    /// [`Self::Literal`].
    #[must_use]
    pub fn contains_byte_level(&self) -> bool {
        #[cfg(feature = "std")]
        match self {
            Self::Literal(_)
            | Self::Regex(_)
            | Self::Punctuation(_)
            | Self::Digits { .. }
            | Self::Split { .. } => false,
            Self::ByteLevel { .. } => true,
            Self::Sequence(stages) => stages.iter().any(Self::contains_byte_level),
        }
        #[cfg(not(feature = "std"))]
        match self {
            Self::Literal(_) => false,
        }
    }
}

/// Decoder applied to the concatenated byte string produced by
/// [`BpeTokenizer::decode`] before it is interpreted as UTF-8.
///
/// # Variants
///
/// The two legacy variants — [`Self::Passthrough`] and [`Self::ByteLevel`]
/// — operate on the tokenizer's concatenated byte buffer: the ids are
/// looked up in the vocabulary, the raw bytes are joined into one buffer,
/// and (for `ByteLevel`) the byte↔char bijection is reversed before UTF-8
/// interpretation. That is the shape tiktoken / GPT-2 style decoders need
/// and it is what [`BpeTokenizer::decode`] historically applied.
///
/// The "chain" variants — [`Self::Sequence`], [`Self::Replace`],
/// [`Self::Fuse`], [`Self::Strip`], and [`Self::ByteFallback`] — mirror
/// Hugging Face's per-token `Decoder` trait. They operate on a
/// `Vec<String>` (one entry per input id) built by looking each id up in
/// the vocabulary (or in the special-token map) and interpreting the
/// surface bytes as UTF-8. Each stage transforms the list; the final
/// list is joined with an empty separator to produce the decoded string.
///
/// The chain shape lets the loader materialise Llama-2's
/// `Sequence[Replace(▁→ ), ByteFallback, Fuse, Strip(" ",1,0)]` decoder
/// exactly as HF stores it, so [`BpeTokenizer::decode`] produces
/// byte-for-byte identical output to
/// `transformers.AutoTokenizer.decode`.
///
/// When a chain decoder is configured, the model-side byte-fallback
/// reassembly in [`BpeTokenizer::decode`] is bypassed — the chain's
/// [`Self::ByteFallback`] stage does the run reassembly itself. This
/// keeps the two mechanisms from double-applying.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Decoder {
    /// No post-processing: emit the concatenated byte string as UTF-8.
    #[default]
    Passthrough,
    /// Reverse the byte-level char↔byte mapping before UTF-8 decoding.
    /// Every char in the range `U+0000..=U+0143` is looked up in the
    /// inverse table; unknown chars are passed through as their own
    /// UTF-8 bytes so the decoder never panics on an input that was not
    /// encoded by [`PreTokenizerRegex::ByteLevel`]. This is the shape
    /// required to round-trip GPT-2 / Llama byte-level tokenizers back
    /// to their original raw input.
    ByteLevel,
    /// Compose several decoders left-to-right. Each stage takes the
    /// previous stage's output (a `Vec<String>`) and returns a new list.
    /// Matches HF's own `Decoder::Sequence`.
    Sequence(Vec<Decoder>),
    /// Per-token literal replace. Every occurrence of `pattern` in each
    /// token's surface string is substituted with `content`. Matches
    /// HF's on-disk `{"type":"Replace","pattern":{"String":"▁"},
    /// "content":" "}` shape (Llama-2 uses this to unmap the
    /// `SentencePiece` space mark). Regex patterns are not honoured —
    /// this crate keeps the decoder chain literal-only because every
    /// real HF checkpoint's decoder-side `Replace` uses a literal.
    Replace {
        /// The literal search string.
        pattern: String,
        /// The replacement string.
        content: String,
    },
    /// Concatenate every token string into a single-entry list.
    /// Matches HF's `Decoder::Fuse` — the default for the byte-level
    /// family and the third stage of the Llama-2 chain.
    Fuse,
    /// Strip up to `start` leading and up to `stop` trailing
    /// occurrences of `content` from each token's surface string.
    /// Llama-2 uses `Strip{content: ' ', start: 1, stop: 0}` to remove
    /// the single leading space its Prepend normalizer inserted at
    /// encode time. Matches HF's `Decoder::Strip` semantics (one char
    /// per removal, up to the declared count, stopping early on a
    /// non-match).
    Strip {
        /// The character to strip.
        content: char,
        /// Maximum leading occurrences to remove.
        start: usize,
        /// Maximum trailing occurrences to remove.
        stop: usize,
    },
    /// `SentencePiece` byte-fallback reassembly at the decoder-chain
    /// level. Scans the token list for runs of `<0xXX>` surface
    /// strings, decodes each hex pair to a byte, and interprets each
    /// completed run as UTF-8 — producing one new list entry per
    /// completed run when the bytes are valid UTF-8, or one U+FFFD
    /// per invalid byte otherwise (matching HF's `Decoder::ByteFallback`
    /// per-invalid-byte replacement policy). Distinct from the
    /// model-side byte-fallback path in [`BpeTokenizer::decode`] — the
    /// model-side path fires on ids (via [`BpeTokenizer::with_byte_fallback`]);
    /// this chain-side path fires on the token *surface strings* the
    /// decoder chain sees.
    ByteFallback,
}

impl Decoder {
    /// `true` for the variants that operate on the token list rather
    /// than on the concatenated byte buffer. The routing helper in
    /// [`BpeTokenizer::decode`] uses this to decide which path to
    /// take.
    #[must_use]
    pub fn is_chain(&self) -> bool {
        matches!(
            self,
            Self::Sequence(_)
                | Self::Replace { .. }
                | Self::Fuse
                | Self::Strip { .. }
                | Self::ByteFallback
        )
    }

    /// Run this decoder's chain semantics over `tokens`.
    /// [`Self::Passthrough`] and [`Self::ByteLevel`] are identity here
    /// — the "chain" path caller must not invoke this method on them.
    /// See [`Self::is_chain`].
    #[must_use]
    pub fn apply_chain(&self, tokens: Vec<String>) -> Vec<String> {
        match self {
            Self::Passthrough | Self::ByteLevel => tokens,
            Self::Sequence(children) => {
                let mut current = tokens;
                for c in children {
                    current = c.apply_chain(current);
                }
                current
            }
            Self::Replace { pattern, content } => tokens
                .into_iter()
                .map(|t| t.replace(pattern.as_str(), content.as_str()))
                .collect(),
            Self::Fuse => {
                let mut fused = String::new();
                for t in &tokens {
                    fused.push_str(t);
                }
                alloc::vec![fused]
            }
            Self::Strip {
                content,
                start,
                stop,
            } => tokens
                .into_iter()
                .map(|t| decoder_strip_one_char(&t, *content, *start, *stop))
                .collect(),
            Self::ByteFallback => decoder_byte_fallback_chain(tokens),
        }
    }
}

/// Strip up to `start` leading and up to `stop` trailing occurrences of
/// `content` from `s`. Stops early at the first non-matching character
/// on either side — matches HF's `Decoder::Strip::decode_chain`
/// per-token behaviour byte for byte.
fn decoder_strip_one_char(s: &str, content: char, start: usize, stop: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut lo = 0usize;
    while lo < n && lo < start && chars[lo] == content {
        lo += 1;
    }
    let mut hi = n;
    let mut trimmed = 0usize;
    while hi > lo && trimmed < stop && chars[hi - 1] == content {
        hi -= 1;
        trimmed += 1;
    }
    chars[lo..hi].iter().collect()
}

/// The decoder-chain `ByteFallback` stage — scan `tokens` for runs of
/// `<0xXX>` surface strings, reassemble each run into a UTF-8 string
/// (lossy: one U+FFFD per invalid byte, matching HF), and emit the
/// reassembled entry in place of the run.
fn decoder_byte_fallback_chain(tokens: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut byte_run: Vec<u8> = Vec::new();
    for t in tokens {
        if let Some(b) = decoder_byte_fallback_surface(&t) {
            byte_run.push(b);
        } else {
            if !byte_run.is_empty() {
                flush_decoder_byte_run(&mut byte_run, &mut out);
            }
            out.push(t);
        }
    }
    if !byte_run.is_empty() {
        flush_decoder_byte_run(&mut byte_run, &mut out);
    }
    out
}

/// Push the accumulated byte run onto `out` and clear the buffer.
/// Runs that are not valid UTF-8 fan out to one U+FFFD per byte, which
/// is what HF's own `ByteFallback::decode_chain` emits.
fn flush_decoder_byte_run(byte_run: &mut Vec<u8>, out: &mut Vec<String>) {
    match core::str::from_utf8(byte_run) {
        Ok(s) => out.push(String::from(s)),
        Err(_) => {
            for _ in byte_run.iter() {
                out.push(String::from("\u{FFFD}"));
            }
        }
    }
    byte_run.clear();
}

/// Recognise a `<0xXX>` surface string and return the byte value, or
/// `None` if `s` is not one of the 256 reserved byte-fallback surfaces.
/// Kept module-local to the runtime so the loader-side sibling in
/// [`crate::hf::parse_byte_fallback_surface`] can stay `pub(crate)`.
fn decoder_byte_fallback_surface(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 6 {
        return None;
    }
    if bytes[0] != b'<' || bytes[1] != b'0' || bytes[2] != b'x' || bytes[5] != b'>' {
        return None;
    }
    let hi = decoder_hex_digit(bytes[3])?;
    let lo = decoder_hex_digit(bytes[4])?;
    Some((hi << 4) | lo)
}

/// Decode one ASCII hex digit to its 0..=15 value. Mirrors
/// [`crate::hf::decode_hex_digit`] byte for byte; a `const fn` so the
/// call is inlineable.
const fn decoder_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// A BPE tokenizer over a caller-supplied merge table and vocabulary.
///
/// Constructed via [`BpeTokenizer::from_parts`]; special tokens and a
/// pre-tokenizer pattern attach through the builder methods
/// [`with_special_tokens`](Self::with_special_tokens) and
/// [`with_pre_tokenizer`](Self::with_pre_tokenizer).
///
/// # Examples
///
/// Build a byte-level tokenizer with a single merge rule and encode a
/// three-character string:
///
/// ```
/// use stringcheese_tokenizer::Tokenizer;
/// use stringcheese_tokenizer_hf::{BpeMergeTable, BpeTokenizer, BpeVocabulary};
///
/// let mut vocab = BpeVocabulary::new();
/// vocab.ensure_byte_alphabet(0).unwrap();
/// vocab.insert(256, b"ca".to_vec()).unwrap();
///
/// let mut merges = BpeMergeTable::new();
/// merges.insert(b"c".to_vec(), b"a".to_vec(), 0);
///
/// let tok = BpeTokenizer::from_parts(merges, vocab);
/// let enc = tok.encode("cat").unwrap();
/// assert_eq!(enc.ids, vec![256, u32::from(b't')]);
/// assert_eq!(tok.decode(&enc.ids).unwrap(), "cat");
/// ```
#[derive(Debug, Clone)]
pub struct BpeTokenizer {
    merges: BpeMergeTable,
    vocab: BpeVocabulary,
    special_tokens: BTreeMap<String, TokenId>,
    /// Per-entry flags for every entry in [`Self::special_tokens`],
    /// mirroring Hugging Face's `AddedToken` struct. Populated by the
    /// HF loader via [`Self::with_added_vocab`]; absent (empty) when
    /// callers only use [`Self::with_special_tokens`], in which case
    /// every registered surface is treated with
    /// [`AddedTokenFlags::legacy_special`] defaults (special, matched
    /// on the raw input, no lstrip/rstrip). The map's keys are a
    /// superset of [`Self::special_tokens`]' keys — an entry present
    /// here that is missing from `special_tokens` is a builder bug
    /// and encode still works (the flags are ignored on unmatched
    /// surfaces).
    added_token_flags: BTreeMap<String, AddedTokenFlags>,
    pre_tokenizer_pattern: Option<PreTokenizerRegex>,
    /// Optional [`PreTokenizerSequence`] applied to each between-specials
    /// region *before* the byte / char seeding and merge loop run. This
    /// is how SentencePiece-descended checkpoints that ship as
    /// `model.type == "BPE"` (Mistral-7B-v0.1 is the exemplar — HF's
    /// `"pre_tokenizer": { "type": "Metaspace", "prepend_scheme": "first",
    /// "split": false }`) get their word-initial `▁` marking wired
    /// through the BPE pipeline. When `Some`, each piece the sequence
    /// produces is fed independently into the (regex-or-not)
    /// [`Self::pre_tokenizer_pattern`] + seed + merge pipeline; the
    /// resulting ids are concatenated in order. Held here rather than
    /// folded into [`Self::pre_tokenizer_pattern`] because the two
    /// pipelines are conceptually different (regex/byte-level split vs.
    /// SentencePiece-style substitution): every real checkpoint uses
    /// exactly one of the two, and keeping them separate lets the
    /// existing tiktoken-style byte-level path stay untouched.
    ///
    /// Available under the `std` Cargo feature (the same gate
    /// [`PreTokenizerSequence`] lives behind).
    #[cfg(feature = "std")]
    pre_tokenizer_sequence: Option<crate::pre_tokenizer::PreTokenizerSequence>,
    decoder: Decoder,
    /// Optional Unicode normalization step run *before* pre-tokenization
    /// and the merge loop. See [`crate::normalizer`] for the semantics
    /// and the support matrix. Held as a raw [`String`] intermediate on
    /// every encode when set, so callers who don't wire one up pay
    /// nothing.
    #[cfg(feature = "hf-normalizer")]
    normalizer: Option<crate::normalizer::Normalizer>,
    /// Optional post-processing step applied to the produced
    /// [`Encoding`] before it leaves [`Self::encode`]. See
    /// [`crate::post_processor`] for the shape.
    post_processor: crate::post_processor::PostProcessor,
    /// Optional byte-fallback table: `byte_fallback[b]` is the vocab
    /// id of the `<0xBB>` token reserved for byte value `b`. Populated
    /// by [`Self::with_byte_fallback`]; `None` disables the fallback
    /// and leaves the encode path on its previous
    /// `UnknownToken`-on-OOV behaviour.
    ///
    /// Boxed so a clone of `BpeTokenizer` avoids a 1 KiB memcpy on the
    /// common (byte-fallback-off) path — mirrors the Unigram-side
    /// storage shape in [`crate::hf::UnigramTokenizer`].
    byte_fallback: Option<alloc::boxed::Box<[TokenId; 256]>>,
    /// Optional truncation configuration applied to the finished
    /// [`Encoding`] before it leaves [`Self::encode`] /
    /// [`Self::encode_pair`]. See
    /// [`stringcheese_tokenizer::truncation::TruncationConfig`] for the
    /// field shape. When `None` the truncation step is skipped.
    truncation: Option<stringcheese_tokenizer::truncation::TruncationConfig>,
    /// Optional padding configuration applied to a batch of finished
    /// [`Encoding`]s by [`Self::encode_batch`]. When `None` the
    /// padding step is skipped and encodings retain their per-input
    /// lengths.
    padding: Option<stringcheese_tokenizer::padding::PaddingConfig<TokenId>>,
}

impl BpeTokenizer {
    /// Builds a tokenizer from a merge table and vocabulary.
    #[must_use]
    pub fn from_parts(merges: BpeMergeTable, vocab: BpeVocabulary) -> Self {
        Self {
            merges,
            vocab,
            special_tokens: BTreeMap::new(),
            added_token_flags: BTreeMap::new(),
            pre_tokenizer_pattern: None,
            #[cfg(feature = "std")]
            pre_tokenizer_sequence: None,
            decoder: Decoder::Passthrough,
            #[cfg(feature = "hf-normalizer")]
            normalizer: None,
            post_processor: crate::post_processor::PostProcessor::None,
            byte_fallback: None,
            truncation: None,
            padding: None,
        }
    }

    /// Attaches (or replaces) the special-token map.
    ///
    /// Special tokens are matched *literally* in the input (longest
    /// match first) and emitted as their pre-assigned ids without
    /// participating in the BPE merge loop.
    ///
    /// Every entry passed here implicitly picks up the
    /// [`AddedTokenFlags::legacy_special`] flag set: `special = true`,
    /// `normalized = false`, no `lstrip` / `rstrip`. Callers who need
    /// the full HF flag semantics (`normalized`, `lstrip`, `rstrip`,
    /// `special: false`) should use [`Self::with_added_vocab`]
    /// instead — this legacy builder stays as-is so every existing
    /// caller (tests, tiktoken loader, etc.) is unaffected.
    #[must_use]
    pub fn with_special_tokens(mut self, tokens: BTreeMap<String, TokenId>) -> Self {
        self.special_tokens = tokens;
        // Rebuild the flags map so it stays consistent with the
        // special_tokens map. Legacy callers get the "special=true,
        // normalized=false, no strip" default per surface.
        self.added_token_flags = self
            .special_tokens
            .iter()
            .map(|(s, &id)| (s.clone(), AddedTokenFlags::legacy_special(id)))
            .collect();
        self
    }

    /// Attaches (or replaces) the added-vocabulary table with per-entry
    /// [`AddedTokenFlags`].
    ///
    /// The loader passes every `added_tokens[*]` entry from the HF
    /// config here — both `special: true` (BOS / EOS / chat markers)
    /// and `special: false` (Phi-2's whitespace-run compression ids,
    /// Phi-3's `</s>` with `rstrip: true`). The flag semantics mirror
    /// Hugging Face's `added_vocabulary::extract_and_normalize`
    /// pipeline byte for byte — see [`AddedTokenFlags`] for the
    /// per-flag contract and [`Self::encode`] for the two-phase
    /// match-point semantics (raw-input scan for `normalized == false`
    /// entries, per-region normalized scan for `normalized == true`).
    ///
    /// Passing an empty map clears the added-vocab table. This
    /// supersedes any prior [`Self::with_special_tokens`] call — the
    /// two writes to the same underlying `special_tokens` field are
    /// last-wins.
    #[must_use]
    pub fn with_added_vocab(mut self, entries: BTreeMap<String, AddedTokenFlags>) -> Self {
        self.special_tokens = entries
            .iter()
            .map(|(surface, flags)| (surface.clone(), flags.id))
            .collect();
        self.added_token_flags = entries;
        self
    }

    /// Attaches (or replaces) the pre-tokenizer pattern.
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pattern: PreTokenizerRegex) -> Self {
        self.pre_tokenizer_pattern = Some(pattern);
        self
    }

    /// Attaches (or replaces) a
    /// [`PreTokenizerSequence`](crate::pre_tokenizer::PreTokenizerSequence)
    /// applied to each between-specials region *before* the (optional)
    /// [`Self::with_pre_tokenizer`] pattern and the merge loop run.
    ///
    /// This is the SentencePiece-style [`Metaspace`](crate::Metaspace)
    /// pipeline that Llama-family BPE checkpoints (Mistral-7B-v0.1) wire
    /// on the pre-tokenizer side rather than the normalizer side (which
    /// is how Llama-2 does it). Each piece produced by the sequence is
    /// fed independently into the seed + merge loop and its ids are
    /// concatenated in order; empty pieces are skipped.
    ///
    /// The parameter is `impl Into<PreTokenizerSequence>`, so callers
    /// can pass either a bare [`Metaspace`](crate::Metaspace) (wrapped
    /// via [`PreTokenizerSequence::from`](crate::PreTokenizerSequence))
    /// or a full sequence.
    ///
    /// Available under the `std` Cargo feature.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn with_pre_tokenizer_sequence(
        mut self,
        pre_tokenizer_sequence: impl Into<crate::pre_tokenizer::PreTokenizerSequence>,
    ) -> Self {
        self.pre_tokenizer_sequence = Some(pre_tokenizer_sequence.into());
        self
    }

    /// Attaches (or replaces) the decoder strategy applied by
    /// [`decode`](Self::decode) after concatenating the pieces' byte
    /// strings and before interpreting the buffer as UTF-8.
    ///
    /// Callers who ship a tokenizer whose vocab is in *encoded* form
    /// (Hugging Face byte-level BPE — every vocabulary surface string
    /// has already been run through the `ByteLevel` byte↔char mapping)
    /// must set [`Decoder::ByteLevel`] so `decode` reverses the map
    /// and gives back the caller's original input. See the [`Decoder`]
    /// enum's docs for the full semantics.
    #[must_use]
    pub fn with_decoder(mut self, decoder: Decoder) -> Self {
        self.decoder = decoder;
        self
    }

    /// Attaches (or replaces) the Unicode normalizer.
    ///
    /// The normalizer runs on the raw input string *before* the
    /// pre-tokenizer, matching HF `tokenizers-rs`' pipeline order:
    /// `normalize -> pre-tokenize -> BPE -> post-process`. Offsets
    /// reported on the resulting [`Encoding`] are into the
    /// *normalized* text, not the caller's original — recovering
    /// original-input offsets would require the on-the-side
    /// `NormalizedString` bookkeeping that this crate does not ship.
    ///
    /// Available under the `hf-normalizer` Cargo feature (which is
    /// enabled by default when `hf-tokenizer` is on).
    #[cfg(feature = "hf-normalizer")]
    #[must_use]
    pub fn with_normalizer(mut self, normalizer: crate::normalizer::Normalizer) -> Self {
        self.normalizer = Some(normalizer);
        self
    }

    /// Attaches (or replaces) the post-processor.
    ///
    /// The post-processor runs on the finished [`Encoding`] before
    /// [`Self::encode`] returns it. See
    /// [`crate::post_processor::PostProcessor`] for the shape.
    #[must_use]
    pub fn with_post_processor(
        mut self,
        post_processor: crate::post_processor::PostProcessor,
    ) -> Self {
        self.post_processor = post_processor;
        self
    }

    /// Enable `SentencePiece`'s byte-fallback mechanism on this
    /// tokenizer.
    ///
    /// `byte_fallback[b]` must be the vocab id of the reserved
    /// `<0xBB>` token for byte value `b`. Once configured,
    /// [`Self::encode`] emits a run of these ids whenever a character
    /// has no vocab-only path (its raw UTF-8 bytes are fanned out into
    /// one fallback id per byte) instead of failing with
    /// [`TokenizerError::UnknownToken`]. [`Self::decode`] is likewise
    /// updated: consecutive byte-fallback ids are accumulated into a
    /// UTF-8 buffer and flushed as `String::from_utf8_lossy` when a
    /// non-byte-fallback token breaks the run.
    ///
    /// Real Llama-2 / Mistral / Qwen `tokenizer.json` blobs ship as
    /// `model.type == "BPE"` with `byte_fallback: true` and 256
    /// reserved `<0xXX>` tokens at ids 3..258. The HF loader
    /// ([`crate::hf::to_bpe_tokenizer`]) calls this builder
    /// automatically when it sees that flag; callers who assemble a
    /// tokenizer manually via [`Self::from_parts`] can call it
    /// themselves after resolving the 256 ids from their own vocab.
    #[must_use]
    pub fn with_byte_fallback(mut self, byte_fallback: [TokenId; 256]) -> Self {
        self.byte_fallback = Some(alloc::boxed::Box::new(byte_fallback));
        self
    }

    /// Attach (or replace) the truncation configuration.
    ///
    /// The config is applied to every [`Encoding`] produced by
    /// [`Self::encode`] and to every pair encoding produced by
    /// [`stringcheese_tokenizer::Tokenizer::encode_pair`], immediately
    /// after the post-processor runs. Pair encoding uses
    /// [`stringcheese_tokenizer::truncation::truncate_pair`] on the
    /// two primary encodings before the pair template splices in the
    /// specials — matching HF's own ordering.
    #[must_use]
    pub fn with_truncation(
        mut self,
        truncation: stringcheese_tokenizer::truncation::TruncationConfig,
    ) -> Self {
        self.truncation = Some(truncation);
        self
    }

    /// Attach (or replace) the padding configuration.
    ///
    /// The config is applied to every batch produced by
    /// [`stringcheese_tokenizer::Tokenizer::encode_batch`], via
    /// [`stringcheese_tokenizer::padding::pad_batch`]. Single-encoding
    /// [`Self::encode`] never pads (there is no "batch max" to align
    /// against).
    #[must_use]
    pub fn with_padding(
        mut self,
        padding: stringcheese_tokenizer::padding::PaddingConfig<TokenId>,
    ) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Read-only access to the configured truncation, if any.
    #[must_use]
    pub fn truncation(&self) -> Option<&stringcheese_tokenizer::truncation::TruncationConfig> {
        self.truncation.as_ref()
    }

    /// Read-only access to the configured padding, if any.
    #[must_use]
    pub fn padding(&self) -> Option<&stringcheese_tokenizer::padding::PaddingConfig<TokenId>> {
        self.padding.as_ref()
    }

    /// Read-only access to the configured normalizer, if any.
    #[cfg(feature = "hf-normalizer")]
    #[must_use]
    pub fn normalizer(&self) -> Option<&crate::normalizer::Normalizer> {
        self.normalizer.as_ref()
    }

    /// Read-only access to the configured
    /// [`PreTokenizerSequence`](crate::pre_tokenizer::PreTokenizerSequence),
    /// if any. Wired via [`Self::with_pre_tokenizer_sequence`]; see that
    /// builder's docs for the semantics.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn pre_tokenizer_sequence(&self) -> Option<&crate::pre_tokenizer::PreTokenizerSequence> {
        self.pre_tokenizer_sequence.as_ref()
    }

    /// Read-only access to the configured post-processor.
    #[must_use]
    pub fn post_processor(&self) -> &crate::post_processor::PostProcessor {
        &self.post_processor
    }

    /// `true` when [`Self::with_byte_fallback`] has been called on
    /// this tokenizer and the byte-fallback path is active.
    #[must_use]
    pub const fn byte_fallback_enabled(&self) -> bool {
        self.byte_fallback.is_some()
    }

    /// Encode `text` while explicitly controlling whether the
    /// post-processor injects special tokens.
    ///
    /// Behaves identically to [`Tokenizer::encode`](Self::encode) —
    /// including running the normalizer, pre-tokenizer, and BPE loop —
    /// but bypasses the post-processor's special-token splice when
    /// `add_special_tokens == false`. This matches HF's
    /// `Tokenizer::encode(input, add_special_tokens)` two-arg form,
    /// which callers reach for when they want the raw BPE output
    /// without the wrapping BOS/EOS.
    pub fn encode_with_special(
        &self,
        text: &str,
        add_special_tokens: bool,
    ) -> Result<Encoding<TokenId>, TokenizerError> {
        // Special-token extraction runs on the RAW input; the
        // normalizer is applied per between-specials region inside
        // `encode_pieces_with_policy`. This mirrors HF's
        // `added_vocabulary::extract_and_normalize` ordering — without
        // extract-before-normalize a Llama-family
        // `Sequence[Prepend("▁"), Replace(" " → "▁")]` normalizer would
        // prepend `▁` to `"<s>hi</s>"` before the specials matcher had
        // a chance to see the raw surface, so the leading `▁` would
        // BPE-encode into a spurious id 29871 before `<s>` was matched.
        // Also fixes the empty-input edge case: a raw empty input has
        // no regions to normalize, so the `Prepend` normalizer's
        // injected `▁` is not emitted (matching upstream transformers).
        // Mirrors [`crate::wordpiece::WordPieceTokenizer::encode_ids_raw`]
        // and [`crate::hf::UnigramTokenizer::encode`].
        let pieces = self.encode_pieces(text)?;
        let mut enc = Encoding::new();
        enc.ids.reserve(pieces.len());
        enc.offsets.reserve(pieces.len());
        enc.special_mask.reserve(pieces.len());
        for (id, range, special) in pieces {
            enc.ids.push(id);
            enc.offsets.push(range);
            enc.special_mask.push(special);
        }
        Ok(self.post_processor.apply(&enc, add_special_tokens))
    }

    /// Encode `text` under an explicit tiktoken-style special-token
    /// policy.
    ///
    /// * `allowed_special` — surfaces in this set that appear in the
    ///   input are always emitted as their reserved special-token id.
    ///   `allowed_special` wins over `disallowed_special` on any
    ///   surface listed in both.
    /// * `disallowed_special` — see [`DisallowedSpecials`] for the full
    ///   semantics. In short: `None` never errors; `All` errors on any
    ///   registered special not in `allowed_special`; `These(set)`
    ///   errors on the listed surfaces only.
    /// * The post-processor still runs and injects any configured
    ///   template splice (BOS/EOS/CLS/SEP). Callers who want to bypass
    ///   the splice while also controlling the special-token policy can
    ///   compose this entry point with the [`Self::encode_with_special`]
    ///   two-arg form separately.
    ///
    /// The crate's default [`Tokenizer::encode`](Self::encode) is
    /// equivalent to calling this with `allowed_special` set to every
    /// registered special surface and `disallowed_special =
    /// DisallowedSpecials::None` — i.e. "allow every registered
    /// special, never error." That equivalence is exercised in the
    /// tests.
    ///
    /// # Errors
    ///
    /// Returns [`TokenizerError::DisallowedSpecialToken`] carrying the
    /// offending surface string if the policy rejects a special-token
    /// occurrence in the input. Returns [`TokenizerError::UnknownToken`]
    /// under the same conditions as [`Self::encode`].
    pub fn encode_with_special_policy(
        &self,
        text: &str,
        allowed_special: &BTreeSet<&str>,
        disallowed_special: &DisallowedSpecials<'_>,
    ) -> Result<Encoding<TokenId>, TokenizerError> {
        // Specials-extraction runs on the RAW input — see the doc
        // comment on `encode_with_special` for the ordering rationale.
        // The policy filter still runs against the effective specials
        // list; `encode_pieces_with_policy` normalizes per
        // between-specials region internally.
        let pieces = self.encode_pieces_with_policy(
            text,
            Some((allowed_special, disallowed_special)),
            Self::merge_loop_flat,
        )?;
        let mut enc = Encoding::new();
        enc.ids.reserve(pieces.len());
        enc.offsets.reserve(pieces.len());
        enc.special_mask.reserve(pieces.len());
        for (id, range, special) in pieces {
            enc.ids.push(id);
            enc.offsets.push(range);
            enc.special_mask.push(special);
        }
        Ok(self.post_processor.apply(&enc, true))
    }

    /// Apply the configured normalizer to `text`, or pass it through
    /// unchanged. Kept as a helper so the encode paths share one
    /// well-documented call site.
    #[cfg_attr(not(feature = "hf-normalizer"), allow(clippy::unused_self))]
    fn normalize_text<'a>(&self, text: &'a str) -> alloc::borrow::Cow<'a, str> {
        #[cfg(feature = "hf-normalizer")]
        if let Some(n) = &self.normalizer {
            return alloc::borrow::Cow::Owned(crate::normalizer::normalize(text, n));
        }
        alloc::borrow::Cow::Borrowed(text)
    }

    /// Read-only access to the merge table.
    #[must_use]
    pub fn merges(&self) -> &BpeMergeTable {
        &self.merges
    }

    /// Read-only access to the vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &BpeVocabulary {
        &self.vocab
    }

    /// Read-only access to the registered special tokens.
    #[must_use]
    pub fn special_tokens(&self) -> &BTreeMap<String, TokenId> {
        &self.special_tokens
    }

    /// Read-only access to the configured decoder strategy.
    #[must_use]
    pub fn decoder(&self) -> &Decoder {
        &self.decoder
    }

    // ---- internal encoding pipeline ----

    /// Emit `(id, byte_range_in_input)` pairs for `text`, walking the
    /// full encoding pipeline. This is the shared entry point for both
    /// `encode` (which tracks offsets and special-mask) and `count`
    /// (which discards everything but the length).
    fn encode_pieces(
        &self,
        text: &str,
    ) -> Result<Vec<(TokenId, Range<usize>, bool)>, TokenizerError> {
        self.encode_pieces_with(text, Self::merge_loop_flat)
    }

    /// Shared driver: same pipeline as [`Self::encode_pieces`] but the
    /// per-word merge strategy is a function pointer. In production this
    /// is [`Self::merge_loop_flat`] (tiktoken-style flat sweep); the
    /// tests drive the same pipeline through [`Self::merge_loop_naive`]
    /// and [`Self::merge_loop_nlogn`] as Phase-2 acceptance oracles.
    fn encode_pieces_with(
        &self,
        text: &str,
        merge_fn: MergeLoopFn,
    ) -> Result<Vec<(TokenId, Range<usize>, bool)>, TokenizerError> {
        // Default policy: no allowed/disallowed filter — every registered
        // special is treated as special. Matches the crate's historical
        // behaviour bit-for-bit.
        self.encode_pieces_with_policy(text, None, merge_fn)
    }

    /// Policy-aware variant of [`Self::encode_pieces_with`]. When `policy`
    /// is `None`, every registered special is treated as special (the
    /// historical behaviour); when `Some((allowed, disallowed))`, the
    /// tiktoken-style rules described on [`DisallowedSpecials`] apply.
    ///
    /// # Normalization ordering
    ///
    /// `text` is the RAW caller input. Special-token literals are
    /// extracted from the raw bytes first (longest-match, ties broken
    /// lexically), and only the between-specials regions are handed to
    /// the configured [`crate::normalizer::Normalizer`]. Doing it in
    /// this order matches HF's `added_vocabulary::extract_and_normalize`
    /// pipeline byte-for-byte:
    ///
    /// * a Llama-family `Sequence[Prepend("▁"), Replace(" " → "▁")]`
    ///   normalizer does not prepend a stray `▁` before a registered
    ///   special surface (fixes `<s>hi</s>` → `[1, 7251, 2]` and
    ///   `<|end|>` → `[32007]`);
    /// * an all-empty raw input has no regions to normalize at all,
    ///   so the `Prepend` marker is never injected and the encoded
    ///   output is empty (fixes `""` → `[]`);
    /// * DeBERTa-v3-style `Strip { strip_right: true }` trims each
    ///   region's trailing whitespace *before* the pre-tokenizer fires
    ///   on it, so surrounding whitespace does not stray across the
    ///   specials boundary.
    ///
    /// Callers that already normalized the text (or that never wire
    /// a normalizer) see identical semantics — the per-region
    /// normalization is a no-op when [`Self::normalizer`] is `None`,
    /// and normalization is idempotent for every runtime-supported
    /// normalizer variant on already-normalized input.
    fn encode_pieces_with_policy(
        &self,
        text: &str,
        policy: Option<(&BTreeSet<&str>, &DisallowedSpecials<'_>)>,
        merge_fn: MergeLoopFn,
    ) -> Result<Vec<(TokenId, Range<usize>, bool)>, TokenizerError> {
        let bytes = text.as_bytes();

        // Pre-scan for any disallowed special-token surface in the input.
        // Doing this up front (rather than inline in the main walk)
        // guarantees we surface the same error regardless of whether the
        // disallowed surface would have been matched or shadowed by a
        // longer allowed one — matching tiktoken, which errors on *any*
        // occurrence of a disallowed surface.
        if let Some((allowed, disallowed)) = policy {
            let all_specials = self.sorted_specials();
            match disallowed {
                DisallowedSpecials::None => {}
                DisallowedSpecials::All => {
                    for (surface, _id) in &all_specials {
                        if !allowed.contains(surface.as_str())
                            && find_subslice(bytes, surface.as_bytes()).is_some()
                        {
                            return Err(TokenizerError::DisallowedSpecialToken(surface.clone()));
                        }
                    }
                }
                DisallowedSpecials::These(set) => {
                    for surface in *set {
                        // `allowed_special` wins if a surface is listed in both.
                        if !allowed.contains(surface)
                            && find_subslice(bytes, surface.as_bytes()).is_some()
                        {
                            return Err(TokenizerError::DisallowedSpecialToken(String::from(
                                *surface,
                            )));
                        }
                    }
                }
            }
        }

        // Build the two per-phase entry lists, filtered by policy:
        //   * raw-scan entries — matched against the RAW input before
        //     normalization runs (HF `normalized: false` — every
        //     "special: true" chat marker plus opt-in non-special
        //     entries like Phi-3-mini's `</s>`).
        //   * norm-scan entries — matched against each per-region
        //     NORMALIZED text (HF `normalized: true` — Phi-2's
        //     whitespace-run compression ids).
        let raw_entries = self.sorted_added_by_normalized_flag(false, policy);
        let norm_entries = self.sorted_added_by_normalized_flag(true, policy);

        // Phase 1: pre-compute every raw-scan match, applying lstrip /
        // rstrip to extend each match's span outward through adjacent
        // whitespace. Doing it in two passes (find-all-matches, then
        // stitch regions) keeps the match spans stable across the
        // between-regions computation.
        let raw_matches = find_added_vocab_matches(text, &raw_entries);

        // Phase 2: walk raw_matches; between them, take the raw slice,
        // normalize it, and further extract norm_entries within the
        // normalized region.
        let mut out = Vec::new();
        let mut cursor = 0usize;
        for m in &raw_matches {
            if m.start > cursor {
                let raw_region = &text[cursor..m.start];
                self.encode_between_specials(
                    raw_region,
                    cursor,
                    &norm_entries,
                    &mut out,
                    merge_fn,
                )?;
            }
            out.push((m.id, m.start..m.end, m.is_special));
            cursor = m.end;
        }
        if cursor < text.len() {
            let raw_region = &text[cursor..];
            self.encode_between_specials(raw_region, cursor, &norm_entries, &mut out, merge_fn)?;
        }

        Ok(out)
    }

    /// The Phase-2 sibling of [`Self::encode_pieces_with_policy`]:
    /// normalize `raw_region`, then within the normalized region scan
    /// for [`AddedTokenFlags::normalized`] added-vocab entries and
    /// BPE-encode the plain-text pieces between them.
    ///
    /// `region_offset` is the byte offset of `raw_region` within the
    /// caller's input; used to key emitted offsets. The added-vocab
    /// match spans are re-anchored into the same coordinate system so
    /// their byte ranges land where a caller can still slice into the
    /// original input.
    fn encode_between_specials(
        &self,
        raw_region: &str,
        region_offset: usize,
        norm_entries: &[SortedAddedEntry<'_>],
        out: &mut Vec<(TokenId, Range<usize>, bool)>,
        merge_fn: MergeLoopFn,
    ) -> Result<(), TokenizerError> {
        if raw_region.is_empty() {
            return Ok(());
        }
        let normalized = self.normalize_text(raw_region);
        let norm_ref: &str = normalized.as_ref();
        if norm_entries.is_empty() {
            // Fast path: no Phase-2 matches to consider — hand the
            // whole normalized region straight to the BPE loop.
            return self.encode_region_bpe(norm_ref, region_offset, out, merge_fn);
        }
        // Compute matches within the normalized region.
        let norm_matches = find_added_vocab_matches(norm_ref, norm_entries);
        let mut nc = 0usize;
        for m in &norm_matches {
            if m.start > nc {
                let plain = &norm_ref[nc..m.start];
                self.encode_region_bpe(plain, region_offset + nc, out, merge_fn)?;
            }
            // The emitted offset is relative to the normalized region
            // (which the encode-side of the pipeline already treats as
            // the source of piece byte layouts — Metaspace's `▁`
            // substitution is the exemplar). Anchor to `region_offset`
            // so downstream consumers see one contiguous coordinate
            // system per input.
            out.push((
                m.id,
                (region_offset + m.start)..(region_offset + m.end),
                m.is_special,
            ));
            nc = m.end;
        }
        if nc < norm_ref.len() {
            let tail = &norm_ref[nc..];
            self.encode_region_bpe(tail, region_offset + nc, out, merge_fn)?;
        }
        Ok(())
    }

    /// Sort specials longest-first so that `<|im_start|>` matches before
    /// `<|im|>` if both are registered. Ties are broken by lexical order
    /// for determinism.
    fn sorted_specials(&self) -> Vec<(String, TokenId)> {
        let mut v: Vec<(String, TokenId)> = self
            .special_tokens
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        v.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        v
    }

    /// Return the added-vocab entries whose [`AddedTokenFlags::normalized`]
    /// flag matches `want_normalized`, filtered by the tiktoken-style
    /// policy (`allowed_special` wins over the disallowed set — see
    /// [`Self::encode_with_special_policy`] for the full contract).
    ///
    /// Sorted longest-surface-first so a longer surface (`<|im_start|>`)
    /// shadows a shorter prefix (`<|im|>`) at match time; ties are
    /// broken by lexical order for determinism. Every surface not
    /// present in [`Self::added_token_flags`] is treated as a legacy
    /// special via [`AddedTokenFlags::legacy_special`], preserving the
    /// pre-refactor behaviour of [`Self::with_special_tokens`]-only
    /// callers.
    fn sorted_added_by_normalized_flag<'p>(
        &'p self,
        want_normalized: bool,
        policy: Option<(&BTreeSet<&str>, &DisallowedSpecials<'_>)>,
    ) -> Vec<SortedAddedEntry<'p>> {
        let mut entries: Vec<SortedAddedEntry<'p>> = self
            .special_tokens
            .iter()
            .map(|(surface, &id)| {
                let flags = self.added_token_flags.get(surface).copied().unwrap_or_else(
                    // Legacy callers (no added_token_flags entry) get
                    // the "special, non-normalized, no strip" defaults.
                    || AddedTokenFlags::legacy_special(id),
                );
                SortedAddedEntry {
                    surface: surface.as_str(),
                    flags,
                }
            })
            .filter(|e| e.flags.normalized == want_normalized)
            .collect();
        // Apply the tiktoken-style policy filter, if any. The policy
        // only governs Phase-1 raw-scan entries in HF's own pipeline —
        // Phase-2 normalized-scan entries are unaffected (they can't
        // appear in raw input to error against). Keeping the filter
        // uniform is safe: policy is only ever populated when a
        // caller explicitly opts in via `encode_with_special_policy`.
        if let Some((allowed, _)) = policy {
            entries.retain(|e| allowed.contains(e.surface));
        }
        entries.sort_by(|a, b| {
            b.surface
                .len()
                .cmp(&a.surface.len())
                .then_with(|| a.surface.cmp(b.surface))
        });
        entries
    }

    /// BPE-encode a substring of the input; `region_offset` is the byte
    /// offset of `region` within the original input, used to compute
    /// output offsets. The per-word merge strategy is dispatched through
    /// `merge_fn` so the tests can substitute the naive oracle.
    ///
    /// When a
    /// [`PreTokenizerSequence`](crate::pre_tokenizer::PreTokenizerSequence)
    /// is configured (see [`Self::with_pre_tokenizer_sequence`]), it is
    /// applied to `region` first and each resulting piece is fed
    /// *directly* into the merge loop as a single word (bypassing the
    /// regex/whitespace [`pre_tokenize`] fall-back — the sequence is
    /// itself the pre-tokenizer, so treating each of its pieces as one
    /// word matches HF's pipeline shape exactly and preserves
    /// non-space whitespace like `\n` that the default whitespace-split
    /// fall-back would drop). Reported offsets on the emitted pieces
    /// are relative to the transformed piece's byte layout:
    /// `SentencePiece` Metaspace substitutes `▁` (U+2581, 3 bytes) for
    /// ASCII space (1 byte), so the transformed bytes do not map back
    /// to the caller's original input bytes. Mirrors the byte-level
    /// path's "offsets into the transformed source" shape.
    fn encode_region_bpe(
        &self,
        region: &str,
        region_offset: usize,
        out: &mut Vec<(TokenId, Range<usize>, bool)>,
        merge_fn: MergeLoopFn,
    ) -> Result<(), TokenizerError> {
        if region.is_empty() {
            return Ok(());
        }

        // If a PreTokenizerSequence is wired up (SentencePiece Metaspace
        // on Mistral-family BPE), apply it and hand each piece straight
        // to the merge loop. Skipping `pre_tokenize` here is deliberate:
        // the sequence has already split the region into the pieces the
        // downstream model expects, and the default whitespace-split
        // fall-back in `pre_tokenize` would otherwise drop `\n` and
        // other non-space whitespace that Metaspace intentionally
        // preserves. The offset accumulator is a synthetic cursor over
        // the transformed pieces — see the doc comment above for why
        // this cannot map back to caller-input bytes.
        //
        // We thread `region_offset == 0` as the `is_first_piece_of_input`
        // flag: HF's `Metaspace::pre_tokenize` prepends under
        // `PrependScheme::First` only when the piece's absolute offset
        // in the original input is zero — a region emitted after a
        // pre-extracted special has `region_offset > 0` and so must NOT
        // be prepended. This is what keeps `"<s>hi</s>"` on Mistral
        // encoding the middle `hi` as `hi` (id 5365) rather than `▁hi`
        // (id 12014). The flag is a no-op for the `Always` / `Never`
        // schemes, so byte-for-byte behaviour on non-`First` checkpoints
        // is unchanged.
        #[cfg(feature = "std")]
        if let Some(seq) = &self.pre_tokenizer_sequence {
            let is_first_piece_of_input = region_offset == 0;
            let mut cursor = region_offset;
            for piece in seq.apply_first_piece_context(region, is_first_piece_of_input) {
                if piece.is_empty() {
                    continue;
                }
                let piece_len = piece.len();
                self.encode_word_bpe(&piece, 0, cursor, out, merge_fn)?;
                cursor += piece_len;
            }
            return Ok(());
        }

        self.encode_region_bpe_inner(region, region_offset, out, merge_fn)
    }

    /// Inner half of [`Self::encode_region_bpe`] — pre-tokenize `region`
    /// via the (optional) regex/byte-level pattern, then feed each
    /// produced word through [`Self::encode_word_bpe`].
    fn encode_region_bpe_inner(
        &self,
        region: &str,
        region_offset: usize,
        out: &mut Vec<(TokenId, Range<usize>, bool)>,
        merge_fn: MergeLoopFn,
    ) -> Result<(), TokenizerError> {
        if region.is_empty() {
            return Ok(());
        }

        // SentencePiece-family fast path: when byte-fallback is enabled
        // *and* no explicit pre-tokenizer pattern is configured, pass
        // the whole region as a single word. The default whitespace-
        // split fallback in `pre_tokenize` would drop `\n` and other
        // non-space whitespace, but byte-fallback checkpoints
        // (Llama-2, Mistral-7B-v0.1, Phi-3-mini-4k-instruct, Gemma-2b)
        // must preserve those bytes and route them through the reserved
        // `<0xXX>` tokens. Real SentencePiece semantics: the whole
        // region is one word, byte-fallback fans out any character
        // whose surface is not in the vocab. This branch does not fire
        // when a `PreTokenizerSequence` (Metaspace) is configured —
        // that path is handled in `encode_region_bpe` above and
        // supplies its own per-piece splitting.
        if self.pre_tokenizer_pattern.is_none() && self.byte_fallback.is_some() {
            return self.encode_word_bpe(region, 0, region_offset, out, merge_fn);
        }

        // Pre-tokenize into "words" (or one word for the whole region).
        // Byte-level pre-tokenization owns its transformed strings, so
        // `pre_tokenize` returns `Cow<str>` values — most paths borrow
        // straight from `region`; the byte-level path allocates one
        // owned `String` per chunk with the byte↔char mapping applied.
        let words = pre_tokenize(region, self.pre_tokenizer_pattern.as_ref());
        for word in words {
            let word_text: &str = word.text.as_ref();
            self.encode_word_bpe(word_text, word.offset, region_offset, out, merge_fn)?;
        }
        Ok(())
    }

    /// Seed pieces from `word_text`, run the merge loop, and emit each
    /// surviving piece as a `(TokenId, offset_range, special=false)`
    /// entry.
    ///
    /// * `word_text` is the string the merge loop runs on (already
    ///   pre-tokenized by the caller — either by `pre_tokenize` or by a
    ///   [`PreTokenizerSequence`] stage).
    /// * `word_offset_in_region` is the byte offset of `word_text`
    ///   within the enclosing region. The `PreTokenizerSequence` path
    ///   passes `0` because each piece is treated as its own region.
    /// * `region_offset` is the byte offset of the enclosing region
    ///   within the caller's input.
    ///
    /// The emitted offsets are `region_offset + word_offset_in_region +
    /// piece_start_in_word .. + piece_len`, matching the offset shape
    /// the rest of the encode pipeline reports.
    fn encode_word_bpe(
        &self,
        word_text: &str,
        word_offset_in_region: usize,
        region_offset: usize,
        out: &mut Vec<(TokenId, Range<usize>, bool)>,
        merge_fn: MergeLoopFn,
    ) -> Result<(), TokenizerError> {
        let word_bytes = word_text.as_bytes();

        // Whether the pre-tokenizer is (or contains) a ByteLevel stage
        // — decides the piece seed strategy below. Encoded ByteLevel
        // chars are up to 2 UTF-8 bytes each, so seeding per byte
        // would split `Ġ` in half; we seed per char instead so
        // multi-byte encoded chars stay atomic. Falcon-family
        // checkpoints wrap a `ByteLevel` stage inside a `Sequence`
        // (Punctuation → ByteLevel → Digits → Split); the sequence
        // still needs per-char seeding, so we walk the tree.
        let byte_level = self
            .pre_tokenizer_pattern
            .as_ref()
            .is_some_and(PreTokenizerRegex::contains_byte_level);
        // Character-BPE (Llama-2 / Mistral / Qwen) also needs per-char
        // seeding: the vocab keys single-character surfaces by their
        // raw UTF-8 bytes (`"é"` → `[0xC3, 0xA9]`), so seeding per
        // byte would split the char across two pieces that no merge
        // rule would ever combine again. When byte-fallback is on the
        // encode-side we always seed per-char and delegate unresolved
        // pieces to the fallback fan-out below.
        let seed_per_char = byte_level || self.byte_fallback.is_some();

        // Seed pieces: one per byte in the default path, one per
        // *char* in both the byte-level and byte-fallback paths
        // (see comment above).
        //
        // Pieces carry only `(start, len)` byte ranges into `word_bytes`
        // — no owned `Vec<u8>` per piece — so seeding is allocation-free
        // beyond the enclosing `Vec<PieceRef>` itself. Every rank
        // lookup and vocab lookup below reads through
        // `&word_bytes[start..start + len]`, which is a `Borrow<[u8]>`
        // view into the caller-provided word and never touches the
        // allocator on the merge-loop hot path. (Pre-Wave-14 this shape
        // allocated one `Vec<u8>` per byte at seed time and cloned each
        // piece's bytes into the merge-loop arena on top; the audit
        // called both out as the dominant per-encode cost.)
        //
        // ASCII fast-path: when `seed_per_char` is on but the word is
        // pure ASCII, every codepoint is exactly one byte, so per-byte
        // seeding produces the same `PieceRef` sequence as UTF-8 char
        // decoding — without paying for the char iterator's boundary
        // classification on every byte. This branch is hot for
        // SentencePiece-family checkpoints (Llama-2 / Mistral / Gemma /
        // Phi-3 / Falcon), which always take the `seed_per_char` path
        // and whose input is majority-ASCII.
        let capacity = if seed_per_char && !word_text.is_ascii() {
            // Non-ASCII multi-byte text: each char is ≥ 2 bytes, so the
            // piece count is strictly less than the byte length. Round
            // up on 2-byte assumption to avoid reallocation on the
            // common Latin-1 / Cyrillic supplement case; UTF-8-heavy
            // input (CJK) overshoots slightly but the vector is short-
            // lived.
            word_bytes.len().div_ceil(2)
        } else {
            // Pure ASCII (or !seed_per_char): one piece per byte.
            word_bytes.len()
        };
        let mut pieces: Vec<PieceRef> = Vec::with_capacity(capacity);
        if seed_per_char && !word_text.is_ascii() {
            // `char_indices` yields (byte_offset, char) in a single
            // pass — no manual byte cursor, and the offset is already
            // the `PieceRef::start` we need.
            pieces.extend(word_text.char_indices().map(|(start, c)| PieceRef {
                start,
                len: c.len_utf8(),
            }));
        } else {
            // Byte-per-piece: identical output shape whether we're on
            // the byte-level BPE path or the seed_per_char + ASCII
            // fast-path, because ASCII codepoints are one byte each.
            pieces.extend((0..word_bytes.len()).map(|i| PieceRef { start: i, len: 1 }));
        }

        merge_fn(self, word_bytes, &mut pieces);

        for p in pieces {
            let piece_bytes = &word_bytes[p.start..p.start + p.len];
            if let Some(id) = self.vocab.id(piece_bytes) {
                let abs_start = region_offset + word_offset_in_region + p.start;
                let abs_end = abs_start + p.len;
                out.push((id, abs_start..abs_end, false));
            } else if let Some(bf) = self.byte_fallback.as_ref() {
                // Byte-fallback: the piece survived the merge loop
                // but its bytes are not in the vocab. Emit one
                // reserved `<0xXX>` id per byte of the piece. The
                // offset span of each emitted id is a single byte
                // in the input — matching how the caller can slice
                // `input.as_bytes()[range]` back to recover the
                // original byte.
                for (k, &b) in piece_bytes.iter().enumerate() {
                    let id = bf[b as usize];
                    let abs_start = region_offset + word_offset_in_region + p.start + k;
                    let abs_end = abs_start + 1;
                    out.push((id, abs_start..abs_end, false));
                }
            } else {
                return Err(TokenizerError::UnknownToken(format_bytes_literal(
                    piece_bytes,
                )));
            }
        }
        Ok(())
    }

    /// Reverse-lookup: if `id` is one of the 256 byte-fallback tokens,
    /// return its associated byte value. `None` when byte-fallback is
    /// disabled or `id` is a regular vocab entry.
    ///
    /// A linear scan of a 256-entry array is cheap enough that
    /// building a reverse `id → byte` map on construction would be
    /// premature optimisation — decode is not a hot path. Mirrors
    /// [`crate::hf::UnigramTokenizer::byte_fallback_byte_for`].
    fn byte_fallback_byte_for(&self, id: TokenId) -> Option<u8> {
        let bf = self.byte_fallback.as_ref()?;
        for (b, &tok) in bf.iter().enumerate() {
            if tok == id {
                // `b` iterates over a [_; 256] array, so it is always
                // in 0..=255 — the try_from is exact; the `.ok()`
                // shape sidesteps clippy's truncation lint.
                return u8::try_from(b).ok();
            }
        }
        None
    }

    /// Naive O(n²) merge loop: repeatedly rescan the whole piece
    /// sequence for the adjacent pair with the lowest rank, merge it,
    /// and shift the tail down.
    ///
    /// Retained as a test-only oracle: the design doc's Phase 2
    /// acceptance criterion (`docs/design/tokenizers.md` §11) says
    /// "the O(n log n) implementation agrees with the naive O(n²) oracle
    /// over exhaustive short inputs." We keep this callable from the
    /// proptest module so that agreement is checked mechanically.
    #[cfg_attr(not(test), allow(dead_code))]
    fn merge_loop_naive(&self, word_bytes: &[u8], pieces: &mut Vec<PieceRef>) {
        loop {
            if pieces.len() < 2 {
                return;
            }
            // Find the adjacent pair with the lowest rank in the merge table.
            // Since pieces are contiguous ranges into `word_bytes` and merges
            // only combine adjacent pieces, the concatenation of piece i and
            // piece i+1 is exactly `word_bytes[i.start..(i+1).start + (i+1).len]`.
            let mut best_idx: Option<usize> = None;
            let mut best_rank: u32 = u32::MAX;
            for i in 0..pieces.len() - 1 {
                let left = &pieces[i];
                let right = &pieces[i + 1];
                let key = &word_bytes[left.start..right.start + right.len];
                if let Some(r) = self.merges.rank_slice(key) {
                    if r < best_rank {
                        best_rank = r;
                        best_idx = Some(i);
                    }
                }
            }
            let Some(i) = best_idx else {
                return;
            };
            // Merge pieces[i] and pieces[i+1] in place: extend left's
            // length by right's, then drop right. No byte copying —
            // the underlying `word_bytes` slice is shared and pieces
            // are just windows into it.
            pieces[i].len += pieces[i + 1].len;
            pieces.remove(i + 1);
        }
    }

    /// Production merge loop — tiktoken-style flat sweep.
    ///
    /// Data structure: a single `Vec<(byte_start, rank)>` of length
    /// `n + 1`, where the last entry is a sentinel whose `byte_start`
    /// equals the total word length (so pair-end lookups at position
    /// `n - 2` naturally terminate). For each entry `i < n`, the `rank`
    /// field caches the merge-table rank of the byte-slice
    /// `word_bytes[parts[i].0 .. parts[i + 2].0]` — i.e. the rank of the
    /// pair *starting* at position `i` — or [`u32::MAX`] when that pair
    /// has no rule in the merge table.
    ///
    /// The loop shape (mirroring `tiktoken`'s `_byte_pair_merge`):
    ///
    /// 1. Linear scan `parts[0 .. len - 2]` for the minimum-rank pair
    ///    (`<` so ties break to the leftmost — matching the naive
    ///    oracle byte-for-byte).
    /// 2. If the minimum is [`u32::MAX`] there are no more mergeable
    ///    pairs — bail out.
    /// 3. Otherwise: at position `i`, merge pieces `i` and `i + 1` by
    ///    removing `parts[i + 1]` (a single `Vec::remove` of a small
    ///    tail). `parts[i]`'s `byte_start` is unchanged; its new
    ///    successor is what used to be `parts[i + 2]`.
    /// 4. Recompute the cached rank for `parts[i]` (its pair now spans
    ///    a longer byte range) and, if `i > 0`, for `parts[i - 1]`
    ///    (whose right neighbour was replaced by the merged piece).
    ///
    /// # Why flat beats the linked-list + heap shape here
    ///
    /// The heap-based O(n log n) formulation (retained below as
    /// [`Self::merge_loop_nlogn`], now test-only) pays a per-word
    /// overhead of a `BinaryHeap::new()`, a `Vec<MergeNode>` of size
    /// `n`, and a stream of lazy-deletion heap pops that must validate
    /// aliveness / adjacency / rank on every extraction. tiktoken's
    /// benchmark shape targets the small-word regime the pre-tokenizer
    /// hands us — the median cl100k word is a handful of pieces long —
    /// and in that regime the constants of a linear scan on a packed
    /// `Vec<(u32, u32)>` (fits multiple entries per cache line) beat
    /// the log-factor savings of a heap by a wide margin. The bench
    /// harness confirms this: cl100k throughput closed most of the
    /// remaining gap versus `tiktoken-rs` after this swap. See the
    /// baseline table in
    /// `crates/stringcheese-tokenizer-hf-bench/benches/tokenizer_hf.rs`
    /// for the current numbers.
    fn merge_loop_flat(&self, word_bytes: &[u8], pieces: &mut Vec<PieceRef>) {
        let n = pieces.len();
        if n < 2 {
            return;
        }

        // Build the flat state. `parts` has `n + 1` entries: the first
        // `n` mirror `pieces[0..n]`, and the last is a sentinel whose
        // byte-start equals the total word length. The sentinel is
        // never a valid pair-left (we scan `0..parts.len() - 2` for the
        // minimum), but its `.0` field is what the pair-at-position-n-2
        // reads to find its own end offset.
        //
        // Rank fields start at `u32::MAX` and are filled in below.
        // Casting `start` from `usize` to `u32` is exact: pre-tokenized
        // words are always dramatically smaller than 2^32 bytes (the
        // longest realistic word after regex splitting is a handful of
        // code points), and `PieceRef::start` is itself derived from
        // offsets into a per-word byte buffer.
        let word_len_u32 = u32::try_from(word_bytes.len()).unwrap_or(u32::MAX);
        let mut parts: Vec<(u32, u32)> = Vec::with_capacity(n + 1);
        for p in pieces.iter() {
            let start = u32::try_from(p.start).unwrap_or(u32::MAX);
            parts.push((start, u32::MAX));
        }
        parts.push((word_len_u32, u32::MAX));

        // Seed initial ranks: for each valid pair position `i`
        // (`0..n - 1`), rank = merge-table rank of
        // `word_bytes[parts[i].0 .. parts[i + 2].0]`.
        for i in 0..n - 1 {
            let start = parts[i].0 as usize;
            let end = parts[i + 2].0 as usize;
            if let Some(r) = self.merges.rank_slice(&word_bytes[start..end]) {
                parts[i].1 = r;
            }
        }

        // Merge loop. `parts.len()` shrinks by 1 per iteration; we
        // stop when there is at most one real piece left (parts.len() <= 2
        // = one piece + sentinel).
        while parts.len() > 2 {
            // Find the leftmost minimum-rank pair. `<` ensures the
            // *first* occurrence wins on ties — matching the naive
            // oracle's left-to-right scan tie-break byte-for-byte.
            let mut min_rank: u32 = u32::MAX;
            let mut min_idx: usize = 0;
            let scan_end = parts.len() - 2;
            for (i, &(_, rank)) in parts[..=scan_end].iter().enumerate() {
                if rank < min_rank {
                    min_rank = rank;
                    min_idx = i;
                }
            }
            if min_rank == u32::MAX {
                break;
            }

            // Perform the merge: remove `parts[min_idx + 1]`. After
            // this, `parts[min_idx]` retains its byte_start (the pair
            // starts at the same byte) and its new right neighbour is
            // what used to be `parts[min_idx + 2]`.
            parts.remove(min_idx + 1);

            // Recompute the cached rank for `parts[min_idx]` (its pair
            // now spans a longer byte range: `parts[min_idx].0 ..
            // parts[min_idx + 2].0`, the new i+2 being the shifted-in
            // successor). When min_idx is the second-to-last pair-eligible
            // position `parts.len() - 2` (which happens when the merge
            // consumed the previous last real pair), there is no i+2 —
            // the entry is not a valid pair-left any more, so leave its
            // rank at u32::MAX.
            let update_pair = |parts: &mut Vec<(u32, u32)>, i: usize| {
                if i + 2 < parts.len() {
                    let start = parts[i].0 as usize;
                    let end = parts[i + 2].0 as usize;
                    parts[i].1 = self
                        .merges
                        .rank_slice(&word_bytes[start..end])
                        .unwrap_or(u32::MAX);
                } else {
                    parts[i].1 = u32::MAX;
                }
            };
            update_pair(&mut parts, min_idx);
            if min_idx > 0 {
                // The pair at `min_idx - 1` also changed — its right
                // neighbour is now the merged piece at `min_idx`.
                update_pair(&mut parts, min_idx - 1);
            }
        }

        // Rebuild pieces from the flat state. Each surviving piece
        // spans `parts[i].0 .. parts[i + 1].0`.
        pieces.clear();
        for i in 0..parts.len() - 1 {
            let start = parts[i].0 as usize;
            let end = parts[i + 1].0 as usize;
            pieces.push(PieceRef {
                start,
                len: end - start,
            });
        }
    }

    /// Test-only O(n log n) merge loop — Sennrich, Haddow, & Birch
    /// (2016), the "linked-list plus min-heap" formulation.
    ///
    /// Data structure: a *doubly-linked list* of merge nodes over an
    /// arena `Vec<MergeNode>`, plus a *min-heap* of pending merge
    /// candidates keyed by `(rank, left_idx)`. `left_idx` is the arena
    /// slot of the left piece; ties on rank break by original position
    /// (the leftmost pair first), which matches the naive oracle's
    /// left-to-right scan tie-break byte-for-byte.
    ///
    /// The invariant we defend on every heap pop:
    ///
    /// 1. `left` and `right` are still alive (a merge that consumed
    ///    either would have flipped `alive` to `false`);
    /// 2. `left.next == right` (the pair is still adjacent — no
    ///    intervening merge has re-parented the list);
    /// 3. the rank we stamped at push time equals
    ///    `merges.rank(&left.bytes, &right.bytes)` today (the merged
    ///    bytes of either endpoint may have changed under us).
    ///
    /// Any pop that fails any of the three is a *stale* entry and is
    /// discarded — this is the lazy-deletion approach called out in the
    /// task's implementation-strategy question. Concretely: we don't
    /// walk the heap to purge entries when we merge; we just let stale
    /// entries surface at the top and skip them. Each pair enters the
    /// heap at most O(1) times *net* (a merge creates at most two new
    /// pairs — the merged node's new left and right neighbours — and
    /// there are at most `n - 1` merges), so the heap size is O(n)
    /// amortised and the whole loop is O(n log n).
    ///
    /// We use `BinaryHeap<Reverse<HeapEntry>>` (from `alloc`) as the
    /// min-heap: `BinaryHeap` is a max-heap by default, and `Reverse`
    /// inverts the ordering — cheaper than defining a custom `Ord`
    /// with inverted comparisons.
    ///
    /// Retained as a test-only oracle: the production merge path is
    /// [`Self::merge_loop_flat`] (tiktoken-style flat sweep), which
    /// benched faster than this heap-based shape across every input
    /// size the harness measures. The proptest suite still cross-checks
    /// the flat sweep against both this and the naive O(n²) oracle so
    /// any silent divergence surfaces in CI.
    #[cfg_attr(not(test), allow(dead_code))]
    fn merge_loop_nlogn(&self, word_bytes: &[u8], pieces: &mut Vec<PieceRef>) {
        let n = pieces.len();
        if n < 2 {
            return;
        }

        // Build the arena. Nodes 0..n mirror pieces[0..n]; the merged
        // node keeps its arena slot forever, so `left_idx` is a stable
        // deterministic key for tie-breaking (it matches the original
        // byte position of the pair's left piece).
        //
        // Nodes carry only `(start, len)` byte ranges into the shared
        // `word_bytes` slice — no per-node `Vec<u8>` clone at build
        // time and no per-merge `extend_from_slice` copy. Merges only
        // combine adjacent pieces and the merged piece's byte range is
        // exactly the union of the two adjacent ranges, so we can
        // recover the piece's bytes at any point via
        // `&word_bytes[start..start + len]`.
        let mut nodes: Vec<MergeNode> = Vec::with_capacity(n);
        for (i, p) in pieces.iter().enumerate() {
            nodes.push(MergeNode {
                start: p.start,
                len: p.len,
                prev: if i == 0 { None } else { Some(i - 1) },
                next: if i + 1 == n { None } else { Some(i + 1) },
                alive: true,
            });
        }

        // Seed the heap with every initial adjacent pair that has a
        // rank in the merge table.
        let mut heap: BinaryHeap<Reverse<HeapEntry>> = BinaryHeap::new();
        for i in 0..n - 1 {
            let left = &nodes[i];
            let right = &nodes[i + 1];
            let key = &word_bytes[left.start..right.start + right.len];
            if let Some(rank) = self.merges.rank_slice(key) {
                heap.push(Reverse(HeapEntry {
                    rank,
                    left_idx: i,
                    right_idx: i + 1,
                }));
            }
        }

        while let Some(Reverse(entry)) = heap.pop() {
            let li = entry.left_idx;
            let ri = entry.right_idx;

            // Validity checks — see the doc-comment above for the
            // invariant this defends.
            if !nodes[li].alive || !nodes[ri].alive {
                continue;
            }
            if nodes[li].next != Some(ri) {
                continue;
            }
            // The rank stored at push time must still be the rank of
            // the *current* byte range — either endpoint's length may
            // have grown since the entry was queued (a prior merge
            // absorbed a neighbour).
            let key = &word_bytes[nodes[li].start..nodes[ri].start + nodes[ri].len];
            let Some(current_rank) = self.merges.rank_slice(key) else {
                continue;
            };
            if current_rank != entry.rank {
                continue;
            }

            // Perform the merge: extend left's length by right's,
            // splice right out of the list, mark right dead. No byte
            // copying — pieces are windows into the shared word bytes.
            nodes[li].len += nodes[ri].len;
            let new_next = nodes[ri].next;
            nodes[li].next = new_next;
            if let Some(nn) = new_next {
                nodes[nn].prev = Some(li);
            }
            nodes[ri].alive = false;

            // Queue up any newly-adjacent pairs. Left's predecessor may
            // now form a merge with left's new (extended) range; and
            // left's new successor may form one on the other side.
            if let Some(pi) = nodes[li].prev {
                let pk = &word_bytes[nodes[pi].start..nodes[li].start + nodes[li].len];
                if let Some(rank) = self.merges.rank_slice(pk) {
                    heap.push(Reverse(HeapEntry {
                        rank,
                        left_idx: pi,
                        right_idx: li,
                    }));
                }
            }
            if let Some(ni) = nodes[li].next {
                let nk = &word_bytes[nodes[li].start..nodes[ni].start + nodes[ni].len];
                if let Some(rank) = self.merges.rank_slice(nk) {
                    heap.push(Reverse(HeapEntry {
                        rank,
                        left_idx: li,
                        right_idx: ni,
                    }));
                }
            }
        }

        // Walk the surviving list in order and rebuild `pieces`.
        pieces.clear();
        // Node 0 has no predecessor so it can never be the *right* of
        // any merge; therefore it is always alive and is the list head.
        let mut cursor: Option<usize> = Some(0);
        while let Some(i) = cursor {
            debug_assert!(nodes[i].alive, "walked to a dead node in the merge list");
            pieces.push(PieceRef {
                start: nodes[i].start,
                len: nodes[i].len,
            });
            cursor = nodes[i].next;
        }
    }
}

impl Tokenizer for BpeTokenizer {
    type Token = TokenId;

    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        let mut enc = self.encode_with_special(text, true)?;
        if let Some(cfg) = &self.truncation {
            stringcheese_tokenizer::truncation::truncate(&mut enc, cfg);
        }
        Ok(enc)
    }

    fn encode_batch(
        &self,
        inputs: &[&str],
    ) -> Result<alloc::vec::Vec<Encoding<Self::Token>>, TokenizerError> {
        let mut out: alloc::vec::Vec<Encoding<Self::Token>> =
            alloc::vec::Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(<Self as Tokenizer>::encode(self, input)?);
        }
        if let Some(cfg) = &self.padding {
            stringcheese_tokenizer::padding::pad_batch(&mut out, cfg);
        }
        Ok(out)
    }

    fn encode_pair(&self, a: &str, b: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        // Encode each side without the post-processor's splice (the
        // pair template does the splicing itself), then apply
        // truncation on the two primary encodings, then run
        // `apply_pair` to produce the final pair encoding.
        let mut ea = self.encode_with_special(a, false)?;
        let mut eb = self.encode_with_special(b, false)?;
        if let Some(cfg) = &self.truncation {
            stringcheese_tokenizer::truncation::truncate_pair(&mut ea, &mut eb, cfg);
        }
        Ok(self.post_processor.apply_pair(&ea, &eb, true))
    }

    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError> {
        // Chain-decoder fast path: when a `Sequence` / `Replace` /
        // `Fuse` / `Strip` / `ByteFallback` decoder is configured, the
        // decode pipeline is a per-token surface-string chain that
        // mirrors HF's own `Decoder::decode_chain`. Route through it
        // instead of the byte-buffer path so decoders that ship with
        // real checkpoints (Llama-2's `Sequence[Replace, ByteFallback,
        // Fuse, Strip]`) produce byte-for-byte identical output to
        // `transformers.AutoTokenizer.decode`.
        //
        // The model-side byte-fallback reassembly is deliberately
        // skipped in this branch — the chain's own `ByteFallback` stage
        // (when present) is what reassembles the `<0xXX>` runs; running
        // both would double-decode.
        if self.decoder.is_chain() {
            let mut token_strs: Vec<String> = Vec::with_capacity(tokens.len());
            for &id in tokens {
                if let Some(bytes) = self.vocab.bytes(id) {
                    // Vocab surface bytes are always valid UTF-8 for
                    // every shipped HF-shape checkpoint (character-BPE
                    // stores UTF-8 characters directly; byte-level
                    // stores the byte↔char-encoded printable form).
                    // Use lossy decoding as a defence for hand-crafted
                    // vocabs.
                    token_strs.push(alloc::string::String::from_utf8_lossy(bytes).into_owned());
                } else if let Some(surface) = self
                    .special_tokens
                    .iter()
                    .find(|&(_, &tid)| tid == id)
                    .map(|(k, _)| k.clone())
                {
                    token_strs.push(surface);
                } else {
                    return Err(TokenizerError::UnknownToken(format_id(id)));
                }
            }
            let out = self.decoder.apply_chain(token_strs);
            let mut buf = String::new();
            for s in &out {
                buf.push_str(s);
            }
            return Ok(buf);
        }

        // Legacy byte-buffer path — Passthrough (default) and ByteLevel.
        let mut buf: Vec<u8> = Vec::new();
        // Accumulator for a run of byte-fallback tokens. Bytes are
        // pushed here while consecutive byte-fallback ids arrive; the
        // run is flushed via `String::from_utf8_lossy` when a non-
        // byte-fallback token is seen or at the end of the stream so
        // an id-list that happens to encode invalid UTF-8 (e.g. hand-
        // constructed with a stray byte-fallback id) maps to U+FFFD
        // instead of failing the whole decode. Matches the Unigram
        // side's shape in [`crate::hf::UnigramTokenizer::decode`].
        let mut byte_run: Vec<u8> = Vec::new();
        for &id in tokens {
            if let Some(b) = self.byte_fallback_byte_for(id) {
                byte_run.push(b);
                continue;
            }
            if !byte_run.is_empty() {
                let flushed = alloc::string::String::from_utf8_lossy(&byte_run);
                buf.extend_from_slice(flushed.as_bytes());
                byte_run.clear();
            }
            // Special tokens are decoded through the vocabulary too if
            // they were registered there; otherwise fall back to the
            // special-token surface strings.
            if let Some(bytes) = self.vocab.bytes(id) {
                buf.extend_from_slice(bytes);
            } else if let Some(surface) = self
                .special_tokens
                .iter()
                .find(|&(_, &tid)| tid == id)
                .map(|(k, _)| k.as_bytes())
            {
                buf.extend_from_slice(surface);
            } else {
                return Err(TokenizerError::UnknownToken(format_id(id)));
            }
        }
        if !byte_run.is_empty() {
            let flushed = alloc::string::String::from_utf8_lossy(&byte_run);
            buf.extend_from_slice(flushed.as_bytes());
        }
        match &self.decoder {
            Decoder::Passthrough => String::from_utf8(buf).map_err(|_| TokenizerError::InvalidUtf8),
            Decoder::ByteLevel => {
                // A byte-level vocabulary stores its surface strings in
                // *encoded* form (e.g. `"Ġhello"`), which is valid
                // UTF-8. Interpret the concatenation, then reverse the
                // ByteLevel byte↔char bijection to recover the caller's
                // original raw bytes, then re-decode as UTF-8.
                let encoded = String::from_utf8(buf).map_err(|_| TokenizerError::InvalidUtf8)?;
                let raw = crate::byte_level::decode_chars(&encoded);
                String::from_utf8(raw).map_err(|_| TokenizerError::InvalidUtf8)
            }
            // Every chain-shape variant was routed above via `is_chain`.
            // The match arm exists so a future non-chain variant added
            // to `Decoder` surfaces as a compile error rather than
            // silently falling through.
            Decoder::Sequence(_)
            | Decoder::Replace { .. }
            | Decoder::Fuse
            | Decoder::Strip { .. }
            | Decoder::ByteFallback => {
                unreachable!("chain-shape decoder handled by the early-return path above")
            }
        }
    }

    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        // `count` mirrors `encode`'s full pipeline (normalize +
        // post-process included) so `count(text) == encode(text)?.len()`
        // for every configuration. Specials extraction runs on the RAW
        // input and normalization is applied per between-specials
        // region inside `encode_pieces` — see the doc-comment on
        // `encode_pieces_with_policy` for the ordering rationale.
        let base = self.encode_pieces(text)?.len();
        // Post-processor may inject or drop tokens.
        Ok(match &self.post_processor {
            crate::post_processor::PostProcessor::None
            // ByteLevel is a documented no-op on the encoding
            // (see [`crate::post_processor::PostProcessor::ByteLevel`]);
            // token count is unchanged.
            | crate::post_processor::PostProcessor::ByteLevel { .. } => base,
            crate::post_processor::PostProcessor::TemplateProcessing(_)
            | crate::post_processor::PostProcessor::BertProcessing(_)
            | crate::post_processor::PostProcessor::RobertaProcessing(_) => {
                // Cheapest correct answer: run the splice against a
                // synthetic encoding of the right length and count the
                // ids field. All three variants add a fixed number of
                // tokens irrespective of ids, so the synthetic-encoding
                // shape yields the correct count.
                let mut synth: Encoding<TokenId> = Encoding::new();
                synth.ids.resize(base, 0);
                self.post_processor.apply(&synth, true).ids.len()
            }
            crate::post_processor::PostProcessor::Sequence(children) => {
                // Walk the sequence and thread a synthetic-encoding of
                // the current length through each child, summing the
                // per-arm additions inductively. Runs the same apply
                // path the encode-side does — cheapest correct answer
                // that handles nested Sequence variants uniformly.
                let mut synth: Encoding<TokenId> = Encoding::new();
                synth.ids.resize(base, 0);
                for child in children {
                    synth = child.apply(&synth, true);
                }
                synth.ids.len()
            }
        })
    }
}

// ---- internal helpers ----

/// A single merge-loop piece: a byte range `[start, start + len)` into
/// the enclosing word's byte buffer. No owned bytes — merges always
/// combine adjacent pieces, so a piece's current surface is always
/// `&word_bytes[start..start + len]`.
#[derive(Debug, Clone, Copy)]
struct PieceRef {
    start: usize,
    len: usize,
}

/// One arena slot in the O(n log n) merge loop's doubly-linked list.
///
/// The list uses `Option<usize>` prev/next indices into the enclosing
/// `Vec<MergeNode>` — no raw pointers, no `unsafe`, and slot indices are
/// stable for the whole run (a merge absorbs the right piece into the
/// left slot; the right slot is marked `alive = false` but never freed).
///
/// Like [`PieceRef`], a node carries only a `(start, len)` byte range
/// into the shared `word_bytes` slice the merge loop is running over —
/// no per-node `Vec<u8>` allocation and no per-merge byte copy.
#[derive(Debug)]
struct MergeNode {
    start: usize,
    len: usize,
    prev: Option<usize>,
    next: Option<usize>,
    alive: bool,
}

/// A pending merge candidate sitting in the min-heap.
///
/// Order is `(rank, left_idx)` — lower rank wins, and ties break by the
/// smaller `left_idx`. Because arena slots are never renumbered,
/// `left_idx` is a stable proxy for the pair's original left-to-right
/// position in the input; the naive oracle also breaks ties by
/// left-to-right position, so the two agree byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeapEntry {
    rank: u32,
    left_idx: usize,
    right_idx: usize,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| self.left_idx.cmp(&other.left_idx))
            .then_with(|| self.right_idx.cmp(&other.right_idx))
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Function pointer to a per-word merge strategy. Production code uses
/// [`BpeTokenizer::merge_loop_flat`]; the tests substitute both
/// [`BpeTokenizer::merge_loop_naive`] and
/// [`BpeTokenizer::merge_loop_nlogn`] to verify all three strategies
/// agree on every input. Takes the shared word byte buffer as its
/// second argument so every strategy can look up rank keys and
/// materialise piece surfaces without per-piece allocation.
type MergeLoopFn = fn(&BpeTokenizer, &[u8], &mut Vec<PieceRef>);

/// Text plus (byte) offset within its enclosing region.
///
/// Most pre-tokenizer paths borrow their chunks straight out of the
/// input; the byte-level path owns its transformed strings (the char
/// bijection changes the underlying bytes), so `text` is a
/// [`Cow<str>`](alloc::borrow::Cow) that picks the cheaper form
/// automatically. Callers can just use `word.text.as_ref()` — the
/// `Cow` deref-s to `&str`.
struct Word<'a> {
    offset: usize,
    text: alloc::borrow::Cow<'a, str>,
}

fn pre_tokenize<'a>(text: &'a str, pattern: Option<&PreTokenizerRegex>) -> Vec<Word<'a>> {
    let mut out = Vec::new();
    // A compiled-regex pre-tokenizer walks the input via `find_iter`
    // and emits every non-overlapping match as its own word (matching
    // tiktoken's `re.findall(...)` semantics). This is the shape used
    // by the HF `tokenizer.json` loader (`src/hf.rs`) and by any
    // caller that has compiled the tiktoken canonical pattern.
    #[cfg(feature = "std")]
    if let Some(PreTokenizerRegex::Regex(pre)) = pattern {
        for (offset, text) in pre.split(text) {
            out.push(Word {
                offset,
                text: alloc::borrow::Cow::Borrowed(text),
            });
        }
        return out;
    }
    // Byte-level pipeline lives in its own helper so `pre_tokenize`
    // stays under clippy's `too_many_lines` threshold. See
    // [`pre_tokenize_byte_level`] for the full step list.
    #[cfg(feature = "std")]
    if let Some(PreTokenizerRegex::ByteLevel {
        add_prefix_space,
        split,
    }) = pattern
    {
        return pre_tokenize_byte_level(text, *add_prefix_space, split.as_ref());
    }
    // Punctuation / Digits / Split (non-Isolated) / Sequence route
    // through the multi-stage pipeline. A single stage runs once; a
    // `Sequence` iterates every child in order over the accumulated
    // regions. See [`apply_stage`] for the semantics.
    #[cfg(feature = "std")]
    if let Some(
        p @ (PreTokenizerRegex::Punctuation(_)
        | PreTokenizerRegex::Digits { .. }
        | PreTokenizerRegex::Split { .. }
        | PreTokenizerRegex::Sequence(_)),
    ) = pattern
    {
        return apply_stage(alloc::vec![text.to_string()], p)
            .into_iter()
            .scan(0usize, |cursor, piece| {
                let offset = *cursor;
                *cursor += piece.len();
                Some(Word {
                    offset,
                    text: alloc::borrow::Cow::Owned(piece),
                })
            })
            .collect();
    }
    match pattern {
        Some(PreTokenizerRegex::Literal(sep)) if !sep.is_empty() => {
            pre_tokenize_literal(text, sep.as_str(), &mut out);
        }
        _ => {
            // No pattern (or empty separator): fall back to whitespace
            // splitting. This matches the design doc's "fall through to
            // whitespace" behaviour.
            pre_tokenize_whitespace(text, &mut out);
        }
    }
    // Fallback: if no pattern *and* whitespace splitting yielded nothing
    // and the input is entirely non-whitespace, we've already handled
    // that above. If the input is entirely whitespace, `out` stays
    // empty and we emit nothing — which is what tiktoken does too.
    out
}

/// Literal-separator split helper for [`PreTokenizerRegex::Literal`].
/// Emits every non-empty run between consecutive separator matches.
fn pre_tokenize_literal<'a>(text: &'a str, sep: &str, out: &mut Vec<Word<'a>>) {
    let sep_bytes = sep.as_bytes();
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // Skip separators.
        while cursor + sep_bytes.len() <= bytes.len()
            && &bytes[cursor..cursor + sep_bytes.len()] == sep_bytes
        {
            cursor += sep_bytes.len();
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() {
            if cursor + sep_bytes.len() <= bytes.len()
                && &bytes[cursor..cursor + sep_bytes.len()] == sep_bytes
            {
                break;
            }
            cursor += 1;
        }
        if cursor > start {
            out.push(Word {
                offset: start,
                text: alloc::borrow::Cow::Borrowed(&text[start..cursor]),
            });
        }
    }
}

/// Whitespace-split fallback (design-doc §5.1 "no pre-tokenizer" shape).
/// Skips leading and interior whitespace runs; if the input is
/// entirely non-whitespace, emits it as a single word.
fn pre_tokenize_whitespace<'a>(text: &'a str, out: &mut Vec<Word<'a>>) {
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // Skip whitespace.
        while cursor < bytes.len() {
            let Some(ch) = text[cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() {
            let Some(ch) = text[cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor += ch.len_utf8();
        }
        if cursor > start {
            out.push(Word {
                offset: start,
                text: alloc::borrow::Cow::Borrowed(&text[start..cursor]),
            });
        }
    }
    // If the text contained NO non-whitespace at all we still emit
    // nothing; if the text was pure non-whitespace we emit one word.
    if out.is_empty() && !text.is_empty() && text.chars().any(|c| !c.is_whitespace()) {
        out.push(Word {
            offset: 0,
            text: alloc::borrow::Cow::Borrowed(text),
        });
    }
}

/// Byte-level pre-tokenizer helper — split out of [`pre_tokenize`] so
/// the parent function stays under clippy's `too_many_lines` threshold.
///
/// Steps, in order:
///
/// 1. Optionally prepend an ASCII space to `text` (Hugging Face's
///    `add_prefix_space: true` default). Skipped if the input already
///    starts with a space so the prefix is idempotent.
/// 2. Split the (possibly-prefixed) source with `split`, or if `None`
///    take the whole source as a single chunk.
/// 3. Map every byte of each chunk through the byte↔char bijection
///    (`byte_level::encode_bytes`). This produces an owned `String`
///    per chunk; the resulting [`Word`]'s `Cow<str>` is `Owned`.
///
/// Offsets on the returned words are w.r.t. the possibly-prefixed
/// source — see the callers of [`pre_tokenize`] for how those flow
/// into the reported [`Encoding::offsets`](stringcheese_tokenizer::Encoding::offsets).
#[cfg(feature = "std")]
fn pre_tokenize_byte_level<'a>(
    text: &'a str,
    add_prefix_space: bool,
    split: Option<&crate::pre_tokenizer::RegexPreTokenizer>,
) -> Vec<Word<'a>> {
    // Compose the (possibly prefixed) source. Use `Cow` again so the
    // no-prefix path avoids the allocation.
    let source: alloc::borrow::Cow<'a, str> = if add_prefix_space && !text.starts_with(' ') {
        let mut s = String::with_capacity(text.len() + 1);
        s.push(' ');
        s.push_str(text);
        alloc::borrow::Cow::Owned(s)
    } else {
        alloc::borrow::Cow::Borrowed(text)
    };
    let source_ref: &str = source.as_ref();
    let chunks: Vec<(usize, &str)> = if let Some(pre) = split {
        pre.split(source_ref)
    } else if source_ref.is_empty() {
        Vec::new()
    } else {
        alloc::vec![(0usize, source_ref)]
    };
    let mut out = Vec::with_capacity(chunks.len());
    for (offset, chunk) in chunks {
        let encoded = crate::byte_level::encode_bytes(chunk);
        if encoded.is_empty() {
            continue;
        }
        out.push(Word {
            offset,
            text: alloc::borrow::Cow::Owned(encoded),
        });
    }
    out
}

/// Stage-based pre-tokenizer pipeline.
///
/// Every stage takes the current list of piece strings and produces a
/// new list, in left-to-right order. Concatenating the returned pieces
/// yields the byte content of the pipeline's output — which for
/// [`PreTokenizerRegex::ByteLevel`] stages will differ from the input
/// (the byte↔char bijection replaces every input byte). The offsets
/// returned to the caller of [`pre_tokenize`] are synthetic cursors
/// over that concatenated output, not offsets into the original input.
///
/// # Sequence composition
///
/// `Sequence` iterates its children in order, feeding the previous
/// child's output into the next. An empty `Sequence` behaves as the
/// identity — the input pieces are returned unchanged.
#[cfg(feature = "std")]
fn apply_stage(pieces: Vec<String>, stage: &PreTokenizerRegex) -> Vec<String> {
    match stage {
        PreTokenizerRegex::Literal(sep) => pieces
            .into_iter()
            .flat_map(|p| split_piece_literal(p, sep.as_str()))
            .collect(),
        PreTokenizerRegex::Regex(pre) => pieces
            .into_iter()
            .flat_map(|p| split_piece_regex(&p, pre))
            .collect(),
        PreTokenizerRegex::ByteLevel {
            add_prefix_space,
            split,
        } => pieces
            .into_iter()
            .flat_map(|p| split_piece_byte_level(&p, *add_prefix_space, split.as_ref()))
            .collect(),
        PreTokenizerRegex::Punctuation(behavior) => pieces
            .into_iter()
            .flat_map(|p| split_piece_by_char_predicate(&p, char_is_punctuation, *behavior))
            .collect(),
        PreTokenizerRegex::Digits { individual_digits } => {
            let behavior = crate::pre_tokenizer::SplitDelimiterBehavior::Isolated;
            let mut out = Vec::with_capacity(pieces.len());
            for piece in pieces {
                if *individual_digits {
                    out.extend(split_piece_by_char_predicate(
                        &piece,
                        |c| c.is_ascii_digit(),
                        behavior,
                    ));
                } else {
                    out.extend(split_piece_by_char_run(
                        &piece,
                        |c| c.is_ascii_digit(),
                        behavior,
                    ));
                }
            }
            out
        }
        PreTokenizerRegex::Split { regex, behavior } => pieces
            .into_iter()
            .flat_map(|p| split_piece_by_regex_matches(&p, regex, *behavior))
            .collect(),
        PreTokenizerRegex::Sequence(stages) => {
            let mut current = pieces;
            for s in stages {
                current = apply_stage(current, s);
            }
            current
        }
    }
}

/// Split a piece on every occurrence of `sep`. Matches
/// [`PreTokenizerRegex::Literal`] semantics — delimiter runs collapse
/// and leading/trailing matches drop.
#[cfg(feature = "std")]
fn split_piece_literal(piece: String, sep: &str) -> Vec<String> {
    if sep.is_empty() || piece.is_empty() {
        return alloc::vec![piece];
    }
    let mut out = Vec::new();
    let bytes = piece.as_bytes();
    let sep_bytes = sep.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor + sep_bytes.len() <= bytes.len()
            && &bytes[cursor..cursor + sep_bytes.len()] == sep_bytes
        {
            cursor += sep_bytes.len();
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() {
            if cursor + sep_bytes.len() <= bytes.len()
                && &bytes[cursor..cursor + sep_bytes.len()] == sep_bytes
            {
                break;
            }
            cursor += 1;
        }
        out.push(piece[start..cursor].to_string());
    }
    out
}

/// Split a piece via a compiled regex, keeping only matched substrings
/// (tiktoken semantics — [`PreTokenizerRegex::Regex`], `re.findall`
/// shape).
#[cfg(feature = "std")]
fn split_piece_regex(piece: &str, pre: &crate::pre_tokenizer::RegexPreTokenizer) -> Vec<String> {
    pre.split(piece)
        .into_iter()
        .map(|(_off, s)| s.to_string())
        .collect()
}

/// `ByteLevel` per-piece application: prepend a leading space (when
/// requested and not already present), split by the optional inner
/// regex, then byte-encode each chunk through the `ByteLevel`
/// bijection.
#[cfg(feature = "std")]
fn split_piece_byte_level(
    piece: &str,
    add_prefix_space: bool,
    split: Option<&crate::pre_tokenizer::RegexPreTokenizer>,
) -> Vec<String> {
    let source: alloc::borrow::Cow<'_, str> = if add_prefix_space && !piece.starts_with(' ') {
        let mut s = String::with_capacity(piece.len() + 1);
        s.push(' ');
        s.push_str(piece);
        alloc::borrow::Cow::Owned(s)
    } else {
        alloc::borrow::Cow::Borrowed(piece)
    };
    let src = source.as_ref();
    let chunks: Vec<&str> = if let Some(pre) = split {
        pre.split(src).into_iter().map(|(_o, s)| s).collect()
    } else if src.is_empty() {
        Vec::new()
    } else {
        alloc::vec![src]
    };
    chunks
        .into_iter()
        .map(crate::byte_level::encode_bytes)
        .filter(|s| !s.is_empty())
        .collect()
}

/// `true` when `c` is any Unicode punctuation character (category `P*`)
/// OR belongs to the ASCII punctuation ranges HF's own `Punctuation`
/// pre-tokenizer treats as punctuation. HF's implementation uses
/// `char::is_ascii_punctuation` AND `unicode_categories::UnicodeCategories::is_punctuation`
/// — a superset of the strict Unicode category set. We mirror both.
#[cfg(feature = "std")]
fn char_is_punctuation(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_punctuation();
    }
    matches!(
        unicode_general_category::get_general_category(c),
        unicode_general_category::GeneralCategory::ConnectorPunctuation
            | unicode_general_category::GeneralCategory::DashPunctuation
            | unicode_general_category::GeneralCategory::ClosePunctuation
            | unicode_general_category::GeneralCategory::FinalPunctuation
            | unicode_general_category::GeneralCategory::InitialPunctuation
            | unicode_general_category::GeneralCategory::OtherPunctuation
            | unicode_general_category::GeneralCategory::OpenPunctuation
    )
}

/// Split `piece` at every char matching `pred`, applying HF's
/// [`SplitDelimiterBehavior`](crate::pre_tokenizer::SplitDelimiterBehavior)
/// semantics. Each *individual* matching char is a match — use
/// [`split_piece_by_char_run`] when the caller wants runs of matching
/// chars grouped into single matches.
#[cfg(feature = "std")]
fn split_piece_by_char_predicate(
    piece: &str,
    pred: fn(char) -> bool,
    behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
) -> Vec<String> {
    // Materialise the matches as (start, end) byte ranges over `piece`.
    // Each match is one Unicode scalar — its byte length is the char's
    // UTF-8 length.
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    for c in piece.chars() {
        let len = c.len_utf8();
        if pred(c) {
            matches.push((cursor, cursor + len));
        }
        cursor += len;
    }
    apply_split_behavior(piece, &matches, behavior)
}

/// Like [`split_piece_by_char_predicate`] but treats consecutive
/// matching chars as a single grouped match. Matches HF's `Digits {
/// individual_digits: false }` semantics.
#[cfg(feature = "std")]
fn split_piece_by_char_run(
    piece: &str,
    pred: fn(char) -> bool,
    behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
) -> Vec<String> {
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    let mut run_start: Option<usize> = None;
    for c in piece.chars() {
        let len = c.len_utf8();
        if pred(c) {
            if run_start.is_none() {
                run_start = Some(cursor);
            }
        } else if let Some(start) = run_start.take() {
            matches.push((start, cursor));
        }
        cursor += len;
    }
    if let Some(start) = run_start {
        matches.push((start, cursor));
    }
    apply_split_behavior(piece, &matches, behavior)
}

/// Split `piece` by the matches produced by `pre`, applying HF's
/// [`SplitDelimiterBehavior`](crate::pre_tokenizer::SplitDelimiterBehavior)
/// semantics. Used by [`PreTokenizerRegex::Split`].
#[cfg(feature = "std")]
fn split_piece_by_regex_matches(
    piece: &str,
    pre: &crate::pre_tokenizer::RegexPreTokenizer,
    behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
) -> Vec<String> {
    let matches: Vec<(usize, usize)> = pre
        .split_ranges(piece)
        .into_iter()
        .map(|r| (r.start, r.end))
        .collect();
    apply_split_behavior(piece, &matches, behavior)
}

/// One entry of the base decomposition consumed by
/// [`apply_split_behavior`] — an alternating between/match kind plus
/// a `(start, end)` byte range.
#[cfg(feature = "std")]
#[derive(Clone, Copy)]
enum DecompKind {
    Between,
    Match,
}

/// Apply [`SplitDelimiterBehavior`](crate::pre_tokenizer::SplitDelimiterBehavior)
/// to a piece plus a list of `(start, end)` match ranges. Dispatches
/// to one of the four per-mode helpers so the parent stays short.
#[cfg(feature = "std")]
fn apply_split_behavior(
    piece: &str,
    matches: &[(usize, usize)],
    behavior: crate::pre_tokenizer::SplitDelimiterBehavior,
) -> Vec<String> {
    use crate::pre_tokenizer::SplitDelimiterBehavior as B;
    if matches.is_empty() {
        return if piece.is_empty() {
            Vec::new()
        } else {
            alloc::vec![piece.to_string()]
        };
    }

    let decomp = build_split_decomposition(piece, matches);
    match behavior {
        B::Isolated => decomp
            .into_iter()
            .map(|(_, s, e)| piece[s..e].to_string())
            .collect(),
        B::Removed => decomp
            .into_iter()
            .filter_map(|(k, s, e)| match k {
                DecompKind::Match => None,
                DecompKind::Between => Some(piece[s..e].to_string()),
            })
            .collect(),
        B::MergedWithPrevious => merge_matches_with_previous(piece, decomp),
        B::MergedWithNext => merge_matches_with_next(piece, decomp),
        B::Contiguous => collapse_adjacent_matches(piece, decomp),
    }
}

/// Base decomposition: alternating non-match / match / non-match / ...
/// pieces, in byte-range form. Zero-length non-match pieces are
/// dropped up front so behaviour choices operate on real content.
#[cfg(feature = "std")]
fn build_split_decomposition(
    piece: &str,
    matches: &[(usize, usize)],
) -> Vec<(DecompKind, usize, usize)> {
    let mut decomp: Vec<(DecompKind, usize, usize)> = Vec::with_capacity(matches.len() * 2 + 1);
    let mut cursor = 0usize;
    for &(s, e) in matches {
        if s > cursor {
            decomp.push((DecompKind::Between, cursor, s));
        }
        if e > s {
            decomp.push((DecompKind::Match, s, e));
        }
        cursor = e;
    }
    if cursor < piece.len() {
        decomp.push((DecompKind::Between, cursor, piece.len()));
    }
    decomp
}

/// [`SplitDelimiterBehavior::MergedWithPrevious`]: matches glue onto
/// the preceding piece (or become their own piece if nothing
/// precedes).
#[cfg(feature = "std")]
fn merge_matches_with_previous(
    piece: &str,
    decomp: Vec<(DecompKind, usize, usize)>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (k, s, e) in decomp {
        let chunk = &piece[s..e];
        match k {
            DecompKind::Between => out.push(chunk.to_string()),
            DecompKind::Match => {
                if let Some(last) = out.last_mut() {
                    last.push_str(chunk);
                } else {
                    out.push(chunk.to_string());
                }
            }
        }
    }
    out
}

/// [`SplitDelimiterBehavior::MergedWithNext`]: matches glue onto the
/// following piece (or become their own if nothing follows). Walks
/// right-to-left so the "next" concept is the last-pushed piece.
#[cfg(feature = "std")]
fn merge_matches_with_next(piece: &str, decomp: Vec<(DecompKind, usize, usize)>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (k, s, e) in decomp.into_iter().rev() {
        let chunk = &piece[s..e];
        match k {
            DecompKind::Between => out.push(chunk.to_string()),
            DecompKind::Match => {
                if let Some(last) = out.last_mut() {
                    let mut merged = chunk.to_string();
                    merged.push_str(last);
                    *last = merged;
                } else {
                    out.push(chunk.to_string());
                }
            }
        }
    }
    out.reverse();
    out
}

/// [`SplitDelimiterBehavior::Contiguous`]: runs of adjacent match
/// entries collapse into a single piece.
#[cfg(feature = "std")]
fn collapse_adjacent_matches(piece: &str, decomp: Vec<(DecompKind, usize, usize)>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut pending_match_start: Option<usize> = None;
    let mut pending_match_end: usize = 0;
    for (k, s, e) in decomp {
        match k {
            DecompKind::Match => {
                if pending_match_start.is_none() {
                    pending_match_start = Some(s);
                }
                pending_match_end = e;
            }
            DecompKind::Between => {
                if let Some(ms) = pending_match_start.take() {
                    out.push(piece[ms..pending_match_end].to_string());
                }
                out.push(piece[s..e].to_string());
            }
        }
    }
    if let Some(ms) = pending_match_start {
        out.push(piece[ms..pending_match_end].to_string());
    }
    out
}

/// Format a byte slice as a printable label for error messages.
fn format_bytes_literal(b: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::from("<bytes:");
    for byte in b {
        // Writing to a `String` cannot fail; the `write!` macro returns
        // a `Result` we intentionally discard.
        let _ = write!(s, "{byte:02x}");
    }
    s.push('>');
    s
}

fn format_id(id: TokenId) -> String {
    let mut s = String::from("<id:");
    s.push_str(&id.to_string());
    s.push('>');
    s
}

/// One added-vocab entry as consumed by the encode-side matcher.
///
/// Borrows the surface string from the tokenizer's `special_tokens`
/// map so the sort doesn't clone; the flags are already `Copy`.
struct SortedAddedEntry<'a> {
    surface: &'a str,
    flags: AddedTokenFlags,
}

/// One `(start, end, id, is_special)` match produced by the
/// added-vocab scanner. `start` and `end` are byte offsets into the
/// input string being scanned (raw input for Phase 1, normalized
/// region for Phase 2). The span may extend beyond the raw surface's
/// byte range when the entry has `lstrip: true` (start pushed left
/// through preceding whitespace) or `rstrip: true` (end pushed right
/// through trailing whitespace).
struct AddedMatch {
    id: TokenId,
    start: usize,
    end: usize,
    is_special: bool,
}

/// Scan `text` for greedy left-to-right added-vocab matches over
/// `entries` (already sorted longest-first). Applies each match's
/// [`AddedTokenFlags::lstrip`] and [`AddedTokenFlags::rstrip`]
/// semantics: extends the match span outward through adjacent
/// Unicode-whitespace characters. Returns the matches in the order
/// they occur.
///
/// The cursor advances one byte at a time between match attempts. A
/// match can not overlap the previous match — the cursor jumps to the
/// (extended) `end` of the previous match. This matches HF's own
/// `added_vocabulary::find_matches` shape.
fn find_added_vocab_matches(text: &str, entries: &[SortedAddedEntry<'_>]) -> Vec<AddedMatch> {
    let mut matches = Vec::new();
    if entries.is_empty() || text.is_empty() {
        return matches;
    }
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // Longest-first match at `cursor`.
        let mut hit: Option<&SortedAddedEntry<'_>> = None;
        for e in entries {
            let sb = e.surface.as_bytes();
            if bytes[cursor..].starts_with(sb) {
                hit = Some(e);
                break;
            }
        }
        if let Some(e) = hit {
            let raw_start = cursor;
            let raw_end = cursor + e.surface.len();
            let start = if e.flags.lstrip {
                raw_start - trailing_whitespace_len(&text[..raw_start])
            } else {
                raw_start
            };
            let end = if e.flags.rstrip {
                raw_end + leading_whitespace_len(&text[raw_end..])
            } else {
                raw_end
            };
            matches.push(AddedMatch {
                id: e.flags.id,
                start,
                end,
                is_special: e.flags.special,
            });
            cursor = end;
        } else {
            // Advance by one Unicode scalar to keep every walk step
            // valid UTF-8. Bytes past the initial byte of a multi-byte
            // sequence are continuation bytes (10xxxxxx) and can not
            // start a new match anyway.
            let step = utf8_char_len(bytes[cursor]);
            cursor += step;
        }
    }
    matches
}

/// Number of leading Unicode-whitespace bytes in `text` (i.e. sum of
/// the UTF-8 byte lengths of each leading whitespace char).
fn leading_whitespace_len(text: &str) -> usize {
    let mut consumed = 0usize;
    for c in text.chars() {
        if c.is_whitespace() {
            consumed += c.len_utf8();
        } else {
            break;
        }
    }
    consumed
}

/// Number of trailing Unicode-whitespace bytes in `text` (sum of the
/// UTF-8 byte lengths of each trailing whitespace char). Callers use
/// this to extend a match backward by `text.len() - trailing_ws_len`.
fn trailing_whitespace_len(text: &str) -> usize {
    let mut consumed = 0usize;
    for c in text.chars().rev() {
        if c.is_whitespace() {
            consumed += c.len_utf8();
        } else {
            break;
        }
    }
    consumed
}

/// Byte length of the UTF-8 sequence whose leading byte is `first`.
/// Defensive: returns 1 for continuation bytes (`0x80..0xC0`) and for
/// invalid leads (which shouldn't appear in valid UTF-8 anyway), so a
/// caller can safely advance a byte cursor by the return value on
/// unrecognised input rather than looping forever.
const fn utf8_char_len(first: u8) -> usize {
    // Continuation bytes and ASCII both step by 1 — deliberately.
    // The `if first < 0xC0` guard collapses those two ranges and keeps
    // the function total across every u8, including invalid leads.
    if first < 0xC0 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

/// Straight substring search over byte slices — no third-party regex.
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    if hay.len() < needle.len() {
        return None;
    }
    for i in 0..=hay.len() - needle.len() {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

#[cfg(feature = "alloc")]
#[allow(unused_imports)]
use alloc::format;

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Build a byte-alphabet vocabulary starting at id 0, then extend
    /// with each of `extras` (in order) at the next available id.
    /// Returns the vocabulary and the ids assigned to the extras.
    fn byte_vocab_with_extras(extras: &[&[u8]]) -> (BpeVocabulary, Vec<TokenId>) {
        let mut v = BpeVocabulary::new();
        let start = v.ensure_byte_alphabet(0).unwrap();
        let mut ids = Vec::new();
        for (i, e) in extras.iter().enumerate() {
            let id = start + u32::try_from(i).unwrap();
            v.insert(id, e.to_vec()).unwrap();
            ids.push(id);
        }
        (v, ids)
    }

    #[test]
    fn empty_input_encodes_to_empty() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        let enc = tok.encode("").unwrap();
        assert!(enc.is_empty());
        assert_eq!(tok.count("").unwrap(), 0);
    }

    #[test]
    fn single_ascii_char_one_token_per_byte_before_merges() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        // "a" is one byte; with no merges it becomes one token whose id
        // equals the byte value under our byte-alphabet layout.
        let enc = tok.encode("a").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'a')]);
        assert_eq!(enc.offsets, vec![0..1]);
    }

    #[test]
    fn multi_char_ascii_word_no_merges_emits_one_token_per_byte() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        let enc = tok.encode("cat").unwrap();
        assert_eq!(
            enc.ids,
            vec![u32::from(b'c'), u32::from(b'a'), u32::from(b't')]
        );
        assert_eq!(enc.offsets, vec![0..1, 1..2, 2..3]);
    }

    #[test]
    fn single_merge_fires() {
        // Vocabulary: 0..=255 = bytes, then "ca" at id 256.
        let (vocab, ids) = byte_vocab_with_extras(&[b"ca"]);
        let mut merges = BpeMergeTable::new();
        merges.insert(b"c".to_vec(), b"a".to_vec(), 0);
        let tok = BpeTokenizer::from_parts(merges, vocab);
        let enc = tok.encode("cat").unwrap();
        // After the merge, pieces are ["ca", "t"] → ids [256, b't'].
        assert_eq!(enc.ids, vec![ids[0], u32::from(b't')]);
        assert_eq!(enc.offsets, vec![0..2, 2..3]);
    }

    #[test]
    fn merge_rank_priorities_are_honoured() {
        // Input "abc". Two possible merges: ("a","b") rank 1, ("b","c") rank 0.
        // Rank 0 wins → first merge is "bc", leaving ["a","bc"].
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        vocab.insert(256, b"bc".to_vec()).unwrap();
        vocab.insert(257, b"ab".to_vec()).unwrap();
        let mut merges = BpeMergeTable::new();
        merges.insert(b"b".to_vec(), b"c".to_vec(), 0);
        merges.insert(b"a".to_vec(), b"b".to_vec(), 1);
        let tok = BpeTokenizer::from_parts(merges, vocab);
        let enc = tok.encode("abc").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'a'), 256]);
    }

    #[test]
    fn iterative_merges_compose_into_longer_pieces() {
        // Vocabulary carries "ca", "cat" (from ca+t), starting at 256.
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        vocab.insert(256, b"ca".to_vec()).unwrap();
        vocab.insert(257, b"cat".to_vec()).unwrap();
        let mut merges = BpeMergeTable::new();
        merges.insert(b"c".to_vec(), b"a".to_vec(), 0);
        merges.insert(b"ca".to_vec(), b"t".to_vec(), 1);
        let tok = BpeTokenizer::from_parts(merges, vocab);
        let enc = tok.encode("cat").unwrap();
        assert_eq!(enc.ids, vec![257]);
    }

    #[test]
    fn decode_reconstructs_input_for_ascii() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        let enc = tok.encode("hello").unwrap();
        let s = tok.decode(&enc.ids).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn decode_reconstructs_input_after_merges() {
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        vocab.insert(256, b"he".to_vec()).unwrap();
        vocab.insert(257, b"llo".to_vec()).unwrap();
        vocab.insert(258, b"ll".to_vec()).unwrap();
        let mut merges = BpeMergeTable::new();
        merges.insert(b"h".to_vec(), b"e".to_vec(), 0);
        merges.insert(b"l".to_vec(), b"l".to_vec(), 1);
        merges.insert(b"ll".to_vec(), b"o".to_vec(), 2);
        let tok = BpeTokenizer::from_parts(merges, vocab);
        let enc = tok.encode("hello").unwrap();
        let s = tok.decode(&enc.ids).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn special_tokens_bypass_bpe() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut specials = BTreeMap::new();
        specials.insert(String::from("<|endoftext|>"), 50000);
        let tok =
            BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_special_tokens(specials);
        let enc = tok.encode("hi<|endoftext|>").unwrap();
        // Expect: 'h', 'i', <|endoftext|>
        assert_eq!(enc.ids, vec![u32::from(b'h'), u32::from(b'i'), 50000]);
        assert_eq!(enc.special_mask, vec![false, false, true]);
    }

    #[test]
    fn special_tokens_matched_longest_first() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut specials = BTreeMap::new();
        specials.insert(String::from("<|a|>"), 1000);
        specials.insert(String::from("<|abc|>"), 1001);
        let tok =
            BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_special_tokens(specials);
        let enc = tok.encode("<|abc|>").unwrap();
        assert_eq!(enc.ids, vec![1001]);
    }

    #[test]
    fn decode_round_trip_with_special_tokens() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut specials = BTreeMap::new();
        specials.insert(String::from("<|s|>"), 500);
        let tok =
            BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_special_tokens(specials);
        let enc = tok.encode("a<|s|>b").unwrap();
        let s = tok.decode(&enc.ids).unwrap();
        assert_eq!(s, "a<|s|>b");
    }

    #[test]
    fn pre_tokenizer_literal_splits_input() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab)
            .with_pre_tokenizer(PreTokenizerRegex::literal(","));
        let enc = tok.encode("ab,cd").unwrap();
        // With no merges, each byte becomes one token. Splitting on "," doesn't
        // change the count of *tokens* — the comma character is a delimiter and
        // is discarded (per our pre-tokenizer contract). So we expect 4 tokens.
        assert_eq!(enc.ids.len(), 4);
        assert_eq!(
            enc.ids,
            vec![
                u32::from(b'a'),
                u32::from(b'b'),
                u32::from(b'c'),
                u32::from(b'd')
            ]
        );
    }

    // ---------------------------------------------------------------
    // PreTokenizerSequence — SentencePiece Metaspace on the BPE side
    // (Mistral-7B-v0.1 layout: Metaspace with prepend_scheme="first"
    // and split=false, character-level merges over a vocab that
    // stores UTF-8 surface strings). These tests exercise the encode
    // path with a hand-crafted vocab so they run without the real
    // Mistral tokenizer.json on disk.
    // ---------------------------------------------------------------

    #[cfg(feature = "std")]
    #[test]
    fn pre_tokenizer_sequence_metaspace_split_false_encodes_transformed_bytes() {
        use crate::pre_tokenizer::{Metaspace, PrependScheme};

        // Build a vocab that resembles a tiny character-BPE (Mistral-shape)
        // vocab: byte alphabet at 0..256, then UTF-8 surfaces for
        // `▁`, `h`, `e`, `l`, `o`, `w`, `r`, `d`. Merges compose
        // `▁hello` and `▁world` end-to-end so the full-word tokens beat
        // the character-by-character seeds.
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        let space_utf8 = "\u{2581}".as_bytes();
        vocab.insert(256, space_utf8.to_vec()).unwrap();
        vocab.insert(257, "\u{2581}h".as_bytes().to_vec()).unwrap();
        vocab.insert(258, "\u{2581}he".as_bytes().to_vec()).unwrap();
        vocab
            .insert(259, "\u{2581}hel".as_bytes().to_vec())
            .unwrap();
        vocab
            .insert(260, "\u{2581}hell".as_bytes().to_vec())
            .unwrap();
        vocab
            .insert(261, "\u{2581}hello".as_bytes().to_vec())
            .unwrap();
        vocab.insert(262, "\u{2581}w".as_bytes().to_vec()).unwrap();
        vocab.insert(263, "\u{2581}wo".as_bytes().to_vec()).unwrap();
        vocab
            .insert(264, "\u{2581}wor".as_bytes().to_vec())
            .unwrap();
        vocab
            .insert(265, "\u{2581}worl".as_bytes().to_vec())
            .unwrap();
        vocab
            .insert(266, "\u{2581}world".as_bytes().to_vec())
            .unwrap();

        let mut merges = BpeMergeTable::new();
        // Compose ▁hello left-to-right.
        merges.insert(space_utf8.to_vec(), b"h".to_vec(), 0);
        merges.insert("\u{2581}h".as_bytes().to_vec(), b"e".to_vec(), 1);
        merges.insert("\u{2581}he".as_bytes().to_vec(), b"l".to_vec(), 2);
        merges.insert("\u{2581}hel".as_bytes().to_vec(), b"l".to_vec(), 3);
        merges.insert("\u{2581}hell".as_bytes().to_vec(), b"o".to_vec(), 4);
        // And ▁world.
        merges.insert(space_utf8.to_vec(), b"w".to_vec(), 5);
        merges.insert("\u{2581}w".as_bytes().to_vec(), b"o".to_vec(), 6);
        merges.insert("\u{2581}wo".as_bytes().to_vec(), b"r".to_vec(), 7);
        merges.insert("\u{2581}wor".as_bytes().to_vec(), b"l".to_vec(), 8);
        merges.insert("\u{2581}worl".as_bytes().to_vec(), b"d".to_vec(), 9);

        // Metaspace with split=false and prepend_scheme=first (Mistral's
        // exact shape). Enable byte_fallback so seed_per_char turns on
        // — required because the vocab surfaces are UTF-8 characters,
        // not per-byte alphabet entries.
        let byte_fallback: [TokenId; 256] = core::array::from_fn(|b| u32::try_from(b).unwrap());
        let tok = BpeTokenizer::from_parts(merges, vocab)
            .with_byte_fallback(byte_fallback)
            .with_pre_tokenizer_sequence(Metaspace {
                replacement: Metaspace::DEFAULT_REPLACEMENT,
                prepend_scheme: PrependScheme::First,
                split: false,
            });

        let enc = tok.encode("hello world").unwrap();
        // Metaspace transforms "hello world" into "▁hello▁world" (one
        // piece, split=false). Merges compose that into `▁hello` (261)
        // and `▁world` (266).
        assert_eq!(enc.ids, vec![261, 266]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pre_tokenizer_sequence_metaspace_split_true_yields_per_word_pieces() {
        use crate::pre_tokenizer::Metaspace;

        // Same shape as above, but with split=true: Metaspace produces
        // two separate pieces (`▁hello` and `▁world`) that each feed
        // the merge loop independently. Ids should be identical to the
        // split=false case because per-word merges compose the same
        // full-word tokens.
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        let space_utf8 = "\u{2581}".as_bytes();
        vocab.insert(256, space_utf8.to_vec()).unwrap();
        vocab
            .insert(261, "\u{2581}hello".as_bytes().to_vec())
            .unwrap();
        vocab
            .insert(266, "\u{2581}world".as_bytes().to_vec())
            .unwrap();

        let mut merges = BpeMergeTable::new();
        merges.insert(space_utf8.to_vec(), b"h".to_vec(), 0);
        merges.insert("\u{2581}h".as_bytes().to_vec(), b"e".to_vec(), 1);
        merges.insert("\u{2581}he".as_bytes().to_vec(), b"l".to_vec(), 2);
        merges.insert("\u{2581}hel".as_bytes().to_vec(), b"l".to_vec(), 3);
        merges.insert("\u{2581}hell".as_bytes().to_vec(), b"o".to_vec(), 4);
        merges.insert(space_utf8.to_vec(), b"w".to_vec(), 5);
        merges.insert("\u{2581}w".as_bytes().to_vec(), b"o".to_vec(), 6);
        merges.insert("\u{2581}wo".as_bytes().to_vec(), b"r".to_vec(), 7);
        merges.insert("\u{2581}wor".as_bytes().to_vec(), b"l".to_vec(), 8);
        merges.insert("\u{2581}worl".as_bytes().to_vec(), b"d".to_vec(), 9);
        // Vocab needs the intermediate merged pieces so the merge loop
        // can walk from single-char seeds to the final `▁hello` id.
        // (Filled after `merges` is built because the vocab was
        // constructed above.)
        // (Skipped for brevity: the merge loop still produces `▁hello`
        // because each merge step just needs the produced bytes to be
        // in the merge table — it looks up the vocab id at emit time.)

        let byte_fallback: [TokenId; 256] = core::array::from_fn(|b| u32::try_from(b).unwrap());
        let tok = BpeTokenizer::from_parts(merges, vocab)
            .with_byte_fallback(byte_fallback)
            .with_pre_tokenizer_sequence(Metaspace::new());

        let enc = tok.encode("hello world").unwrap();
        assert_eq!(enc.ids, vec![261, 266]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pre_tokenizer_sequence_getter_reflects_configuration() {
        use crate::pre_tokenizer::Metaspace;

        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok_bare = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab.clone());
        assert!(tok_bare.pre_tokenizer_sequence().is_none());

        let tok_wired = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab)
            .with_pre_tokenizer_sequence(Metaspace::new());
        let seq = tok_wired
            .pre_tokenizer_sequence()
            .expect("Metaspace sequence must be wired");
        assert!(seq.metaspace().is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn pre_tokenizer_sequence_extracts_specials_around_metaspace_region() {
        use crate::pre_tokenizer::Metaspace;

        // A special token embedded inside the input still lands as a
        // special id — the pre_tokenizer_sequence runs per region
        // (between-specials extraction), matching how HF's tokenizers
        // pipeline is layered. Vocab / merges assembled just enough to
        // let `▁hi` compose end-to-end.
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        let space_utf8 = "\u{2581}".as_bytes();
        vocab.insert(256, space_utf8.to_vec()).unwrap();
        vocab.insert(300, "\u{2581}h".as_bytes().to_vec()).unwrap();
        vocab.insert(301, "\u{2581}hi".as_bytes().to_vec()).unwrap();

        let mut merges = BpeMergeTable::new();
        merges.insert(space_utf8.to_vec(), b"h".to_vec(), 0);
        merges.insert("\u{2581}h".as_bytes().to_vec(), b"i".to_vec(), 1);

        let mut specials: BTreeMap<String, TokenId> = BTreeMap::new();
        specials.insert(String::from("<s>"), 1);
        specials.insert(String::from("</s>"), 2);

        let byte_fallback: [TokenId; 256] = core::array::from_fn(|b| u32::try_from(b).unwrap());
        let tok = BpeTokenizer::from_parts(merges, vocab)
            .with_special_tokens(specials)
            .with_byte_fallback(byte_fallback)
            .with_pre_tokenizer_sequence(Metaspace::new());

        let enc = tok.encode("<s>hi</s>").unwrap();
        // <s>, ▁hi, </s>.
        assert_eq!(enc.ids, vec![1, 301, 2]);
        assert_eq!(enc.special_mask, vec![true, false, true]);
    }

    // ---------------------------------------------------------------
    // Punctuation / Digits / Sequence pre-tokenizer stages.
    //
    // These exercise the multi-stage pipeline that Falcon-family
    // checkpoints wire through as
    // `Sequence[Punctuation(Contiguous), ByteLevel, Digits(individual_digits=false),
    // Split(Regex="[0-9][0-9][0-9]")]`. Each `SplitDelimiterBehavior`
    // mode is exercised on a punctuation-dense input.
    // ---------------------------------------------------------------

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_isolated_splits_each_punct_char() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let pieces = apply_stage(
            alloc::vec![String::from("Hello, world!")],
            &PreTokenizerRegex::punctuation(B::Isolated),
        );
        assert_eq!(
            pieces,
            vec![
                String::from("Hello"),
                String::from(","),
                String::from(" world"),
                String::from("!"),
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_contiguous_groups_adjacent_puncts() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let pieces = apply_stage(
            alloc::vec![String::from("Hello,, world!!")],
            &PreTokenizerRegex::punctuation(B::Contiguous),
        );
        // Consecutive `,,` and `!!` collapse into single pieces.
        assert_eq!(
            pieces,
            vec![
                String::from("Hello"),
                String::from(",,"),
                String::from(" world"),
                String::from("!!"),
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_removed_drops_matches() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let pieces = apply_stage(
            alloc::vec![String::from("Hello, world!")],
            &PreTokenizerRegex::punctuation(B::Removed),
        );
        assert_eq!(pieces, vec![String::from("Hello"), String::from(" world")]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_merged_with_previous_glues_left() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let pieces = apply_stage(
            alloc::vec![String::from("Hello, world!")],
            &PreTokenizerRegex::punctuation(B::MergedWithPrevious),
        );
        assert_eq!(
            pieces,
            vec![String::from("Hello,"), String::from(" world!")]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_merged_with_next_glues_right() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let pieces = apply_stage(
            alloc::vec![String::from("Hello, world!")],
            &PreTokenizerRegex::punctuation(B::MergedWithNext),
        );
        // `,` glues onto ` world`; the final `!` has nothing following
        // it and stays as its own piece.
        assert_eq!(
            pieces,
            vec![
                String::from("Hello"),
                String::from(", world"),
                String::from("!")
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn punctuation_stage_handles_unicode_puncts() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        // Ideographic comma (、), fullwidth question mark (？), Chinese
        // enumeration comma. All Unicode `P*` categories.
        let pieces = apply_stage(
            alloc::vec![String::from("Hello，world。")],
            &PreTokenizerRegex::punctuation(B::Isolated),
        );
        assert_eq!(
            pieces,
            vec![
                String::from("Hello"),
                String::from("，"),
                String::from("world"),
                String::from("。"),
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn digits_stage_runs_kept_together() {
        // Falcon uses `individual_digits=false`, which keeps digit runs
        // as single pieces.
        let pieces = apply_stage(
            alloc::vec![String::from("abc123def")],
            &PreTokenizerRegex::digits(false),
        );
        assert_eq!(
            pieces,
            vec![
                String::from("abc"),
                String::from("123"),
                String::from("def")
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn digits_stage_individual_splits_each_digit() {
        let pieces = apply_stage(
            alloc::vec![String::from("a12b")],
            &PreTokenizerRegex::digits(true),
        );
        assert_eq!(
            pieces,
            vec![
                String::from("a"),
                String::from("1"),
                String::from("2"),
                String::from("b"),
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn sequence_stage_applies_children_left_to_right() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        // Punctuation(Isolated) then Digits(false):
        //   "abc, 12!def3"
        //   → Punctuation: ["abc", ",", " 12", "!", "def3"]
        //   → Digits:      ["abc", ",", " ", "12", "!", "def", "3"]
        let pipeline = PreTokenizerRegex::sequence(alloc::vec![
            PreTokenizerRegex::punctuation(B::Isolated),
            PreTokenizerRegex::digits(false),
        ]);
        let pieces = apply_stage(alloc::vec![String::from("abc, 12!def3")], &pipeline);
        assert_eq!(
            pieces,
            vec![
                String::from("abc"),
                String::from(","),
                String::from(" "),
                String::from("12"),
                String::from("!"),
                String::from("def"),
                String::from("3"),
            ]
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn sequence_stage_empty_is_identity() {
        let pipeline = PreTokenizerRegex::sequence(alloc::vec![]);
        let pieces = apply_stage(alloc::vec![String::from("hello")], &pipeline);
        assert_eq!(pieces, vec![String::from("hello")]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn contains_byte_level_walks_nested_sequence() {
        use crate::pre_tokenizer::SplitDelimiterBehavior as B;
        let bl = PreTokenizerRegex::byte_level(false, None);
        assert!(bl.contains_byte_level());
        let pipeline = PreTokenizerRegex::sequence(alloc::vec![
            PreTokenizerRegex::punctuation(B::Isolated),
            bl,
            PreTokenizerRegex::digits(false),
        ]);
        assert!(pipeline.contains_byte_level());
        // No byte-level anywhere → false.
        let no_bl = PreTokenizerRegex::sequence(alloc::vec![
            PreTokenizerRegex::punctuation(B::Isolated),
            PreTokenizerRegex::digits(false),
        ]);
        assert!(!no_bl.contains_byte_level());
    }

    // ---------------------------------------------------------------
    // Llama-family "Prepend(▁) + Replace(' '→'▁')" normalizer +
    // specials-around-normalizer interaction. These regression tests
    // cover the Phi-3-mini failing cases (empty, `<s>hi</s>`,
    // `<|end|>`) without needing the real 32k tokenizer.json on disk.
    // ---------------------------------------------------------------

    /// Assemble the Phi-3 shape by hand: character-BPE with a byte
    /// alphabet at 0..256, a `Metaspace`-marked `▁` at 256, and merges
    /// composing `▁hi` end-to-end. Specials `<s>`, `</s>`, `<|end|>`
    /// are registered with the same ids the real Phi-3 uses (1, 2,
    /// 32007). Byte-fallback is enabled so the seed-per-char path
    /// engages. A `Sequence[Prepend("▁"), Replace(" " → "▁")]`
    /// normalizer is attached — the same one the real Phi-3
    /// tokenizer.json ships. Returns the tokenizer.
    #[cfg(feature = "hf-normalizer")]
    fn build_phi3_shape_tokenizer() -> BpeTokenizer {
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        let space_utf8 = "\u{2581}".as_bytes();
        vocab.insert(256, space_utf8.to_vec()).unwrap();
        vocab.insert(300, "\u{2581}h".as_bytes().to_vec()).unwrap();
        // Real Phi-3 vocab has ▁hi at id 7251 — use that so the
        // fixture-derived ids are directly assertable.
        vocab
            .insert(7251, "\u{2581}hi".as_bytes().to_vec())
            .unwrap();
        // The bare `▁` mark also sits at 29871 in the real Phi-3
        // vocab. We insert a *separate* copy at 29871 too so the
        // "leading ▁ leaked out" bug (were it still live) would
        // surface as id 29871 rather than 256. Keeps this test's
        // "no leaked marker" assertions comparable to the fixture ids.
        vocab.insert(29871, alloc::vec![0xE2, 0x96, 0x81]).ok();

        let mut merges = BpeMergeTable::new();
        merges.insert(space_utf8.to_vec(), b"h".to_vec(), 0);
        merges.insert("\u{2581}h".as_bytes().to_vec(), b"i".to_vec(), 1);

        let mut specials: BTreeMap<String, TokenId> = BTreeMap::new();
        specials.insert(String::from("<s>"), 1);
        specials.insert(String::from("</s>"), 2);
        specials.insert(String::from("<|end|>"), 32007);

        let byte_fallback: [TokenId; 256] = core::array::from_fn(|b| u32::try_from(b).unwrap());
        let normalizer = crate::normalizer::Normalizer::Sequence(alloc::vec![
            crate::normalizer::Normalizer::Prepend {
                prepend: String::from("\u{2581}"),
            },
            crate::normalizer::Normalizer::Replace {
                pattern: String::from(" "),
                content: String::from("\u{2581}"),
            },
        ]);
        BpeTokenizer::from_parts(merges, vocab)
            .with_special_tokens(specials)
            .with_byte_fallback(byte_fallback)
            .with_normalizer(normalizer)
    }

    #[cfg(feature = "hf-normalizer")]
    #[test]
    fn phi3_shape_empty_input_encodes_to_empty() {
        // Regression for the phi-3-mini fixture's `empty` case:
        // reference emits `[]`, we used to emit `[29871]` (the bare
        // `▁` from the Prepend normalizer). With per-region
        // normalization on the RAW input, an empty input yields no
        // regions to normalize, so no marker is injected.
        let tok = build_phi3_shape_tokenizer();
        let enc = tok.encode("").unwrap();
        assert!(
            enc.ids.is_empty(),
            "empty input must produce no ids, got {:?}",
            enc.ids
        );
    }

    #[cfg(feature = "hf-normalizer")]
    #[test]
    fn phi3_shape_bos_eos_surface_form_is_recognised_without_leading_marker() {
        // Regression for `bos-eos-surface-form-raw`: `<s>hi</s>` must
        // encode to [<s>, ▁hi, </s>] with NO leading `▁` mark leaking
        // out of the normalizer. Before the specials-from-raw fix,
        // the Prepend ran first and produced `▁<s>hi</s>`; the `▁`
        // then BPE-encoded into a stray leading id 29871.
        let tok = build_phi3_shape_tokenizer();
        let enc = tok.encode("<s>hi</s>").unwrap();
        assert_eq!(enc.ids, vec![1, 7251, 2]);
        assert_eq!(enc.special_mask, vec![true, false, true]);
    }

    #[cfg(feature = "hf-normalizer")]
    #[test]
    fn phi3_shape_added_special_at_start_has_no_leading_marker() {
        // Regression for `chat-end-surface-form-raw`: `<|end|>` alone
        // must encode to [32007] with no leading `▁` marker. Before
        // the fix, the Prepend ran on the raw input and produced
        // `▁<|end|>`; the leading `▁` BPE-encoded into id 29871
        // before the special-token matcher had a chance to see the
        // raw `<|end|>` surface.
        let tok = build_phi3_shape_tokenizer();
        let enc = tok.encode("<|end|>").unwrap();
        assert_eq!(enc.ids, vec![32007]);
    }

    #[cfg(feature = "hf-normalizer")]
    #[test]
    fn phi3_shape_special_between_words_normalizes_each_region_independently() {
        // Sanity check the per-region normalization: a special
        // between two regions must have each region prepended
        // independently. For `hi<s>hi`, the pre-cursor `hi` region
        // normalises to `▁hi` (id 7251), then `<s>` (id 1), then the
        // post-cursor `hi` region ALSO normalises to `▁hi` (id 7251)
        // — matching how HF's added_vocabulary+extract_and_normalize
        // pipeline runs. This is the same behaviour Metaspace's
        // `PrependScheme::First` skips (offset > 0), but Prepend the
        // *normalizer* is unconditional and each region is offset 0
        // relative to itself.
        let tok = build_phi3_shape_tokenizer();
        let enc = tok.encode("hi<s>hi").unwrap();
        assert_eq!(enc.ids, vec![7251, 1, 7251]);
    }

    // ---------------------------------------------------------------
    // Added-vocab flag semantics (normalized / lstrip / rstrip /
    // special:false and their combinations). These exercise the
    // Phase-1/Phase-2 matcher in isolation from any real vocab.
    // ---------------------------------------------------------------

    /// Build a byte-alphabet tokenizer with a single [`AddedTokenFlags`]
    /// entry.
    fn added_vocab_tokenizer(surface: &str, flags: AddedTokenFlags) -> BpeTokenizer {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut entries = BTreeMap::new();
        entries.insert(String::from(surface), flags);
        BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_added_vocab(entries)
    }

    #[test]
    fn added_vocab_special_false_still_matched_and_emitted() {
        // Regression for the Phi-3-mini `</s>` failure: the entry is
        // `special: false` but must still be pre-extracted from the
        // raw input, otherwise it BPE-encodes character by character.
        let tok = added_vocab_tokenizer(
            "</s>",
            AddedTokenFlags {
                id: 42,
                normalized: false,
                lstrip: false,
                rstrip: false,
                special: false,
            },
        );
        let enc = tok.encode("</s>").unwrap();
        assert_eq!(enc.ids, vec![42]);
        // But its `special_mask` bit is FALSE because the entry is
        // `special: false` — matches HF's `is_special` field on the
        // produced encoding.
        assert_eq!(enc.special_mask, vec![false]);
    }

    #[test]
    fn added_vocab_rstrip_true_consumes_trailing_whitespace() {
        // A `rstrip: true` entry absorbs trailing whitespace so the
        // adjacent BPE region never sees it. `</s>  ab` becomes
        // `[42, id(a), id(b)]` — the two trailing spaces are eaten
        // and there is no whitespace region between them.
        let tok = added_vocab_tokenizer(
            "</s>",
            AddedTokenFlags {
                id: 42,
                normalized: false,
                lstrip: false,
                rstrip: true,
                special: true,
            },
        );
        let enc = tok.encode("</s>  ab").unwrap();
        assert_eq!(enc.ids, vec![42, u32::from(b'a'), u32::from(b'b')]);
    }

    #[test]
    fn added_vocab_lstrip_true_consumes_preceding_whitespace() {
        // A `lstrip: true` entry absorbs leading whitespace so the
        // adjacent BPE region on the left never emits it. `ab  <|end|>`
        // becomes `[id(a), id(b), 42]`.
        let tok = added_vocab_tokenizer(
            "<|end|>",
            AddedTokenFlags {
                id: 42,
                normalized: false,
                lstrip: true,
                rstrip: false,
                special: true,
            },
        );
        let enc = tok.encode("ab  <|end|>").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'a'), u32::from(b'b'), 42]);
    }

    #[test]
    fn added_vocab_lstrip_and_rstrip_compose() {
        // Combining both flags: the entry eats whitespace on both
        // sides. `ab \t<|end|> \ncd` → `[id(a), id(b), 42, id(c),
        // id(d)]`.
        let tok = added_vocab_tokenizer(
            "<|end|>",
            AddedTokenFlags {
                id: 42,
                normalized: false,
                lstrip: true,
                rstrip: true,
                special: true,
            },
        );
        let enc = tok.encode("ab \t<|end|> \ncd").unwrap();
        assert_eq!(
            enc.ids,
            vec![
                u32::from(b'a'),
                u32::from(b'b'),
                42,
                u32::from(b'c'),
                u32::from(b'd')
            ]
        );
    }

    #[cfg(feature = "hf-normalizer")]
    #[test]
    fn added_vocab_normalized_true_matches_normalized_region() {
        // `normalized: true` entries are matched AFTER per-region
        // normalization. A `Replace(" " → "_")` normalizer converts
        // `hello world` to `hello_world`; a `normalized: true` entry
        // for `_w` (id 900) matches on the normalized text.
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let normalizer = crate::normalizer::Normalizer::Replace {
            pattern: String::from(" "),
            content: String::from("_"),
        };
        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("_w"),
            AddedTokenFlags {
                id: 900,
                normalized: true,
                lstrip: false,
                rstrip: false,
                special: false,
            },
        );
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab)
            .with_added_vocab(entries)
            .with_normalizer(normalizer);
        let enc = tok.encode("hello world").unwrap();
        // Expected: "hello" (5 byte ids) + `_w` (900) + "orld" (4 byte
        // ids). The normalizer swaps the space for `_`, then the
        // Phase-2 scanner matches `_w` and emits 900.
        assert_eq!(
            enc.ids,
            vec![
                u32::from(b'h'),
                u32::from(b'e'),
                u32::from(b'l'),
                u32::from(b'l'),
                u32::from(b'o'),
                900,
                u32::from(b'o'),
                u32::from(b'r'),
                u32::from(b'l'),
                u32::from(b'd'),
            ]
        );
    }

    #[test]
    fn added_vocab_normalized_true_with_special_false_matched_and_flagged_nonspecial() {
        // The `normalized: true + special: false` combination is
        // Phi-2's whitespace-run compression shape: `"    "` (4
        // spaces) at id 50284, matched inside the normalized region
        // and NOT flagged as special. Verifying both flags compose
        // together in the Phase-2 matcher.
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut entries = BTreeMap::new();
        entries.insert(
            String::from("    "),
            AddedTokenFlags {
                id: 50284,
                normalized: true,
                lstrip: false,
                rstrip: false,
                special: false,
            },
        );
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_added_vocab(entries);
        let enc = tok.encode("a    b").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'a'), 50284, u32::from(b'b')]);
        assert_eq!(enc.special_mask, vec![false, false, false]);
    }

    #[test]
    fn added_vocab_longest_first_wins_over_shorter_prefix() {
        // If two added-vocab entries share a common prefix, the longer
        // wins at match time (`<|im_start|>` beats `<|im|>`). This is
        // the same longest-match invariant `sorted_specials` enforced,
        // now applied per phase.
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut entries = BTreeMap::new();
        entries.insert(String::from("<|im|>"), AddedTokenFlags::legacy_special(10));
        entries.insert(
            String::from("<|im_start|>"),
            AddedTokenFlags::legacy_special(11),
        );
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_added_vocab(entries);
        let enc = tok.encode("<|im_start|>").unwrap();
        assert_eq!(enc.ids, vec![11]);
    }

    #[test]
    fn added_vocab_matches_across_utf8_boundaries_safely() {
        // A between-matches walk that encounters a multi-byte UTF-8
        // sequence must advance one Unicode scalar at a time, never
        // splitting a continuation byte. Regression sentinel for the
        // Phase-1 raw-scan cursor step.
        let tok = added_vocab_tokenizer("<|end|>", AddedTokenFlags::legacy_special(42));
        // "café<|end|>" — the `é` is 2 UTF-8 bytes (0xC3 0xA9); a
        // byte-by-byte cursor would land on the continuation byte.
        let enc = tok.encode("café<|end|>").unwrap();
        // The 4 bytes of "café" become 4 vocab lookups (the bytes are
        // in the byte-alphabet vocab), then id 42 for the special.
        assert_eq!(enc.ids.last(), Some(&42));
    }

    #[test]
    fn count_matches_encode_length() {
        let (mut vocab, _) = byte_vocab_with_extras(&[]);
        vocab.insert(256, b"he".to_vec()).unwrap();
        let mut merges = BpeMergeTable::new();
        merges.insert(b"h".to_vec(), b"e".to_vec(), 0);
        let tok = BpeTokenizer::from_parts(merges, vocab);
        let text = "hello world";
        assert_eq!(tok.count(text).unwrap(), tok.encode(text).unwrap().len());
    }

    #[test]
    fn unknown_byte_returns_unknown_token_error() {
        // Build a vocabulary that does NOT include every byte — omit 'z'.
        let mut v = BpeVocabulary::new();
        for b in b'a'..=b'y' {
            v.insert(u32::from(b), alloc::vec![b]).unwrap();
        }
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
        let err = tok.encode("z").unwrap_err();
        assert!(matches!(err, TokenizerError::UnknownToken(_)));
    }

    #[test]
    fn utf8_input_decoded_correctly() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        let text = "héllo";
        let enc = tok.encode(text).unwrap();
        let round = tok.decode(&enc.ids).unwrap();
        assert_eq!(round, text);
    }

    #[test]
    fn empty_merge_table_still_encodes() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        let enc = tok.encode("hi").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'h'), u32::from(b'i')]);
    }

    #[test]
    fn vocabulary_insert_rejects_conflicts() {
        let mut v = BpeVocabulary::new();
        v.insert(0, alloc::vec![b'a']).unwrap();
        assert!(v.insert(0, alloc::vec![b'b']).is_err());
        assert!(v.insert(1, alloc::vec![b'a']).is_err());
        // Same mapping twice: idempotent.
        assert!(v.insert(0, alloc::vec![b'a']).is_ok());
    }

    #[test]
    fn merge_table_rank_lookup() {
        let mut t = BpeMergeTable::new();
        t.insert(b"a".to_vec(), b"b".to_vec(), 42);
        assert_eq!(t.rank(b"a", b"b"), Some(42));
        assert_eq!(t.rank(b"x", b"y"), None);
        assert_eq!(t.len(), 1);
        assert!(!t.is_empty());
    }

    // ------------------------------------------------------------------
    // Reference test — a hand-constructed small merge table + vocabulary
    // over the toy corpus {"cat","cats","dog","dogs"}. Roughly follows
    // the shape of the merges a BPE trainer would produce over such a
    // corpus and captures the *deterministic* encoding under those
    // merges.
    // ------------------------------------------------------------------

    fn build_reference_tokenizer() -> BpeTokenizer {
        // Byte alphabet 0..=255, plus these merged entries:
        //   256 = "ca"
        //   257 = "at"
        //   258 = "cat"
        //   259 = "do"
        //   260 = "og"
        //   261 = "dog"
        //   262 = "ts"
        //   263 = "gs"
        //   264 = "cats"
        //   265 = "dogs"
        //   266 = "he"
        //   267 = "ll"
        //   268 = "hell"
        //   269 = "hello"
        //   270 = "or"
        //   271 = "wo"
        //   272 = "rl"
        //   273 = "wor"
        //   274 = "world"
        //   275 = "lo"
        //   276 = "llo"
        //   277 = "orld"
        //   278 = "rld"
        //   279 = "ld"
        // Twenty distinct merges spanning the substrings needed to
        // encode "cat cats dog dogs hello world" deterministically.
        let mut v = BpeVocabulary::new();
        let start = v.ensure_byte_alphabet(0).unwrap();
        let extras: &[&[u8]] = &[
            b"ca", b"at", b"cat", b"do", b"og", b"dog", b"ts", b"gs", b"cats", b"dogs", b"he",
            b"ll", b"hell", b"hello", b"or", b"wo", b"rl", b"wor", b"world", b"lo", b"llo",
            b"orld", b"rld", b"ld",
        ];
        for (i, e) in extras.iter().enumerate() {
            v.insert(start + u32::try_from(i).unwrap(), e.to_vec())
                .unwrap();
        }
        let mut m = BpeMergeTable::new();
        // Ranks encode the priority order. Lower is earlier.
        // Prefer whole-word merges first; then smaller building blocks.
        // 20 distinct merges (as promised in the task).
        let rules: &[(&[u8], &[u8], u32)] = &[
            (b"c", b"a", 0),
            (b"ca", b"t", 1),
            (b"cat", b"s", 2),
            (b"d", b"o", 3),
            (b"do", b"g", 4),
            (b"dog", b"s", 5),
            (b"h", b"e", 6),
            (b"l", b"l", 7),
            (b"he", b"ll", 8),
            (b"hell", b"o", 9),
            (b"w", b"o", 10),
            (b"wo", b"r", 11),
            (b"wor", b"l", 12),
            (b"worl", b"d", 13),
            (b"o", b"r", 14),
            (b"r", b"l", 15),
            (b"rl", b"d", 16),
            (b"or", b"ld", 17),
            (b"l", b"o", 18),
            (b"ll", b"o", 19),
        ];
        for &(l, r, rank) in rules {
            m.insert(l.to_vec(), r.to_vec(), rank);
        }
        BpeTokenizer::from_parts(m, v)
    }

    #[test]
    fn reference_cat_encodes_as_expected() {
        let tok = build_reference_tokenizer();
        let enc = tok.encode("cat").unwrap();
        assert_eq!(enc.ids, vec![258]); // "cat"
        assert_eq!(tok.decode(&enc.ids).unwrap(), "cat");
    }

    #[test]
    fn reference_cats_encodes_as_expected() {
        let tok = build_reference_tokenizer();
        let enc = tok.encode("cats").unwrap();
        // Rank-0 merge is ("c","a") → "ca"
        // Then merge ("ca","t") rank 1 → "cat"
        // Then merge ("cat","s") rank 2 → "cats" (id 264 doesn't matter for the ids
        // because we're checking that BPE reaches "cats"; the vocab lookup
        // uses whatever id maps to b"cats").
        let cats_id = tok.vocab().id(b"cats").unwrap();
        assert_eq!(enc.ids, vec![cats_id]);
        assert_eq!(tok.decode(&enc.ids).unwrap(), "cats");
    }

    #[test]
    fn reference_dog_and_dogs_encode() {
        let tok = build_reference_tokenizer();
        let singular = tok.vocab().id(b"dog").unwrap();
        let plural = tok.vocab().id(b"dogs").unwrap();
        assert_eq!(tok.encode("dog").unwrap().ids, vec![singular]);
        assert_eq!(tok.encode("dogs").unwrap().ids, vec![plural]);
    }

    #[test]
    fn reference_hello_and_world_round_trip() {
        let tok = build_reference_tokenizer();
        for w in ["cat", "cats", "dog", "dogs", "hello", "world"] {
            let enc = tok.encode(w).unwrap();
            assert_eq!(tok.decode(&enc.ids).unwrap(), w, "roundtrip failed for {w}");
        }
    }

    #[test]
    fn reference_prefix_words_multi_word_input_defaults_to_whitespace_split() {
        // Without a pre-tokenizer, the fallback is whitespace split.
        let tok = build_reference_tokenizer();
        let enc = tok.encode("cat dog").unwrap();
        // Expect: 258 ("cat"), then 261 ("dog").
        let cat_id = tok.vocab().id(b"cat").unwrap();
        let dog_id = tok.vocab().id(b"dog").unwrap();
        assert_eq!(enc.ids, vec![cat_id, dog_id]);
        // Round-trip loses the space (whitespace is dropped by the
        // pre-tokenizer). This is a documented lossy exception.
        let round = tok.decode(&enc.ids).unwrap();
        assert_eq!(round, "catdog");
    }

    // ------------------------------------------------------------------
    // Phase 2 acceptance oracle: the O(n log n) implementation must
    // agree with the naive O(n²) implementation "over exhaustive short
    // inputs." The proptest module (below) covers the random-input case;
    // this block enumerates short inputs deterministically so the
    // agreement is checked by every host in every CI run — no proptest
    // seed to lose, no wasm target skip.
    // ------------------------------------------------------------------

    /// Drive the naive merge loop through the full encoding pipeline —
    /// same as `BpeTokenizer::encode`, but with the merge strategy
    /// swapped for the O(n²) oracle. Test-only helper.
    fn encode_via_naive(tok: &BpeTokenizer, text: &str) -> Vec<TokenId> {
        let pieces = tok
            .encode_pieces_with(text, BpeTokenizer::merge_loop_naive)
            .expect("naive encode should succeed on byte-alphabet vocab");
        pieces.into_iter().map(|(id, _, _)| id).collect()
    }

    fn encode_via_nlogn(tok: &BpeTokenizer, text: &str) -> Vec<TokenId> {
        let pieces = tok
            .encode_pieces_with(text, BpeTokenizer::merge_loop_nlogn)
            .expect("nlogn encode should succeed on byte-alphabet vocab");
        pieces.into_iter().map(|(id, _, _)| id).collect()
    }

    /// Drive the flat (tiktoken-style) merge loop through the full
    /// encoding pipeline — same as [`encode_via_naive`] / [`encode_via_nlogn`]
    /// but with the production merge strategy explicit at the call site.
    fn encode_via_flat(tok: &BpeTokenizer, text: &str) -> Vec<TokenId> {
        let pieces = tok
            .encode_pieces_with(text, BpeTokenizer::merge_loop_flat)
            .expect("flat encode should succeed on byte-alphabet vocab");
        pieces.into_iter().map(|(id, _, _)| id).collect()
    }

    /// Enumerate every string of length 0..=`max_len` over `alphabet`.
    fn enumerate_strings(alphabet: &[u8], max_len: usize) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = vec![Vec::new()];
        let mut frontier: Vec<Vec<u8>> = vec![Vec::new()];
        for _ in 1..=max_len {
            let mut next = Vec::new();
            for s in &frontier {
                for &c in alphabet {
                    let mut x = s.clone();
                    x.push(c);
                    next.push(x);
                }
            }
            out.extend(next.iter().cloned());
            frontier = next;
        }
        out
    }

    #[test]
    fn nlogn_matches_naive_exhaustive_len_0_to_6_alphabet_ab() {
        // Alphabet {a, b}: |alphabet|=2, len<=6 → 1 + 2 + 4 + 8 + 16 + 32 + 64 = 127 inputs.
        // Merge table covers every 2-byte pair, plus a few longer pieces
        // so merges compose non-trivially and the two strategies have
        // room to diverge on tie-break order (they mustn't).
        let mut v = BpeVocabulary::new();
        v.ensure_byte_alphabet(0).unwrap();
        v.insert(256, b"ab".to_vec()).unwrap();
        v.insert(257, b"ba".to_vec()).unwrap();
        v.insert(258, b"aa".to_vec()).unwrap();
        v.insert(259, b"bb".to_vec()).unwrap();
        v.insert(260, b"aba".to_vec()).unwrap();
        v.insert(261, b"abab".to_vec()).unwrap();
        v.insert(262, b"bab".to_vec()).unwrap();
        v.insert(263, b"aabb".to_vec()).unwrap();
        v.insert(264, b"abba".to_vec()).unwrap();
        let mut m = BpeMergeTable::new();
        m.insert(b"a".to_vec(), b"b".to_vec(), 0);
        m.insert(b"b".to_vec(), b"a".to_vec(), 1);
        m.insert(b"ab".to_vec(), b"a".to_vec(), 2);
        m.insert(b"ab".to_vec(), b"ab".to_vec(), 3);
        m.insert(b"b".to_vec(), b"ab".to_vec(), 4);
        m.insert(b"a".to_vec(), b"a".to_vec(), 5);
        m.insert(b"b".to_vec(), b"b".to_vec(), 6);
        m.insert(b"a".to_vec(), b"abb".to_vec(), 7);
        m.insert(b"ab".to_vec(), b"ba".to_vec(), 8);
        let tok = BpeTokenizer::from_parts(m, v);

        for s in enumerate_strings(b"ab", 6) {
            let text = core::str::from_utf8(&s).unwrap();
            let via_naive = encode_via_naive(&tok, text);
            let via_nlogn = encode_via_nlogn(&tok, text);
            let via_flat = encode_via_flat(&tok, text);
            assert_eq!(via_naive, via_nlogn, "nlogn disagrees on {text:?}");
            assert_eq!(via_naive, via_flat, "flat disagrees on {text:?}");
        }
    }

    #[test]
    fn nlogn_matches_naive_exhaustive_len_0_to_4_alphabet_abc() {
        // Alphabet {a, b, c}: len<=4 → 1 + 3 + 9 + 27 + 81 = 121 inputs.
        // Broader alphabet exposes cases where the leftmost pair of a
        // given rank is not adjacent to the previous merge — a shape the
        // naive scanner and the heap-driven walker resolve differently
        // if the tie-break is wrong.
        let mut v = BpeVocabulary::new();
        v.ensure_byte_alphabet(0).unwrap();
        // Every 2-byte pair over {a,b,c} lives in the vocab; a couple
        // of 3-4 byte pieces so merges compose.
        let extras: &[&[u8]] = &[
            b"ab", b"ac", b"ba", b"bc", b"ca", b"cb", b"aa", b"bb", b"cc", b"abc", b"cab", b"aba",
            b"cbc", b"abca", b"caba",
        ];
        for (i, e) in extras.iter().enumerate() {
            v.insert(256 + u32::try_from(i).unwrap(), e.to_vec())
                .unwrap();
        }
        let mut m = BpeMergeTable::new();
        let rules: &[(&[u8], &[u8], u32)] = &[
            (b"a", b"b", 0),
            (b"b", b"c", 1),
            (b"c", b"a", 2),
            (b"a", b"c", 3),
            (b"b", b"a", 4),
            (b"c", b"b", 5),
            (b"ab", b"c", 6),
            (b"c", b"ab", 7),
            (b"a", b"bc", 8),
            (b"ca", b"b", 9),
            (b"ab", b"ca", 10),
        ];
        for &(l, r, rank) in rules {
            m.insert(l.to_vec(), r.to_vec(), rank);
        }
        let tok = BpeTokenizer::from_parts(m, v);

        for s in enumerate_strings(b"abc", 4) {
            let text = core::str::from_utf8(&s).unwrap();
            let via_naive = encode_via_naive(&tok, text);
            let via_nlogn = encode_via_nlogn(&tok, text);
            let via_flat = encode_via_flat(&tok, text);
            assert_eq!(via_naive, via_nlogn, "nlogn disagrees on {text:?}");
            assert_eq!(via_naive, via_flat, "flat disagrees on {text:?}");
        }
    }

    #[test]
    fn nlogn_matches_naive_over_reference_corpus() {
        let tok = build_reference_tokenizer();
        // Includes the whitespace-split fallback (multi-word input) and
        // pure single-word inputs so both the region-splitting seam and
        // the merge loop itself are exercised.
        let corpus = [
            "",
            "c",
            "a",
            "t",
            "cat",
            "cats",
            "dog",
            "dogs",
            "hello",
            "world",
            "cat dog",
            "hello world",
            "cats dogs hello world",
            "acat",
            "catsdogs",
        ];
        for text in corpus {
            let via_naive = encode_via_naive(&tok, text);
            let via_nlogn = encode_via_nlogn(&tok, text);
            let via_flat = encode_via_flat(&tok, text);
            assert_eq!(via_naive, via_nlogn, "nlogn disagrees on {text:?}");
            assert_eq!(via_naive, via_flat, "flat disagrees on {text:?}");
        }
    }

    #[test]
    fn nlogn_matches_naive_on_all_specials_input() {
        // Input consisting entirely of special-token surface strings.
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut specials = BTreeMap::new();
        specials.insert(String::from("<|s|>"), 500);
        specials.insert(String::from("<|t|>"), 501);
        let tok =
            BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_special_tokens(specials);
        for text in ["<|s|>", "<|s|><|t|>", "<|s|><|s|><|t|>"] {
            assert_eq!(encode_via_naive(&tok, text), encode_via_nlogn(&tok, text));
            assert_eq!(encode_via_naive(&tok, text), encode_via_flat(&tok, text));
        }
    }

    #[test]
    fn nlogn_matches_naive_on_unicode_edge_cases() {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        // A pinch of multibyte code points: Latin-1 supplement, CJK,
        // combining marks, an emoji-sized 4-byte code point.
        let corpus = [
            "",
            "a",
            "é",
            "héllo",
            "日本語",
            "e\u{0301}",
            "\u{1F600}",
            "混ぜ書き",
        ];
        for text in corpus {
            let via_naive = encode_via_naive(&tok, text);
            let via_nlogn = encode_via_nlogn(&tok, text);
            let via_flat = encode_via_flat(&tok, text);
            assert_eq!(via_naive, via_nlogn, "nlogn disagrees on {text:?}");
            assert_eq!(via_naive, via_flat, "flat disagrees on {text:?}");
        }
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "manual perf comparison, not correctness"]
    fn perf_compare_naive_vs_nlogn() {
        use std::time::Instant;
        for n in [50usize, 100, 200, 400] {
            let mut v = BpeVocabulary::new();
            v.ensure_byte_alphabet(0).unwrap();
            let mut m = BpeMergeTable::new();
            let mut prefix = b"a".to_vec();
            for (next_id, k) in (256_u32..).zip(0..(n - 1)) {
                let left = prefix.clone();
                m.insert(left.clone(), b"a".to_vec(), u32::try_from(k).unwrap());
                let mut merged = left;
                merged.push(b'a');
                v.insert(next_id, merged.clone()).unwrap();
                prefix = merged;
            }
            let tok = BpeTokenizer::from_parts(m, v);
            let input = "a".repeat(n);
            let iters = 10;

            let start = Instant::now();
            for _ in 0..iters {
                let _ = tok
                    .encode_pieces_with(&input, BpeTokenizer::merge_loop_naive)
                    .unwrap();
            }
            let naive_ns = start.elapsed().as_nanos() / iters;

            let start = Instant::now();
            for _ in 0..iters {
                let _ = tok
                    .encode_pieces_with(&input, BpeTokenizer::merge_loop_nlogn)
                    .unwrap();
            }
            let nlogn_ns = start.elapsed().as_nanos() / iters;

            let start = Instant::now();
            for _ in 0..iters {
                let _ = tok
                    .encode_pieces_with(&input, BpeTokenizer::merge_loop_flat)
                    .unwrap();
            }
            let flat_ns = start.elapsed().as_nanos() / iters;

            #[allow(clippy::cast_precision_loss)]
            let nlogn_speedup = (naive_ns as f64) / (nlogn_ns as f64);
            #[allow(clippy::cast_precision_loss)]
            let flat_speedup = (naive_ns as f64) / (flat_ns as f64);
            #[allow(clippy::cast_precision_loss)]
            let flat_vs_nlogn = (nlogn_ns as f64) / (flat_ns as f64);
            eprintln!(
                "n={n:4}: naive {naive_ns:>10} ns  nlogn {nlogn_ns:>10} ns  flat {flat_ns:>10} ns  \
                 nlogn/naive {nlogn_speedup:.2}x  flat/naive {flat_speedup:.2}x  flat/nlogn {flat_vs_nlogn:.2}x"
            );
        }
    }

    /// A short cl100k-shape workload: many small "words" (short byte
    /// sequences) with a dense merge table. Matches the actual pattern
    /// of BPE input: a stream of pre-tokenized regex chunks, most under
    /// 10 bytes long. Compares all three merge strategies.
    #[cfg(feature = "std")]
    #[test]
    #[ignore = "manual perf comparison, not correctness"]
    fn perf_compare_short_words_bpe() {
        use std::time::Instant;
        // Build a synthetic byte-alphabet tokenizer with dense merges
        // over 2-byte pairs — a cheap stand-in for the shape a real BPE
        // pipeline hands the merge loop after regex splitting.
        let mut v = BpeVocabulary::new();
        v.ensure_byte_alphabet(0).unwrap();
        let mut m = BpeMergeTable::new();
        // Insert every 2-byte pair over the printable ASCII range.
        // ~5000 merge rules — realistic density.
        let alphabet: Vec<u8> = (b'a'..=b'z').collect();
        let mut next_id: u32 = 256;
        let mut rank: u32 = 0;
        for &a in &alphabet {
            for &b in &alphabet {
                m.insert(vec![a], vec![b], rank);
                let pair = vec![a, b];
                if v.id(&pair).is_none() {
                    v.insert(next_id, pair).unwrap();
                    next_id += 1;
                }
                rank += 1;
            }
        }
        let tok = BpeTokenizer::from_parts(m, v);

        // Deterministic corpus of short "words" like a real pre-tokenized
        // regex chunk stream. ~50 KB total to give stable timings.
        let words: &[&str] = &[
            " the", " quick", " brown", " fox", " jumps", " over", " lazy", " dog", " and",
            " then", " runs", " back", " through", " forest", " toward", " little", " cabin",
            " on", " hill", " where", " smoke", " curls",
        ];
        let mut input = String::new();
        while input.len() < 50_000 {
            for w in words {
                input.push_str(w);
            }
        }
        let iters = 200;

        let start = Instant::now();
        for _ in 0..iters {
            let _ = tok
                .encode_pieces_with(&input, BpeTokenizer::merge_loop_nlogn)
                .unwrap();
        }
        let nlogn_ns = start.elapsed().as_nanos() / iters;

        let start = Instant::now();
        for _ in 0..iters {
            let _ = tok
                .encode_pieces_with(&input, BpeTokenizer::merge_loop_flat)
                .unwrap();
        }
        let flat_ns = start.elapsed().as_nanos() / iters;

        #[allow(clippy::cast_precision_loss)]
        let speedup = (nlogn_ns as f64) / (flat_ns as f64);
        eprintln!(
            "short-words ({} B input): nlogn {nlogn_ns:>10} ns  flat {flat_ns:>10} ns  \
             flat/nlogn {speedup:.2}x",
            input.len()
        );
    }

    // ------------------------------------------------------------------
    // encode_with_special_policy — tiktoken-style allowed_special /
    // disallowed_special enforcement at encode time.
    // ------------------------------------------------------------------

    /// Build a byte-alphabet tokenizer with three registered specials.
    /// The chosen surface strings are deliberately not BPE-mergeable
    /// under the empty merge table so the "flow through as regular
    /// text" case is easy to distinguish from the "emit as special" case
    /// in id-space.
    fn tokenizer_with_three_specials() -> (BpeTokenizer, TokenId, TokenId, TokenId) {
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let mut specials = BTreeMap::new();
        specials.insert(String::from("<|endoftext|>"), 50000);
        specials.insert(String::from("<|foo|>"), 50001);
        specials.insert(String::from("<|bar|>"), 50002);
        let tok =
            BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_special_tokens(specials);
        (tok, 50000, 50001, 50002)
    }

    #[test]
    fn policy_allowed_all_registered_matches_default_encode() {
        let (tok, _endoftext, _foo, _bar) = tokenizer_with_three_specials();
        let text = "hi<|endoftext|> world <|foo|>x<|bar|>";
        // Default encode: every registered special treated as special.
        let default = tok.encode(text).unwrap();
        // Policy with allowed = every registered surface + disallowed = None:
        // must match byte-for-byte.
        let allowed: BTreeSet<&str> = tok.special_tokens().keys().map(String::as_str).collect();
        let policy = tok
            .encode_with_special_policy(text, &allowed, &DisallowedSpecials::None)
            .unwrap();
        assert_eq!(policy.ids, default.ids);
        assert_eq!(policy.offsets, default.offsets);
        assert_eq!(policy.special_mask, default.special_mask);
    }

    #[test]
    fn policy_disallowed_all_empty_allowed_errors_on_any_registered_special() {
        let (tok, _endoftext, _foo, _bar) = tokenizer_with_three_specials();
        let allowed: BTreeSet<&str> = BTreeSet::new();
        // Each of the three surfaces should trigger the error when it
        // appears in the input, carrying its own surface string.
        let err = tok
            .encode_with_special_policy("hi<|endoftext|>", &allowed, &DisallowedSpecials::All)
            .unwrap_err();
        assert!(
            matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|endoftext|>")
        );
        let err = tok
            .encode_with_special_policy("<|foo|>", &allowed, &DisallowedSpecials::All)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|foo|>"));
        let err = tok
            .encode_with_special_policy("<|bar|>", &allowed, &DisallowedSpecials::All)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|bar|>"));
        // Input with no special: no error, encodes as plain bytes.
        let enc = tok
            .encode_with_special_policy("hi", &allowed, &DisallowedSpecials::All)
            .unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'h'), u32::from(b'i')]);
        assert!(enc.special_mask.iter().all(|&b| !b));
    }

    #[test]
    fn policy_disallowed_all_with_allowed_endoftext_lets_endoftext_through_errors_others() {
        let (tok, endoftext_id, _foo, _bar) = tokenizer_with_three_specials();
        let mut allowed: BTreeSet<&str> = BTreeSet::new();
        allowed.insert("<|endoftext|>");
        // endoftext is permitted — emitted as special.
        let enc = tok
            .encode_with_special_policy("a<|endoftext|>b", &allowed, &DisallowedSpecials::All)
            .unwrap();
        assert_eq!(
            enc.ids,
            vec![u32::from(b'a'), endoftext_id, u32::from(b'b')]
        );
        assert_eq!(enc.special_mask, vec![false, true, false]);
        // foo is not in allowed and disallowed=All → error.
        let err = tok
            .encode_with_special_policy("hi<|foo|>", &allowed, &DisallowedSpecials::All)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|foo|>"));
        // bar is not in allowed and disallowed=All → error.
        let err = tok
            .encode_with_special_policy("hi<|bar|>", &allowed, &DisallowedSpecials::All)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|bar|>"));
    }

    #[test]
    fn policy_disallowed_these_errors_only_on_listed_surfaces() {
        let (tok, endoftext_id, _foo, bar_id) = tokenizer_with_three_specials();
        // Empty allowed; only "<|foo|>" is disallowed. endoftext and bar
        // are neither allowed nor disallowed — their bytes flow through
        // the BPE loop as regular text.
        let allowed: BTreeSet<&str> = BTreeSet::new();
        let mut disallowed_set: BTreeSet<&str> = BTreeSet::new();
        disallowed_set.insert("<|foo|>");
        let disallowed = DisallowedSpecials::These(&disallowed_set);
        // foo alone → error.
        let err = tok
            .encode_with_special_policy("hi<|foo|>", &allowed, &disallowed)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|foo|>"));
        // endoftext + bar → no error, but they are NOT emitted as
        // specials — they flow through as raw bytes. To make that
        // observable, only check we get *no* special-id in the output
        // and no error.
        let enc = tok
            .encode_with_special_policy("<|endoftext|>", &allowed, &disallowed)
            .unwrap();
        assert!(enc.special_mask.iter().all(|&b| !b));
        assert!(!enc.ids.contains(&endoftext_id));
        assert!(!enc.ids.contains(&bar_id));
        // But foo is still forbidden even next to bar.
        let err = tok
            .encode_with_special_policy("<|bar|><|foo|>", &allowed, &disallowed)
            .unwrap_err();
        assert!(matches!(err, TokenizerError::DisallowedSpecialToken(ref s) if s == "<|foo|>"));
    }

    #[test]
    fn policy_allowed_wins_over_disallowed_when_surface_is_in_both() {
        let (tok, endoftext_id, _foo, _bar) = tokenizer_with_three_specials();
        // Surface listed in both sets: allowed wins → emitted as
        // special, no error.
        let mut allowed: BTreeSet<&str> = BTreeSet::new();
        allowed.insert("<|endoftext|>");
        let mut disallowed_set: BTreeSet<&str> = BTreeSet::new();
        disallowed_set.insert("<|endoftext|>");
        let disallowed = DisallowedSpecials::These(&disallowed_set);
        let enc = tok
            .encode_with_special_policy("a<|endoftext|>", &allowed, &disallowed)
            .unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'a'), endoftext_id]);
        assert_eq!(enc.special_mask, vec![false, true]);
    }

    #[test]
    fn policy_non_allowed_special_flows_through_as_regular_bytes() {
        // Verify that a registered special not in allowed_special is
        // encoded byte-by-byte (BPE loop over its surface bytes). Uses a
        // vocab where every byte has an id so we can assert exact ids.
        let (tok, _endoftext, foo_id, _bar) = tokenizer_with_three_specials();
        let mut allowed: BTreeSet<&str> = BTreeSet::new();
        allowed.insert("<|endoftext|>");
        let enc = tok
            .encode_with_special_policy("<|foo|>", &allowed, &DisallowedSpecials::None)
            .unwrap();
        // Should be 7 byte-level tokens (`<`, `|`, `f`, `o`, `o`, `|`, `>`),
        // not the special id 50001.
        let expected: Vec<u32> = "<|foo|>".bytes().map(u32::from).collect();
        assert_eq!(enc.ids, expected);
        assert!(!enc.ids.contains(&foo_id));
        assert!(enc.special_mask.iter().all(|&b| !b));
    }

    // ------------------------------------------------------------------
    // Byte-fallback (SentencePiece-style) — the mechanism that emits
    // a run of reserved `<0xXX>` tokens for a character with no
    // vocab-only path. Mirrors the Unigram-side coverage in
    // `crate::hf::UnigramTokenizer`.
    // ------------------------------------------------------------------

    /// Build a synthetic character-BPE vocabulary that resembles the
    /// Llama-2 / Mistral / Qwen shape:
    ///
    /// * ids 0..=2 — `<unk>`, `<s>`, `</s>` (three specials; not
    ///   registered as `special_tokens` here — the test cares about
    ///   the byte-fallback fan-out, not the special-token splice).
    /// * ids 3..=258 — the 256 reserved `<0x00>`..`<0xFF>` byte
    ///   tokens (id 3 = byte 0x00, id 3 + b = byte `b`).
    /// * id 259..= — a handful of single-character surfaces plus one
    ///   merged pair so the merge loop has something to do.
    ///
    /// Returns the tokenizer with byte-fallback enabled and the id of
    /// the merged surface for the caller to assert against.
    fn build_bpe_with_byte_fallback() -> (BpeTokenizer, [TokenId; 256], TokenId) {
        let mut v = BpeVocabulary::new();
        // Three fake specials at ids 0..=2 to match the real-world
        // layout of the byte-fallback region.
        v.insert(0, b"<unk>".to_vec()).unwrap();
        v.insert(1, b"<s>".to_vec()).unwrap();
        v.insert(2, b"</s>".to_vec()).unwrap();
        // The 256 reserved byte-fallback tokens at ids 3..=258.
        let mut byte_ids = [0u32; 256];
        for b in 0u32..=255 {
            let surface = alloc::format!("<0x{b:02X}>");
            let id = 3 + b;
            v.insert(id, surface.into_bytes()).unwrap();
            byte_ids[b as usize] = id;
        }
        // Single-character vocab entries the merge loop can chew on.
        let ch: &[u8] = b"hielowrd";
        let mut next: TokenId = 259;
        for &c in ch {
            v.insert(next, alloc::vec![c]).unwrap();
            next += 1;
        }
        // One merged pair so the encoder demonstrably reaches a
        // non-byte-fallback path when the input is vocab-covered.
        let hi_id = next;
        v.insert(next, b"hi".to_vec()).unwrap();
        let mut m = BpeMergeTable::new();
        m.insert(b"h".to_vec(), b"i".to_vec(), 0);
        let tok = BpeTokenizer::from_parts(m, v).with_byte_fallback(byte_ids);
        (tok, byte_ids, hi_id)
    }

    #[test]
    fn byte_fallback_enabled_reports_configuration() {
        let (tok, _, _) = build_bpe_with_byte_fallback();
        assert!(tok.byte_fallback_enabled());
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let bare = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab);
        assert!(!bare.byte_fallback_enabled());
    }

    #[test]
    fn byte_fallback_encodes_ascii_vocab_path_unchanged() {
        let (tok, _, hi_id) = build_bpe_with_byte_fallback();
        // "hi" merges to a single vocab id — the byte-fallback path
        // must NOT fire when a vocab-only path exists.
        let enc = tok.encode("hi").unwrap();
        assert_eq!(enc.ids, vec![hi_id]);
        assert!(enc.special_mask.iter().all(|&b| !b));
    }

    #[test]
    fn byte_fallback_fans_out_multibyte_oov_char() {
        let (tok, byte_ids, _) = build_bpe_with_byte_fallback();
        // `😀` is a 4-byte UTF-8 char (F0 9F 98 80) not in the vocab.
        // Every byte must be emitted as its reserved `<0xXX>` id.
        let enc = tok.encode("😀").unwrap();
        let expected: Vec<TokenId> = "😀".bytes().map(|b| byte_ids[b as usize]).collect();
        assert_eq!(enc.ids, expected);
        assert_eq!(enc.ids.len(), 4);
        // Each fanned-out id spans one byte of the input.
        assert_eq!(enc.offsets, vec![0..1, 1..2, 2..3, 3..4]);
    }

    #[test]
    fn byte_fallback_mixed_word_and_oov_char() {
        let (tok, byte_ids, hi_id) = build_bpe_with_byte_fallback();
        // "hi😀" → merge (h,i) → "hi" (vocab id) + byte-fallback for 😀.
        let enc = tok.encode("hi😀").unwrap();
        let mut expected = vec![hi_id];
        expected.extend("😀".bytes().map(|b| byte_ids[b as usize]));
        assert_eq!(enc.ids, expected);
    }

    #[test]
    fn byte_fallback_ascii_oov_char() {
        let (tok, byte_ids, _) = build_bpe_with_byte_fallback();
        // `?` (0x3F) is not in the vocab. Byte-fallback must emit a
        // single reserved id for byte 0x3F.
        let enc = tok.encode("?").unwrap();
        assert_eq!(enc.ids, vec![byte_ids[0x3F]]);
    }

    #[test]
    fn byte_fallback_all_oov_only_fires_fallback() {
        let (tok, byte_ids, _) = build_bpe_with_byte_fallback();
        // "?!" — both ASCII, neither in the vocab; both fall through.
        let enc = tok.encode("?!").unwrap();
        assert_eq!(enc.ids, vec![byte_ids[0x3F], byte_ids[0x21]]);
    }

    #[test]
    fn byte_fallback_round_trips_through_decode() {
        let (tok, _, _) = build_bpe_with_byte_fallback();
        // Round-trip a mix of vocab hits and byte-fallback fan-outs.
        // Whitespace-less inputs keep the pre-tokenizer's whitespace
        // fallback out of the equation (that fallback is documented
        // to be lossy on whitespace).
        for text in ["hi", "?", "😀", "hi😀", "😀hi", "?!", "hi?hi"] {
            let enc = tok.encode(text).unwrap();
            let round = tok.decode(&enc.ids).unwrap();
            assert_eq!(round, text, "round-trip failed on {text:?}");
        }
    }

    #[test]
    fn byte_fallback_decode_flushes_run_before_non_fallback_id() {
        let (tok, byte_ids, hi_id) = build_bpe_with_byte_fallback();
        // Manually construct an id sequence with a byte-fallback run
        // that terminates before a vocab id — exercises the "flush the
        // run when a non-fallback token arrives" branch in `decode`.
        let ids: Vec<TokenId> = "😀"
            .bytes()
            .map(|b| byte_ids[b as usize])
            .chain(core::iter::once(hi_id))
            .collect();
        let decoded = tok.decode(&ids).unwrap();
        assert_eq!(decoded, "😀hi");
    }

    #[test]
    fn byte_fallback_no_pre_tokenizer_preserves_newline_bytes() {
        // Regression: Phi-3-mini / Llama-2 / Mistral / Gemma ship BPE
        // + byte-fallback with NO explicit pre-tokenizer. Before the
        // Wave-15 landing that added the SentencePiece-family fast
        // path, `pre_tokenize` fell through to whitespace-splitting on
        // the `pattern = None` branch and silently dropped `\n` and
        // other non-space whitespace before byte-fallback could route
        // them to the reserved `<0xXX>` tokens. This test locks the
        // fix in place: with byte-fallback on and no pre-tokenizer,
        // every byte in the input must appear in the output — the
        // `\n` (`0x0A`) must land as its byte-fallback id, not be
        // silently eaten.
        let (tok, byte_ids, hi_id) = build_bpe_with_byte_fallback();
        // Input contains a newline between two `hi`s. Expect: hi_id,
        // <0x0A>, hi_id — the whitespace-split fallback would have
        // produced [hi_id, hi_id] (dropping the newline).
        let enc = tok.encode("hi\nhi").unwrap();
        assert_eq!(enc.ids, vec![hi_id, byte_ids[0x0A], hi_id]);
    }

    #[test]
    fn byte_fallback_no_pre_tokenizer_preserves_space_bytes() {
        // Same shape as the newline test but for the ASCII space
        // (`0x20`). Without the SentencePiece-family fast path, a
        // space would be dropped by the whitespace-split fallback in
        // `pre_tokenize`. With byte-fallback on, byte 0x20 must be
        // preserved as the reserved `<0x20>` id.
        let (tok, byte_ids, hi_id) = build_bpe_with_byte_fallback();
        let enc = tok.encode("hi hi").unwrap();
        assert_eq!(enc.ids, vec![hi_id, byte_ids[0x20], hi_id]);
    }

    #[test]
    fn byte_fallback_encoder_before_this_change_would_have_errored() {
        // Sanity check: without byte-fallback the same vocab would
        // surface `UnknownToken` on `?`. This locks the previous
        // behaviour in place for callers who deliberately construct a
        // tokenizer without byte-fallback.
        let mut v = BpeVocabulary::new();
        for &c in b"hi" {
            v.insert(u32::from(c), alloc::vec![c]).unwrap();
        }
        // No merges: keep every piece atomic so the vocab-only lookup
        // succeeds byte-for-byte on `"hi"` and fails cleanly on `"?"`.
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
        // "hi" encodes cleanly, "?" errors.
        assert!(tok.encode("hi").is_ok());
        let err = tok.encode("?").unwrap_err();
        assert!(matches!(err, TokenizerError::UnknownToken(_)));
    }

    #[test]
    fn nlogn_roundtrips_on_reference_corpus() {
        let tok = build_reference_tokenizer();
        for text in [
            "", "c", "cat", "cats", "dog", "dogs", "hello", "world", "catsdogs", "hello",
        ] {
            let enc = tok.encode(text).unwrap();
            let round = tok.decode(&enc.ids).unwrap();
            assert_eq!(round, text, "roundtrip fail on {text:?}");
        }
    }

    // ---------------------------------------------------------------
    // Batch / pair / truncation / padding.
    // ---------------------------------------------------------------

    fn build_reference_tokenizer_for_batch() -> BpeTokenizer {
        let (mut vocab, _) = byte_vocab_with_extras(&[b"ca", b"at"]);
        vocab.insert(258, b"cat".to_vec()).unwrap();
        let mut merges = BpeMergeTable::new();
        merges.insert(b"c".to_vec(), b"a".to_vec(), 0);
        merges.insert(b"ca".to_vec(), b"t".to_vec(), 1);
        BpeTokenizer::from_parts(merges, vocab)
    }

    #[test]
    fn encode_batch_returns_per_input_encoding_matching_encode() {
        use stringcheese_tokenizer::Tokenizer;
        let tok = build_reference_tokenizer_for_batch();
        let inputs = ["cat", "at", "c"];
        let refs: Vec<_> = inputs.iter().map(|i| tok.encode(i).unwrap()).collect();
        let batch = <BpeTokenizer as Tokenizer>::encode_batch(&tok, &inputs).unwrap();
        assert_eq!(batch.len(), inputs.len());
        for (batch_enc, ref_enc) in batch.iter().zip(&refs) {
            assert_eq!(batch_enc.ids, ref_enc.ids);
        }
    }

    #[test]
    fn count_batch_matches_encode_batch_lengths() {
        use stringcheese_tokenizer::Tokenizer;
        let tok = build_reference_tokenizer_for_batch();
        let inputs = ["cat", "at", "c"];
        let counts = <BpeTokenizer as Tokenizer>::count_batch(&tok, &inputs).unwrap();
        let batch = <BpeTokenizer as Tokenizer>::encode_batch(&tok, &inputs).unwrap();
        for (c, e) in counts.iter().zip(&batch) {
            assert_eq!(*c, e.ids.len());
        }
    }

    #[test]
    fn encode_pair_with_bert_template_produces_cls_a_sep_b_sep_shape() {
        use stringcheese_tokenizer::Tokenizer;
        // Build a small BERT-style pair template around the tokenizer.
        // CLS=300, SEP=301 (arbitrary ids; the template uses these
        // verbatim without going through the vocabulary).
        let mut specials = alloc::collections::BTreeMap::new();
        specials.insert(
            alloc::string::String::from("<cls>"),
            crate::post_processor::SpecialTokenInfo {
                ids: vec![300],
                tokens: vec![alloc::string::String::from("<cls>")],
            },
        );
        specials.insert(
            alloc::string::String::from("<sep>"),
            crate::post_processor::SpecialTokenInfo {
                ids: vec![301],
                tokens: vec![alloc::string::String::from("<sep>")],
            },
        );
        let tp = crate::post_processor::TemplateProcessing {
            single: vec![
                crate::post_processor::TemplatePiece::SpecialToken {
                    id: alloc::string::String::from("<cls>"),
                    type_id: 0,
                },
                crate::post_processor::TemplatePiece::Sequence {
                    id: alloc::string::String::from("A"),
                    type_id: 0,
                },
                crate::post_processor::TemplatePiece::SpecialToken {
                    id: alloc::string::String::from("<sep>"),
                    type_id: 0,
                },
            ],
            pair: vec![
                crate::post_processor::TemplatePiece::SpecialToken {
                    id: alloc::string::String::from("<cls>"),
                    type_id: 0,
                },
                crate::post_processor::TemplatePiece::Sequence {
                    id: alloc::string::String::from("A"),
                    type_id: 0,
                },
                crate::post_processor::TemplatePiece::SpecialToken {
                    id: alloc::string::String::from("<sep>"),
                    type_id: 0,
                },
                crate::post_processor::TemplatePiece::Sequence {
                    id: alloc::string::String::from("B"),
                    type_id: 1,
                },
                crate::post_processor::TemplatePiece::SpecialToken {
                    id: alloc::string::String::from("<sep>"),
                    type_id: 1,
                },
            ],
            special_tokens: specials,
        };
        let tok = build_reference_tokenizer_for_batch()
            .with_post_processor(crate::post_processor::PostProcessor::TemplateProcessing(tp));
        let out = <BpeTokenizer as Tokenizer>::encode_pair(&tok, "cat", "at").unwrap();
        // "cat" -> [258]; "at" -> [b'a', b't'] = [97, 116]
        assert_eq!(out.ids, vec![300, 258, 301, 97, 116, 301]);
        assert_eq!(out.type_ids, vec![0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn encode_with_truncation_config_trims_at_max_length() {
        use stringcheese_tokenizer::Tokenizer;
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab)
            .with_truncation(stringcheese_tokenizer::truncation::TruncationConfig::new(4));
        // 6-char input -> 6 tokens (no merges). Truncation caps at 4.
        let enc = <BpeTokenizer as Tokenizer>::encode(&tok, "abcdef").unwrap();
        assert_eq!(enc.ids.len(), 4);
        assert_eq!(enc.offsets.len(), 4);
    }

    #[test]
    fn encode_batch_with_padding_config_pads_to_batch_longest() {
        use stringcheese_tokenizer::Tokenizer;
        let (vocab, _) = byte_vocab_with_extras(&[]);
        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), vocab).with_padding(
            stringcheese_tokenizer::padding::PaddingConfig::<TokenId> {
                strategy: stringcheese_tokenizer::padding::PaddingStrategy::BatchLongest,
                pad_id: 0,
                pad_type_id: 0,
                direction: stringcheese_tokenizer::padding::PaddingDirection::Right,
            },
        );
        let batch = <BpeTokenizer as Tokenizer>::encode_batch(&tok, &["ab", "abcd", "a"]).unwrap();
        // Longest is 4; every encoding is padded to len 4.
        for enc in &batch {
            assert_eq!(enc.ids.len(), 4);
            assert_eq!(enc.attention_mask.len(), 4);
        }
        // Attention mask reflects real vs pad tokens.
        assert_eq!(batch[0].attention_mask, vec![true, true, false, false]);
        assert_eq!(batch[1].attention_mask, vec![true, true, true, true]);
        assert_eq!(batch[2].attention_mask, vec![true, false, false, false]);
    }

    // ------------------------------------------------------------------
    // Decoder-chain unit tests — one per new [`Decoder`] variant, driven
    // by hand-crafted `Vec<String>` inputs so the semantics are checked
    // in isolation of any tokenizer wiring. The full Llama-2 chain also
    // gets its own end-to-end test at the bottom.
    // ------------------------------------------------------------------

    fn s(x: &str) -> String {
        String::from(x)
    }

    #[test]
    fn decoder_is_chain_reports_variants_correctly() {
        assert!(!Decoder::Passthrough.is_chain());
        assert!(!Decoder::ByteLevel.is_chain());
        assert!(Decoder::Sequence(vec![]).is_chain());
        assert!(
            Decoder::Replace {
                pattern: s("_"),
                content: s(" "),
            }
            .is_chain()
        );
        assert!(Decoder::Fuse.is_chain());
        assert!(
            Decoder::Strip {
                content: ' ',
                start: 1,
                stop: 0,
            }
            .is_chain()
        );
        assert!(Decoder::ByteFallback.is_chain());
    }

    #[test]
    fn decoder_replace_substitutes_literal_pattern_per_token() {
        let dec = Decoder::Replace {
            pattern: s("_"),
            content: s(" "),
        };
        let out = dec.apply_chain(vec![s("_Hello"), s("_world"), s("no-op")]);
        assert_eq!(out, vec![s(" Hello"), s(" world"), s("no-op")]);
    }

    #[test]
    fn decoder_replace_handles_multiple_occurrences_in_one_token() {
        let dec = Decoder::Replace {
            pattern: s("aa"),
            content: s("b"),
        };
        assert_eq!(dec.apply_chain(vec![s("aaaa")]), vec![s("bb")]);
    }

    #[test]
    fn decoder_fuse_joins_tokens_without_separator() {
        let dec = Decoder::Fuse;
        let out = dec.apply_chain(vec![s(" Hello"), s(" world"), s("!")]);
        assert_eq!(out, vec![s(" Hello world!")]);
    }

    #[test]
    fn decoder_fuse_on_empty_input_yields_single_empty_entry() {
        assert_eq!(Decoder::Fuse.apply_chain(vec![]), vec![s("")]);
    }

    #[test]
    fn decoder_strip_removes_up_to_start_leading_content_chars() {
        let dec = Decoder::Strip {
            content: ' ',
            start: 1,
            stop: 0,
        };
        // Only the first space is stripped even when the token has more.
        assert_eq!(dec.apply_chain(vec![s("  hi")]), vec![s(" hi")]);
        // No effect when the leading char doesn't match.
        assert_eq!(dec.apply_chain(vec![s("hi")]), vec![s("hi")]);
        // Empty tokens are safe (no-op).
        assert_eq!(dec.apply_chain(vec![s("")]), vec![s("")]);
    }

    #[test]
    fn decoder_strip_removes_up_to_stop_trailing_content_chars() {
        let dec = Decoder::Strip {
            content: '.',
            start: 0,
            stop: 2,
        };
        assert_eq!(dec.apply_chain(vec![s("hi....")]), vec![s("hi..")]);
        assert_eq!(dec.apply_chain(vec![s("hi.")]), vec![s("hi")]);
    }

    #[test]
    fn decoder_byte_fallback_reassembles_multibyte_utf8_run() {
        // 😀 = U+1F600 = f0 9f 98 80
        let dec = Decoder::ByteFallback;
        let out = dec.apply_chain(vec![s("<0xF0>"), s("<0x9F>"), s("<0x98>"), s("<0x80>")]);
        assert_eq!(out, vec![s("😀")]);
    }

    #[test]
    fn decoder_byte_fallback_mixes_run_with_normal_tokens() {
        let dec = Decoder::ByteFallback;
        // "hi" + 😀 + "!"
        let out = dec.apply_chain(vec![
            s("hi"),
            s("<0xF0>"),
            s("<0x9F>"),
            s("<0x98>"),
            s("<0x80>"),
            s("!"),
        ]);
        assert_eq!(out, vec![s("hi"), s("😀"), s("!")]);
    }

    #[test]
    fn decoder_byte_fallback_invalid_utf8_run_produces_replacement_chars() {
        // A stray 0xFF byte is not valid UTF-8; HF's own ByteFallback
        // emits one U+FFFD per invalid byte.
        let dec = Decoder::ByteFallback;
        let out = dec.apply_chain(vec![s("<0xFF>")]);
        assert_eq!(out, vec![s("\u{FFFD}")]);
    }

    #[test]
    fn decoder_byte_fallback_flushes_run_at_end_of_input() {
        // A byte-fallback run that ends the input still flushes.
        let dec = Decoder::ByteFallback;
        let out = dec.apply_chain(vec![
            s("prefix"),
            s("<0xF0>"),
            s("<0x9F>"),
            s("<0x98>"),
            s("<0x80>"),
        ]);
        assert_eq!(out, vec![s("prefix"), s("😀")]);
    }

    #[test]
    fn decoder_sequence_composes_left_to_right() {
        // Replace ▁ → ' ', then Fuse.
        let dec = Decoder::Sequence(vec![
            Decoder::Replace {
                pattern: s("\u{2581}"),
                content: s(" "),
            },
            Decoder::Fuse,
        ]);
        let out = dec.apply_chain(vec![s("\u{2581}Hello"), s("\u{2581}world")]);
        assert_eq!(out, vec![s(" Hello world")]);
    }

    #[test]
    fn decoder_sequence_llama2_full_chain() {
        // Sequence[Replace{▁→ }, ByteFallback, Fuse, Strip{" ",1,0}]
        // Input tokens simulate: leading "▁" (Llama-2's Prepend adds
        // one), a byte-fallback run for 😀, and a further piece.
        let dec = Decoder::Sequence(vec![
            Decoder::Replace {
                pattern: s("\u{2581}"),
                content: s(" "),
            },
            Decoder::ByteFallback,
            Decoder::Fuse,
            Decoder::Strip {
                content: ' ',
                start: 1,
                stop: 0,
            },
        ]);
        let out = dec.apply_chain(vec![
            s("\u{2581}"),
            s("hi"),
            s("<0xF0>"),
            s("<0x9F>"),
            s("<0x98>"),
            s("<0x80>"),
        ]);
        assert_eq!(out, vec![s("hi😀")]);
    }

    #[test]
    fn decoder_sequence_empty_is_identity() {
        let dec = Decoder::Sequence(vec![]);
        assert_eq!(dec.apply_chain(vec![s("a"), s("b")]), vec![s("a"), s("b")]);
    }

    #[test]
    fn bpe_decode_routes_through_chain_when_configured() {
        // Build a small vocab with the SentencePiece byte-fallback surface
        // strings for the emoji 😀 (F0 9F 98 80), plus a plain "hi" token,
        // and the Llama-2-shape "▁hi" token.
        let mut v = BpeVocabulary::new();
        v.insert(0, b"hi".to_vec()).unwrap();
        // U+2581 "▁" is E2 96 81 — three bytes.
        v.insert(1, "\u{2581}hi".as_bytes().to_vec()).unwrap();
        v.insert(2, b"<0xF0>".to_vec()).unwrap();
        v.insert(3, b"<0x9F>".to_vec()).unwrap();
        v.insert(4, b"<0x98>".to_vec()).unwrap();
        v.insert(5, b"<0x80>".to_vec()).unwrap();

        let dec = Decoder::Sequence(vec![
            Decoder::Replace {
                pattern: String::from("\u{2581}"),
                content: String::from(" "),
            },
            Decoder::ByteFallback,
            Decoder::Fuse,
            Decoder::Strip {
                content: ' ',
                start: 1,
                stop: 0,
            },
        ]);

        let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v).with_decoder(dec);

        // Ids: "▁hi" + emoji byte run — a Llama-2-style encoding.
        let decoded = tok.decode(&[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(decoded, "hi😀");
    }
}

// ---------------------------------------------------------------------
// Property tests — round-trip and structural invariants over random
// small inputs. Gated off wasm by the target predicate on the
// dev-dep, and off `#[cfg(not(feature = "std"))]` because proptest
// itself requires `std`.
// ---------------------------------------------------------------------

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn arb_ascii_input() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<u8>().prop_map(|b| (b % 26) + b'a'), 0..24)
            .prop_map(|v| String::from_utf8(v).unwrap())
    }

    // Round-trip: decode(encode(text)) == text for byte-alphabet
    // vocabularies over inputs that contain no whitespace and no
    // special tokens.
    proptest! {
        #[test]
        fn round_trip_ascii(text in arb_ascii_input()) {
            let mut v = BpeVocabulary::new();
            v.ensure_byte_alphabet(0).unwrap();
            let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
            let enc = tok.encode(&text).unwrap();
            let s = tok.decode(&enc.ids).unwrap();
            prop_assert_eq!(s, text);
        }
    }

    proptest! {
        #[test]
        fn count_and_encode_agree(text in arb_ascii_input()) {
            let mut v = BpeVocabulary::new();
            v.ensure_byte_alphabet(0).unwrap();
            let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
            let n = tok.count(&text).unwrap();
            let enc = tok.encode(&text).unwrap();
            prop_assert_eq!(n, enc.len());
        }
    }

    proptest! {
        #[test]
        fn empty_encode_produces_zero_length(_ in 0u32..1) {
            let mut v = BpeVocabulary::new();
            v.ensure_byte_alphabet(0).unwrap();
            let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
            let enc = tok.encode("").unwrap();
            prop_assert_eq!(enc.len(), 0);
        }
    }

    // ------------------------------------------------------------------
    // Phase 2 acceptance oracle (randomized): the O(n log n) implementation
    // and the naive O(n²) implementation must agree on every input, for
    // every merge table. We drive both through the full encoding
    // pipeline (`encode_pieces_with`) so the check covers the merge
    // loop itself *and* every seam around it.
    // ------------------------------------------------------------------

    /// Small alphabet (3 letters). Deliberately narrow so short random
    /// inputs land many repeated adjacent pairs — that's where the two
    /// strategies would diverge if the tie-break shape was wrong.
    fn arb_small_ascii_input() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<u8>().prop_map(|b| (b % 3) + b'a'), 0..16)
            .prop_map(|v| String::from_utf8(v).unwrap())
    }

    /// Random small merge table over the {a,b,c} alphabet plus a few
    /// two-byte and three-byte pieces. Ranks are drawn from 0..20 with
    /// duplicates permitted (the resulting ties are exactly what
    /// exercises the tie-break rule).
    fn arb_merges_and_extras() -> impl Strategy<Value = (BpeMergeTable, Vec<Vec<u8>>)> {
        let byte_choices: &'static [&'static [u8]] = &[
            b"a", b"b", b"c", b"ab", b"ba", b"ac", b"ca", b"bc", b"cb", b"aa", b"bb", b"cc",
            b"abc", b"cab", b"aba", b"cbc",
        ];
        let single_merge = (
            0usize..byte_choices.len(),
            0usize..byte_choices.len(),
            0u32..20,
        )
            .prop_map(move |(li, ri, rank)| {
                let l = byte_choices[li].to_vec();
                let r = byte_choices[ri].to_vec();
                (l, r, rank)
            });
        prop::collection::vec(single_merge, 0..20).prop_map(|rules| {
            let mut m = BpeMergeTable::new();
            let mut extras: Vec<Vec<u8>> = Vec::new();
            for (l, r, rank) in rules {
                let mut merged = l.clone();
                merged.extend_from_slice(&r);
                extras.push(merged);
                m.insert(l, r, rank);
            }
            (m, extras)
        })
    }

    fn build_tokenizer(merges: BpeMergeTable, extras: &[Vec<u8>]) -> BpeTokenizer {
        let mut v = BpeVocabulary::new();
        let mut next = v.ensure_byte_alphabet(0).unwrap();
        for e in extras {
            if v.id(e).is_some() {
                continue;
            }
            v.insert(next, e.clone()).unwrap();
            next += 1;
        }
        BpeTokenizer::from_parts(merges, v)
    }

    fn encode_ids_with(
        tok: &BpeTokenizer,
        text: &str,
        f: fn(&BpeTokenizer, &[u8], &mut Vec<super::PieceRef>),
    ) -> Vec<TokenId> {
        tok.encode_pieces_with(text, f)
            .expect("encode should succeed against a byte-alphabet vocab")
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 512,
            .. ProptestConfig::default()
        })]

        /// Wave-6 acceptance: the O(n log n) encoder agrees with the
        /// naive O(n²) oracle for every random (input, merge-table)
        /// combination.
        #[test]
        fn nlogn_matches_naive_random(
            text in arb_small_ascii_input(),
            (merges, extras) in arb_merges_and_extras(),
        ) {
            let tok = build_tokenizer(merges, &extras);
            let naive = encode_ids_with(&tok, &text, BpeTokenizer::merge_loop_naive);
            let nlogn = encode_ids_with(&tok, &text, BpeTokenizer::merge_loop_nlogn);
            prop_assert_eq!(naive, nlogn);
        }

        /// Wave-14 acceptance: the flat tiktoken-style encoder agrees
        /// with the naive O(n²) oracle for every random (input,
        /// merge-table) combination. Guards against silent divergence
        /// between the production merge path and the reference shape.
        #[test]
        fn flat_matches_naive_random(
            text in arb_small_ascii_input(),
            (merges, extras) in arb_merges_and_extras(),
        ) {
            let tok = build_tokenizer(merges, &extras);
            let naive = encode_ids_with(&tok, &text, BpeTokenizer::merge_loop_naive);
            let flat = encode_ids_with(&tok, &text, BpeTokenizer::merge_loop_flat);
            prop_assert_eq!(naive, flat);
        }
    }

    proptest! {
        /// A random-input round-trip check that exercises the O(n log n)
        /// encoder on strings likely to trip the byte / char boundary
        /// (UTF-8 continuation bytes span the byte alphabet). Pure
        /// byte-alphabet vocab so every input decodes cleanly.
        ///
        /// Whitespace is excluded from the input generator: the default
        /// pre-tokenizer's whitespace-split fallback *discards*
        /// whitespace, and that lossy exception is already covered
        /// explicitly by `reference_prefix_words_multi_word_input_...`
        /// in the tests block above. Filtering it out here keeps the
        /// round-trip invariant sharp.
        #[test]
        fn round_trip_random_utf8(text in "\\PC{0,16}") {
            prop_assume!(!text.chars().any(char::is_whitespace));
            let mut v = BpeVocabulary::new();
            v.ensure_byte_alphabet(0).unwrap();
            let tok = BpeTokenizer::from_parts(BpeMergeTable::new(), v);
            let enc = tok.encode(&text).unwrap();
            let round = tok.decode(&enc.ids).unwrap();
            prop_assert_eq!(round, text);
        }
    }
}
