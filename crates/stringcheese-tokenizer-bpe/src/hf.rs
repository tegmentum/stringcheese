//! Hugging Face `tokenizer.json` parser (Phase 5, BPE-only slice).
//!
//! # What this module does
//!
//! Hugging Face's [`tokenizers`](https://huggingface.co/docs/tokenizers)
//! crate ships a JSON serialisation of a `Tokenizer` value:
//! normaliser, pre-tokenizer, model (BPE / WordPiece / Unigram /
//! WordLevel), post-processor, and decoder, each with its own type-tagged
//! config record. Every model on the Hub that ships tokenizer
//! configuration ships a `tokenizer.json` conforming to this spec —
//! Llama, Mistral, Qwen, DeepSeek, Phi, GPT-J, GPT-Neo, and so on.
//!
//! This module implements the **BPE slice** of that parser:
//!
//! * [`parse_tokenizer_json`] deserialises a `tokenizer.json` blob into
//!   an [`HfTokenizerConfig`] value that mirrors the on-disk shape.
//! * [`to_bpe_tokenizer`] converts the config into a runnable
//!   [`BpeTokenizer`], threading the vocabulary, merge table, added
//!   special tokens, and (if present) the `Split` / `Regex`
//!   pre-tokenizer pattern through.
//!
//! # What is supported
//!
//! * `model.type == "BPE"` — vocabulary and merges are extracted; the
//!   merge table's rank is the index of each entry in the `merges`
//!   array (lower index = earlier merge, matching HF's convention).
//! * `model.merges` in both shipped shapes: the newer pair form
//!   `[["a", "b"], ...]` and the older space-separated string form
//!   `["a b", ...]`.
//! * `pre_tokenizer.type == "Split"` with `pattern.Regex` — compiled
//!   through the crate's [`RegexPreTokenizer`].
//! * `pre_tokenizer.type == "Sequence"` — if its `pretokenizers` array
//!   contains exactly one supported `Split`/`Regex` entry (and no
//!   deferred siblings such as `ByteLevel`), that Split is taken.
//! * `added_tokens[*]` — every entry with `special == true` is
//!   registered as a special token on the produced [`BpeTokenizer`];
//!   entries with `special == false` are added to the base vocabulary
//!   so the caller-visible token id ↔ surface mapping is complete.
//!
//! # What is deferred
//!
//! All errors below carry the offending type name in their message so
//! callers can diagnose immediately.
//!
//! * `model.type ∈ {"WordPiece", "Unigram", "WordLevel"}` — separate
//!   algorithm crates, out of scope for this landing.
//! * `pre_tokenizer.type == "ByteLevel"` (and any `ByteLevel` component
//!   inside a `Sequence`) — byte-level BPE needs a whole additional
//!   input-remapping layer (space → `Ġ`, etc.) plus the matching
//!   decoder path; a separate landing.
//! * All other `pre_tokenizer` types (`Whitespace`,
//!   `WhitespaceSplit`, `Punctuation`, `Metaspace`, `CharDelimiterSplit`,
//!   `BertPreTokenizer`, `Digits`, `UnicodeScripts`, ...).
//! * `normalizer`, `post_processor`, `decoder` — the raw configs are
//!   preserved on [`HfTokenizerConfig`] so callers can inspect them,
//!   but they are **not applied** by [`to_bpe_tokenizer`]. Encodings
//!   from the produced tokenizer will therefore differ from upstream
//!   `tokenizers-rs` for any input whose normaliser/post-processor
//!   would have altered it (adding a BOS token, applying NFC, etc.).
//!   This is the documented lossy conversion.
//!
//! # Errors
//!
//! Two error types cover the two stages:
//!
//! * [`HfParseError`] — the JSON parse itself failed (malformed JSON
//!   or a top-level shape mismatch).
//! * [`HfConversionError`] — the JSON parsed but referenced an
//!   unsupported feature or contained an internally inconsistent
//!   value (duplicate token id, merge with the wrong number of
//!   sub-words, etc.).
//!
//! Both types implement [`std::error::Error`] and carry a message
//! naming the specific feature that was rejected.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::Deserialize;

use crate::bpe::{
    BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, TokenId, VocabularyBuilderError,
};
use crate::pre_tokenizer::{PreTokenizerCompileError, RegexPreTokenizer};

// ---------------------------------------------------------------------
// Config shapes — mirror the on-disk `tokenizer.json` layout.
// ---------------------------------------------------------------------

/// Deserialised top-level `tokenizer.json` value.
///
/// Field names match the on-disk JSON exactly. Any field not listed
/// here is silently ignored on parse (serde's default behaviour), so
/// forward-compatible additions to the spec do not break the parser;
/// [`to_bpe_tokenizer`] surfaces unsupported features at conversion
/// time via [`HfConversionError`], not at parse time.
///
/// Fields other than `model` are stored as [`serde_json::Value`] when
/// this module does not otherwise interpret them — that keeps the
/// parse tolerant while still letting callers reach into the raw
/// config for their own inspection (e.g. to check whether a
/// normaliser is present before calling [`to_bpe_tokenizer`]).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HfTokenizerConfig {
    /// The `"version"` string from the top of the file, if present.
    #[serde(default)]
    pub version: Option<String>,
    /// Optional `"truncation"` block, preserved verbatim.
    #[serde(default)]
    pub truncation: Option<serde_json::Value>,
    /// Optional `"padding"` block, preserved verbatim.
    #[serde(default)]
    pub padding: Option<serde_json::Value>,
    /// The `"added_tokens"` array, deserialised. Each entry becomes a
    /// registered token: `special == true` entries become BPE
    /// special tokens; the rest are added to the base vocabulary.
    #[serde(default)]
    pub added_tokens: Vec<HfAddedToken>,
    /// The `"normalizer"` config, preserved verbatim.
    ///
    /// **Not applied** by [`to_bpe_tokenizer`] — see the module-level
    /// docs for the caveats.
    #[serde(default)]
    pub normalizer: Option<serde_json::Value>,
    /// The `"pre_tokenizer"` config, deserialised into a
    /// [`HfPreTokenizer`]. Only `Split`/`Regex` and single-child
    /// `Sequence` wrappers thereof are honoured; other variants are
    /// accepted at parse time but rejected at conversion.
    #[serde(default)]
    pub pre_tokenizer: Option<HfPreTokenizer>,
    /// The `"post_processor"` config, preserved verbatim. Not applied.
    #[serde(default)]
    pub post_processor: Option<serde_json::Value>,
    /// The `"decoder"` config, preserved verbatim. Not applied — the
    /// crate's built-in decoder concatenates each id's byte string as
    /// stored in the vocabulary and reinterprets as UTF-8.
    #[serde(default)]
    pub decoder: Option<serde_json::Value>,
    /// The `"model"` block. Required by the spec; parse fails if it
    /// is missing.
    pub model: HfModel,
}

/// One entry in `added_tokens`.
///
/// Only the fields we act on are typed; the rest of the HF schema
/// (`normalized`, `single_word`, `lstrip`, `rstrip`) is captured under
/// [`Self::extra`] so callers can still inspect them.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HfAddedToken {
    /// The token id — the numeric value emitted by the tokenizer when
    /// [`Self::content`] appears in the input.
    pub id: TokenId,
    /// The surface string of the token (`"<|endoftext|>"`, `"<pad>"`).
    pub content: String,
    /// If `true`, the token bypasses the BPE merge loop and is emitted
    /// as [`Self::id`] whenever [`Self::content`] appears literally in
    /// the input. If missing from the JSON, defaults to `false`.
    #[serde(default)]
    pub special: bool,
    /// Anything else in the entry — kept for caller inspection.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// The `model` block of a `tokenizer.json` file.
///
/// Only [`HfModel::Bpe`] carries typed fields; the other variants keep
/// the raw JSON so callers can inspect what was rejected and — once
/// the corresponding algorithm crates land — pass the same config to
/// them. [`to_bpe_tokenizer`] returns
/// [`HfConversionError::UnsupportedModel`] for any variant other than
/// `Bpe`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfModel {
    /// A byte-pair-encoding model — the only variant this crate
    /// materialises today.
    #[serde(rename = "BPE")]
    Bpe(HfBpeModel),
    /// `WordPiece` model. Deferred — separate algorithm crate.
    #[serde(rename = "WordPiece")]
    WordPiece(serde_json::Value),
    /// `Unigram` model. Deferred — separate algorithm crate.
    #[serde(rename = "Unigram")]
    Unigram(serde_json::Value),
    /// Simple word-level model (a plain vocabulary lookup). Deferred.
    #[serde(rename = "WordLevel")]
    WordLevel(serde_json::Value),
}

/// The BPE-specific fields of a `model` block.
///
/// Every field except [`Self::vocab`] and [`Self::merges`] is optional
/// and captured only so callers can inspect what the source config
/// declared. [`to_bpe_tokenizer`] does not act on any of the optional
/// fields today — matching the "MVP-with-ignore" contract documented
/// at the module level. Setting `byte_fallback = true` in particular
/// requires a matching input-remapping layer we do not ship yet;
/// converting a config that turns it on produces a functional
/// tokenizer whose outputs may differ from upstream for any input that
/// would have triggered the fallback.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HfBpeModel {
    /// The token surface ↔ id map. Keys are the token strings (which,
    /// for byte-level tokenizers, are the *remapped* forms — the
    /// literal Unicode representation of the encoded bytes, e.g. `"Ġ"`
    /// for a leading space).
    pub vocab: BTreeMap<String, TokenId>,
    /// The merge table, in priority order (index 0 = highest
    /// priority, i.e. lowest rank).
    pub merges: Vec<HfMerge>,
    /// The optional dropout rate. Preserved but not applied.
    #[serde(default)]
    pub dropout: Option<f32>,
    /// The `unk_token` surface string. Preserved but not applied.
    #[serde(default)]
    pub unk_token: Option<String>,
    /// The `continuing_subword_prefix`. Preserved but not applied.
    #[serde(default)]
    pub continuing_subword_prefix: Option<String>,
    /// The `end_of_word_suffix`. Preserved but not applied.
    #[serde(default)]
    pub end_of_word_suffix: Option<String>,
    /// Whether adjacent unknowns should be fused. Preserved but not
    /// applied.
    #[serde(default)]
    pub fuse_unk: Option<bool>,
    /// Whether byte-fallback is enabled for out-of-vocab characters.
    /// Preserved but not applied.
    #[serde(default)]
    pub byte_fallback: Option<bool>,
    /// Whether the model should skip its merge loop when an input
    /// token is already in the vocabulary. Preserved but not applied.
    #[serde(default)]
    pub ignore_merges: Option<bool>,
}

/// One entry in `model.merges`.
///
/// The HF spec has shipped two encodings and this module accepts
/// both — see [`HfMerge::Pair`] and [`HfMerge::Joined`]. Both are
/// interpreted identically at conversion time: the two sub-words become
/// the left and right of a BPE merge whose rank is the entry's array
/// index.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HfMerge {
    /// The newer, unambiguous form: `["a", "b"]`.
    Pair([String; 2]),
    /// The older, space-joined form: `"a b"`. Split on the first
    /// space at conversion time. Rejected if the string does not
    /// contain exactly one space.
    Joined(String),
}

/// The `pre_tokenizer` block.
///
/// Types other than [`Self::Split`] and [`Self::Sequence`] parse
/// successfully but are rejected at [`to_bpe_tokenizer`] time — this
/// keeps the parser tolerant so callers can inspect a config before
/// deciding whether to convert.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfPreTokenizer {
    /// A regex- or literal-driven splitter. Only `pattern: Regex` is
    /// honoured at conversion time; `pattern: String` is deferred.
    Split(HfSplitPreTokenizer),
    /// A wrapper around a list of pre-tokenizers, applied left to
    /// right. Handled at conversion time as follows: the sequence
    /// must contain **exactly one** supported entry (a `Split` with a
    /// `Regex` pattern) and no deferred entries (in particular no
    /// `ByteLevel`); otherwise a specific error is returned.
    Sequence {
        /// The child pre-tokenizers. HF calls this field
        /// `"pretokenizers"` (note the missing hyphen).
        pretokenizers: Vec<HfPreTokenizer>,
    },
    /// Byte-level pre-tokenizer (GPT-2 / Llama-2 shape). Deferred.
    ByteLevel(serde_json::Value),
    /// Whitespace splitter. Deferred.
    Whitespace(serde_json::Value),
    /// Whitespace splitter that keeps runs together. Deferred.
    WhitespaceSplit(serde_json::Value),
    /// Punctuation splitter. Deferred.
    Punctuation(serde_json::Value),
    /// SentencePiece-style metaspace pre-tokenizer. Deferred.
    Metaspace(serde_json::Value),
    /// Single-character delimiter split. Deferred.
    CharDelimiterSplit(serde_json::Value),
    /// BERT-style pre-tokenizer. Deferred.
    BertPreTokenizer(serde_json::Value),
    /// Digit-run splitter. Deferred.
    Digits(serde_json::Value),
    /// Unicode-script splitter. Deferred.
    UnicodeScripts(serde_json::Value),
    /// Fixed-length splitter. Deferred.
    FixedLength(serde_json::Value),
}

/// A `Split` pre-tokenizer's configuration.
///
/// HF's schema also declares `behavior` (`"Isolated"`, `"Removed"`,
/// `"MergedWithPrevious"`, ...) and `invert`. The crate's
/// [`RegexPreTokenizer`] approximates `behavior == "Isolated"` and
/// `invert == false` — the shape used by every canonical
/// `tokenizer.json` we have seen for a Llama- or Mistral-family
/// checkpoint. Callers can still inspect the raw fields via
/// [`Self::behavior`] and [`Self::invert`].
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HfSplitPreTokenizer {
    /// The split pattern. Only [`HfPattern::Regex`] is honoured at
    /// conversion; [`HfPattern::String`] is deferred.
    pub pattern: HfPattern,
    /// HF's `behavior` field. Preserved but ignored.
    #[serde(default)]
    pub behavior: Option<String>,
    /// HF's `invert` field. Preserved but ignored.
    #[serde(default)]
    pub invert: Option<bool>,
}

/// The `pattern` sub-block of a `Split` pre-tokenizer.
///
/// Externally tagged in the on-disk JSON: `{"Regex": "…"}` or
/// `{"String": "…"}`.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub enum HfPattern {
    /// A regex pattern to match on. Compiled via `fancy-regex` at
    /// conversion time.
    Regex(String),
    /// A literal string separator. Deferred — the crate's own
    /// `PreTokenizerRegex::Literal` fallback covers this shape but
    /// wiring it up under HF semantics (which for `String` patterns
    /// depends on `behavior`) is not yet done.
    String(String),
}

// ---------------------------------------------------------------------
// Errors.
// ---------------------------------------------------------------------

/// Error returned by [`parse_tokenizer_json`] when the JSON blob is
/// malformed or does not match [`HfTokenizerConfig`]'s shape.
///
/// The wrapped [`serde_json::Error`] carries the exact position and
/// diagnostic; its exact format is not part of the stability contract.
#[derive(Debug)]
#[non_exhaustive]
pub enum HfParseError {
    /// The underlying JSON parser reported an error. The wrapped
    /// message is the parser's diagnostic (line, column, and cause);
    /// its exact format is not part of the stability contract.
    Json(String),
}

impl fmt::Display for HfParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(msg) => write!(f, "invalid tokenizer.json: {msg}"),
        }
    }
}

impl std::error::Error for HfParseError {}

impl From<serde_json::Error> for HfParseError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}

/// Error returned by [`to_bpe_tokenizer`] when the parsed config
/// references a feature this landing does not yet materialise, or when
/// the config is internally inconsistent.
///
/// Every variant's [`fmt::Display`] impl names the specific feature
/// or offending entry so callers can diagnose without inspecting the
/// enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HfConversionError {
    /// `model.type` is not `"BPE"`. Carries the specific type name
    /// (`"WordPiece"`, `"Unigram"`, `"WordLevel"`).
    UnsupportedModel {
        /// The `model.type` string from the source config.
        type_name: String,
    },
    /// The `pre_tokenizer` block used an unsupported type.
    UnsupportedPreTokenizer {
        /// The `pre_tokenizer.type` string from the source config
        /// (or a synthesised name for nested-Sequence rejections).
        type_name: String,
        /// A short human-readable reason (`"deferred to a later
        /// landing"`, `"nested sequence not supported"`, ...).
        reason: &'static str,
    },
    /// A `Split` pre-tokenizer used a non-regex pattern. Only
    /// `pattern: Regex` is honoured today.
    UnsupportedPattern {
        /// A short label for the pattern variant (`"String"`).
        variant: &'static str,
    },
    /// A `Sequence` pre-tokenizer contained more than one entry that
    /// could conceivably drive the split. Callers must strip the
    /// unsupported entries (typically `ByteLevel`) before retrying.
    AmbiguousSequencePreTokenizer {
        /// Number of children in the offending sequence.
        child_count: usize,
    },
    /// A merge entry could not be interpreted: the joined form did
    /// not contain exactly one space, or the pair form was empty.
    InvalidMerge {
        /// Zero-based index of the offending entry in
        /// `model.merges`.
        index: usize,
        /// A short human-readable reason.
        reason: &'static str,
    },
    /// The number of merges exceeds [`u32::MAX`], which the BPE merge
    /// table cannot represent (rank is a `u32`). Not reachable in
    /// practice — the largest shipped BPE tokenizer has ~200 000
    /// merges — but surfaced explicitly so the numeric cast is not a
    /// silent truncation.
    MergeRankOverflow {
        /// Zero-based index of the first entry beyond the limit.
        index: usize,
    },
    /// Building the [`BpeVocabulary`] from `model.vocab` (or from
    /// [`HfAddedToken`] entries) failed because the same id or byte
    /// string appears twice with different bindings.
    Vocabulary(VocabularyBuilderError),
    /// The `Split` pre-tokenizer's regex pattern failed to compile.
    Regex(PreTokenizerCompileError),
}

impl fmt::Display for HfConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedModel { type_name } => write!(
                f,
                "unsupported HF model type {type_name:?} \
                 (this crate materialises only \"BPE\"; \
                 WordPiece/Unigram/WordLevel are deferred to later landings)"
            ),
            Self::UnsupportedPreTokenizer { type_name, reason } => write!(
                f,
                "unsupported HF pre_tokenizer type {type_name:?}: {reason}"
            ),
            Self::UnsupportedPattern { variant } => write!(
                f,
                "unsupported HF Split pattern variant {variant:?} \
                 (only \"Regex\" is honoured today)"
            ),
            Self::AmbiguousSequencePreTokenizer { child_count } => write!(
                f,
                "unsupported HF pre_tokenizer Sequence with {child_count} children: \
                 exactly one supported Split(Regex) entry is required, \
                 and no deferred siblings such as ByteLevel"
            ),
            Self::InvalidMerge { index, reason } => {
                write!(f, "invalid merge entry at index {index}: {reason}")
            }
            Self::MergeRankOverflow { index } => write!(
                f,
                "merge table too large: entry {index} would overflow the u32 rank space"
            ),
            Self::Vocabulary(err) => write!(f, "invalid vocabulary: {err:?}"),
            Self::Regex(err) => write!(f, "invalid pre-tokenizer regex: {err}"),
        }
    }
}

impl std::error::Error for HfConversionError {}

impl From<VocabularyBuilderError> for HfConversionError {
    fn from(err: VocabularyBuilderError) -> Self {
        Self::Vocabulary(err)
    }
}

impl From<PreTokenizerCompileError> for HfConversionError {
    fn from(err: PreTokenizerCompileError) -> Self {
        Self::Regex(err)
    }
}

// ---------------------------------------------------------------------
// Public API.
// ---------------------------------------------------------------------

/// Parse a `tokenizer.json` blob into an [`HfTokenizerConfig`].
///
/// This is a pure JSON parse; no semantic validation of the config
/// happens here. In particular, references to unsupported model types
/// or pre-tokenizer variants parse successfully — surface them later
/// via [`to_bpe_tokenizer`].
///
/// # Errors
///
/// Returns [`HfParseError::Json`] wrapping the underlying
/// [`serde_json::Error`] if the input is not valid JSON or does not
/// match the top-level [`HfTokenizerConfig`] shape (missing `model`,
/// wrong type for a known field, etc.).
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::hf::parse_tokenizer_json;
///
/// // Minimal, valid config: BPE model with a byte-alphabet-adjacent
/// // vocabulary and one merge.
/// let json = r#"{
///     "version": "1.0",
///     "added_tokens": [],
///     "model": {
///         "type": "BPE",
///         "vocab": {"a": 0, "b": 1, "ab": 2},
///         "merges": [["a", "b"]]
///     }
/// }"#;
/// let config = parse_tokenizer_json(json).unwrap();
/// assert_eq!(config.version.as_deref(), Some("1.0"));
/// ```
pub fn parse_tokenizer_json(json: &str) -> Result<HfTokenizerConfig, HfParseError> {
    let config: HfTokenizerConfig = serde_json::from_str(json)?;
    Ok(config)
}

/// Materialise an [`HfTokenizerConfig`] as a runnable [`BpeTokenizer`].
///
/// See the module-level documentation for the full support matrix.
/// In short: the `model` must be `BPE`; the optional `pre_tokenizer`
/// must be `Split(Regex)` (or a `Sequence` around exactly one such
/// entry); `added_tokens` are folded into the vocabulary and, when
/// `special == true`, also into the special-token map.
///
/// # Errors
///
/// Returns [`HfConversionError`] with a variant naming the offending
/// feature — see that type's docs for the full list. Common causes:
/// a non-BPE `model.type`, a `ByteLevel` pre-tokenizer, a `Sequence`
/// pre-tokenizer with siblings that need dedicated support, or a
/// merge entry that is neither a two-element array nor a
/// single-space-joined string.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::Tokenizer;
/// use stringcheese_tokenizer_bpe::hf::{parse_tokenizer_json, to_bpe_tokenizer};
///
/// // A tiny BPE that merges 'a' + 'b' → "ab".
/// let json = r#"{
///     "added_tokens": [],
///     "model": {
///         "type": "BPE",
///         "vocab": {"a": 0, "b": 1, "ab": 2},
///         "merges": [["a", "b"]]
///     }
/// }"#;
/// let config = parse_tokenizer_json(json).unwrap();
/// let tok = to_bpe_tokenizer(&config).unwrap();
/// let enc = tok.encode("ab").unwrap();
/// assert_eq!(enc.ids, vec![2]);
/// ```
pub fn to_bpe_tokenizer(config: &HfTokenizerConfig) -> Result<BpeTokenizer, HfConversionError> {
    // Model — must be BPE.
    let bpe = match &config.model {
        HfModel::Bpe(bpe) => bpe,
        HfModel::WordPiece(_) => {
            return Err(HfConversionError::UnsupportedModel {
                type_name: "WordPiece".to_string(),
            });
        }
        HfModel::Unigram(_) => {
            return Err(HfConversionError::UnsupportedModel {
                type_name: "Unigram".to_string(),
            });
        }
        HfModel::WordLevel(_) => {
            return Err(HfConversionError::UnsupportedModel {
                type_name: "WordLevel".to_string(),
            });
        }
    };

    // Vocabulary — every entry in model.vocab, then every added_token.
    let mut vocab = BpeVocabulary::new();
    for (surface, &id) in &bpe.vocab {
        vocab.insert(id, surface.as_bytes().to_vec())?;
    }
    for at in &config.added_tokens {
        // Idempotent for entries that are already in model.vocab under
        // the same (id, bytes) mapping; propagates the vocabulary
        // builder's `DuplicateTokenId` / `DuplicateByteString` errors
        // otherwise. Most HF configs include added_tokens that are
        // *also* in model.vocab, so this is expected to be a no-op
        // most of the time.
        vocab.insert(at.id, at.content.as_bytes().to_vec())?;
    }

    // Merge table — index in the array = rank.
    let mut merges = BpeMergeTable::new();
    for (i, merge) in bpe.merges.iter().enumerate() {
        let rank =
            u32::try_from(i).map_err(|_| HfConversionError::MergeRankOverflow { index: i })?;
        let (left, right) = merge_pair(merge, i)?;
        merges.insert(left, right, rank);
    }

    // Pre-tokenizer — Split(Regex), or a single-child Sequence
    // around one, or nothing at all.
    let pre_tokenizer_regex = match &config.pre_tokenizer {
        None => None,
        Some(pt) => extract_split_regex(pt)?,
    };

    // Special tokens — added_tokens with special == true.
    let mut specials: BTreeMap<String, TokenId> = BTreeMap::new();
    for at in &config.added_tokens {
        if at.special {
            specials.insert(at.content.clone(), at.id);
        }
    }

    // Assemble.
    let mut tok = BpeTokenizer::from_parts(merges, vocab);
    if !specials.is_empty() {
        tok = tok.with_special_tokens(specials);
    }
    if let Some(pattern) = pre_tokenizer_regex {
        let compiled = RegexPreTokenizer::new(pattern)?;
        tok = tok.with_pre_tokenizer(PreTokenizerRegex::regex(compiled));
    }
    Ok(tok)
}

// ---------------------------------------------------------------------
// Conversion helpers.
// ---------------------------------------------------------------------

/// Interpret one merge entry as a `(left, right)` byte pair.
fn merge_pair(merge: &HfMerge, index: usize) -> Result<(Vec<u8>, Vec<u8>), HfConversionError> {
    match merge {
        HfMerge::Pair([left, right]) => {
            if left.is_empty() || right.is_empty() {
                return Err(HfConversionError::InvalidMerge {
                    index,
                    reason: "merge entry has an empty sub-word",
                });
            }
            Ok((left.as_bytes().to_vec(), right.as_bytes().to_vec()))
        }
        HfMerge::Joined(joined) => {
            // Exactly one space; anything else is malformed.
            let mut it = joined.splitn(2, ' ');
            let Some(left) = it.next() else {
                return Err(HfConversionError::InvalidMerge {
                    index,
                    reason: "merge entry has no space separator",
                });
            };
            let Some(right) = it.next() else {
                return Err(HfConversionError::InvalidMerge {
                    index,
                    reason: "merge entry has no space separator",
                });
            };
            if left.is_empty() || right.is_empty() {
                return Err(HfConversionError::InvalidMerge {
                    index,
                    reason: "merge entry has an empty sub-word",
                });
            }
            // A merge entry with more than one space (e.g. "a b c") is
            // ill-defined; reject rather than silently taking the first
            // pair.
            if right.contains(' ') {
                return Err(HfConversionError::InvalidMerge {
                    index,
                    reason: "joined merge entry contains more than one space",
                });
            }
            Ok((left.as_bytes().to_vec(), right.as_bytes().to_vec()))
        }
    }
}

/// Walk a [`HfPreTokenizer`] value and produce the regex pattern that
/// the underlying [`RegexPreTokenizer`] should be built from.
///
/// Returns `Ok(None)` when the pre-tokenizer is trivially empty (an
/// empty `Sequence`), `Ok(Some(pattern))` when a single supported
/// `Split(Regex)` is found, and an [`HfConversionError`] otherwise.
fn extract_split_regex(pt: &HfPreTokenizer) -> Result<Option<String>, HfConversionError> {
    match pt {
        HfPreTokenizer::Split(split) => match &split.pattern {
            HfPattern::Regex(pattern) => Ok(Some(pattern.clone())),
            HfPattern::String(_) => {
                Err(HfConversionError::UnsupportedPattern { variant: "String" })
            }
        },
        HfPreTokenizer::Sequence { pretokenizers } => {
            if pretokenizers.is_empty() {
                return Ok(None);
            }
            // If any child is a deferred type, reject that specifically
            // — it produces the most actionable message. Otherwise, if
            // exactly one child is a Split(Regex), take it.
            for child in pretokenizers {
                if let Some(err) = deferred_pre_tokenizer_reason(child) {
                    return Err(err);
                }
            }
            let mut chosen: Option<String> = None;
            for child in pretokenizers {
                let inner = extract_split_regex(child)?;
                if let Some(pat) = inner {
                    if chosen.is_some() {
                        return Err(HfConversionError::AmbiguousSequencePreTokenizer {
                            child_count: pretokenizers.len(),
                        });
                    }
                    chosen = Some(pat);
                }
            }
            Ok(chosen)
        }
        // Deferred variants: return a targeted error.
        other => {
            if let Some(err) = deferred_pre_tokenizer_reason(other) {
                Err(err)
            } else {
                // Unreachable in practice — every non-Split /
                // non-Sequence variant of `HfPreTokenizer` is covered
                // by `deferred_pre_tokenizer_reason`. Guard it anyway.
                Err(HfConversionError::UnsupportedPreTokenizer {
                    type_name: "unknown".to_string(),
                    reason: "unhandled pre_tokenizer variant",
                })
            }
        }
    }
}

/// Map an [`HfPreTokenizer`] to a specific "deferred" error if it is a
/// known-unsupported variant. Returns `None` for `Split` and
/// `Sequence` (both of which are handled inline by
/// [`extract_split_regex`]).
fn deferred_pre_tokenizer_reason(pt: &HfPreTokenizer) -> Option<HfConversionError> {
    let (type_name, reason) = match pt {
        HfPreTokenizer::Split(_) | HfPreTokenizer::Sequence { .. } => return None,
        HfPreTokenizer::ByteLevel(_) => (
            "ByteLevel",
            "byte-level pre-tokenization is deferred; \
             it requires the matching input-remapping and decoder layers",
        ),
        HfPreTokenizer::Whitespace(_) => ("Whitespace", "deferred to a later landing"),
        HfPreTokenizer::WhitespaceSplit(_) => ("WhitespaceSplit", "deferred to a later landing"),
        HfPreTokenizer::Punctuation(_) => ("Punctuation", "deferred to a later landing"),
        HfPreTokenizer::Metaspace(_) => ("Metaspace", "deferred to a later landing"),
        HfPreTokenizer::CharDelimiterSplit(_) => {
            ("CharDelimiterSplit", "deferred to a later landing")
        }
        HfPreTokenizer::BertPreTokenizer(_) => ("BertPreTokenizer", "deferred to a later landing"),
        HfPreTokenizer::Digits(_) => ("Digits", "deferred to a later landing"),
        HfPreTokenizer::UnicodeScripts(_) => ("UnicodeScripts", "deferred to a later landing"),
        HfPreTokenizer::FixedLength(_) => ("FixedLength", "deferred to a later landing"),
    };
    Some(HfConversionError::UnsupportedPreTokenizer {
        type_name: type_name.to_string(),
        reason,
    })
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bpe::TokenId;
    use alloc::vec;
    use stringcheese_tokenizer::Tokenizer;

    /// A minimal but shape-representative config: BPE model with a
    /// small vocab and one merge, no pre-tokenizer, no `added_tokens`.
    const MINIMAL_JSON: &str = r#"{
        "version": "1.0",
        "added_tokens": [],
        "model": {
            "type": "BPE",
            "vocab": {"a": 0, "b": 1, "c": 2, "ab": 3},
            "merges": [["a", "b"]]
        }
    }"#;

    #[test]
    fn parse_minimal_config() {
        let config = parse_tokenizer_json(MINIMAL_JSON).unwrap();
        assert_eq!(config.version.as_deref(), Some("1.0"));
        assert!(config.pre_tokenizer.is_none());
        assert!(config.normalizer.is_none());
        match &config.model {
            HfModel::Bpe(bpe) => {
                assert_eq!(bpe.vocab.len(), 4);
                assert_eq!(bpe.merges.len(), 1);
            }
            _ => panic!("expected BPE model"),
        }
    }

    #[test]
    fn convert_minimal_config_and_encode() {
        let config = parse_tokenizer_json(MINIMAL_JSON).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // "ab" is in the vocab and there is a merge ("a","b"): expect
        // one token with id 3.
        let enc = tok.encode("ab").unwrap();
        assert_eq!(enc.ids, vec![3]);
        assert_eq!(tok.decode(&enc.ids).unwrap(), "ab");
        // "c" is in the vocab, no merges apply: expect one token id 2.
        let enc = tok.encode("c").unwrap();
        assert_eq!(enc.ids, vec![2]);
    }

    #[test]
    fn merges_accepts_pair_form() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"x": 0, "y": 1, "xy": 2},
                "merges": [["x", "y"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("xy").unwrap().ids, vec![2]);
    }

    #[test]
    fn merges_accepts_space_joined_form() {
        // The older HF format used space-joined strings for merges.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"x": 0, "y": 1, "xy": 2},
                "merges": ["x y"]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("xy").unwrap().ids, vec![2]);
    }

    #[test]
    fn merges_reject_joined_with_extra_space() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"x": 0, "y": 1, "z": 2},
                "merges": ["x y z"]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(
            matches!(err, HfConversionError::InvalidMerge { index: 0, .. }),
            "unexpected error {err:?}"
        );
    }

    #[test]
    fn merges_reject_empty_subword_in_pair() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0},
                "merges": [["", "a"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::InvalidMerge { index: 0, .. }
        ));
    }

    #[test]
    fn merge_ranks_are_priority_order() {
        // If both ("a","b") and ("b","c") could apply, the earlier
        // merge in the array (lower rank) wins. Verified through the
        // BPE encoding, which for "abc" with merges [("a","b"),
        // ("b","c")] should produce ["ab", "c"].
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, "c": 2, "ab": 3, "bc": 4},
                "merges": [["a", "b"], ["b", "c"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let ids = tok.encode("abc").unwrap().ids;
        // "ab" has id 3, "c" has id 2.
        assert_eq!(ids, vec![3, 2]);
    }

    #[test]
    fn added_special_tokens_bypass_bpe() {
        let json = r#"{
            "added_tokens": [
                {"id": 50256, "content": "<|endoftext|>", "special": true}
            ],
            "model": {
                "type": "BPE",
                "vocab": {"h": 0, "i": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let enc = tok.encode("hi<|endoftext|>").unwrap();
        assert_eq!(enc.ids, vec![0, 1, 50256]);
        assert_eq!(enc.special_mask, vec![false, false, true]);
        // Decode round-trips through the special-tokens surface map.
        assert_eq!(tok.decode(&enc.ids).unwrap(), "hi<|endoftext|>");
    }

    #[test]
    fn added_non_special_tokens_land_in_vocab_only() {
        // A non-special added token is expected to already be in
        // model.vocab; the parser inserts it either way and does not
        // mark it as a special. `<pad>` in this config gets id 42 but
        // is *not* matched literally in input — a plain "<" character
        // would tokenize to its byte value.
        let json = r#"{
            "added_tokens": [
                {"id": 42, "content": "<pad>", "special": false}
            ],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // No specials registered.
        assert!(tok.special_tokens().is_empty());
        // Vocabulary contains the added token by its bytes.
        assert_eq!(tok.vocab().id(b"<pad>"), Some(42));
    }

    #[test]
    fn split_pre_tokenizer_with_regex_pattern_takes_effect() {
        // Split on commas. The regex captures every non-comma run,
        // so "a,b" splits into ["a", "b"] and the comma byte is dropped.
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "Split",
                "pattern": {"Regex": "[^,]+"},
                "behavior": "Isolated",
                "invert": false
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let ids = tok.encode("a,b").unwrap().ids;
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn sequence_pre_tokenizer_with_single_split_is_accepted() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "Sequence",
                "pretokenizers": [
                    {
                        "type": "Split",
                        "pattern": {"Regex": "[^,]+"},
                        "behavior": "Isolated",
                        "invert": false
                    }
                ]
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("a,b").unwrap().ids, vec![0, 1]);
    }

    #[test]
    fn sequence_with_bytelevel_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "Sequence",
                "pretokenizers": [
                    {
                        "type": "Split",
                        "pattern": {"Regex": "[^,]+"},
                        "behavior": "Isolated",
                        "invert": false
                    },
                    {
                        "type": "ByteLevel",
                        "add_prefix_space": false,
                        "trim_offsets": true,
                        "use_regex": true
                    }
                ]
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedPreTokenizer { type_name, .. } => {
                assert_eq!(type_name, "ByteLevel");
            }
            other => panic!("expected UnsupportedPreTokenizer(ByteLevel), got {other:?}"),
        }
    }

    #[test]
    fn bytelevel_pre_tokenizer_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "ByteLevel",
                "add_prefix_space": false,
                "trim_offsets": true,
                "use_regex": true
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ByteLevel"), "message: {msg}");
        assert!(msg.contains("deferred"), "message: {msg}");
    }

    #[test]
    fn split_with_string_pattern_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "Split",
                "pattern": {"String": ","},
                "behavior": "Isolated",
                "invert": false
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::UnsupportedPattern { variant: "String" }
        ));
    }

    #[test]
    fn wordpiece_model_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordPiece",
                "vocab": {"a": 0},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModel { type_name } => {
                assert_eq!(type_name, "WordPiece");
            }
            other => panic!("expected UnsupportedModel(WordPiece), got {other:?}"),
        }
    }

    #[test]
    fn unigram_model_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "Unigram",
                "vocab": [["a", 0.0], ["b", -1.0]],
                "unk_id": 0
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModel { type_name } => {
                assert_eq!(type_name, "Unigram");
            }
            other => panic!("expected UnsupportedModel(Unigram), got {other:?}"),
        }
    }

    #[test]
    fn wordlevel_model_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordLevel",
                "vocab": {"a": 0},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModel { type_name } => {
                assert_eq!(type_name, "WordLevel");
            }
            other => panic!("expected UnsupportedModel(WordLevel), got {other:?}"),
        }
    }

    #[test]
    fn parse_error_on_malformed_json() {
        let err = parse_tokenizer_json("{not json").unwrap_err();
        assert!(matches!(err, HfParseError::Json(_)));
        assert!(err.to_string().contains("invalid tokenizer.json"));
    }

    #[test]
    fn parse_error_on_missing_model() {
        // `model` is required — omit it.
        let err = parse_tokenizer_json(r#"{"added_tokens": []}"#).unwrap_err();
        assert!(matches!(err, HfParseError::Json(_)));
    }

    #[test]
    fn normalizer_post_processor_decoder_are_preserved_but_ignored() {
        // These fields exist in real tokenizer.json blobs; we
        // preserve them for caller inspection but do not apply them,
        // and the conversion still succeeds.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {"type": "NFC"},
            "post_processor": {"type": "TemplateProcessing", "single": [], "pair": []},
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(config.normalizer.is_some());
        assert!(config.post_processor.is_some());
        assert!(config.decoder.is_some());
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("ab").unwrap().ids, vec![0, 1]);
    }

    // ---------------------------------------------------------------------
    // Representative GPT-2-shape blob.
    //
    // Real GPT-2 tokenizer.json uses a ByteLevel pre-tokenizer + a
    // ByteLevel decoder, both of which are deferred by this landing.
    // The blob below reproduces the *shape* — top-level layout,
    // added_tokens with `<|endoftext|>`, space-joined merges, an
    // unk_token on the BPE model — but omits the ByteLevel pieces so
    // the conversion succeeds. Encodings are of course not
    // bit-identical to real GPT-2; that requires the ByteLevel
    // landing.
    // ---------------------------------------------------------------------

    fn gpt2_style_config() -> HfTokenizerConfig {
        let json = r#"{
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [
                {
                    "id": 50256,
                    "content": "<|endoftext|>",
                    "single_word": false,
                    "lstrip": false,
                    "rstrip": false,
                    "normalized": false,
                    "special": true
                }
            ],
            "normalizer": null,
            "pre_tokenizer": null,
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "BPE",
                "dropout": null,
                "unk_token": null,
                "continuing_subword_prefix": null,
                "end_of_word_suffix": null,
                "fuse_unk": false,
                "byte_fallback": false,
                "vocab": {
                    "h": 0, "e": 1, "l": 2, "o": 3, "w": 4, "r": 5, "d": 6,
                    "he": 7, "ll": 8, "lo": 9, "hell": 10, "hello": 11,
                    "wo": 12, "or": 13, "wor": 14, "ld": 15, "world": 16
                },
                "merges": [
                    "h e", "l l", "he ll", "l o", "hell o",
                    "w o", "o r", "wo r", "l d", "wor ld"
                ]
            }
        }"#;
        parse_tokenizer_json(json).unwrap()
    }

    #[test]
    fn gpt2_style_config_parses_and_converts() {
        let config = gpt2_style_config();
        // Top-level shape.
        assert_eq!(config.version.as_deref(), Some("1.0"));
        assert_eq!(config.added_tokens.len(), 1);
        assert_eq!(config.added_tokens[0].content, "<|endoftext|>");
        assert!(config.added_tokens[0].special);
        // Merges parsed in space-joined form.
        match &config.model {
            HfModel::Bpe(bpe) => {
                assert_eq!(bpe.merges.len(), 10);
                assert!(matches!(&bpe.merges[0], HfMerge::Joined(s) if s == "h e"));
            }
            _ => panic!("expected BPE"),
        }
        // Round-trip through the produced tokenizer on a couple of
        // words the merge table can reach.
        let tok = to_bpe_tokenizer(&config).unwrap();
        // "hello" merges: h+e → he, l+l → ll, he+ll → hell, hell+o → hello (id 11).
        let hello = tok.encode("hello").unwrap();
        assert_eq!(hello.ids, vec![11]);
        assert_eq!(tok.decode(&hello.ids).unwrap(), "hello");
        // "world" merges reach id 16.
        let world = tok.encode("world").unwrap();
        assert_eq!(world.ids, vec![16]);
        // <|endoftext|> is honoured as a special.
        let mixed = tok.encode("hello<|endoftext|>world").unwrap();
        assert_eq!(mixed.ids, vec![11, 50256, 16]);
        assert_eq!(mixed.special_mask, vec![false, true, false]);
    }

    // ---------------------------------------------------------------------
    // Representative Llama-3-shape blob.
    //
    // Real Llama-3 tokenizer.json ships a Split pre-tokenizer whose
    // regex is a close relative of tiktoken's canonical pattern
    // (Llama-3 uses tiktoken under the hood). The blob below carries
    // that pre-tokenizer plus a small BPE vocab in the newer pair-form
    // merges shape and one special token in the `<|begin_of_text|>`
    // slot Llama-3 reserves.
    // ---------------------------------------------------------------------

    fn llama_style_config() -> HfTokenizerConfig {
        // A small piece of the Llama-3 canonical pre-tokenizer regex,
        // narrowed to keep the test focused on the parse/conversion
        // path rather than on regex behaviour (which is covered in
        // `pre_tokenizer.rs`).
        let json = r#"{
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [
                {
                    "id": 128000,
                    "content": "<|begin_of_text|>",
                    "single_word": false,
                    "lstrip": false,
                    "rstrip": false,
                    "normalized": false,
                    "special": true
                }
            ],
            "normalizer": null,
            "pre_tokenizer": {
                "type": "Split",
                "pattern": {
                    "Regex": "\\p{L}+|\\p{N}+|[^\\s\\p{L}\\p{N}]+|\\s+"
                },
                "behavior": "Isolated",
                "invert": false
            },
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "BPE",
                "dropout": null,
                "unk_token": null,
                "continuing_subword_prefix": null,
                "end_of_word_suffix": null,
                "fuse_unk": false,
                "byte_fallback": false,
                "ignore_merges": true,
                "vocab": {
                    "h": 0, "i": 1, "e": 2, "l": 3, "o": 4,
                    "hi": 5, "he": 6, "ll": 7, "hell": 8, "hello": 9
                },
                "merges": [
                    ["h", "i"], ["h", "e"], ["l", "l"], ["he", "ll"], ["hell", "o"]
                ]
            }
        }"#;
        parse_tokenizer_json(json).unwrap()
    }

    #[test]
    fn llama_style_config_parses_and_converts() {
        let config = llama_style_config();
        // Special token registered.
        assert_eq!(config.added_tokens.len(), 1);
        let at = &config.added_tokens[0];
        assert_eq!(at.id, 128_000 as TokenId);
        assert_eq!(at.content, "<|begin_of_text|>");
        assert!(at.special);
        // Pair-form merges.
        match &config.model {
            HfModel::Bpe(bpe) => {
                assert_eq!(bpe.merges.len(), 5);
                assert!(matches!(&bpe.merges[0], HfMerge::Pair([l, r]) if l == "h" && r == "i"));
                assert_eq!(bpe.ignore_merges, Some(true));
            }
            _ => panic!("expected BPE"),
        }
        // Split(Regex) pre-tokenizer preserved.
        match config.pre_tokenizer.as_ref().unwrap() {
            HfPreTokenizer::Split(split) => match &split.pattern {
                HfPattern::Regex(pat) => {
                    assert!(pat.contains("\\p{L}+"));
                }
                other => panic!("expected Regex pattern, got {other:?}"),
            },
            other => panic!("expected Split pre-tokenizer, got {other:?}"),
        }
        // Convert and encode. "hello" reaches id 9 via h+e → he,
        // l+l → ll, he+ll → hell, hell+o → hello.
        let tok = to_bpe_tokenizer(&config).unwrap();
        let hello = tok.encode("hello").unwrap();
        assert_eq!(hello.ids, vec![9]);
        // "hi" reaches id 5.
        let hi = tok.encode("hi").unwrap();
        assert_eq!(hi.ids, vec![5]);
        // The pre-tokenizer splits on whitespace so "hi hello"
        // becomes [hi, space-chunk, hello]. Because the pre-tokenizer's
        // regex is `\p{L}+|\p{N}+|[^\s\p{L}\p{N}]+|\s+`, the space
        // between the words matches `\s+` and is emitted as its own
        // chunk — but the space byte is *not* in the vocab, so
        // encoding fails. Verify the failure surface rather than
        // pretending the toy vocab covers ASCII space.
        let err = tok.encode("hi hello").unwrap_err();
        // TokenizerError::UnknownToken carries a label of the missing
        // bytes; we only care that we surfaced a specific error, not
        // its exact string.
        let msg = format!("{err:?}");
        assert!(msg.contains("UnknownToken"), "unexpected err {msg}");
        // Round-trip on inputs the vocab covers.
        assert_eq!(tok.decode(&hello.ids).unwrap(), "hello");
        // <|begin_of_text|> as special.
        let mixed = tok.encode("<|begin_of_text|>hi").unwrap();
        assert_eq!(mixed.ids, vec![128_000, 5]);
        assert_eq!(mixed.special_mask, vec![true, false]);
    }

    #[test]
    fn added_tokens_extra_fields_are_captured() {
        let json = r#"{
            "added_tokens": [
                {
                    "id": 7,
                    "content": "<x>",
                    "special": true,
                    "single_word": true,
                    "lstrip": false,
                    "rstrip": false,
                    "normalized": false
                }
            ],
            "model": {"type": "BPE", "vocab": {}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let at = &config.added_tokens[0];
        assert!(at.extra.contains_key("single_word"));
        assert!(at.extra.contains_key("lstrip"));
        assert_eq!(
            at.extra.get("single_word"),
            Some(&serde_json::Value::Bool(true))
        );
    }
}
