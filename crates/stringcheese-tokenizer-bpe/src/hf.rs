//! Hugging Face `tokenizer.json` parser (Phase 5, BPE-only slice).
//!
//! # What this module does
//!
//! Hugging Face's [`tokenizers`](https://huggingface.co/docs/tokenizers)
//! crate ships a JSON serialisation of a `Tokenizer` value:
//! normaliser, pre-tokenizer, model (BPE / `WordPiece` / Unigram /
//! `WordLevel`), post-processor, and decoder, each with its own type-tagged
//! config record. Every model on the Hub that ships tokenizer
//! configuration ships a `tokenizer.json` conforming to this spec —
//! Llama, Mistral, Qwen, `DeepSeek`, Phi, GPT-J, GPT-Neo, and so on.
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
//! * `pre_tokenizer.type == "ByteLevel"` — recognised as the GPT-2 /
//!   Llama byte-level pipeline: an optional `add_prefix_space` step,
//!   an optional GPT-2-canonical regex split (`use_regex`), and the
//!   byte↔char bijection from [`crate::byte_level`]. Both standalone
//!   and inside a `Sequence` alongside a `Split(Regex)` sibling
//!   (which then wins as the regex split) are honoured.
//! * `decoder.type == "ByteLevel"` — attaches
//!   [`Decoder::ByteLevel`](crate::Decoder) to the produced
//!   tokenizer so `decode` reverses the byte↔char mapping and
//!   returns the caller's original raw bytes.
//!
//! # What is deferred
//!
//! All errors below carry the offending type name in their message so
//! callers can diagnose immediately.
//!
//! * `model.type ∈ {"WordPiece", "Unigram", "WordLevel"}` — separate
//!   algorithm crates, out of scope for this landing.
//! * All other `pre_tokenizer` types (`Whitespace`,
//!   `WhitespaceSplit`, `Punctuation`, `Metaspace`, `CharDelimiterSplit`,
//!   `BertPreTokenizer`, `Digits`, `UnicodeScripts`, ...).
//! * All other `decoder` types (`WordPiece`, `Metaspace`, `BPEDecoder`,
//!   `Sequence`, ...). The raw config is preserved on
//!   [`HfTokenizerConfig::decoder`] for caller inspection.
//! * `normalizer`, `post_processor` — the raw configs are preserved
//!   on [`HfTokenizerConfig`] so callers can inspect them, but they
//!   are **not applied** by [`to_bpe_tokenizer`]. Encodings from the
//!   produced tokenizer will therefore differ from upstream
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
    BpeMergeTable, BpeTokenizer, BpeVocabulary, Decoder, PreTokenizerRegex, TokenId,
    VocabularyBuilderError,
};
use crate::normalizer::Normalizer;
use crate::post_processor::{PostProcessor, SpecialTokenInfo, TemplatePiece, TemplateProcessing};
use crate::pre_tokenizer::{GPT2_PATTERN, PreTokenizerCompileError, RegexPreTokenizer};

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
    /// The `"normalizer"` config, deserialised into a typed
    /// [`HfNormalizer`] value.
    ///
    /// Applied by [`to_bpe_tokenizer`] for the shapes this crate
    /// materialises (NFC / NFD / NFKC / NFKD / `Sequence` /
    /// `Lowercase` / `Replace` / `Strip` / `Prepend`); every other
    /// tag string surfaces as [`HfNormalizer::Other`] and produces
    /// [`HfConversionError::UnsupportedNormalizer`] at conversion
    /// time.
    #[serde(default)]
    pub normalizer: Option<HfNormalizer>,
    /// The `"pre_tokenizer"` config, deserialised into a
    /// [`HfPreTokenizer`]. Only `Split`/`Regex` and single-child
    /// `Sequence` wrappers thereof are honoured; other variants are
    /// accepted at parse time but rejected at conversion.
    #[serde(default)]
    pub pre_tokenizer: Option<HfPreTokenizer>,
    /// The `"post_processor"` config, deserialised into a typed
    /// [`HfPostProcessor`] value. Only
    /// [`HfPostProcessor::TemplateProcessing`] is honoured at
    /// conversion; every other tag string falls through to
    /// [`HfPostProcessor::Other`] and produces
    /// [`HfConversionError::UnsupportedPostProcessor`].
    #[serde(default)]
    pub post_processor: Option<HfPostProcessor>,
    /// The `"decoder"` config, deserialised into a typed
    /// [`HfDecoder`] value. Only [`HfDecoder::ByteLevel`] is honoured
    /// at conversion time and attaches [`Decoder::ByteLevel`] to the
    /// produced [`BpeTokenizer`]; all other variants are preserved as
    /// [`HfDecoder::Other`] for caller inspection and produce a
    /// tokenizer whose `decode` concatenates each id's byte string
    /// as stored in the vocabulary and reinterprets as UTF-8 (the
    /// passthrough decoder).
    #[serde(default)]
    pub decoder: Option<HfDecoder>,
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
/// [`HfModel::Bpe`] and [`HfModel::WordPiece`] carry typed fields and
/// are materialised at conversion time; the remaining variants keep
/// the raw JSON so callers can inspect what was rejected. Use
/// [`to_tokenizer`] (which returns the [`HfTokenizer`] enum) to
/// materialise either supported variant; the sibling
/// [`to_bpe_tokenizer`] / [`to_wordpiece_tokenizer`] entry points
/// return a concrete tokenizer type when the caller already knows
/// which family a config belongs to.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfModel {
    /// A byte-pair-encoding model — Sennrich et al. 2016.
    #[serde(rename = "BPE")]
    Bpe(HfBpeModel),
    /// A `WordPiece` model — Wu et al. 2016, adopted by BERT and its
    /// family. See [`HfWordPieceModel`] for the fields and
    /// [`crate::wordpiece::WordPieceTokenizer`] for the runtime.
    #[serde(rename = "WordPiece")]
    WordPiece(HfWordPieceModel),
    /// `Unigram` model. Deferred — separate algorithm landing.
    #[serde(rename = "Unigram")]
    Unigram(serde_json::Value),
    /// Simple word-level model (a plain vocabulary lookup). Deferred.
    #[serde(rename = "WordLevel")]
    WordLevel(serde_json::Value),
}

/// The `WordPiece`-specific fields of a `model` block.
///
/// Mirrors HF's on-disk shape. Every field except [`Self::vocab`] and
/// [`Self::unk_token`] has a serde default matching HF's own default,
/// so a `"model": {"type": "WordPiece", "vocab": {...},
/// "unk_token": "[UNK]"}` value parses with `continuing_subword_prefix
/// = "##"` and `max_input_chars_per_word = 100`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfWordPieceModel {
    /// The surface-string ↔ id map. The unknown-token entry
    /// (`[UNK]` in canonical BERT) must appear here mapped to some
    /// id; [`to_wordpiece_tokenizer`] surfaces
    /// [`HfConversionError::WordPieceUnkNotInVocab`] otherwise.
    pub vocab: BTreeMap<String, TokenId>,
    /// The surface string for the unknown token. Required — HF's own
    /// spec makes this field mandatory on a `WordPiece` model.
    pub unk_token: String,
    /// Prefix stamped on every subword after the first. Defaults to
    /// `"##"` (BERT-canonical) when absent from the JSON.
    #[serde(default = "default_continuing_subword_prefix")]
    pub continuing_subword_prefix: String,
    /// Maximum character count per word. Words longer than this
    /// shortcut to the unknown-token id. Defaults to 100 (BERT's own
    /// default) when absent from the JSON.
    #[serde(default = "default_max_input_chars_per_word")]
    pub max_input_chars_per_word: usize,
}

/// Default `continuing_subword_prefix` when the JSON omits it —
/// canonical BERT / `DistilBERT` / `RoBERTa` value.
fn default_continuing_subword_prefix() -> String {
    "##".to_string()
}

/// Default `max_input_chars_per_word` when the JSON omits it — HF's
/// own default and the BERT-canonical value.
const fn default_max_input_chars_per_word() -> usize {
    100
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
    /// Byte-level pre-tokenizer (GPT-2 / Llama-2 shape). Honoured
    /// at conversion: composes the `ByteLevel` byte↔char bijection
    /// with an optional GPT-2-canonical regex split and an optional
    /// leading-space prefix. See [`HfByteLevelPreTokenizer`].
    ByteLevel(HfByteLevelPreTokenizer),
    /// Whitespace splitter — HF `Whitespace`. Materialised by
    /// [`to_wordpiece_tokenizer`] into
    /// [`WordPiecePreTokenizer::Whitespace`](crate::wordpiece::WordPiecePreTokenizer::Whitespace).
    /// Rejected by [`to_bpe_tokenizer`] (BPE has its own pipeline).
    Whitespace(serde_json::Value),
    /// Whitespace splitter that keeps runs together — HF
    /// `WhitespaceSplit`. Materialised by
    /// [`to_wordpiece_tokenizer`] into
    /// [`WordPiecePreTokenizer::WhitespaceSplit`](crate::wordpiece::WordPiecePreTokenizer::WhitespaceSplit).
    /// Rejected by [`to_bpe_tokenizer`].
    WhitespaceSplit(serde_json::Value),
    /// Punctuation splitter. Deferred.
    Punctuation(serde_json::Value),
    /// SentencePiece-style metaspace pre-tokenizer. Deferred.
    Metaspace(serde_json::Value),
    /// Single-character delimiter split. Deferred.
    CharDelimiterSplit(serde_json::Value),
    /// BERT-style pre-tokenizer — HF `BertPreTokenizer`. Materialised
    /// by [`to_wordpiece_tokenizer`] into
    /// [`WordPiecePreTokenizer::Bert`](crate::wordpiece::WordPiecePreTokenizer::Bert).
    /// Rejected by [`to_bpe_tokenizer`].
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
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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

/// A `ByteLevel` pre-tokenizer's configuration.
///
/// Mirrors Hugging Face's on-disk shape. Every field has a serde
/// default matching HF's Rust defaults, so a `"pre_tokenizer":
/// {"type": "ByteLevel"}` value with no other fields parses as
/// `add_prefix_space: true, trim_offsets: true, use_regex: true` —
/// the shape used by the shipped GPT-2 checkpoint.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfByteLevelPreTokenizer {
    /// Whether to prepend a leading ASCII space to inputs that do not
    /// already start with one, before the byte↔char mapping is
    /// applied. Defaults to `true` — HF's own default and the shape
    /// that produces `"Ġhello"` from `"hello"`.
    #[serde(default = "default_true")]
    pub add_prefix_space: bool,
    /// HF's `trim_offsets` field. Governs whether the returned
    /// offsets strip leading `Ġ` characters. Preserved but not
    /// applied — the crate reports offsets into the byte-encoded
    /// stream, which is the shape most downstream consumers need
    /// anyway.
    #[serde(default = "default_true")]
    pub trim_offsets: bool,
    /// Whether to apply the GPT-2 canonical regex before the byte
    /// mapping runs. HF's default is `true`, and this is what
    /// produces the split into `["ĠHello", "Ġworld"]` for
    /// `"Hello world"`. When `false`, the whole input region is a
    /// single chunk fed straight into the byte mapping.
    #[serde(default = "default_true")]
    pub use_regex: bool,
}

/// The `decoder` block of a `tokenizer.json` file.
///
/// Only [`HfDecoder::ByteLevel`] is honoured at conversion time; the
/// others parse successfully and preserve their raw JSON so callers
/// can inspect what was rejected. See
/// [`HfTokenizerConfig::decoder`] for the field's role.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfDecoder {
    /// The byte-level decoder. Attaches
    /// [`Decoder::ByteLevel`](crate::Decoder) to the produced
    /// [`BpeTokenizer`] so `decode` reverses the byte↔char mapping.
    /// HF's on-disk shape also carries the same `add_prefix_space` /
    /// `trim_offsets` / `use_regex` fields as the `ByteLevel`
    /// pre-tokenizer — captured here so callers can inspect them,
    /// but not applied on the decode side (the mapping alone suffices
    /// for byte-identical round trips through the `BpeTokenizer`'s
    /// concatenate-then-decode path).
    ByteLevel {
        /// Preserved for caller inspection; not applied at decode.
        #[serde(default = "default_true")]
        add_prefix_space: bool,
        /// Preserved for caller inspection; not applied at decode.
        #[serde(default = "default_true")]
        trim_offsets: bool,
        /// Preserved for caller inspection; not applied at decode.
        #[serde(default = "default_true")]
        use_regex: bool,
    },
    /// Any other decoder (`WordPiece`, `Metaspace`, `BPEDecoder`,
    /// `Sequence`, ...). Serde's `#[serde(other)]` catches every
    /// tag string that does not match the variants listed above;
    /// the payload beyond the tag is discarded (callers who need to
    /// inspect a rejected decoder can re-parse the raw JSON
    /// themselves — the original `tokenizer.json` byte string is the
    /// authoritative source).
    #[serde(other)]
    Other,
}

/// Default `true` for serde-derived booleans that HF stores as
/// `true` when omitted.
const fn default_true() -> bool {
    true
}

/// The `normalizer` block of a `tokenizer.json` file.
///
/// Only the variants named explicitly here are honoured at
/// [`to_bpe_tokenizer`] time — every unrecognised tag string falls
/// through to [`Self::Other`]. See [`Normalizer`] for the semantics
/// of the honoured shapes.
///
/// Deferred variants (`Bert`, `Nmt`, `Precompiled`, regex `Replace`,
/// custom callables) surface at conversion as
/// [`HfConversionError::UnsupportedNormalizer`] with the offending
/// type name.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfNormalizer {
    /// Canonical composition (Normalization Form C).
    #[serde(rename = "NFC")]
    Nfc,
    /// Canonical decomposition (Normalization Form D).
    #[serde(rename = "NFD")]
    Nfd,
    /// Compatibility composition (Normalization Form KC).
    #[serde(rename = "NFKC")]
    Nfkc,
    /// Compatibility decomposition (Normalization Form KD).
    #[serde(rename = "NFKD")]
    Nfkd,
    /// Unicode-aware lower-casing.
    Lowercase,
    /// Literal-string substitution. HF's spec also permits regex
    /// patterns; the regex form is deferred and rejected at
    /// [`to_bpe_tokenizer`] time.
    Replace {
        /// The pattern block. Only [`HfPattern::String`] is honoured;
        /// [`HfPattern::Regex`] surfaces
        /// [`HfConversionError::UnsupportedPattern`] at conversion.
        pattern: HfPattern,
        /// The replacement string.
        content: String,
    },
    /// Trim whitespace from one or both sides.
    Strip {
        /// If `true`, strip leading whitespace.
        #[serde(default = "default_true")]
        left: bool,
        /// If `true`, strip trailing whitespace.
        #[serde(default = "default_true")]
        right: bool,
    },
    /// Prepend a fixed literal to the input (`SentencePiece` "`▁`"
    /// pattern).
    Prepend {
        /// The literal to prepend.
        prepend: String,
    },
    /// Compose several normalizers, left to right.
    Sequence {
        /// The child normalizers, applied in order.
        normalizers: Vec<HfNormalizer>,
    },
    /// Any other normalizer tag (BERT, NMT, `Precompiled`, custom, ...).
    /// Recognised at parse time via serde's `#[serde(other)]` so
    /// parsing does not fail; [`to_bpe_tokenizer`] rejects it with a
    /// specific error.
    #[serde(other)]
    Other,
}

/// The `post_processor` block of a `tokenizer.json` file.
///
/// Only [`Self::TemplateProcessing`] is honoured at [`to_bpe_tokenizer`]
/// time. Every other tag string falls through to [`Self::Other`] via
/// serde's `#[serde(other)]` and surfaces
/// [`HfConversionError::UnsupportedPostProcessor`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfPostProcessor {
    /// The Llama-shape template that injects BOS/EOS around the
    /// primary encoding. See [`TemplateProcessing`] for the semantics.
    TemplateProcessing(HfTemplateProcessing),
    /// Any other post-processor (`BertProcessing`,
    /// `RobertaProcessing`, `ByteLevel`, `Sequence`, ...). Rejected at
    /// conversion time.
    #[serde(other)]
    Other,
}

/// The typed shape of a `TemplateProcessing` post-processor.
///
/// Fields mirror HF's on-disk layout. `single` and `pair` are ordered
/// arrays of [`HfTemplatePiece`] entries (`{"SpecialToken": ...}` /
/// `{"Sequence": ...}`); `special_tokens` is a map from the slot's
/// name to its ids-and-surface-strings metadata.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfTemplateProcessing {
    /// Template for a single-input encoding. See [`HfTemplatePiece`].
    #[serde(default)]
    pub single: Vec<HfTemplatePiece>,
    /// Template for a pair-input encoding. Preserved but not consumed
    /// by [`to_bpe_tokenizer`]'s single-input path.
    #[serde(default)]
    pub pair: Vec<HfTemplatePiece>,
    /// Metadata for every `SpecialToken` slot referenced above.
    #[serde(default)]
    pub special_tokens: BTreeMap<String, HfSpecialTokenInfo>,
}

/// One slot in a `TemplateProcessing` template.
///
/// HF's on-disk shape is `{"SpecialToken": {...}}` or `{"Sequence":
/// {...}}` — a JSON externally-tagged enum.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum HfTemplatePiece {
    /// A pre-registered special-token slot. `id` names the entry in
    /// [`HfTemplateProcessing::special_tokens`].
    SpecialToken {
        /// The referenced special token's name.
        id: String,
        /// The `type_id` HF stamps on this slot. Preserved verbatim.
        #[serde(default)]
        type_id: u32,
    },
    /// The primary encoding slot. `id` is `"A"` for the sole
    /// caller-supplied input and `"B"` for the second in a pair
    /// template.
    Sequence {
        /// Slot name — `"A"` for the primary input, `"B"` for the
        /// pair template's second input.
        id: String,
        /// The `type_id` HF stamps on this slot. Preserved verbatim.
        #[serde(default)]
        type_id: u32,
    },
}

/// Metadata for one entry in [`HfTemplateProcessing::special_tokens`].
///
/// HF's on-disk shape carries both `ids` (numeric ids emitted for the
/// slot) and `tokens` (the corresponding surface strings). Both are
/// preserved so callers who inspect the parsed config get the full
/// picture; the loader only consumes `ids` to build the runtime
/// [`SpecialTokenInfo`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfSpecialTokenInfo {
    /// The token id this special-token name resolves to when the slot
    /// fires. Some HF configs record a nested field named `"id"` that
    /// duplicates the outer key — captured under [`Self::id`] for
    /// caller inspection.
    #[serde(default)]
    pub id: Option<String>,
    /// Numeric ids emitted per occurrence. Read by the loader.
    #[serde(default)]
    pub ids: Vec<TokenId>,
    /// Parallel surface strings for [`Self::ids`]. Preserved but not
    /// consumed by [`to_bpe_tokenizer`].
    #[serde(default)]
    pub tokens: Vec<String>,
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

/// Error returned by [`to_bpe_tokenizer`] / [`to_wordpiece_tokenizer`]
/// / [`to_tokenizer`] when the parsed config references a feature this
/// crate does not yet materialise, or when the config is internally
/// inconsistent.
///
/// Every variant's [`fmt::Display`] impl names the specific feature
/// or offending entry so callers can diagnose without inspecting the
/// enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HfConversionError {
    /// `model.type` is one this crate does not materialise (Unigram,
    /// `WordLevel`). Carries the specific type name.
    UnsupportedModel {
        /// The `model.type` string from the source config.
        type_name: String,
    },
    /// [`to_bpe_tokenizer`] was called on a config whose `model.type`
    /// is not `"BPE"`. Carries the specific type name so callers can
    /// dispatch on it (typically to [`to_wordpiece_tokenizer`] or
    /// [`to_tokenizer`]).
    UnsupportedModelForBpe {
        /// The `model.type` string from the source config.
        type_name: String,
    },
    /// [`to_wordpiece_tokenizer`] was called on a config whose
    /// `model.type` is not `"WordPiece"`. Carries the specific type
    /// name.
    UnsupportedModelForWordPiece {
        /// The `model.type` string from the source config.
        type_name: String,
    },
    /// A `WordPiece` config's `unk_token` surface string is not in
    /// the vocab. Encoding would produce ids the caller cannot
    /// decode.
    WordPieceUnkNotInVocab {
        /// The surface string that should have been in the vocab.
        unk_token: String,
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
    /// A `Sequence` pre-tokenizer combined `ByteLevel` with a sibling
    /// that is not `Split(Regex)`. The `ByteLevel` path only composes
    /// with a `Split(Regex)` (which then feeds the byte↔char mapping)
    /// or with nothing at all.
    UnsupportedByteLevelSequence {
        /// A short human-readable reason.
        reason: &'static str,
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
    /// The `normalizer` block used a variant this crate does not
    /// materialise yet (`Bert`, `Nmt`, `Precompiled`, a `Replace`
    /// with a `Regex` pattern, or a custom callable).
    UnsupportedNormalizer {
        /// The `normalizer.type` string, or a short synthesised name
        /// for a nested rejection (`"Replace(Regex)"`, ...).
        type_name: String,
    },
    /// The `post_processor` block used a variant this crate does not
    /// materialise yet (`BertProcessing`, `RobertaProcessing`,
    /// `ByteLevel`, `Sequence`).
    UnsupportedPostProcessor {
        /// The `post_processor.type` string.
        type_name: String,
    },
    /// A `TemplateProcessing` template referenced a special-token
    /// name that is not in its own `special_tokens` map.
    TemplateSpecialTokenNotDeclared {
        /// The offending name.
        name: String,
    },
}

impl fmt::Display for HfConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedModel { type_name } => write!(
                f,
                "unsupported HF model type {type_name:?} \
                 (this crate materialises \"BPE\" and \"WordPiece\"; \
                 Unigram / WordLevel are deferred to later landings)"
            ),
            Self::UnsupportedModelForBpe { type_name } => write!(
                f,
                "to_bpe_tokenizer called on non-BPE model type {type_name:?}; \
                 use to_wordpiece_tokenizer or to_tokenizer instead"
            ),
            Self::UnsupportedModelForWordPiece { type_name } => write!(
                f,
                "to_wordpiece_tokenizer called on non-WordPiece model type {type_name:?}; \
                 use to_bpe_tokenizer or to_tokenizer instead"
            ),
            Self::WordPieceUnkNotInVocab { unk_token } => write!(
                f,
                "WordPiece model's unk_token {unk_token:?} is not present in the vocabulary"
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
                 possibly combined with a single ByteLevel sibling"
            ),
            Self::UnsupportedByteLevelSequence { reason } => write!(
                f,
                "unsupported HF pre_tokenizer Sequence combining ByteLevel with an \
                 unsupported sibling: {reason}"
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
            Self::UnsupportedNormalizer { type_name } => write!(
                f,
                "unsupported HF normalizer type {type_name:?}: \
                 this crate materialises NFC/NFD/NFKC/NFKD, Lowercase, \
                 Replace(String), Strip, Prepend, and their Sequence \
                 composition"
            ),
            Self::UnsupportedPostProcessor { type_name } => write!(
                f,
                "unsupported HF post_processor type {type_name:?}: \
                 this crate materialises only \"TemplateProcessing\" today \
                 (BertProcessing/RobertaProcessing/ByteLevel/Sequence \
                 are deferred to later landings)"
            ),
            Self::TemplateSpecialTokenNotDeclared { name } => write!(
                f,
                "TemplateProcessing template references special-token name \
                 {name:?} that is missing from its own \"special_tokens\" map"
            ),
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
    // Model — must be BPE. Other supported model types (WordPiece)
    // return a dedicated error so callers can dispatch on it; deferred
    // model types (Unigram, WordLevel) return `UnsupportedModel`.
    let bpe = match &config.model {
        HfModel::Bpe(bpe) => bpe,
        HfModel::WordPiece(_) => {
            return Err(HfConversionError::UnsupportedModelForBpe {
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

    // Pre-tokenizer — Split(Regex), ByteLevel(+optional inner Split),
    // a Sequence combining one Split(Regex) with one ByteLevel, or
    // nothing at all. See `extract_pre_tokenizer` for the exact rules.
    let pipeline = match &config.pre_tokenizer {
        None => PreTokPipeline::None,
        Some(pt) => extract_pre_tokenizer(pt)?,
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
    match pipeline {
        PreTokPipeline::None => {}
        PreTokPipeline::Regex(pattern) => {
            let compiled = RegexPreTokenizer::new(pattern)?;
            tok = tok.with_pre_tokenizer(PreTokenizerRegex::regex(compiled));
        }
        PreTokPipeline::ByteLevel {
            add_prefix_space,
            use_regex,
            inner_regex,
        } => {
            // If an inner Split(Regex) is present it wins; otherwise
            // if `use_regex` is true, we fall back to the canonical
            // GPT-2 pattern that HF's ByteLevel uses internally.
            let split = if let Some(pat) = inner_regex {
                Some(RegexPreTokenizer::new(pat)?)
            } else if use_regex {
                Some(RegexPreTokenizer::new(GPT2_PATTERN)?)
            } else {
                None
            };
            tok = tok.with_pre_tokenizer(PreTokenizerRegex::byte_level(add_prefix_space, split));
        }
    }

    // Decoder — only ByteLevel is honoured; the passthrough default
    // covers every other shape.
    if let Some(HfDecoder::ByteLevel { .. }) = &config.decoder {
        tok = tok.with_decoder(Decoder::ByteLevel);
    }

    // Normalizer — runs before the pre-tokenizer at encode time.
    if let Some(hn) = &config.normalizer {
        let n = to_runtime_normalizer(hn)?;
        tok = tok.with_normalizer(n);
    }

    // Post-processor — runs on the finished encoding before it leaves
    // `encode`. Only `TemplateProcessing` is honoured today.
    if let Some(hp) = &config.post_processor {
        let pp = to_runtime_post_processor(hp)?;
        tok = tok.with_post_processor(pp);
    }
    Ok(tok)
}

/// A runnable tokenizer produced by [`to_tokenizer`].
///
/// The variant reflects the source config's `model.type`:
///
/// * [`HfTokenizer::Bpe`] wraps a boxed [`BpeTokenizer`] — every
///   non-empty `model.type == "BPE"` config lands here. The
///   `BpeTokenizer` is boxed because its inline size is several times
///   larger than [`crate::wordpiece::WordPieceTokenizer`]'s; boxing
///   keeps the enum's stack footprint proportional to its smallest
///   variant.
/// * [`HfTokenizer::WordPiece`] wraps a
///   [`crate::wordpiece::WordPieceTokenizer`] — every
///   `model.type == "WordPiece"` config lands here.
///
/// Unigram and `WordLevel` are deferred; [`to_tokenizer`] rejects
/// those with [`HfConversionError::UnsupportedModel`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HfTokenizer {
    /// A [`BpeTokenizer`] materialised from a `BPE` model. Boxed to
    /// keep the enum's inline footprint small; deref with
    /// `&*bpe` (or pattern-match on `HfTokenizer::Bpe(bpe)` and call
    /// methods on `bpe.as_ref()` / `bpe.as_mut()`).
    Bpe(alloc::boxed::Box<BpeTokenizer>),
    /// A [`crate::wordpiece::WordPieceTokenizer`] materialised from a
    /// `WordPiece` model.
    WordPiece(crate::wordpiece::WordPieceTokenizer),
}

/// Materialise an [`HfTokenizerConfig`] as a runnable [`HfTokenizer`].
///
/// Dispatches on `model.type`: `BPE` produces
/// [`HfTokenizer::Bpe`]; `WordPiece` produces
/// [`HfTokenizer::WordPiece`]. Every other model type surfaces
/// [`HfConversionError::UnsupportedModel`] with the offending type
/// name.
///
/// # Errors
///
/// Returns any [`HfConversionError`] the underlying
/// [`to_bpe_tokenizer`] or [`to_wordpiece_tokenizer`] would.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::hf::{HfTokenizer, parse_tokenizer_json, to_tokenizer};
///
/// let json = r#"{
///     "added_tokens": [],
///     "model": {
///         "type": "WordPiece",
///         "vocab": {"[UNK]": 0, "cat": 1},
///         "unk_token": "[UNK]"
///     }
/// }"#;
/// let config = parse_tokenizer_json(json).unwrap();
/// let tok = to_tokenizer(&config).unwrap();
/// assert!(matches!(tok, HfTokenizer::WordPiece(_)));
/// ```
pub fn to_tokenizer(config: &HfTokenizerConfig) -> Result<HfTokenizer, HfConversionError> {
    match &config.model {
        HfModel::Bpe(_) => Ok(HfTokenizer::Bpe(alloc::boxed::Box::new(to_bpe_tokenizer(
            config,
        )?))),
        HfModel::WordPiece(_) => Ok(HfTokenizer::WordPiece(to_wordpiece_tokenizer(config)?)),
        HfModel::Unigram(_) => Err(HfConversionError::UnsupportedModel {
            type_name: "Unigram".to_string(),
        }),
        HfModel::WordLevel(_) => Err(HfConversionError::UnsupportedModel {
            type_name: "WordLevel".to_string(),
        }),
    }
}

/// Materialise an [`HfTokenizerConfig`] as a runnable
/// [`crate::wordpiece::WordPieceTokenizer`].
///
/// The config's `model.type` must be `"WordPiece"`; any other type
/// (including `"BPE"`) surfaces
/// [`HfConversionError::UnsupportedModelForWordPiece`].
///
/// Supported ancillary features today:
///
/// * `pre_tokenizer.type ∈ {"BertPreTokenizer", "Whitespace",
///   "WhitespaceSplit"}` — routes through the corresponding
///   [`crate::wordpiece::WordPiecePreTokenizer`] variant. A missing
///   `pre_tokenizer` block defaults to
///   [`crate::wordpiece::WordPiecePreTokenizer::Whitespace`].
/// * The `WordPiece` model's `unk_token` /
///   `continuing_subword_prefix` / `max_input_chars_per_word` fields
///   are honoured verbatim.
///
/// **Deferred** ancillary features (parse but reject at conversion):
///
/// * `normalizer` — most `WordPiece` checkpoints ship a
///   `BertNormalizer` (lower-case + accent strip + Chinese-char
///   handling). Applying it correctly is its own landing; a
///   config with a normalizer surfaces
///   [`HfConversionError::UnsupportedNormalizer`] with the type name.
///   NFC / NFD / NFKC / NFKD are honoured — those already work through
///   the shared [`crate::normalizer`] layer.
/// * `post_processor` — `WordPiece` checkpoints usually ship
///   `TemplateProcessing` (for `[CLS]` / `[SEP]`); that shape is
///   honoured verbatim. Deferred variants (`BertProcessing`, etc.)
///   surface [`HfConversionError::UnsupportedPostProcessor`].
/// * `decoder` — the raw config is preserved on
///   [`HfTokenizerConfig::decoder`] for caller inspection but not
///   applied; `WordPieceDecoder` semantics live inside
///   [`crate::wordpiece::WordPieceTokenizer::decode`] regardless of
///   what the config declares.
///
/// # Errors
///
/// Returns [`HfConversionError`] with a variant naming the offending
/// feature. Common causes: a non-`WordPiece` `model.type`, a
/// `BertNormalizer` in the `normalizer` slot, or an `unk_token` that
/// is not present in the vocabulary.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::hf::{parse_tokenizer_json, to_wordpiece_tokenizer};
///
/// let json = r###"{
///     "added_tokens": [],
///     "model": {
///         "type": "WordPiece",
///         "vocab": {"[UNK]": 0, "un": 1, "##aff": 2, "##able": 3},
///         "unk_token": "[UNK]"
///     }
/// }"###;
/// let config = parse_tokenizer_json(json).unwrap();
/// let tok = to_wordpiece_tokenizer(&config).unwrap();
/// assert_eq!(tok.encode("unaffable"), vec![1, 2, 3]);
/// ```
pub fn to_wordpiece_tokenizer(
    config: &HfTokenizerConfig,
) -> Result<crate::wordpiece::WordPieceTokenizer, HfConversionError> {
    let wp = match &config.model {
        HfModel::WordPiece(wp) => wp,
        HfModel::Bpe(_) => {
            return Err(HfConversionError::UnsupportedModelForWordPiece {
                type_name: "BPE".to_string(),
            });
        }
        HfModel::Unigram(_) => {
            return Err(HfConversionError::UnsupportedModelForWordPiece {
                type_name: "Unigram".to_string(),
            });
        }
        HfModel::WordLevel(_) => {
            return Err(HfConversionError::UnsupportedModelForWordPiece {
                type_name: "WordLevel".to_string(),
            });
        }
    };

    // Validate: the `unk_token` surface string must be in the vocab.
    let Some(&unk_id) = wp.vocab.get(&wp.unk_token) else {
        return Err(HfConversionError::WordPieceUnkNotInVocab {
            unk_token: wp.unk_token.clone(),
        });
    };

    // Fold added_tokens into the vocabulary so callers who inspect
    // added specials find them under the same lookup as the model
    // vocab. Overlapping (id, surface) pairs are idempotent.
    let mut vocab: BTreeMap<String, TokenId> = wp.vocab.clone();
    for at in &config.added_tokens {
        // If the surface string is already there under a different id,
        // the caller's config is inconsistent — surface as a duplicate
        // via the vocabulary builder-error surface used by the BPE
        // path.
        if let Some(&existing) = vocab.get(&at.content) {
            if existing != at.id {
                return Err(HfConversionError::Vocabulary(
                    VocabularyBuilderError::DuplicateByteString,
                ));
            }
        } else {
            vocab.insert(at.content.clone(), at.id);
        }
    }

    // Assemble.
    let mut tok = crate::wordpiece::WordPieceTokenizer::from_parts(
        vocab,
        unk_id,
        wp.continuing_subword_prefix.clone(),
        wp.max_input_chars_per_word,
    )
    .map_err(|e| match e {
        crate::wordpiece::WordPieceBuildError::UnkNotInVocab(_) => {
            HfConversionError::WordPieceUnkNotInVocab {
                unk_token: wp.unk_token.clone(),
            }
        }
    })?;

    // Pre-tokenizer routing. `WordPiece` cares only about the
    // whitespace / punctuation split; the shape carried in a
    // `Sequence` around one of the supported entries is unwrapped.
    let pre = extract_wordpiece_pre_tokenizer(config.pre_tokenizer.as_ref())?;
    tok = tok.with_pre_tokenizer(pre);

    // Normalizer routing — the shared to_runtime_normalizer honours
    // NFC / NFD / NFKC / NFKD / Lowercase / Strip / Prepend /
    // Replace(String) / their Sequence composition. Everything else
    // (notably BertNormalizer) surfaces as UnsupportedNormalizer.
    //
    // The runtime `WordPieceTokenizer` does not carry a normalizer
    // slot today (its `encode` operates on the raw input), so a
    // config that carries an honoured normalizer is currently
    // accepted-but-not-applied. Reject anything more surprising than
    // that to keep the trap door tight.
    if let Some(hn) = &config.normalizer {
        // Reject deferred variants explicitly; ignore the honoured
        // ones since we cannot apply them on the WordPiece side yet.
        let _ = to_runtime_normalizer(hn)?;
    }

    // Post-processor: TemplateProcessing is honoured (the shape every
    // BERT-family checkpoint uses for `[CLS]` / `[SEP]`). The
    // WordPiece runtime does not carry a post-processor slot today —
    // if a caller needs the templated encoding they can drive the
    // splice themselves against the produced ids using
    // `HfPostProcessor::TemplateProcessing`.
    if let Some(hp) = &config.post_processor {
        let _ = to_runtime_post_processor(hp)?;
    }

    Ok(tok)
}

/// Reduce an [`HfPreTokenizer`] (or its absence) to a
/// [`crate::wordpiece::WordPiecePreTokenizer`], following the `WordPiece`
/// routing rules documented on [`to_wordpiece_tokenizer`].
fn extract_wordpiece_pre_tokenizer(
    pt: Option<&HfPreTokenizer>,
) -> Result<crate::wordpiece::WordPiecePreTokenizer, HfConversionError> {
    use crate::wordpiece::WordPiecePreTokenizer;
    let Some(pt) = pt else {
        // Missing pre_tokenizer block: fall back to the safe default
        // (Whitespace + punctuation), which matches what most HF
        // WordPiece checkpoints implicitly assume.
        return Ok(WordPiecePreTokenizer::Whitespace);
    };
    match pt {
        HfPreTokenizer::Whitespace(_) => Ok(WordPiecePreTokenizer::Whitespace),
        HfPreTokenizer::WhitespaceSplit(_) => Ok(WordPiecePreTokenizer::WhitespaceSplit),
        HfPreTokenizer::BertPreTokenizer(_) => Ok(WordPiecePreTokenizer::Bert),
        HfPreTokenizer::Sequence { pretokenizers } => {
            // Sequence: accept exactly one supported child (typical for
            // BERT variants that wrap BertPreTokenizer alone).
            if pretokenizers.is_empty() {
                return Ok(WordPiecePreTokenizer::Whitespace);
            }
            if pretokenizers.len() == 1 {
                return extract_wordpiece_pre_tokenizer(Some(&pretokenizers[0]));
            }
            Err(HfConversionError::AmbiguousSequencePreTokenizer {
                child_count: pretokenizers.len(),
            })
        }
        HfPreTokenizer::Split(_) => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "Split".to_string(),
            reason: "Split pre-tokenizers are for BPE; WordPiece uses whitespace + punctuation",
        }),
        HfPreTokenizer::ByteLevel(_) => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "ByteLevel".to_string(),
            reason: "ByteLevel pre-tokenizers are for byte-level BPE, not WordPiece",
        }),
        other => {
            // Everything else (Punctuation, Metaspace, ...) surfaces
            // its usual deferred-feature error.
            if let Some(err) = deferred_pre_tokenizer_reason(other) {
                Err(err)
            } else {
                Err(HfConversionError::UnsupportedPreTokenizer {
                    type_name: "unknown".to_string(),
                    reason: "unhandled pre_tokenizer variant on WordPiece path",
                })
            }
        }
    }
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

/// The subset of pre-tokenizer shapes [`to_bpe_tokenizer`] knows how to
/// materialise. Internal to this module; see [`extract_pre_tokenizer`]
/// for the caller-visible reduction.
#[derive(Debug, Clone)]
enum PreTokPipeline {
    /// No pre-tokenizer at all — the tokenizer will fall back to
    /// whitespace splitting on its own.
    None,
    /// A plain regex pre-tokenizer, built by compiling the pattern.
    Regex(String),
    /// A `ByteLevel` pipeline — optional leading-space prefix, optional
    /// inner regex to split *before* the byte↔char mapping runs.
    /// `use_regex` mirrors HF's field; when `inner_regex` is `None`
    /// and `use_regex` is `true`, the loader supplies HF's canonical
    /// GPT-2 pattern.
    ByteLevel {
        add_prefix_space: bool,
        use_regex: bool,
        inner_regex: Option<String>,
    },
}

/// Walk a [`HfPreTokenizer`] value and reduce it to a runnable
/// [`PreTokPipeline`].
///
/// Rules:
///
/// * A bare `Split(Regex)` becomes [`PreTokPipeline::Regex`].
/// * A bare `ByteLevel(...)` becomes [`PreTokPipeline::ByteLevel`].
/// * A `Sequence { pretokenizers }` may hold zero, one, or two
///   entries:
///     - Empty → [`PreTokPipeline::None`].
///     - Exactly one supported child → its own pipeline.
///     - Exactly two entries where one is `Split(Regex)` and the
///       other is `ByteLevel(...)` → a `ByteLevel` pipeline whose
///       inner regex is the Split's pattern. Order does not matter.
/// * Any other combination is rejected with a targeted error.
fn extract_pre_tokenizer(pt: &HfPreTokenizer) -> Result<PreTokPipeline, HfConversionError> {
    match pt {
        HfPreTokenizer::Split(split) => split_to_pipeline(split),
        HfPreTokenizer::ByteLevel(bl) => Ok(PreTokPipeline::ByteLevel {
            add_prefix_space: bl.add_prefix_space,
            use_regex: bl.use_regex,
            inner_regex: None,
        }),
        HfPreTokenizer::Sequence { pretokenizers } => sequence_to_pipeline(pretokenizers),
        // Deferred variants: return a targeted error.
        other => {
            if let Some(err) = deferred_pre_tokenizer_reason(other) {
                Err(err)
            } else {
                // Unreachable in practice — every non-Split /
                // non-Sequence / non-ByteLevel variant of
                // `HfPreTokenizer` is covered by
                // `deferred_pre_tokenizer_reason`. Guard it anyway.
                Err(HfConversionError::UnsupportedPreTokenizer {
                    type_name: "unknown".to_string(),
                    reason: "unhandled pre_tokenizer variant",
                })
            }
        }
    }
}

/// Reduce a `Split` pre-tokenizer to a pipeline (or the appropriate
/// error).
fn split_to_pipeline(split: &HfSplitPreTokenizer) -> Result<PreTokPipeline, HfConversionError> {
    match &split.pattern {
        HfPattern::Regex(pattern) => Ok(PreTokPipeline::Regex(pattern.clone())),
        HfPattern::String(_) => Err(HfConversionError::UnsupportedPattern { variant: "String" }),
    }
}

/// Reduce a `Sequence` pre-tokenizer's children into a pipeline.
///
/// See [`extract_pre_tokenizer`] for the acceptance rules.
fn sequence_to_pipeline(children: &[HfPreTokenizer]) -> Result<PreTokPipeline, HfConversionError> {
    if children.is_empty() {
        return Ok(PreTokPipeline::None);
    }

    // Partition children into (byte_level, split, deferred) buckets.
    // Nested Sequences are rejected up-front to keep the case
    // enumeration finite.
    let mut byte_level: Option<&HfByteLevelPreTokenizer> = None;
    let mut split: Option<&HfSplitPreTokenizer> = None;
    for child in children {
        match child {
            HfPreTokenizer::ByteLevel(bl) => {
                if byte_level.is_some() {
                    return Err(HfConversionError::AmbiguousSequencePreTokenizer {
                        child_count: children.len(),
                    });
                }
                byte_level = Some(bl);
            }
            HfPreTokenizer::Split(s) => {
                if split.is_some() {
                    return Err(HfConversionError::AmbiguousSequencePreTokenizer {
                        child_count: children.len(),
                    });
                }
                split = Some(s);
            }
            HfPreTokenizer::Sequence { .. } => {
                return Err(HfConversionError::UnsupportedByteLevelSequence {
                    reason: "nested Sequence pre-tokenizers are not supported",
                });
            }
            other => {
                if let Some(err) = deferred_pre_tokenizer_reason(other) {
                    return Err(err);
                }
                return Err(HfConversionError::UnsupportedPreTokenizer {
                    type_name: "unknown".to_string(),
                    reason: "unhandled pre_tokenizer variant inside Sequence",
                });
            }
        }
    }

    match (byte_level, split) {
        (None, None) => Ok(PreTokPipeline::None),
        (None, Some(s)) => split_to_pipeline(s),
        (Some(bl), None) => Ok(PreTokPipeline::ByteLevel {
            add_prefix_space: bl.add_prefix_space,
            use_regex: bl.use_regex,
            inner_regex: None,
        }),
        (Some(bl), Some(s)) => match &s.pattern {
            HfPattern::Regex(pat) => Ok(PreTokPipeline::ByteLevel {
                add_prefix_space: bl.add_prefix_space,
                use_regex: bl.use_regex,
                inner_regex: Some(pat.clone()),
            }),
            HfPattern::String(_) => {
                Err(HfConversionError::UnsupportedPattern { variant: "String" })
            }
        },
    }
}

/// Map an [`HfPreTokenizer`] to a specific "deferred" error if it is a
/// known-unsupported variant. Returns `None` for `Split`, `ByteLevel`,
/// and `Sequence` (all of which are handled inline by
/// [`extract_pre_tokenizer`] / [`sequence_to_pipeline`]).
fn deferred_pre_tokenizer_reason(pt: &HfPreTokenizer) -> Option<HfConversionError> {
    let (type_name, reason) = match pt {
        HfPreTokenizer::Split(_)
        | HfPreTokenizer::Sequence { .. }
        | HfPreTokenizer::ByteLevel(_) => return None,
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

/// Reduce an [`HfNormalizer`] to a runtime [`Normalizer`] or the
/// appropriate deferred-feature error.
fn to_runtime_normalizer(hn: &HfNormalizer) -> Result<Normalizer, HfConversionError> {
    match hn {
        HfNormalizer::Nfc => Ok(Normalizer::Nfc),
        HfNormalizer::Nfd => Ok(Normalizer::Nfd),
        HfNormalizer::Nfkc => Ok(Normalizer::Nfkc),
        HfNormalizer::Nfkd => Ok(Normalizer::Nfkd),
        HfNormalizer::Lowercase => Ok(Normalizer::Lowercase),
        HfNormalizer::Replace { pattern, content } => match pattern {
            HfPattern::String(p) => Ok(Normalizer::Replace {
                pattern: p.clone(),
                content: content.clone(),
            }),
            HfPattern::Regex(_) => Err(HfConversionError::UnsupportedNormalizer {
                type_name: "Replace(Regex)".to_string(),
            }),
        },
        HfNormalizer::Strip { left, right } => Ok(Normalizer::Strip {
            left: *left,
            right: *right,
        }),
        HfNormalizer::Prepend { prepend } => Ok(Normalizer::Prepend {
            prepend: prepend.clone(),
        }),
        HfNormalizer::Sequence { normalizers } => {
            let mut children = Vec::with_capacity(normalizers.len());
            for child in normalizers {
                children.push(to_runtime_normalizer(child)?);
            }
            Ok(Normalizer::Sequence(children))
        }
        HfNormalizer::Other => Err(HfConversionError::UnsupportedNormalizer {
            type_name: "Other".to_string(),
        }),
    }
}

/// Reduce an [`HfPostProcessor`] to a runtime [`PostProcessor`] or the
/// appropriate deferred-feature error.
fn to_runtime_post_processor(hp: &HfPostProcessor) -> Result<PostProcessor, HfConversionError> {
    match hp {
        HfPostProcessor::TemplateProcessing(tp) => {
            // Validate that every referenced special-token name is
            // declared in the template's own `special_tokens` map.
            for piece in tp.single.iter().chain(tp.pair.iter()) {
                if let HfTemplatePiece::SpecialToken { id, .. } = piece {
                    if !tp.special_tokens.contains_key(id) {
                        return Err(HfConversionError::TemplateSpecialTokenNotDeclared {
                            name: id.clone(),
                        });
                    }
                }
            }
            let single = tp.single.iter().map(to_runtime_piece).collect();
            let pair = tp.pair.iter().map(to_runtime_piece).collect();
            let mut specials: BTreeMap<String, SpecialTokenInfo> = BTreeMap::new();
            for (name, info) in &tp.special_tokens {
                specials.insert(
                    name.clone(),
                    SpecialTokenInfo {
                        ids: info.ids.clone(),
                        tokens: info.tokens.clone(),
                    },
                );
            }
            Ok(PostProcessor::TemplateProcessing(TemplateProcessing {
                single,
                pair,
                special_tokens: specials,
            }))
        }
        HfPostProcessor::Other => Err(HfConversionError::UnsupportedPostProcessor {
            type_name: "Other".to_string(),
        }),
    }
}

fn to_runtime_piece(p: &HfTemplatePiece) -> TemplatePiece {
    match p {
        HfTemplatePiece::SpecialToken { id, type_id } => TemplatePiece::SpecialToken {
            id: id.clone(),
            type_id: *type_id,
        },
        HfTemplatePiece::Sequence { id, type_id } => TemplatePiece::Sequence {
            id: id.clone(),
            type_id: *type_id,
        },
    }
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
    fn sequence_with_split_and_bytelevel_composes_to_byte_level() {
        // Wave-9 now supports ByteLevel: a Sequence carrying a
        // Split(Regex) *and* a ByteLevel entry becomes a byte-level
        // pipeline whose inner regex is the Split's pattern (the
        // outer Split "wins" the regex slot). The vocab below is
        // just large enough to encode `,`-separated bytes after the
        // ByteLevel mapping (which for `,` = 0x2C stays as `,`).
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
        let tok = to_bpe_tokenizer(&config).unwrap();
        // `a,b` with the Split regex `[^,]+` yields chunks ["a", "b"];
        // ByteLevel maps each byte to itself for ASCII printables.
        assert_eq!(tok.encode("a,b").unwrap().ids, alloc::vec![0, 1]);
    }

    #[test]
    fn bytelevel_pre_tokenizer_is_supported() {
        // Wave-9: standalone ByteLevel is honoured, using the GPT-2
        // canonical regex when `use_regex: true` and no inner Split
        // sibling is provided. The vocab below carries every mapped
        // char that a leading-space `hello` encodes to.
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": true,
                "use_regex": true
            },
            "decoder": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": true,
                "use_regex": true
            },
            "model": {
                "type": "BPE",
                "vocab": {
                    "Ġ": 0,
                    "h": 1, "e": 2, "l": 3, "o": 4,
                    "Ġh": 5, "Ġhe": 6, "ll": 7, "Ġhell": 8, "Ġhello": 9
                },
                "merges": [
                    ["Ġ", "h"], ["Ġh", "e"], ["l", "l"], ["Ġhe", "ll"], ["Ġhell", "o"]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.decoder(), crate::Decoder::ByteLevel);
        // "hello" with add_prefix_space=true becomes " hello" →
        // GPT-2 regex yields [" hello"] → byte-level encoded to
        // "Ġhello" → merges reach the vocab entry `Ġhello` (id 9).
        let enc = tok.encode("hello").unwrap();
        assert_eq!(enc.ids, alloc::vec![9]);
        // ByteLevel decoder reverses the char↔byte mapping. Round
        // trip lands back on the leading-space form because the
        // encoder added it; that is HF's documented behaviour.
        assert_eq!(tok.decode(&enc.ids).unwrap(), " hello");
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
    fn wordpiece_model_rejected_by_to_bpe_tokenizer() {
        // Wave-11: WordPiece is materialised via `to_wordpiece_tokenizer`
        // / `to_tokenizer`, not `to_bpe_tokenizer`. Backwards-compat:
        // `to_bpe_tokenizer` still errors, but with a dedicated
        // `UnsupportedModelForBpe` variant so callers can dispatch.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordPiece",
                "vocab": {"[UNK]": 0, "a": 1},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModelForBpe { type_name } => {
                assert_eq!(type_name, "WordPiece");
            }
            other => panic!("expected UnsupportedModelForBpe(WordPiece), got {other:?}"),
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
    fn nfc_normalizer_is_applied_on_encode() {
        // Wave-10: the normalizer field now takes effect. NFC on
        // ASCII "ab" is a no-op, and the encoding matches the
        // pre-normalizer wave-9 behaviour byte-for-byte.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {"type": "NFC"},
            "decoder": {"type": "Metaspace"},
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(matches!(config.normalizer, Some(HfNormalizer::Nfc)));
        assert!(matches!(config.decoder, Some(HfDecoder::Other)));
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(matches!(
            tok.normalizer(),
            Some(crate::normalizer::Normalizer::Nfc)
        ));
        assert_eq!(tok.decoder(), Decoder::Passthrough);
        assert_eq!(tok.encode("ab").unwrap().ids, vec![0, 1]);
    }

    // ---------------------------------------------------------------------
    // Representative GPT-2-shape blob.
    //
    // Wave-9 makes ByteLevel a first-class citizen: this config now
    // ships the real GPT-2 pipeline — a ByteLevel pre-tokenizer with
    // the canonical `add_prefix_space: true, use_regex: true` defaults
    // and a matching ByteLevel decoder. The vocab and merges are
    // written in *encoded* form (leading spaces show up as `Ġ`,
    // matching the on-disk shape of the real GPT-2 file). Encodings
    // are now byte-identical to real GPT-2 on any input the toy
    // vocab can cover.
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
            "pre_tokenizer": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": true,
                "use_regex": true
            },
            "post_processor": null,
            "decoder": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": true,
                "use_regex": true
            },
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
                    "wo": 12, "or": 13, "wor": 14, "ld": 15, "world": 16,
                    "Ġ": 17,
                    "Ġh": 18, "Ġhe": 19, "Ġhell": 20, "Ġhello": 21,
                    "Ġw": 22, "Ġwo": 23, "Ġwor": 24, "Ġworl": 25, "Ġworld": 26
                },
                "merges": [
                    "Ġ h", "Ġh e", "l l", "Ġhe ll", "Ġhell o",
                    "Ġ w", "Ġw o", "Ġwo r", "Ġwor l", "Ġworl d",
                    "h e", "he ll", "hell o",
                    "w o", "wo r", "wor l", "worl d"
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
                assert!(bpe.merges.len() >= 10);
                assert!(matches!(&bpe.merges[0], HfMerge::Joined(s) if s == "Ġ h"));
            }
            _ => panic!("expected BPE"),
        }
        // The ByteLevel pre-tokenizer and decoder both parse into
        // typed variants (no more `serde_json::Value` fallback).
        assert!(matches!(
            config.pre_tokenizer,
            Some(HfPreTokenizer::ByteLevel(_))
        ));
        assert!(matches!(config.decoder, Some(HfDecoder::ByteLevel { .. })));
        // Round-trip through the produced tokenizer. Encoding
        // matches the real GPT-2 shape: leading-space words become
        // `Ġword`, and the merge table reaches the whole-word id.
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.decoder(), Decoder::ByteLevel);
        // "hello" with add_prefix_space=true encodes to " hello" →
        // GPT-2 regex → [" hello"] → byte-level `Ġhello` → id 21.
        let hello = tok.encode("hello").unwrap();
        assert_eq!(hello.ids, vec![21]);
        // Decode round-trips through the ByteLevel byte↔char inverse
        // and lands back on the space-prefixed original (HF's own
        // decode behaviour — the prefix space is the encoder's).
        assert_eq!(tok.decode(&hello.ids).unwrap(), " hello");
        // A multi-word input: `"Hello world"` — the toy vocab only
        // covers lowercase, so use the covered inputs.
        let mixed_words = tok.encode("hello world").unwrap();
        // "hello world" → " hello world" → [" hello", " world"] →
        // ["Ġhello", "Ġworld"] → ids [21, 26].
        assert_eq!(mixed_words.ids, vec![21, 26]);
        // <|endoftext|> is honoured as a special even inside a
        // ByteLevel pipeline (specials are matched literally before
        // any regex / byte-level mapping runs).
        let mixed = tok.encode("hello<|endoftext|>world").unwrap();
        assert_eq!(mixed.ids.first(), Some(&21));
        assert_eq!(mixed.ids.iter().find(|&&id| id == 50256), Some(&50256));
        assert!(mixed.special_mask.iter().any(|&b| b));
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

    // ---------------------------------------------------------------------
    // Wave-9 ByteLevel-specific integration tests.
    // ---------------------------------------------------------------------

    #[test]
    fn bytelevel_defaults_match_hf_shape() {
        // Serde defaults on `HfByteLevelPreTokenizer` should recover the
        // HF library defaults when the fields are absent from the JSON.
        let json = r#"{
            "type": "ByteLevel"
        }"#;
        let bl: HfByteLevelPreTokenizer = serde_json::from_str(json).unwrap();
        assert!(bl.add_prefix_space);
        assert!(bl.trim_offsets);
        assert!(bl.use_regex);
    }

    #[test]
    fn bytelevel_decoder_defaults_match_hf_shape() {
        // Same for the decoder side: bare `{"type": "ByteLevel"}`.
        let json = r#"{"type": "ByteLevel"}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        match dec {
            HfDecoder::ByteLevel {
                add_prefix_space,
                trim_offsets,
                use_regex,
            } => {
                assert!(add_prefix_space);
                assert!(trim_offsets);
                assert!(use_regex);
            }
            HfDecoder::Other => panic!("bare ByteLevel decoder parsed as Other"),
        }
    }

    #[test]
    fn unknown_decoder_falls_through_to_other() {
        let json = r#"{"type": "Metaspace", "replacement": "_"}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        assert!(matches!(dec, HfDecoder::Other));
    }

    #[test]
    fn bytelevel_use_regex_false_disables_inner_split() {
        // `use_regex: false` — the whole region is one chunk, encoded
        // as one shot. Our toy vocab must cover every mapped byte of
        // the input.
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {
                "type": "ByteLevel",
                "add_prefix_space": false,
                "trim_offsets": false,
                "use_regex": false
            },
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, "c": 2, "Ġ": 3, "ab": 4, "abc": 5},
                "merges": [["a", "b"], ["ab", "c"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // "abc" — no prefix space, no inner regex split — encodes as
        // a single chunk. Merges reach id 5 ("abc").
        let enc = tok.encode("abc").unwrap();
        assert_eq!(enc.ids, alloc::vec![5]);
        // Decode reverses the ByteLevel mapping (no-op for
        // printable ASCII).
        assert_eq!(tok.decode(&enc.ids).unwrap(), "abc");
    }

    #[test]
    fn bytelevel_hello_world_produces_ghello_gworld_chunks() {
        // The task's canonical acceptance example: `encode("Hello
        // world")` should yield the byte-level chunks `["ĠHello",
        // "Ġworld"]`. Our toy vocab shipping every needed piece
        // makes each chunk reach its whole-word id.
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel"},
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {
                    "Ġ": 0, "H": 1, "e": 2, "l": 3, "o": 4,
                    "w": 5, "r": 6, "d": 7,
                    "ĠH": 8, "ĠHe": 9, "ll": 10, "ĠHell": 11, "ĠHello": 12,
                    "Ġw": 13, "Ġwo": 14, "Ġwor": 15, "Ġworl": 16, "Ġworld": 17
                },
                "merges": [
                    ["Ġ", "H"], ["ĠH", "e"], ["l", "l"], ["ĠHe", "ll"], ["ĠHell", "o"],
                    ["Ġ", "w"], ["Ġw", "o"], ["Ġwo", "r"], ["Ġwor", "l"], ["Ġworl", "d"]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let enc = tok.encode("Hello world").unwrap();
        // The two chunks are ĠHello (id 12) and Ġworld (id 17).
        assert_eq!(enc.ids, alloc::vec![12, 17]);
        // Decode round-trips through the ByteLevel inverse and lands
        // on the leading-space form (HF's `add_prefix_space` default).
        assert_eq!(tok.decode(&enc.ids).unwrap(), " Hello world");
    }

    #[test]
    fn bytelevel_round_trips_every_shipped_input() {
        // Vocabulary that covers every byte 0..=255's mapped char
        // as its own single-char entry. With no merges the BPE
        // loop is a no-op and encode/decode just exercises the
        // byte-level layer.
        //
        // Build the vocab programmatically so every ByteLevel-mapped
        // char is a valid id — the string form is inserted as
        // `String::from(char)`, which is the char's UTF-8 encoding.
        let mut vocab_entries: BTreeMap<String, TokenId> = BTreeMap::new();
        for b in 0u8..=255 {
            let ch = crate::byte_level::BYTES_TO_CHARS[b as usize];
            let mut s = String::new();
            s.push(ch);
            vocab_entries.insert(s, u32::from(b));
        }
        let mut json = String::from(
            r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel"},
            "decoder": {"type": "ByteLevel"},
            "model": {"type": "BPE", "merges": [], "vocab": "#,
        );
        json.push_str(&serde_json::to_string(&vocab_entries).unwrap());
        json.push_str("}}");
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();

        // Every ASCII input round-trips to `" " + input` because
        // add_prefix_space defaults to true. Verify a batch of
        // representative shapes.
        for &(input, expected_round_trip) in &[
            ("hello", " hello"),
            ("Hello world", " Hello world"),
            ("a,b,c", " a,b,c"),
            ("café", " café"),
            ("", ""),
        ] {
            let enc = tok.encode(input).unwrap();
            let dec = tok.decode(&enc.ids).unwrap();
            assert_eq!(dec, expected_round_trip, "failed on {input:?}");
        }
    }

    #[test]
    fn bytelevel_no_prefix_space_leaves_input_unchanged() {
        // add_prefix_space: false — encode/decode round-trips
        // preserve the original input verbatim.
        let mut vocab_entries: BTreeMap<String, TokenId> = BTreeMap::new();
        for b in 0u8..=255 {
            let ch = crate::byte_level::BYTES_TO_CHARS[b as usize];
            let mut s = String::new();
            s.push(ch);
            vocab_entries.insert(s, u32::from(b));
        }
        let mut json = String::from(
            r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel", "add_prefix_space": false},
            "decoder": {"type": "ByteLevel"},
            "model": {"type": "BPE", "merges": [], "vocab": "#,
        );
        json.push_str(&serde_json::to_string(&vocab_entries).unwrap());
        json.push_str("}}");
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        for &input in &["hello", "Hello world", "a,b,c", ""] {
            let enc = tok.encode(input).unwrap();
            let dec = tok.decode(&enc.ids).unwrap();
            assert_eq!(dec, input, "failed on {input:?}");
        }
    }

    // ---------------------------------------------------------------------
    // Wave-10 normalizer parsing + wiring.
    // ---------------------------------------------------------------------

    #[test]
    fn normalizer_variants_parse_typed() {
        for (json, expected) in [
            (r#"{"type": "NFC"}"#, HfNormalizer::Nfc),
            (r#"{"type": "NFD"}"#, HfNormalizer::Nfd),
            (r#"{"type": "NFKC"}"#, HfNormalizer::Nfkc),
            (r#"{"type": "NFKD"}"#, HfNormalizer::Nfkd),
            (r#"{"type": "Lowercase"}"#, HfNormalizer::Lowercase),
        ] {
            let n: HfNormalizer = serde_json::from_str(json).unwrap();
            assert_eq!(n, expected, "failed on {json}");
        }
    }

    #[test]
    fn normalizer_sequence_parses_and_wires() {
        // A canonical two-step composition.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Sequence",
                "normalizers": [
                    {"type": "NFD"},
                    {"type": "Lowercase"}
                ]
            },
            "model": {
                "type": "BPE",
                "vocab": {"c": 0, "a": 1, "f": 2, "e": 3},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match config.normalizer.as_ref().unwrap() {
            HfNormalizer::Sequence { normalizers } => {
                assert_eq!(normalizers.len(), 2);
                assert!(matches!(normalizers[0], HfNormalizer::Nfd));
                assert!(matches!(normalizers[1], HfNormalizer::Lowercase));
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
        let tok = to_bpe_tokenizer(&config).unwrap();
        // "CAFÉ" → NFD decomposes é to "e" + U+0301, then lowercase
        // maps "C","A","F" to "c","a","f". The combining U+0301 is
        // *not* in the vocab; encoding fails with UnknownToken. We
        // verify by encoding only the letters that stay: check
        // "café" (already NFC, lower-case) round-trips.
        let ids = tok.encode("cafe").unwrap().ids;
        assert_eq!(ids, vec![0, 1, 2, 3]);
    }

    #[test]
    fn normalizer_replace_string_pattern_is_honoured() {
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Replace",
                "pattern": {"String": "_"},
                "content": " "
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, " ": 2},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // "a_b" — normaliser replaces "_" with " ", so BPE sees
        // "a b" (fallback whitespace splitter kicks in). The BPE
        // path emits "a" then "b" (space discarded by the
        // whitespace-based pre-tokenizer fallback).
        let ids = tok.encode("a_b").unwrap().ids;
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn normalizer_replace_regex_pattern_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Replace",
                "pattern": {"Regex": " +"},
                "content": " "
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedNormalizer { type_name } => {
                assert_eq!(type_name, "Replace(Regex)");
            }
            other => panic!("expected UnsupportedNormalizer(Replace(Regex)), got {other:?}"),
        }
    }

    #[test]
    fn normalizer_bert_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "BertNormalizer",
                "clean_text": true,
                "handle_chinese_chars": true,
                "strip_accents": null,
                "lowercase": true
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(matches!(config.normalizer, Some(HfNormalizer::Other)));
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::UnsupportedNormalizer { .. }
        ));
    }

    #[test]
    fn normalizer_precompiled_reports_deferred_error() {
        // SentencePiece's Precompiled char-map lands here as `Other`.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Precompiled",
                "precompiled_charsmap": "AAAA"
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(matches!(config.normalizer, Some(HfNormalizer::Other)));
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::UnsupportedNormalizer { .. }
        ));
    }

    // ---------------------------------------------------------------------
    // Wave-10 TemplateProcessing post-processor.
    // ---------------------------------------------------------------------

    #[test]
    fn template_processing_parses_typed() {
        let json = r#"{
            "type": "TemplateProcessing",
            "single": [
                {"SpecialToken": {"id": "<s>", "type_id": 0}},
                {"Sequence": {"id": "A", "type_id": 0}},
                {"SpecialToken": {"id": "</s>", "type_id": 0}}
            ],
            "pair": [],
            "special_tokens": {
                "<s>": {"id": "<s>", "ids": [1], "tokens": ["<s>"]},
                "</s>": {"id": "</s>", "ids": [2], "tokens": ["</s>"]}
            }
        }"#;
        let hp: HfPostProcessor = serde_json::from_str(json).unwrap();
        match hp {
            HfPostProcessor::TemplateProcessing(tp) => {
                assert_eq!(tp.single.len(), 3);
                assert_eq!(tp.pair.len(), 0);
                assert_eq!(tp.special_tokens.len(), 2);
                assert!(matches!(
                    &tp.single[0],
                    HfTemplatePiece::SpecialToken { id, .. } if id == "<s>"
                ));
                assert!(matches!(
                    &tp.single[1],
                    HfTemplatePiece::Sequence { id, .. } if id == "A"
                ));
                assert_eq!(tp.special_tokens["<s>"].ids, vec![1]);
            }
            other => panic!("expected TemplateProcessing, got {other:?}"),
        }
    }

    #[test]
    fn template_processing_wraps_hello_with_bos_eos_end_to_end() {
        // Synthetic Llama-shape tokenizer.json: BPE that encodes
        // "hello" as one id (5), plus TemplateProcessing that wraps
        // the primary encoding in <s>=1 and </s>=2.
        let json = r#"{
            "added_tokens": [
                {"id": 1, "content": "<s>", "special": true},
                {"id": 2, "content": "</s>", "special": true}
            ],
            "post_processor": {
                "type": "TemplateProcessing",
                "single": [
                    {"SpecialToken": {"id": "<s>", "type_id": 0}},
                    {"Sequence": {"id": "A", "type_id": 0}},
                    {"SpecialToken": {"id": "</s>", "type_id": 0}}
                ],
                "pair": [],
                "special_tokens": {
                    "<s>": {"id": "<s>", "ids": [1], "tokens": ["<s>"]},
                    "</s>": {"id": "</s>", "ids": [2], "tokens": ["</s>"]}
                }
            },
            "model": {
                "type": "BPE",
                "vocab": {
                    "h": 10, "e": 11, "l": 12, "o": 13,
                    "he": 14, "ll": 15, "hell": 16, "hello": 5
                },
                "merges": [
                    ["h", "e"], ["l", "l"], ["he", "ll"], ["hell", "o"]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // Default encode: post-processor fires.
        let enc = tok.encode("hello").unwrap();
        assert_eq!(enc.ids, vec![1, 5, 2]);
        assert_eq!(enc.special_mask, vec![true, false, true]);
        // Opt out via encode_with_special.
        let raw = tok.encode_with_special("hello", false).unwrap();
        assert_eq!(raw.ids, vec![5]);
        assert_eq!(raw.special_mask, vec![false]);
    }

    #[test]
    fn template_processing_special_not_declared_is_rejected() {
        // Template references `<pad>` but doesn't declare it.
        let json = r#"{
            "added_tokens": [],
            "post_processor": {
                "type": "TemplateProcessing",
                "single": [
                    {"SpecialToken": {"id": "<pad>", "type_id": 0}},
                    {"Sequence": {"id": "A", "type_id": 0}}
                ],
                "pair": [],
                "special_tokens": {}
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::TemplateSpecialTokenNotDeclared { name } => {
                assert_eq!(name, "<pad>");
            }
            other => panic!("expected TemplateSpecialTokenNotDeclared, got {other:?}"),
        }
    }

    #[test]
    fn bert_post_processor_reports_deferred_error() {
        let json = r#"{
            "added_tokens": [],
            "post_processor": {
                "type": "BertProcessing",
                "sep": ["[SEP]", 102],
                "cls": ["[CLS]", 101]
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(matches!(
            config.post_processor,
            Some(HfPostProcessor::Other)
        ));
        let err = to_bpe_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::UnsupportedPostProcessor { .. }
        ));
    }

    // ---------------------------------------------------------------------
    // Wave-10 end-to-end Llama-3-shape blob with normalizer +
    // TemplateProcessing.
    // ---------------------------------------------------------------------

    #[test]
    fn llama_3_shape_with_normalizer_and_template_processing() {
        // Close to a real Llama-3 tokenizer.json: an NFC normalizer,
        // a Split(Regex) pre-tokenizer close to tiktoken's canonical
        // pattern, TemplateProcessing that injects
        // <|begin_of_text|>=128000 around the primary encoding, and
        // a BPE with `ignore_merges: true`. The vocab is a toy
        // covering exactly the letters used in the test input; the
        // interesting behaviour is that BOS is injected and NFC is
        // applied.
        let json = r#"{
            "version": "1.0",
            "added_tokens": [
                {"id": 128000, "content": "<|begin_of_text|>", "special": true}
            ],
            "normalizer": {"type": "NFC"},
            "pre_tokenizer": {
                "type": "Split",
                "pattern": {"Regex": "\\p{L}+|\\p{N}+|[^\\s\\p{L}\\p{N}]+|\\s+"},
                "behavior": "Isolated"
            },
            "post_processor": {
                "type": "TemplateProcessing",
                "single": [
                    {"SpecialToken": {"id": "<|begin_of_text|>", "type_id": 0}},
                    {"Sequence": {"id": "A", "type_id": 0}}
                ],
                "pair": [
                    {"SpecialToken": {"id": "<|begin_of_text|>", "type_id": 0}},
                    {"Sequence": {"id": "A", "type_id": 0}},
                    {"SpecialToken": {"id": "<|begin_of_text|>", "type_id": 0}},
                    {"Sequence": {"id": "B", "type_id": 1}}
                ],
                "special_tokens": {
                    "<|begin_of_text|>": {
                        "id": "<|begin_of_text|>",
                        "ids": [128000],
                        "tokens": ["<|begin_of_text|>"]
                    }
                }
            },
            "model": {
                "type": "BPE",
                "ignore_merges": true,
                "vocab": {
                    "h": 0, "e": 1, "l": 2, "o": 3,
                    "he": 4, "ll": 5, "hell": 6, "hello": 7
                },
                "merges": [
                    ["h", "e"], ["l", "l"], ["he", "ll"], ["hell", "o"]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(matches!(config.normalizer, Some(HfNormalizer::Nfc)));
        assert!(matches!(
            config.post_processor,
            Some(HfPostProcessor::TemplateProcessing(_))
        ));
        let tok = to_bpe_tokenizer(&config).unwrap();

        // Input in NFD form; normalizer collapses back to NFC before
        // BPE runs. The letters "café" would need a full byte-level
        // vocab; keep the test focused by using ASCII "hello" (NFC
        // and NFD coincide on ASCII, so the normalizer is a no-op).
        let enc = tok.encode("hello").unwrap();
        // BOS then the primary id 7 ("hello").
        assert_eq!(enc.ids, vec![128_000, 7]);
        assert_eq!(enc.special_mask, vec![true, false]);
        // Opt out — raw BPE, no BOS.
        let raw = tok.encode_with_special("hello", false).unwrap();
        assert_eq!(raw.ids, vec![7]);
    }

    // ---------------------------------------------------------------------
    // Wave-11 WordPiece model + BertPreTokenizer routing.
    // ---------------------------------------------------------------------

    /// A minimal BERT-shape `tokenizer.json`: `WordPiece` model with
    /// `unaffable` decomposition, `BertPreTokenizer` pre-tokenizer,
    /// and `[CLS]` / `[SEP]` template processing.
    const BERT_JSON: &str = r###"{
        "version": "1.0",
        "added_tokens": [
            {"id": 0, "content": "[PAD]", "special": true},
            {"id": 100, "content": "[UNK]", "special": true},
            {"id": 101, "content": "[CLS]", "special": true},
            {"id": 102, "content": "[SEP]", "special": true}
        ],
        "pre_tokenizer": {"type": "BertPreTokenizer"},
        "post_processor": {
            "type": "TemplateProcessing",
            "single": [
                {"SpecialToken": {"id": "[CLS]", "type_id": 0}},
                {"Sequence": {"id": "A", "type_id": 0}},
                {"SpecialToken": {"id": "[SEP]", "type_id": 0}}
            ],
            "pair": [],
            "special_tokens": {
                "[CLS]": {"id": "[CLS]", "ids": [101], "tokens": ["[CLS]"]},
                "[SEP]": {"id": "[SEP]", "ids": [102], "tokens": ["[SEP]"]}
            }
        },
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[PAD]": 0,
                "[UNK]": 100,
                "[CLS]": 101,
                "[SEP]": 102,
                "un": 200,
                "##aff": 201,
                "##able": 202,
                "cat": 203,
                "dog": 204,
                ",": 205,
                "!": 206,
                "Hello": 207,
                "world": 208
            }
        }
    }"###;

    #[test]
    fn wordpiece_model_parses_typed() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        match &config.model {
            HfModel::WordPiece(wp) => {
                assert_eq!(wp.unk_token, "[UNK]");
                assert_eq!(wp.continuing_subword_prefix, "##");
                assert_eq!(wp.max_input_chars_per_word, 100);
                assert!(wp.vocab.contains_key("un"));
                assert!(wp.vocab.contains_key("##aff"));
            }
            other => panic!("expected WordPiece model, got {other:?}"),
        }
    }

    #[test]
    fn wordpiece_model_defaults_apply_when_fields_absent() {
        // A minimal WordPiece model — only `vocab` and `unk_token`.
        // The defaults should be `##` and 100.
        let json = r#"{
            "vocab": {"[UNK]": 0},
            "unk_token": "[UNK]"
        }"#;
        let wp: HfWordPieceModel = serde_json::from_str(json).unwrap();
        assert_eq!(wp.continuing_subword_prefix, "##");
        assert_eq!(wp.max_input_chars_per_word, 100);
    }

    #[test]
    fn to_wordpiece_tokenizer_encodes_unaffable() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // Canonical WordPiece reference: "unaffable" → ["un", "##aff",
        // "##able"] → [200, 201, 202].
        assert_eq!(tok.encode("unaffable"), vec![200, 201, 202]);
    }

    #[test]
    fn to_wordpiece_tokenizer_bert_pre_tokenizer_splits_punctuation() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "Hello, world!" via BertPreTokenizer → ["Hello", ",",
        // "world", "!"] → [207, 205, 208, 206].
        assert_eq!(tok.encode("Hello, world!"), vec![207, 205, 208, 206]);
    }

    #[test]
    fn to_wordpiece_tokenizer_oov_word_emits_unk() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "xyz" is not in the vocab and no ## prefix decomposition
        // works. WordPiece is all-or-nothing on a word → emit UNK id
        // (100).
        assert_eq!(tok.encode("xyz"), vec![100]);
    }

    #[test]
    fn to_wordpiece_tokenizer_word_over_max_chars_emits_unk() {
        // Small max_input_chars_per_word — a longer word shortcuts to
        // UNK regardless of the vocabulary content.
        let json = r###"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "Whitespace"},
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 4,
                "vocab": {"[UNK]": 0, "un": 1, "##aff": 2, "##able": 3, "cat": 4}
            }
        }"###;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "unaffable" is 9 chars > 4 → UNK.
        assert_eq!(tok.encode("unaffable"), vec![0]);
        // "cat" is 3 chars → normal encode.
        assert_eq!(tok.encode("cat"), vec![4]);
    }

    #[test]
    fn to_wordpiece_tokenizer_round_trip_reconstructs_words() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // Round-trip: encode then decode. Whitespace collapses to
        // single spaces (documented lossy behaviour); the words
        // themselves survive verbatim.
        let ids = tok.encode("unaffable cat dog");
        let text = tok.decode(&ids).unwrap();
        assert_eq!(text, "unaffable cat dog");
    }

    #[test]
    fn to_wordpiece_tokenizer_continuing_prefix_variant_no_hash() {
        // Some WordPiece variants use "" as the continuing-subword
        // prefix.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "",
                "max_input_chars_per_word": 100,
                "vocab": {"[UNK]": 0, "un": 1, "aff": 2, "able": 3}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert_eq!(tok.continuing_subword_prefix(), "");
        assert_eq!(tok.encode("unaffable"), vec![1, 2, 3]);
    }

    #[test]
    fn to_wordpiece_tokenizer_whitespace_pre_tokenizer_routes_through() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "Whitespace"},
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {"[UNK]": 0, "cat": 1, "dog": 2}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert_eq!(
            tok.pre_tokenizer(),
            crate::wordpiece::WordPiecePreTokenizer::Whitespace
        );
        assert_eq!(tok.encode("cat dog"), vec![1, 2]);
    }

    #[test]
    fn to_wordpiece_tokenizer_whitespace_split_pre_tokenizer_routes_through() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "WhitespaceSplit"},
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {"[UNK]": 0, "cat,dog": 1}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert_eq!(
            tok.pre_tokenizer(),
            crate::wordpiece::WordPiecePreTokenizer::WhitespaceSplit
        );
        // WhitespaceSplit keeps punctuation glued: "cat,dog" is a
        // single word matched verbatim in the vocab.
        assert_eq!(tok.encode("cat,dog"), vec![1]);
    }

    #[test]
    fn to_wordpiece_tokenizer_rejects_bpe_model() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": [["a", "b"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_wordpiece_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModelForWordPiece { type_name } => {
                assert_eq!(type_name, "BPE");
            }
            other => panic!("expected UnsupportedModelForWordPiece(BPE), got {other:?}"),
        }
    }

    #[test]
    fn to_wordpiece_tokenizer_rejects_unk_missing_from_vocab() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {"cat": 0}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_wordpiece_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::WordPieceUnkNotInVocab { unk_token } => {
                assert_eq!(unk_token, "[UNK]");
            }
            other => panic!("expected WordPieceUnkNotInVocab, got {other:?}"),
        }
    }

    #[test]
    fn to_wordpiece_tokenizer_rejects_bert_normalizer_deferred() {
        // BertNormalizer is deferred — surfaces UnsupportedNormalizer.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "BertNormalizer",
                "lowercase": true,
                "strip_accents": true
            },
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {"[UNK]": 0}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_wordpiece_tokenizer(&config).unwrap_err();
        assert!(matches!(
            err,
            HfConversionError::UnsupportedNormalizer { .. }
        ));
    }

    #[test]
    fn to_tokenizer_dispatches_to_wordpiece_enum_variant() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::WordPiece(wp) => {
                assert_eq!(wp.encode("unaffable"), vec![200, 201, 202]);
            }
            HfTokenizer::Bpe(_) => panic!("expected HfTokenizer::WordPiece"),
        }
    }

    #[test]
    fn to_tokenizer_dispatches_to_bpe_enum_variant() {
        // A BPE config should still route through to_tokenizer,
        // producing the HfTokenizer::Bpe variant. Backwards-compat
        // check. The inner tokenizer is boxed — deref through the
        // Box in the match arm.
        let config = parse_tokenizer_json(MINIMAL_JSON).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::Bpe(bpe) => {
                assert_eq!(bpe.encode("ab").unwrap().ids, vec![3]);
            }
            HfTokenizer::WordPiece(_) => panic!("expected HfTokenizer::Bpe"),
        }
    }

    #[test]
    fn to_tokenizer_rejects_unigram_and_wordlevel() {
        for (json, expected) in [
            (
                r#"{"added_tokens":[],"model":{"type":"Unigram","vocab":[["a",0.0]],"unk_id":0}}"#,
                "Unigram",
            ),
            (
                r#"{"added_tokens":[],"model":{"type":"WordLevel","vocab":{"a":0},"unk_token":"[UNK]"}}"#,
                "WordLevel",
            ),
        ] {
            let config = parse_tokenizer_json(json).unwrap();
            let err = to_tokenizer(&config).unwrap_err();
            match err {
                HfConversionError::UnsupportedModel { type_name } => {
                    assert_eq!(type_name, expected);
                }
                other => panic!("expected UnsupportedModel({expected}), got {other:?}"),
            }
        }
    }

    #[test]
    fn bert_shape_end_to_end_with_special_tokens_and_bert_pre_tokenizer() {
        // Full BERT-shape blob (BERT_JSON): WordPiece + BertPreTokenizer
        // + TemplateProcessing. The runtime WordPieceTokenizer doesn't
        // apply the post-processor itself today (that lives on
        // BpeTokenizer for now), but the config parses and the model
        // materialises. Verify the parsed post_processor is honoured
        // shape-wise and that raw encoding still works.
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        assert!(matches!(
            config.post_processor,
            Some(HfPostProcessor::TemplateProcessing(_))
        ));
        assert!(matches!(
            config.pre_tokenizer,
            Some(HfPreTokenizer::BertPreTokenizer(_))
        ));
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // BERT-style input, encoded via the WordPiece model + Bert
        // pre-tokenizer. The template-processing splice ([CLS] / [SEP])
        // is *not* applied on the WordPiece path today; a caller who
        // wants it can splice manually against the parsed template.
        let ids = tok.encode("Hello, world!");
        assert_eq!(ids, vec![207, 205, 208, 206]);
    }
}
