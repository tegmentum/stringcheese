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
//! * `model.type == "WordLevel"` — a separate algorithm landing, out of
//!   scope for the current wave. `Unigram` is materialised via
//!   [`to_unigram_tokenizer`] into a [`UnigramTokenizer`] whose
//!   `encode` runs the Viterbi forward-DP over the vocabulary's log
//!   probabilities; see that type's docs for the algorithm.
//! * `pre_tokenizer.type == "Metaspace"` parses into typed fields
//!   ([`HfPreTokenizer::Metaspace`]) and can be materialised via
//!   [`to_runtime_metaspace`] into a runtime [`Metaspace`] that
//!   callers can drive themselves against a `Unigram` tokenizer; it
//!   is not wired into [`to_bpe_tokenizer`] (BPE has its own
//!   byte-level pipeline).
//! * All other `pre_tokenizer` types (`Whitespace`,
//!   `WhitespaceSplit`, `Punctuation`, `CharDelimiterSplit`,
//!   `BertPreTokenizer`, `Digits`, `UnicodeScripts`, ...).
//! * All other `decoder` types (`WordPiece`, `Metaspace`, `BPEDecoder`,
//!   `Sequence`, ...). The raw config is preserved on
//!   [`HfTokenizerConfig::decoder`] for caller inspection.
//! * `normalizer` — the honoured shapes (NFC / NFD / NFKC / NFKD /
//!   `Sequence` / `Lowercase` / `Replace(String)` / `Strip` /
//!   `Prepend` / `BertNormalizer` / `Precompiled`) are materialised
//!   at [`to_bpe_tokenizer`] time and applied on every `encode` call;
//!   every other tag string surfaces
//!   [`HfConversionError::UnsupportedNormalizer`]. See
//!   [`HfNormalizer`] for the exhaustive list.
//! * `post_processor` — [`HfPostProcessor::TemplateProcessing`]
//!   (Llama / BERT shape) and [`HfPostProcessor::ByteLevel`] (GPT-2
//!   shape; no-op on the encoding this crate ships — see
//!   [`PostProcessor::ByteLevel`] for the rationale) are materialised
//!   at conversion; every other tag string surfaces
//!   [`HfConversionError::UnsupportedPostProcessor`].
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
use crate::post_processor::{
    PostProcessor, RobertaProcessing, SpecialTokenInfo, TemplatePiece, TemplateProcessing,
};
use crate::pre_tokenizer::{
    GPT2_PATTERN, Metaspace, PreTokenizerCompileError, PrependScheme, RegexPreTokenizer,
};

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
    /// [`HfPostProcessor`] value. Honoured at conversion:
    /// [`HfPostProcessor::TemplateProcessing`] (Llama / BERT shape)
    /// and [`HfPostProcessor::ByteLevel`] (GPT-2 shape — see
    /// [`PostProcessor::ByteLevel`] for the no-op-on-offsets
    /// policy this crate applies). Every other tag string falls
    /// through to [`HfPostProcessor::Other`] and produces
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
    /// A SentencePiece-style `Unigram` language model — Kudo (2018),
    /// used by Llama, Mistral, T5, and XLM-RoBERTa. See
    /// [`HfUnigramModel`] for the fields and [`UnigramTokenizer`] for
    /// the runtime.
    #[serde(rename = "Unigram")]
    Unigram(HfUnigramModel),
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

/// The Unigram-specific fields of a `model` block.
///
/// `SentencePiece`'s Unigram language model — Kudo (2018) — stores its
/// vocabulary as an ordered array of `(surface, log_probability)`
/// pairs. The token id emitted for any given piece is that piece's
/// zero-based index in [`Self::vocab`].
///
/// `unk_id`, when present, is the index of a fallback token used
/// whenever a segment of the input cannot be covered by any vocab
/// entry: the runtime advances one character and emits `unk_id`. A
/// config without `unk_id` will surface
/// [`UnigramEncodeError::UntokenizableChar`] for such inputs.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HfUnigramModel {
    /// The vocabulary, in id order. Each entry is a `(surface,
    /// log_probability)` pair; the index in this vec is the token id.
    pub vocab: Vec<(String, f64)>,
    /// Optional fallback token id (usually the index of `"<unk>"` in
    /// [`Self::vocab`]).
    #[serde(default)]
    pub unk_id: Option<usize>,
    /// Whether byte-fallback is enabled. Preserved but not applied —
    /// callers who need `SentencePiece`'s byte-fallback behaviour
    /// should implement the mapping themselves.
    #[serde(default)]
    pub byte_fallback: Option<bool>,
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
    /// SentencePiece-style Metaspace pre-tokenizer — the shape used by
    /// Llama, Mistral, T5, and XLM-RoBERTa. Parsed into typed fields
    /// (see [`Self::Metaspace::replacement`] /
    /// [`Self::Metaspace::prepend_scheme`] /
    /// [`Self::Metaspace::split`]). Materialised via
    /// [`to_runtime_metaspace`] into a runtime
    /// [`Metaspace`]; a caller who wants to apply
    /// it on a Unigram tokenizer today does so by driving the
    /// returned `Metaspace` themselves. `to_bpe_tokenizer` still
    /// rejects a Metaspace pre-tokenizer at conversion time — BPE has
    /// its own byte-level pipeline.
    Metaspace {
        /// The character substituted for ASCII space. HF's default
        /// (and what a bare `{"type": "Metaspace"}` block picks up)
        /// is `▁` (U+2581).
        #[serde(default = "default_metaspace_replacement")]
        replacement: char,
        /// The prepend policy. HF's default (and what a bare block
        /// picks up) is [`HfPrependScheme::Always`].
        #[serde(default)]
        prepend_scheme: HfPrependScheme,
        /// Whether to split the transformed string on `replacement`.
        /// HF's default is `true`.
        #[serde(default = "default_true")]
        split: bool,
    },
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

/// Default `replacement` for a bare `{"type": "Metaspace"}` block —
/// `▁` (U+2581 LOWER ONE EIGHTH BLOCK), the canonical `SentencePiece`
/// space mark.
const fn default_metaspace_replacement() -> char {
    Metaspace::DEFAULT_REPLACEMENT
}

/// The prepend policy for a [`HfPreTokenizer::Metaspace`] block.
///
/// Mirrors Hugging Face's on-disk shape: the JSON string `"always"`
/// / `"never"` / `"first"` deserialises to the corresponding variant.
/// Default is [`Self::Always`], HF's own default.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum HfPrependScheme {
    /// Always prepend the replacement character to the input.
    #[default]
    Always,
    /// Never prepend.
    Never,
    /// Prepend only if the input does not already start with the
    /// replacement.
    First,
}

impl From<HfPrependScheme> for PrependScheme {
    fn from(v: HfPrependScheme) -> Self {
        match v {
            HfPrependScheme::Always => Self::Always,
            HfPrependScheme::Never => Self::Never,
            HfPrependScheme::First => Self::First,
        }
    }
}

/// The `normalizer` block of a `tokenizer.json` file.
///
/// Only the variants named explicitly here are honoured at
/// [`to_bpe_tokenizer`] time — every unrecognised tag string falls
/// through to [`Self::Other`]. See [`Normalizer`] for the semantics
/// of the honoured shapes.
///
/// [`Self::Precompiled`] parses successfully so real `SentencePiece`
/// `tokenizer.json` blobs load; its runtime pass is a passthrough
/// today — see [`Normalizer::Precompiled`] for the limitation.
///
/// Deferred variants (`Nmt`, regex `Replace`, custom callables)
/// surface at conversion as
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
    /// BERT's classic composite normalizer — control-char cleanup,
    /// CJK spacing, accent stripping, and lower-casing. Materialises
    /// to [`Normalizer::Bert`] on the runtime side. Ships as a single
    /// tag (`"BertNormalizer"`) with four boolean toggles; every
    /// toggle has an HF-canonical default so a bare `{"type":
    /// "BertNormalizer"}` deserialises to the BERT-base shape
    /// (`clean_text: true, handle_chinese_chars: true, strip_accents:
    /// None, lowercase: true`).
    BertNormalizer {
        /// See [`Normalizer::Bert::clean_text`]. Defaults to `true`.
        #[serde(default = "default_true")]
        clean_text: bool,
        /// See [`Normalizer::Bert::handle_chinese_chars`]. Defaults
        /// to `true`.
        #[serde(default = "default_true")]
        handle_chinese_chars: bool,
        /// See [`Normalizer::Bert::strip_accents`]. Defaults to
        /// `None` (which the runtime resolves against
        /// [`Self::BertNormalizer::lowercase`]).
        #[serde(default)]
        strip_accents: Option<bool>,
        /// See [`Normalizer::Bert::lowercase`]. Defaults to `true`.
        #[serde(default = "default_true")]
        lowercase: bool,
    },
    /// `SentencePiece`'s "Precompiled" charsmap normalizer — Llama,
    /// Mistral, T5, and XLM-RoBERTa all ship one, usually inside a
    /// `Sequence` alongside `Prepend` and `Replace`. The
    /// [`Self::Precompiled::precompiled_charsmap`] field carries the
    /// raw base64-encoded payload verbatim.
    ///
    /// # Runtime behaviour
    ///
    /// Materialises into [`Normalizer::Precompiled`], whose apply
    /// step is **currently a passthrough** — see that variant's
    /// doc-comment for the limitation. The parse-time variant exists
    /// so real `tokenizer.json` blobs stop failing at load time.
    Precompiled {
        /// The raw base64 payload from the source `tokenizer.json`.
        /// Not decoded or interpreted at parse time; the string is
        /// preserved verbatim so callers who need the real charsmap
        /// can decode it themselves.
        precompiled_charsmap: String,
    },
    /// Any other normalizer tag (NMT, custom, ...). Recognised at
    /// parse time via serde's `#[serde(other)]` so parsing does not
    /// fail; [`to_bpe_tokenizer`] rejects it with a specific error.
    #[serde(other)]
    Other,
}

/// The `post_processor` block of a `tokenizer.json` file.
///
/// Honoured at [`to_bpe_tokenizer`] time: [`Self::TemplateProcessing`]
/// (Llama / BERT shape) and [`Self::ByteLevel`] (GPT-2 shape). Every
/// other tag string falls through to [`Self::Other`] via serde's
/// `#[serde(other)]` and surfaces
/// [`HfConversionError::UnsupportedPostProcessor`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfPostProcessor {
    /// The Llama-shape template that injects BOS/EOS around the
    /// primary encoding. See [`TemplateProcessing`] for the semantics.
    TemplateProcessing(HfTemplateProcessing),
    /// The `XLM-RoBERTa` / `RoBERTa` `RobertaProcessing` shape — a
    /// fixed `[CLS] $A [SEP]` splice. Both `cls` and `sep` are the
    /// on-disk `[surface_string, id]` two-element tuples HF stores.
    /// See [`RobertaProcessing`] for the runtime semantics.
    RobertaProcessing(HfRobertaProcessing),
    /// The GPT-2-shape `ByteLevel` post-processor. Every field has
    /// an HF-canonical serde default (`add_prefix_space: true`,
    /// `trim_offsets: true`, `use_regex: true`), so a bare `{"type":
    /// "ByteLevel"}` blob deserialises to the GPT-2 baseline. The
    /// three fields are captured verbatim on the parsed value;
    /// see [`PostProcessor::ByteLevel`] for the no-op-on-encoding
    /// policy this crate applies at process time.
    ByteLevel {
        /// Preserved for caller inspection. Not applied at process
        /// time — see [`PostProcessor::ByteLevel::add_prefix_space`].
        #[serde(default = "default_true")]
        add_prefix_space: bool,
        /// Preserved for caller inspection. Not applied — see
        /// [`PostProcessor::ByteLevel::trim_offsets`].
        #[serde(default = "default_true")]
        trim_offsets: bool,
        /// Preserved for caller inspection. Not applied — see
        /// [`PostProcessor::ByteLevel::use_regex`].
        #[serde(default = "default_true")]
        use_regex: bool,
    },
    /// Any other post-processor (`BertProcessing`, `Sequence`, ...).
    /// Rejected at conversion time.
    #[serde(other)]
    Other,
}

/// The typed shape of a `RobertaProcessing` post-processor.
///
/// Field names mirror HF's on-disk layout verbatim. `sep` and `cls`
/// are the two-element `[surface_string, id]` tuples HF writes;
/// `trim_offsets` and `add_prefix_space` default to `true` — the
/// values every real `XLM-RoBERTa` / `RoBERTa` checkpoint on the Hub
/// ships.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfRobertaProcessing {
    /// The `[surface_string, id]` tuple emitted at the SEP slot after
    /// the primary encoding.
    pub sep: HfRobertaSpecial,
    /// The `[surface_string, id]` tuple emitted at the CLS slot before
    /// the primary encoding.
    pub cls: HfRobertaSpecial,
    /// Whether HF would trim ByteLevel-space offsets on the output.
    /// Preserved but not applied — see the runtime
    /// [`RobertaProcessing::trim_offsets`] doc for why.
    #[serde(default = "default_true")]
    pub trim_offsets: bool,
    /// Whether HF would insert a leading space before the primary
    /// text. Preserved but not applied — see the runtime
    /// [`RobertaProcessing::add_prefix_space`] doc for why.
    #[serde(default = "default_true")]
    pub add_prefix_space: bool,
}

/// The `[surface_string, id]` pair HF stores under `sep` / `cls` in a
/// `RobertaProcessing` block. Deserialised via a serde
/// two-element-tuple newtype so `["<s>", 0]` parses directly.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HfRobertaSpecial(pub String, pub TokenId);

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
    /// [`to_unigram_tokenizer`] was called on a config whose
    /// `model.type` is not `"Unigram"`. Carries the specific type
    /// name.
    UnsupportedModelForUnigram {
        /// The `model.type` string from the source config.
        type_name: String,
    },
    /// A `Unigram` config's `unk_id` points past the end of its
    /// vocabulary — the config is internally inconsistent.
    UnigramUnkIdOutOfRange {
        /// The declared `unk_id`.
        unk_id: usize,
        /// The vocabulary size.
        vocab_size: usize,
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
    /// materialise yet (`Nmt`, a `Replace` with a `Regex` pattern, or
    /// a custom callable). `Precompiled` is honoured as a passthrough
    /// and does not surface here.
    UnsupportedNormalizer {
        /// The `normalizer.type` string, or a short synthesised name
        /// for a nested rejection (`"Replace(Regex)"`, ...).
        type_name: String,
    },
    /// The `post_processor` block used a variant this crate does not
    /// materialise yet (`BertProcessing`, `RobertaProcessing`,
    /// `Sequence`).
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
            Self::UnsupportedModelForUnigram { type_name } => write!(
                f,
                "to_unigram_tokenizer called on non-Unigram model type {type_name:?}; \
                 use to_bpe_tokenizer, to_wordpiece_tokenizer, or to_tokenizer instead"
            ),
            Self::UnigramUnkIdOutOfRange { unk_id, vocab_size } => write!(
                f,
                "Unigram model's unk_id {unk_id} is out of range for a vocabulary of size {vocab_size}"
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
                 Replace(String), Strip, Prepend, BertNormalizer, \
                 Precompiled (passthrough), and their Sequence composition"
            ),
            Self::UnsupportedPostProcessor { type_name } => write!(
                f,
                "unsupported HF post_processor type {type_name:?}: \
                 this crate materialises \"TemplateProcessing\", \
                 \"RobertaProcessing\", and \"ByteLevel\" \
                 (BertProcessing/Sequence are deferred to later landings)"
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
/// use stringcheese_tokenizer_hf::hf::parse_tokenizer_json;
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
/// use stringcheese_tokenizer_hf::hf::{parse_tokenizer_json, to_bpe_tokenizer};
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
            return Err(HfConversionError::UnsupportedModelForBpe {
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
/// * [`HfTokenizer::Unigram`] wraps a [`UnigramTokenizer`] — every
///   `model.type == "Unigram"` config (Llama, Mistral, T5,
///   XLM-RoBERTa) lands here.
///
/// `WordLevel` is deferred; [`to_tokenizer`] rejects it with
/// [`HfConversionError::UnsupportedModel`].
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
    /// A [`UnigramTokenizer`] materialised from a `Unigram` model.
    Unigram(UnigramTokenizer),
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
/// use stringcheese_tokenizer_hf::hf::{HfTokenizer, parse_tokenizer_json, to_tokenizer};
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
        HfModel::Unigram(_) => Ok(HfTokenizer::Unigram(to_unigram_tokenizer(config)?)),
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
/// Ancillary features applied on top of the parsed model:
///
/// * `normalizer` — most `WordPiece` checkpoints ship a
///   `BertNormalizer` (lower-case + accent strip + Chinese-char
///   handling). The runtime materialises it (see
///   [`Normalizer::Bert`]) and attaches it to the produced
///   [`crate::wordpiece::WordPieceTokenizer`] via
///   [`crate::wordpiece::WordPieceTokenizer::with_normalizer`], so
///   `encode` runs `normalize -> pre-tokenize -> WordPiece` end to
///   end. NFC / NFD / NFKC / NFKD / Lowercase / Strip / Prepend /
///   `Replace(String)` / their `Sequence` composition are honoured
///   the same way; deferred variants (`Replace(Regex)`, `Nmt`, ...)
///   surface [`HfConversionError::UnsupportedNormalizer`].
/// * `post_processor` — `WordPiece` checkpoints usually ship
///   `TemplateProcessing` (for `[CLS]` / `[SEP]`); that shape is
///   honoured verbatim and attached via
///   [`crate::wordpiece::WordPieceTokenizer::with_post_processor`],
///   so the templated ids appear on the encoding this function's
///   caller receives. Deferred variants (`BertProcessing`, etc.)
///   surface [`HfConversionError::UnsupportedPostProcessor`].
///
/// **Deferred** ancillary features (parse but reject at conversion):
///
/// * `decoder` — the raw config is preserved on
///   [`HfTokenizerConfig::decoder`] for caller inspection but not
///   applied; `WordPieceDecoder` semantics live inside
///   [`crate::wordpiece::WordPieceTokenizer::decode`] regardless of
///   what the config declares.
///
/// # Errors
///
/// Returns [`HfConversionError`] with a variant naming the offending
/// feature. Common causes: a non-`WordPiece` `model.type`, an
/// unrecognised `normalizer.type`, or an `unk_token` that is not
/// present in the vocabulary.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_hf::hf::{parse_tokenizer_json, to_wordpiece_tokenizer};
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

    // Normalizer — runs before the pre-tokenizer at encode time. The
    // shared `to_runtime_normalizer` honours NFC / NFD / NFKC / NFKD /
    // Lowercase / Strip / Prepend / Replace(String) / BertNormalizer /
    // their Sequence composition; everything else surfaces as
    // `UnsupportedNormalizer`. Mirrors `to_bpe_tokenizer`'s wiring —
    // the runtime `WordPieceTokenizer` now carries a normalizer slot,
    // so the parsed value is attached and applied end-to-end.
    if let Some(hn) = &config.normalizer {
        let n = to_runtime_normalizer(hn)?;
        tok = tok.with_normalizer(n);
    }

    // Post-processor — runs on the finished encoding before it leaves
    // `encode`. `TemplateProcessing` is honoured (the shape every
    // BERT-family checkpoint uses for `[CLS]` / `[SEP]`); every other
    // variant surfaces as `UnsupportedPostProcessor`. Mirrors
    // `to_bpe_tokenizer` — the runtime `WordPieceTokenizer` now
    // carries a post-processor slot, so the parsed value is attached
    // and applied end-to-end.
    if let Some(hp) = &config.post_processor {
        let pp = to_runtime_post_processor(hp)?;
        tok = tok.with_post_processor(pp);
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
// Unigram runtime.
// ---------------------------------------------------------------------

/// Runtime for a SentencePiece-style `Unigram` language model — Kudo
/// (2018). Constructed via [`to_unigram_tokenizer`] from a parsed
/// [`HfTokenizerConfig`].
///
/// # Algorithm
///
/// [`Self::encode`] runs a Viterbi forward dynamic-programming pass
/// over character positions of the UTF-8 input. For each position `i`
/// (0-indexed over Unicode scalar values, with `n` being the total
/// character count), we compute the best score reachable at `i` as
///
/// ```text
/// best[i] = max over j < i of best[j] + score(input[j..i])
/// ```
///
/// where `input[j..i]` is the substring covering character positions
/// `j` through `i` and must be present in the vocabulary. `best[0]`
/// starts at `0.0`; the final best segmentation is recovered by
/// backtracking from `best[n]` through the stored predecessors.
///
/// # Unknown-token fallback
///
/// When no vocabulary entry can reach a position `i` from any earlier
/// reachable position, and [`Self::unk_id`] is `Some`, the runtime
/// falls back to a single-character `unk` transition from `i - 1` to
/// `i`, scored by the `unk` token's own log probability minus a fixed
/// penalty. The penalty is chosen large enough that the fallback is
/// only ever preferred when no vocab-only path exists.
///
/// If `unk_id` is `None` and a position is unreachable,
/// [`Self::encode`] returns
/// [`UnigramEncodeError::UntokenizableChar`] pointing at the offending
/// character.
///
/// # Complexity
///
/// `O(n · m)` where `n` is the character count and `m` is the length
/// of the longest vocabulary entry (capped by the input length).
#[derive(Debug, Clone)]
pub struct UnigramTokenizer {
    /// The vocabulary in id order — `vocab[id]` gives the surface
    /// string and its log probability.
    vocab: Vec<(String, f64)>,
    /// Surface string ↔ (id, score) lookup used by the Viterbi loop.
    lookup: BTreeMap<String, (usize, f64)>,
    /// Optional fallback token id.
    unk_id: Option<usize>,
    /// Precomputed penalty added on top of the `unk` score when the
    /// fallback fires. Chosen so a vocab-only path always wins when
    /// one exists.
    unk_penalty: f64,
    /// Optional Unicode normalizer applied to the raw input string
    /// *before* the pre-tokenizer runs. Matches HF `tokenizers-rs`'
    /// pipeline order (`normalize -> pre-tokenize -> Unigram ->
    /// post-process`) and mirrors [`BpeTokenizer::with_normalizer`] /
    /// [`crate::wordpiece::WordPieceTokenizer::with_normalizer`]. A
    /// value of `None` leaves the input unchanged.
    normalizer: Option<Normalizer>,
    /// Optional `SentencePiece` Metaspace pre-tokenizer applied to
    /// the normalized text. When set, each piece produced by
    /// [`Metaspace::apply`] is fed through the Viterbi loop
    /// independently, and the resulting ids are concatenated in
    /// order. When `None` the Viterbi loop runs on the whole
    /// normalized string as one piece — matching the pre-composition
    /// behaviour this type shipped with.
    pre_tokenizer: Option<Metaspace>,
    /// Optional post-processor applied to the finished [`Encoding`]
    /// before it leaves [`Self::encode`]. Mirrors
    /// [`BpeTokenizer::with_post_processor`]; the default
    /// [`PostProcessor::None`] is a pass-through so callers who never
    /// configure one see the raw Viterbi output.
    post_processor: PostProcessor,
}

impl UnigramTokenizer {
    /// Assemble a [`UnigramTokenizer`] from raw vocabulary and
    /// optional `unk_id`. Public so callers who already have the
    /// pieces in hand (e.g. from a hand-written vocab) can build a
    /// tokenizer without going through the JSON parser.
    ///
    /// # Errors
    ///
    /// Returns [`HfConversionError::UnigramUnkIdOutOfRange`] if
    /// `unk_id` is `Some` and points past the end of `vocab`.
    pub fn from_parts(
        vocab: Vec<(String, f64)>,
        unk_id: Option<usize>,
    ) -> Result<Self, HfConversionError> {
        if let Some(u) = unk_id {
            if u >= vocab.len() {
                return Err(HfConversionError::UnigramUnkIdOutOfRange {
                    unk_id: u,
                    vocab_size: vocab.len(),
                });
            }
        }
        let mut lookup: BTreeMap<String, (usize, f64)> = BTreeMap::new();
        for (id, (surface, score)) in vocab.iter().enumerate() {
            // Later duplicates lose to earlier ones — matches HF's
            // "index of first occurrence wins" convention for the id
            // lookup direction.
            lookup.entry(surface.clone()).or_insert((id, *score));
        }
        // Penalty: make the `unk` fallback strictly worse than any
        // vocab-only path could ever be. `10.0` in log space is
        // SentencePiece's own default and comfortably larger than the
        // absolute value of any real vocab score.
        let unk_penalty = 10.0;
        Ok(Self {
            vocab,
            lookup,
            unk_id,
            unk_penalty,
            normalizer: None,
            pre_tokenizer: None,
            post_processor: PostProcessor::None,
        })
    }

    /// Attach (or replace) the Unicode normalizer.
    ///
    /// The normalizer runs on the raw input string *before* the
    /// pre-tokenizer, matching HF `tokenizers-rs`' pipeline order:
    /// `normalize -> pre-tokenize -> Unigram -> post-process`. See
    /// [`Normalizer`] for the supported variants. Mirrors
    /// [`BpeTokenizer::with_normalizer`] and
    /// [`crate::wordpiece::WordPieceTokenizer::with_normalizer`].
    #[must_use]
    pub fn with_normalizer(mut self, normalizer: Normalizer) -> Self {
        self.normalizer = Some(normalizer);
        self
    }

    /// Attach (or replace) the `SentencePiece` Metaspace pre-tokenizer.
    ///
    /// When set, [`Self::encode`] splits the normalized input into
    /// Metaspace pieces (each starting with the replacement `▁`
    /// character, per HF's `MergedWithNext` split policy) and runs the
    /// Viterbi loop on each piece independently. This is what makes
    /// XLM-RoBERTa / Llama / Mistral / T5 encode identically to
    /// upstream `tokenizers-rs`: the Metaspace mark is what carries
    /// word-initial position information into the vocab lookup.
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pre_tokenizer: Metaspace) -> Self {
        self.pre_tokenizer = Some(pre_tokenizer);
        self
    }

    /// Attach (or replace) the post-processor.
    ///
    /// The post-processor runs on the finished `Encoding` before
    /// `Tokenizer::encode` returns it. See
    /// [`crate::post_processor::PostProcessor`] for the shape; the
    /// default [`PostProcessor::None`] is a pass-through, so callers
    /// who never configure one see the unchanged Unigram output.
    #[must_use]
    pub fn with_post_processor(mut self, post_processor: PostProcessor) -> Self {
        self.post_processor = post_processor;
        self
    }

    /// The vocabulary this tokenizer was built from.
    #[must_use]
    pub fn vocab(&self) -> &[(String, f64)] {
        &self.vocab
    }

    /// The `unk_id`, if any.
    #[must_use]
    pub const fn unk_id(&self) -> Option<usize> {
        self.unk_id
    }

    /// Read-only access to the configured normalizer, if any.
    #[must_use]
    pub fn normalizer(&self) -> Option<&Normalizer> {
        self.normalizer.as_ref()
    }

    /// Read-only access to the configured pre-tokenizer, if any.
    #[must_use]
    pub fn pre_tokenizer(&self) -> Option<&Metaspace> {
        self.pre_tokenizer.as_ref()
    }

    /// Read-only access to the configured post-processor.
    #[must_use]
    pub fn post_processor(&self) -> &PostProcessor {
        &self.post_processor
    }

    /// Run the full `SentencePiece` pipeline on `input`, returning
    /// the produced token ids.
    ///
    /// The pipeline is:
    ///
    /// 1. Apply the configured [`Normalizer`] (if any) to `input`.
    /// 2. If a [`Metaspace`] pre-tokenizer is configured, split the
    ///    normalized string into pieces per its `apply` rule; else
    ///    treat the whole normalized string as one piece.
    /// 3. Run `encode_piece_ids` (Viterbi forward-DP) on each piece
    ///    and concatenate the resulting ids in order.
    /// 4. The [`PostProcessor`] is not applied here — it operates on
    ///    an `Encoding`, not a bare `Vec<usize>`, so the trait
    ///    `Tokenizer::encode` entry point is the one that splices
    ///    the CLS / SEP wrapping. This inherent method returns the
    ///    raw piece ids so callers who want them without post-
    ///    processing keep the pre-composition surface.
    ///
    /// # Errors
    ///
    /// Returns [`UnigramEncodeError::UntokenizableChar`] when a
    /// character in the (normalized, pre-tokenized) input is not
    /// covered by any vocab entry and no `unk_id` is configured. The
    /// carried `char_offset` is a character index into the specific
    /// piece that failed — the piece-level split is a runtime detail
    /// and this error type does not attempt to map back to a
    /// caller-visible offset in the original input.
    pub fn encode(&self, input: &str) -> Result<Vec<usize>, UnigramEncodeError> {
        // Step 1: normalize.
        let normalized: alloc::borrow::Cow<'_, str> = match &self.normalizer {
            Some(n) => alloc::borrow::Cow::Owned(crate::normalizer::normalize(input, n)),
            None => alloc::borrow::Cow::Borrowed(input),
        };
        let text: &str = normalized.as_ref();

        // Step 2 + 3: pre-tokenize into pieces and run Viterbi on
        // each. Without a Metaspace configured we run on the whole
        // string (preserving the original pre-composition behaviour
        // for callers that never call `with_pre_tokenizer`).
        let mut ids = Vec::new();
        if let Some(ms) = &self.pre_tokenizer {
            for piece in ms.apply(text) {
                let piece_ids = self.encode_piece_ids(&piece)?;
                ids.extend(piece_ids);
            }
        } else {
            let piece_ids = self.encode_piece_ids(text)?;
            ids.extend(piece_ids);
        }
        Ok(ids)
    }

    /// Decode a slice of Unigram token ids back into a string.
    ///
    /// Concatenates the surface string of each id in order and, when
    /// a [`Metaspace`] pre-tokenizer is configured, reverses its
    /// substitution — every occurrence of
    /// [`Metaspace::replacement`] in the concatenated output is
    /// replaced with an ASCII space, and a leading space (from the
    /// prepend-scheme mark) is trimmed. This mirrors HF's own
    /// `MetaspaceDecoder` behaviour and yields a string that matches
    /// the original caller-visible input modulo the documented
    /// normalization losses.
    ///
    /// # Errors
    ///
    /// Returns [`UnigramDecodeError::UnknownId`] carrying the
    /// offending id if any id in `tokens` is out of range for the
    /// configured vocabulary.
    pub fn decode(&self, tokens: &[usize]) -> Result<String, UnigramDecodeError> {
        let mut buf = String::new();
        for &id in tokens {
            let (surface, _) = self
                .vocab
                .get(id)
                .ok_or(UnigramDecodeError::UnknownId(id))?;
            buf.push_str(surface);
        }
        // If a Metaspace is configured, reverse its substitution:
        // `replacement` -> ' ', and drop the single leading space
        // that the prepend-scheme mark inserted (Always / First for
        // an unmarked input both prepend one).
        if let Some(ms) = &self.pre_tokenizer {
            let replacement = ms.replacement;
            let mut restored = String::with_capacity(buf.len());
            for c in buf.chars() {
                if c == replacement {
                    restored.push(' ');
                } else {
                    restored.push(c);
                }
            }
            // Trim a single leading space introduced by the
            // prepend-scheme mark. Matches HF's MetaspaceDecoder
            // (`add_prefix_space = true` default) which strips exactly
            // one leading space from the decoded output.
            if restored.starts_with(' ') {
                restored.remove(0);
            }
            Ok(restored)
        } else {
            Ok(buf)
        }
    }

    /// Run Viterbi forward-DP over one already-pre-tokenized piece
    /// and return the winning path's ids.
    ///
    /// This is the inner loop separated from [`Self::encode`] so the
    /// Metaspace-composed and non-composed paths share it.
    fn encode_piece_ids(&self, input: &str) -> Result<Vec<usize>, UnigramEncodeError> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Character boundary offsets (byte indices into `input`), with
        // a trailing sentinel at `input.len()` so `boundaries[i..i+1]`
        // brackets the `i`-th character.
        let mut boundaries: Vec<usize> = input.char_indices().map(|(i, _)| i).collect();
        boundaries.push(input.len());
        let n = boundaries.len() - 1;

        // best_score[i] = best log-probability sum reachable at
        // character position `i`; `f64::NEG_INFINITY` means unreachable.
        let mut best_score = alloc::vec![f64::NEG_INFINITY; n + 1];
        // best_prev[i] = (previous char position, emitted token id) for
        // the winning transition into `i`. Sentinel for i=0 is unused.
        let mut best_prev: Vec<(usize, usize)> = alloc::vec![(0, 0); n + 1];
        best_score[0] = 0.0;

        for i in 1..=n {
            // Consider every earlier reachable position `j` and check
            // whether `input[j..i]` is a vocab entry.
            for j in 0..i {
                if !best_score[j].is_finite() {
                    continue;
                }
                let piece = &input[boundaries[j]..boundaries[i]];
                if let Some(&(id, score)) = self.lookup.get(piece) {
                    let candidate = best_score[j] + score;
                    if candidate > best_score[i] {
                        best_score[i] = candidate;
                        best_prev[i] = (j, id);
                    }
                }
            }
            // Fallback: if `i` is unreachable and we have an `unk`
            // token, take a single-character `unk` transition from
            // `i - 1` (if that itself is reachable).
            if !best_score[i].is_finite() {
                if let Some(u) = self.unk_id {
                    if best_score[i - 1].is_finite() {
                        let (_, unk_score) = self.vocab[u];
                        let candidate = best_score[i - 1] + unk_score - self.unk_penalty;
                        best_score[i] = candidate;
                        best_prev[i] = (i - 1, u);
                    }
                }
            }
        }

        if !best_score[n].is_finite() {
            // Locate the first unreachable character to name in the error.
            // Walk forward until we find a position `k > 0` whose
            // predecessor is reachable but `k` itself is not.
            let mut char_offset = 0;
            for k in 1..=n {
                if !best_score[k].is_finite() && best_score[k - 1].is_finite() {
                    char_offset = k - 1;
                    break;
                }
            }
            return Err(UnigramEncodeError::UntokenizableChar { char_offset });
        }

        // Backtrack from n to 0, collecting emitted ids in reverse.
        let mut ids = Vec::new();
        let mut pos = n;
        while pos > 0 {
            let (prev, id) = best_prev[pos];
            ids.push(id);
            pos = prev;
        }
        ids.reverse();
        Ok(ids)
    }
}

impl stringcheese_tokenizer::Tokenizer for UnigramTokenizer {
    type Token = TokenId;

    fn encode(
        &self,
        text: &str,
    ) -> Result<stringcheese_tokenizer::Encoding<Self::Token>, stringcheese_tokenizer::TokenizerError>
    {
        // Reuse the inherent `encode` pipeline for normalize +
        // pre-tokenize + Viterbi, then splice the post-processor on
        // top. The inherent method returns `Vec<usize>`; the trait's
        // `Encoding<TokenId>` uses `u32`. Cast at the boundary — every
        // id from a real SentencePiece vocab fits.
        let raw = Self::encode(self, text).map_err(|e| {
            stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!("{e}"))
        })?;
        let mut enc: stringcheese_tokenizer::Encoding<TokenId> =
            stringcheese_tokenizer::Encoding::new();
        enc.ids.reserve(raw.len());
        for id in raw {
            let tid = TokenId::try_from(id).map_err(|_| {
                stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!(
                    "Unigram id {id} does not fit in TokenId (u32)"
                ))
            })?;
            enc.ids.push(tid);
        }
        // Fast-path the identity post-processor to avoid an extra
        // clone on the common no-post-processor path.
        Ok(if matches!(self.post_processor, PostProcessor::None) {
            enc
        } else {
            self.post_processor.apply(&enc, true)
        })
    }

    fn decode(
        &self,
        tokens: &[Self::Token],
    ) -> Result<String, stringcheese_tokenizer::TokenizerError> {
        // Widen ids to `usize` for the inherent decode. Every shipped
        // Unigram vocab fits in a u32-indexable Vec so the conversion
        // is infallible.
        let widened: Vec<usize> = tokens.iter().map(|&t| t as usize).collect();
        Self::decode(self, &widened).map_err(|e| {
            stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!("{e}"))
        })
    }

    fn count(&self, text: &str) -> Result<usize, stringcheese_tokenizer::TokenizerError> {
        // Mirror `encode`'s full pipeline so `count(text) ==
        // encode(text)?.ids.len()` holds for every configuration. The
        // synthetic-encoding shape follows the same pattern
        // `BpeTokenizer::count` and `WordPieceTokenizer::count` use
        // for the post-processor arm.
        let raw = Self::encode(self, text).map_err(|e| {
            stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!("{e}"))
        })?;
        let base = raw.len();
        Ok(if matches!(self.post_processor, PostProcessor::None) {
            base
        } else {
            let mut synth: stringcheese_tokenizer::Encoding<TokenId> =
                stringcheese_tokenizer::Encoding::new();
            synth.ids.resize(base, 0);
            self.post_processor.apply(&synth, true).ids.len()
        })
    }
}

/// Error returned by [`UnigramTokenizer::decode`] when a token id is
/// out of range for the configured vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnigramDecodeError {
    /// The wrapped id has no entry in the tokenizer's vocabulary. Any
    /// id `>= vocab.len()` fails with this variant.
    UnknownId(usize),
}

impl fmt::Display for UnigramDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId(id) => write!(
                f,
                "Unigram tokenizer could not decode id {id}: out of range for the configured vocabulary"
            ),
        }
    }
}

impl std::error::Error for UnigramDecodeError {}

/// Error returned by [`UnigramTokenizer::encode`] when the input
/// cannot be tokenized under the configured vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnigramEncodeError {
    /// A character in the input is not covered by any vocab entry
    /// and no `unk_id` was configured, so tokenization cannot
    /// proceed. `char_offset` is the zero-based Unicode-scalar-value
    /// index of the offending character.
    UntokenizableChar {
        /// Zero-based Unicode-scalar-value index of the offending
        /// character.
        char_offset: usize,
    },
}

impl fmt::Display for UnigramEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UntokenizableChar { char_offset } => write!(
                f,
                "Unigram tokenizer could not tokenize character at position {char_offset}: \
                 no matching vocab entry and no unk_id configured"
            ),
        }
    }
}

impl std::error::Error for UnigramEncodeError {}

/// Materialise an [`HfTokenizerConfig`] as a runnable
/// [`UnigramTokenizer`].
///
/// The config's `model.type` must be `"Unigram"`; any other type
/// surfaces [`HfConversionError::UnsupportedModelForUnigram`].
///
/// Supported ancillary features today:
///
/// * `normalizer` — every variant `to_runtime_normalizer` materialises
///   is attached to the produced [`UnigramTokenizer`] via
///   [`UnigramTokenizer::with_normalizer`]. This is what runs the
///   XLM-RoBERTa `Precompiled` charsmap on the raw input before
///   Viterbi sees it.
/// * `pre_tokenizer` — the `SentencePiece` [`Metaspace`] shape (either
///   as a bare `pre_tokenizer` or as the sole entry inside a
///   `Sequence`) is materialised via [`to_runtime_metaspace`] and
///   attached via [`UnigramTokenizer::with_pre_tokenizer`]. Every
///   other pre-tokenizer variant surfaces
///   [`HfConversionError::UnsupportedPreTokenizer`] with the offending
///   type name.
/// * `post_processor` — [`HfPostProcessor::TemplateProcessing`] (the
///   Llama shape) and [`HfPostProcessor::RobertaProcessing`] (the
///   XLM-RoBERTa CLS/SEP splice) are honoured and attached via
///   [`UnigramTokenizer::with_post_processor`]. Every other variant
///   surfaces [`HfConversionError::UnsupportedPostProcessor`].
///
/// **Deferred**: `decoder` is preserved on [`HfTokenizerConfig::decoder`]
/// for caller inspection but not applied — [`UnigramTokenizer::decode`]
/// reverses the Metaspace substitution unconditionally, which is the
/// shape every `SentencePiece` checkpoint expects.
///
/// # Errors
///
/// * [`HfConversionError::UnsupportedModelForUnigram`] — the config's
///   `model.type` is not `"Unigram"`.
/// * [`HfConversionError::UnigramUnkIdOutOfRange`] — the config's
///   `unk_id` points past the end of the vocabulary.
/// * [`HfConversionError::UnsupportedNormalizer`] —
///   `to_runtime_normalizer`'s error.
/// * [`HfConversionError::UnsupportedPreTokenizer`] — the
///   `pre_tokenizer` block is not a Metaspace shape (nor a single-entry
///   Sequence wrapping one).
/// * [`HfConversionError::UnsupportedPostProcessor`] —
///   `to_runtime_post_processor`'s error.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_hf::hf::{parse_tokenizer_json, to_unigram_tokenizer};
///
/// let json = r#"{
///     "added_tokens": [],
///     "model": {
///         "type": "Unigram",
///         "vocab": [["<unk>", 0.0], ["hello", -1.0], ["world", -2.0]],
///         "unk_id": 0
///     }
/// }"#;
/// let config = parse_tokenizer_json(json).unwrap();
/// let tok = to_unigram_tokenizer(&config).unwrap();
/// assert_eq!(tok.encode("hello").unwrap(), vec![1]);
/// ```
pub fn to_unigram_tokenizer(
    config: &HfTokenizerConfig,
) -> Result<UnigramTokenizer, HfConversionError> {
    let uni = match &config.model {
        HfModel::Unigram(uni) => uni,
        HfModel::Bpe(_) => {
            return Err(HfConversionError::UnsupportedModelForUnigram {
                type_name: "BPE".to_string(),
            });
        }
        HfModel::WordPiece(_) => {
            return Err(HfConversionError::UnsupportedModelForUnigram {
                type_name: "WordPiece".to_string(),
            });
        }
        HfModel::WordLevel(_) => {
            return Err(HfConversionError::UnsupportedModelForUnigram {
                type_name: "WordLevel".to_string(),
            });
        }
    };
    let mut tok = UnigramTokenizer::from_parts(uni.vocab.clone(), uni.unk_id)?;

    // Normalizer — runs before the pre-tokenizer at encode time.
    if let Some(hn) = &config.normalizer {
        let n = to_runtime_normalizer(hn)?;
        tok = tok.with_normalizer(n);
    }

    // Pre-tokenizer — Metaspace (or a single-entry Sequence wrapping
    // one) is the only shape SentencePiece Unigram checkpoints ship.
    if let Some(pt) = &config.pre_tokenizer {
        let ms = extract_unigram_pre_tokenizer(pt)?;
        tok = tok.with_pre_tokenizer(ms);
    }

    // Post-processor — runs on the finished Encoding before the trait
    // `Tokenizer::encode` returns it.
    if let Some(hp) = &config.post_processor {
        let pp = to_runtime_post_processor(hp)?;
        tok = tok.with_post_processor(pp);
    }

    Ok(tok)
}

/// Reduce an [`HfPreTokenizer`] to a runtime [`Metaspace`], unwrapping
/// a single-entry `Sequence` if that is what the config carries.
/// Every other shape surfaces
/// [`HfConversionError::UnsupportedPreTokenizer`].
fn extract_unigram_pre_tokenizer(pt: &HfPreTokenizer) -> Result<Metaspace, HfConversionError> {
    match pt {
        HfPreTokenizer::Metaspace { .. } => to_runtime_metaspace(pt),
        HfPreTokenizer::Sequence { pretokenizers } => {
            // The audit noted that real XLM-RoBERTa / Llama / T5
            // configs sometimes wrap Metaspace inside a
            // single-entry Sequence. Accept that; ambiguous multi-
            // entry sequences that mix Metaspace with something else
            // are rejected.
            if pretokenizers.len() == 1 {
                extract_unigram_pre_tokenizer(&pretokenizers[0])
            } else {
                Err(HfConversionError::AmbiguousSequencePreTokenizer {
                    child_count: pretokenizers.len(),
                })
            }
        }
        _ => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "non-Metaspace".to_string(),
            reason: "Unigram tokenizers only accept a SentencePiece Metaspace pre-tokenizer",
        }),
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
        HfPreTokenizer::Metaspace { .. } => (
            "Metaspace",
            "SentencePiece Metaspace is not part of the BPE pipeline; \
             use `to_runtime_metaspace` to materialise the runtime `Metaspace` \
             and drive it against a Unigram tokenizer",
        ),
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
        HfNormalizer::BertNormalizer {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        } => Ok(Normalizer::Bert {
            clean_text: *clean_text,
            handle_chinese_chars: *handle_chinese_chars,
            strip_accents: *strip_accents,
            lowercase: *lowercase,
        }),
        HfNormalizer::Precompiled {
            precompiled_charsmap,
        } => Ok(Normalizer::Precompiled {
            charsmap_base64: precompiled_charsmap.clone(),
        }),
        HfNormalizer::Other => Err(HfConversionError::UnsupportedNormalizer {
            type_name: "Other".to_string(),
        }),
    }
}

/// Reduce an [`HfPreTokenizer::Metaspace`] value to a runtime
/// [`Metaspace`].
///
/// `Metaspace` is `SentencePiece`'s pre-tokenizer for Llama, Mistral,
/// T5, and XLM-RoBERTa — see the runtime [`Metaspace`] type for the
/// exact semantics. It is not wired into either [`to_bpe_tokenizer`]
/// (BPE has its own byte-level pipeline) or [`to_unigram_tokenizer`]
/// (the current Unigram runtime encodes raw input) today; callers who
/// need to apply it can obtain the typed runtime value here and drive
/// it themselves against the produced tokenizer.
///
/// # Errors
///
/// Returns [`HfConversionError::UnsupportedPreTokenizer`] with
/// `type_name == "<non-Metaspace variant tag>"` if the argument is not
/// an [`HfPreTokenizer::Metaspace`].
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_hf::{Metaspace, PrependScheme};
/// use stringcheese_tokenizer_hf::hf::{HfPreTokenizer, to_runtime_metaspace};
///
/// let ms_json = r#"{"type": "Metaspace"}"#;
/// let ms: HfPreTokenizer = serde_json::from_str(ms_json).unwrap();
/// let runtime = to_runtime_metaspace(&ms).unwrap();
/// assert_eq!(runtime.replacement, '\u{2581}');
/// assert_eq!(runtime.prepend_scheme, PrependScheme::Always);
/// assert!(runtime.split);
/// assert_eq!(
///     runtime.apply("hello world"),
///     vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()]
/// );
/// # let _ = Metaspace::new();
/// ```
pub fn to_runtime_metaspace(pt: &HfPreTokenizer) -> Result<Metaspace, HfConversionError> {
    match pt {
        HfPreTokenizer::Metaspace {
            replacement,
            prepend_scheme,
            split,
        } => Ok(Metaspace {
            replacement: *replacement,
            prepend_scheme: (*prepend_scheme).into(),
            split: *split,
        }),
        _ => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "non-Metaspace".to_string(),
            reason: "to_runtime_metaspace called on a pre-tokenizer that is not Metaspace",
        }),
    }
}

/// Reduce an [`HfPostProcessor`] to a runtime [`PostProcessor`] or the
/// appropriate deferred-feature error.
fn to_runtime_post_processor(hp: &HfPostProcessor) -> Result<PostProcessor, HfConversionError> {
    match hp {
        HfPostProcessor::ByteLevel {
            add_prefix_space,
            trim_offsets,
            use_regex,
        } => Ok(PostProcessor::ByteLevel {
            add_prefix_space: *add_prefix_space,
            trim_offsets: *trim_offsets,
            use_regex: *use_regex,
        }),
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
        HfPostProcessor::RobertaProcessing(rp) => {
            Ok(PostProcessor::RobertaProcessing(RobertaProcessing {
                sep: (rp.sep.0.clone(), rp.sep.1),
                cls: (rp.cls.0.clone(), rp.cls.1),
                trim_offsets: rp.trim_offsets,
                add_prefix_space: rp.add_prefix_space,
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
    fn unigram_model_rejected_by_to_bpe_tokenizer() {
        // Unigram is materialised via `to_unigram_tokenizer` /
        // `to_tokenizer`, not `to_bpe_tokenizer`. Backwards-compat:
        // `to_bpe_tokenizer` errors with `UnsupportedModelForBpe` so
        // callers can dispatch on the specific model type.
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
            HfConversionError::UnsupportedModelForBpe { type_name } => {
                assert_eq!(type_name, "Unigram");
            }
            other => panic!("expected UnsupportedModelForBpe(Unigram), got {other:?}"),
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
    fn normalizer_bert_parses_and_materialises() {
        // Wave-12: BertNormalizer is a first-class variant. It
        // deserialises into `HfNormalizer::BertNormalizer{..}` and
        // materialises into `Normalizer::Bert{..}` on the runtime
        // side.
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
        match &config.normalizer {
            Some(HfNormalizer::BertNormalizer {
                clean_text,
                handle_chinese_chars,
                strip_accents,
                lowercase,
            }) => {
                assert!(*clean_text);
                assert!(*handle_chinese_chars);
                assert!(strip_accents.is_none());
                assert!(*lowercase);
            }
            other => panic!("expected BertNormalizer, got {other:?}"),
        }
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(matches!(
            tok.normalizer(),
            Some(crate::normalizer::Normalizer::Bert { .. })
        ));
    }

    #[test]
    fn normalizer_bert_defaults_are_hf_canonical() {
        // Bare `{"type": "BertNormalizer"}` — every field defaults
        // to the HF-canonical value (`true` / `None` / `true`).
        let json = r#"{
            "added_tokens": [],
            "normalizer": {"type": "BertNormalizer"},
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.normalizer {
            Some(HfNormalizer::BertNormalizer {
                clean_text,
                handle_chinese_chars,
                strip_accents,
                lowercase,
            }) => {
                assert!(*clean_text);
                assert!(*handle_chinese_chars);
                assert!(strip_accents.is_none());
                assert!(*lowercase);
            }
            other => panic!("expected BertNormalizer, got {other:?}"),
        }
    }

    #[test]
    fn normalizer_precompiled_parses_and_materialises_as_passthrough() {
        // Wave-13: Precompiled is a first-class typed variant. It
        // parses without error and materialises into
        // `Normalizer::Precompiled` on the runtime side — see that
        // variant's doc-comment for the passthrough contract and the
        // deferred full-execution TODO.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Precompiled",
                "precompiled_charsmap": "AAAA"
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.normalizer {
            Some(HfNormalizer::Precompiled {
                precompiled_charsmap,
            }) => {
                assert_eq!(precompiled_charsmap, "AAAA");
            }
            other => panic!("expected Precompiled, got {other:?}"),
        }
        let tok = to_bpe_tokenizer(&config).unwrap();
        match tok.normalizer() {
            Some(crate::normalizer::Normalizer::Precompiled { charsmap_base64 }) => {
                assert_eq!(charsmap_base64, "AAAA");
            }
            other => panic!("expected Normalizer::Precompiled, got {other:?}"),
        }
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
    // ByteLevel post-processor (GPT-2 shape).
    // ---------------------------------------------------------------------

    #[test]
    fn bytelevel_post_processor_parses_typed_with_hf_defaults() {
        // Bare `{"type": "ByteLevel"}` — every field defaults to the
        // HF-canonical `true`.
        let json = r#"{"type": "ByteLevel"}"#;
        let hp: HfPostProcessor = serde_json::from_str(json).unwrap();
        match hp {
            HfPostProcessor::ByteLevel {
                add_prefix_space,
                trim_offsets,
                use_regex,
            } => {
                assert!(add_prefix_space);
                assert!(trim_offsets);
                assert!(use_regex);
            }
            other => panic!("expected ByteLevel post-processor, got {other:?}"),
        }
    }

    #[test]
    fn bytelevel_post_processor_parses_typed_with_explicit_fields() {
        // Real GPT-2 config carries `add_prefix_space: true,
        // trim_offsets: false` on the post-processor. Verify both
        // explicit shapes round-trip.
        let json = r#"{
            "type": "ByteLevel",
            "add_prefix_space": true,
            "trim_offsets": false
        }"#;
        let hp: HfPostProcessor = serde_json::from_str(json).unwrap();
        match hp {
            HfPostProcessor::ByteLevel {
                add_prefix_space,
                trim_offsets,
                use_regex,
            } => {
                assert!(add_prefix_space);
                assert!(!trim_offsets);
                // `use_regex` is absent — serde default `true`.
                assert!(use_regex);
            }
            other => panic!("expected ByteLevel post-processor, got {other:?}"),
        }
    }

    #[test]
    fn bytelevel_post_processor_materialises_as_runtime_variant() {
        // Convert an inline GPT-2-shape config; the runtime tokenizer
        // must carry the ByteLevel post-processor with the parsed
        // fields preserved verbatim.
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel", "add_prefix_space": false},
            "post_processor": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": false
            },
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, "Ġ": 2},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        match tok.post_processor() {
            crate::post_processor::PostProcessor::ByteLevel {
                add_prefix_space,
                trim_offsets,
                use_regex,
            } => {
                assert!(*add_prefix_space);
                assert!(!*trim_offsets);
                assert!(*use_regex);
            }
            other => panic!("expected ByteLevel runtime post-processor, got {other:?}"),
        }
    }

    #[test]
    fn bytelevel_post_processor_is_noop_end_to_end_gpt2_shape() {
        // Full GPT-2-shape blob: ByteLevel pre-tokenizer +
        // ByteLevel post-processor + ByteLevel decoder. Encoding
        // "Hello, world!" must produce ids identical to the same
        // blob with the post-processor omitted — the post-processor
        // is a no-op on the encoding this crate ships. The vocab
        // covers every byte-encoded piece the two chunks decompose
        // to under the toy merges below.
        let with_pp_json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel", "add_prefix_space": false},
            "post_processor": {"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": false},
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {
                    "H": 0, "e": 1, "l": 2, "o": 3, ",": 4,
                    "Ġ": 5, "w": 6, "r": 7, "d": 8, "!": 9,
                    "He": 10, "ll": 11, "Hell": 12, "Hello": 13,
                    "Ġw": 14, "Ġwo": 15, "Ġwor": 16, "Ġworl": 17, "Ġworld": 18
                },
                "merges": [
                    ["H", "e"], ["l", "l"], ["He", "ll"], ["Hell", "o"],
                    ["Ġ", "w"], ["Ġw", "o"], ["Ġwo", "r"], ["Ġwor", "l"], ["Ġworl", "d"]
                ]
            }
        }"#;
        let without_pp_json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "ByteLevel", "add_prefix_space": false},
            "decoder": {"type": "ByteLevel"},
            "model": {
                "type": "BPE",
                "vocab": {
                    "H": 0, "e": 1, "l": 2, "o": 3, ",": 4,
                    "Ġ": 5, "w": 6, "r": 7, "d": 8, "!": 9,
                    "He": 10, "ll": 11, "Hell": 12, "Hello": 13,
                    "Ġw": 14, "Ġwo": 15, "Ġwor": 16, "Ġworl": 17, "Ġworld": 18
                },
                "merges": [
                    ["H", "e"], ["l", "l"], ["He", "ll"], ["Hell", "o"],
                    ["Ġ", "w"], ["Ġw", "o"], ["Ġwo", "r"], ["Ġwor", "l"], ["Ġworl", "d"]
                ]
            }
        }"#;
        let with_pp = to_bpe_tokenizer(&parse_tokenizer_json(with_pp_json).unwrap()).unwrap();
        let without_pp = to_bpe_tokenizer(&parse_tokenizer_json(without_pp_json).unwrap()).unwrap();

        // Hand-computed expected ids for "Hello, world!" under
        // add_prefix_space=false on the pre-tokenizer:
        //   "Hello" chunk → merges land it on id 13 ("Hello").
        //   ","    chunk → id 4.
        //   " world" chunk → merges land on id 18 ("Ġworld").
        //   "!"    chunk → id 9.
        let expected = alloc::vec![13, 4, 18, 9];
        let with_ids = with_pp.encode("Hello, world!").unwrap().ids;
        let without_ids = without_pp.encode("Hello, world!").unwrap().ids;
        assert_eq!(with_ids, expected);
        assert_eq!(with_ids, without_ids);
        // count() also short-circuits on the ByteLevel arm.
        assert_eq!(with_pp.count("Hello, world!").unwrap(), expected.len());
    }

    #[test]
    fn bytelevel_post_processor_gpt2_full_config_loads() {
        // Minimal GPT-2-shape blob mirroring the field layout of the
        // real `gpt2/tokenizer.json`: ByteLevel pre-tokenizer with
        // `add_prefix_space: false` and ByteLevel post-processor
        // with `add_prefix_space: true` (the asymmetry HF's own
        // pipeline carries — the pre-tokenizer's flag governs
        // encoding, the post-processor's flag is inert). Before the
        // ByteLevel post-processor landed, this config panicked at
        // `to_bpe_tokenizer` time with UnsupportedPostProcessor.
        let json = r#"{
            "version": "1.0",
            "added_tokens": [
                {"id": 50256, "content": "<|endoftext|>", "special": true}
            ],
            "normalizer": null,
            "pre_tokenizer": {
                "type": "ByteLevel",
                "add_prefix_space": false,
                "trim_offsets": true
            },
            "post_processor": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": false
            },
            "decoder": {
                "type": "ByteLevel",
                "add_prefix_space": true,
                "trim_offsets": true
            },
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "Ġ": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        // The loader must accept the config without falling through
        // to UnsupportedPostProcessor.
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(matches!(
            tok.post_processor(),
            crate::post_processor::PostProcessor::ByteLevel { .. }
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
        // "##able"] → [200, 201, 202]. BERT_JSON's TemplateProcessing
        // splices `[CLS]` (101) before and `[SEP]` (102) after — the
        // BERT-parity wire-up is applied end-to-end.
        assert_eq!(tok.encode("unaffable"), vec![101, 200, 201, 202, 102]);
    }

    #[test]
    fn to_wordpiece_tokenizer_bert_pre_tokenizer_splits_punctuation() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "Hello, world!" via BertPreTokenizer → ["Hello", ",",
        // "world", "!"] → [207, 205, 208, 206], wrapped by
        // BERT_JSON's `[CLS]` / `[SEP]` template processor.
        assert_eq!(
            tok.encode("Hello, world!"),
            vec![101, 207, 205, 208, 206, 102]
        );
    }

    #[test]
    fn to_wordpiece_tokenizer_oov_word_emits_unk() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "xyz" is not in the vocab and no ## prefix decomposition
        // works. WordPiece is all-or-nothing on a word → emit UNK id
        // (100). BERT_JSON's TemplateProcessing wraps that in
        // `[CLS]` / `[SEP]`.
        assert_eq!(tok.encode("xyz"), vec![101, 100, 102]);
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
        // themselves survive verbatim. `[CLS]` and `[SEP]` come from
        // the BERT_JSON template processor and are decoded back as
        // their surface strings.
        let ids = tok.encode("unaffable cat dog");
        assert_eq!(ids, vec![101, 200, 201, 202, 203, 204, 102]);
        let text = tok.decode(&ids).unwrap();
        assert_eq!(text, "[CLS] unaffable cat dog [SEP]");
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
    fn to_wordpiece_tokenizer_accepts_bert_normalizer() {
        // BertNormalizer is a first-class variant on the typed side
        // and is now attached to the runtime tokenizer — verify the
        // conversion succeeds and that the normalizer is stored on
        // the produced tokenizer.
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
        assert!(matches!(
            &config.normalizer,
            Some(HfNormalizer::BertNormalizer { .. })
        ));
        let tok = to_wordpiece_tokenizer(&config).expect("BertNormalizer must materialise");
        // The normalizer is attached to the runtime tokenizer and
        // consulted by `encode`.
        assert!(matches!(
            tok.normalizer(),
            Some(crate::normalizer::Normalizer::Bert { .. })
        ));
    }

    #[test]
    fn to_tokenizer_dispatches_to_wordpiece_enum_variant() {
        let config = parse_tokenizer_json(BERT_JSON).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::WordPiece(wp) => {
                // BERT_JSON's TemplateProcessing splices `[CLS]` (101)
                // before and `[SEP]` (102) after the primary encoding.
                assert_eq!(wp.encode("unaffable"), vec![101, 200, 201, 202, 102]);
            }
            other => panic!("expected HfTokenizer::WordPiece, got {other:?}"),
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
            other => panic!("expected HfTokenizer::Bpe, got {other:?}"),
        }
    }

    #[test]
    fn to_tokenizer_rejects_wordlevel() {
        // WordLevel remains deferred. Unigram, which used to appear
        // in this test, is now materialised via `to_unigram_tokenizer`
        // — see `to_tokenizer_dispatches_to_unigram_enum_variant`.
        let json = r#"{"added_tokens":[],"model":{"type":"WordLevel","vocab":{"a":0},"unk_token":"[UNK]"}}"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModel { type_name } => {
                assert_eq!(type_name, "WordLevel");
            }
            other => panic!("expected UnsupportedModel(WordLevel), got {other:?}"),
        }
    }

    #[test]
    fn to_tokenizer_dispatches_to_unigram_enum_variant() {
        // A Unigram config now routes through `to_tokenizer` and
        // produces the `HfTokenizer::Unigram` variant.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "Unigram",
                "vocab": [["<unk>", 0.0], ["hello", -1.0]],
                "unk_id": 0
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::Unigram(uni) => {
                assert_eq!(uni.encode("hello").unwrap(), vec![1]);
            }
            other => panic!("expected HfTokenizer::Unigram, got {other:?}"),
        }
    }

    #[test]
    fn bert_shape_end_to_end_with_special_tokens_and_bert_pre_tokenizer() {
        // Full BERT-shape blob (BERT_JSON): WordPiece + BertPreTokenizer
        // + TemplateProcessing. Verify the parsed post_processor is
        // honoured shape-wise and that the runtime tokenizer now
        // applies the template-processing splice end-to-end.
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
        // pre-tokenizer. The template-processing splice `[CLS]` (101)
        // + primary + `[SEP]` (102) is applied end-to-end.
        let ids = tok.encode("Hello, world!");
        assert_eq!(ids, vec![101, 207, 205, 208, 206, 102]);
    }

    /// A minimal BERT-like `tokenizer.json` that exercises both slots
    /// this landing wires up: `BertNormalizer` (lowercase + accent
    /// strip) and `TemplateProcessing` (`[CLS]` / `[SEP]`). The vocab
    /// is deliberately lowercased and accent-stripped so the
    /// normalizer is load-bearing — without it, mixed-case /
    /// accented input would miss the vocab.
    const BERT_NORMALIZED_JSON: &str = r###"{
        "version": "1.0",
        "added_tokens": [
            {"id": 0, "content": "[UNK]", "special": true},
            {"id": 1, "content": "[CLS]", "special": true},
            {"id": 2, "content": "[SEP]", "special": true}
        ],
        "normalizer": {
            "type": "BertNormalizer",
            "clean_text": true,
            "handle_chinese_chars": true,
            "strip_accents": true,
            "lowercase": true
        },
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
                "[CLS]": {"id": "[CLS]", "ids": [1], "tokens": ["[CLS]"]},
                "[SEP]": {"id": "[SEP]", "ids": [2], "tokens": ["[SEP]"]}
            }
        },
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[UNK]": 0,
                "[CLS]": 1,
                "[SEP]": 2,
                "hello": 3,
                "world": 4,
                "cafe": 5,
                ",": 6,
                "!": 7
            }
        }
    }"###;

    #[test]
    fn hf_loader_wires_normalizer_and_post_processor_end_to_end() {
        // Prove the loader now attaches BOTH slots and the encode
        // pipeline runs them in HF's canonical order:
        //   normalize -> pre-tokenize -> WordPiece -> post-process.
        let config = parse_tokenizer_json(BERT_NORMALIZED_JSON).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        let wp = match tok {
            HfTokenizer::WordPiece(wp) => wp,
            other => panic!("expected HfTokenizer::WordPiece, got {other:?}"),
        };
        // Sanity: both slots are attached to the runtime tokenizer.
        assert!(matches!(
            wp.normalizer(),
            Some(crate::normalizer::Normalizer::Bert { .. })
        ));
        assert!(matches!(
            wp.post_processor(),
            crate::post_processor::PostProcessor::TemplateProcessing(_)
        ));

        // Hand-computed expected sequence for "Hello, world!":
        //   BertNormalizer -> "hello, world!"   (lowercase)
        //   BertPreTokenizer -> ["hello", ",", "world", "!"]
        //   WordPiece lookup -> [3, 6, 4, 7]
        //   TemplateProcessing -> [1, 3, 6, 4, 7, 2]
        assert_eq!(wp.encode("Hello, world!"), vec![1, 3, 6, 4, 7, 2]);

        // Accented input traverses the strip_accents pass too:
        // "CAFÉ" -> "cafe" -> [5] -> [1, 5, 2].
        assert_eq!(wp.encode("CAFÉ"), vec![1, 5, 2]);
    }
}
