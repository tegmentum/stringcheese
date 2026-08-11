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
//! * `model.type == "WordLevel"` is materialised via
//!   [`to_wordlevel_tokenizer`] into a
//!   [`crate::wordlevel::WordLevelTokenizer`] — a plain whole-word
//!   vocabulary lookup — and `Unigram` is materialised via
//!   [`to_unigram_tokenizer`] into a [`UnigramTokenizer`] whose
//!   `encode` runs the Viterbi forward-DP over the vocabulary's log
//!   probabilities; see each type's docs for the algorithm.
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
//!   `Sequence` / `Lowercase` / `Replace(String)` / `Replace(Regex)` /
//!   `Strip` / `Prepend` / `BertNormalizer` / `Precompiled`) are
//!   materialised at [`to_bpe_tokenizer`] time and applied on every
//!   `encode` call; every other tag string surfaces
//!   [`HfConversionError::UnsupportedNormalizer`]. See
//!   [`HfNormalizer`] for the exhaustive list.
//! * `post_processor` — [`HfPostProcessor::TemplateProcessing`]
//!   (Llama / BERT shape), [`HfPostProcessor::BertProcessing`] (stock
//!   BERT shape), [`HfPostProcessor::RobertaProcessing`] (`XLM-RoBERTa`
//!   / `RoBERTa`), [`HfPostProcessor::ByteLevel`] (GPT-2 shape; no-op
//!   on the encoding this crate ships — see [`PostProcessor::ByteLevel`]
//!   for the rationale), and [`HfPostProcessor::Sequence`] (composition
//!   of nested post-processors) are materialised at conversion; every
//!   other tag string surfaces
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
    BertProcessing, PostProcessor, RobertaProcessing, SpecialTokenInfo, TemplatePiece,
    TemplateProcessing,
};
use crate::pre_tokenizer::{
    GPT2_PATTERN, Metaspace, PreTokenizer, PreTokenizerCompileError, PreTokenizerSequence,
    PrependScheme, RegexPreTokenizer,
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
    /// Optional `"truncation"` block, deserialised into a typed
    /// [`HfTruncationParams`] value. When present, the runtime
    /// tokenizer's `.with_truncation()` is called with the equivalent
    /// [`stringcheese_tokenizer::truncation::TruncationConfig`] at
    /// conversion time, so [`stringcheese_tokenizer::Tokenizer::encode`]
    /// applies it end to end.
    #[serde(default)]
    pub truncation: Option<HfTruncationParams>,
    /// Optional `"padding"` block, deserialised into a typed
    /// [`HfPaddingParams`] value. When present, the runtime
    /// tokenizer's `.with_padding()` is called with the equivalent
    /// [`stringcheese_tokenizer::padding::PaddingConfig`] at conversion
    /// time, so
    /// [`stringcheese_tokenizer::Tokenizer::encode_batch`] pads the
    /// resulting batch by default.
    #[serde(default)]
    pub padding: Option<HfPaddingParams>,
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
    /// [`HfPostProcessor::TemplateProcessing`] (Llama / BERT shape),
    /// [`HfPostProcessor::BertProcessing`] (stock BERT),
    /// [`HfPostProcessor::RobertaProcessing`] (`XLM-RoBERTa` /
    /// `RoBERTa`), [`HfPostProcessor::ByteLevel`] (GPT-2 shape — see
    /// [`PostProcessor::ByteLevel`] for the no-op-on-offsets policy
    /// this crate applies), and [`HfPostProcessor::Sequence`]
    /// (composition of nested post-processors). Every other tag string
    /// falls through to [`HfPostProcessor::Other`] and produces
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
/// Every variant carries typed fields and is materialised at
/// conversion time. Use [`to_tokenizer`] (which returns the
/// [`HfTokenizer`] enum) to materialise a config without caring which
/// family it belongs to; the sibling [`to_bpe_tokenizer`] /
/// [`to_wordpiece_tokenizer`] / [`to_unigram_tokenizer`] /
/// [`to_wordlevel_tokenizer`] entry points return a concrete
/// tokenizer type when the caller already knows which family a
/// config belongs to.
///
/// # Deserialisation
///
/// The primary shape is Hugging Face's own `#[serde(tag = "type")]`
/// layout — `{"type": "BPE", ...}`, `{"type": "WordPiece", ...}`,
/// `{"type": "Unigram", ...}`, `{"type": "WordLevel", ...}`. Every
/// modern `tokenizer.json` on the Hub carries a `"type"` field on the
/// `model` node, and the tagged path is what routes them.
///
/// The `openai-community/gpt2/tokenizer.json` v1.0 blob (still the
/// shipped GPT-2 config) is the notable outlier for BPE: it omits
/// `"type"` entirely, mirroring HF's own `#[serde(untagged)]` inner
/// enum which autodetects BPE from the `{vocab, merges}` shape.
/// `FacebookAI/xlm-roberta-base/tokenizer.json` is the analogous
/// outlier for Unigram: it too omits `"type"`, with a
/// `vocab: [[surface, score], ...]` array plus `unk_id`.
/// `google-bert/bert-base-multilingual-cased/tokenizer.json` is the
/// third — a typeless `WordPiece` config with an object `"vocab"`
/// and a `"unk_token"` but no `"merges"`. To load every such blob
/// without special-casing the caller, deserialisation falls back to
/// [`HfModel::Bpe`] when the typeless `model` object carries a
/// `"vocab"` object and a `"merges"` array; to [`HfModel::Unigram`]
/// when it instead carries a `"vocab"` JSON array (of
/// `[surface, score]` pairs); and to [`HfModel::WordPiece`] when it
/// carries a `"vocab"` object plus a `"unk_token"` string but no
/// `"merges"` (the `"merges"`-absent gate is what disambiguates it
/// from the BPE branch, which requires `"merges"`; the `"unk_token"`
/// gate is what distinguishes an mBERT-shape config from a corrupt
/// BPE that dropped its merges by author error). Any other typeless
/// shape (a bare `{vocab: {...}}` with neither `"merges"` nor
/// `"unk_token"`; a config missing `"vocab"` entirely) is rejected
/// rather than silently misclassified — see the module doc for the
/// rationale.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HfModel {
    /// A byte-pair-encoding model — Sennrich et al. 2016.
    Bpe(HfBpeModel),
    /// A `WordPiece` model — Wu et al. 2016, adopted by BERT and its
    /// family. See [`HfWordPieceModel`] for the fields and
    /// [`crate::wordpiece::WordPieceTokenizer`] for the runtime.
    WordPiece(HfWordPieceModel),
    /// A SentencePiece-style `Unigram` language model — Kudo (2018),
    /// used by Llama, Mistral, T5, and XLM-RoBERTa. See
    /// [`HfUnigramModel`] for the fields and [`UnigramTokenizer`] for
    /// the runtime.
    Unigram(HfUnigramModel),
    /// Simple word-level model — a plain vocabulary lookup. See
    /// [`HfWordLevelModel`] for the fields and
    /// [`crate::wordlevel::WordLevelTokenizer`] for the runtime.
    WordLevel(HfWordLevelModel),
}

/// Internal helper mirroring HF's canonical `#[serde(tag = "type")]`
/// shape for the `model` block. The public [`HfModel`] wraps this via
/// a hand-rolled [`Deserialize`] impl that first tries the tagged
/// dispatch (which every modern checkpoint's `tokenizer.json` matches)
/// and only falls back to the typeless-BPE shape when the input has
/// no `"type"` field — see [`HfModel`]'s doc for the shipped example
/// (`openai-community/gpt2/tokenizer.json` v1.0).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum HfModelTagged {
    #[serde(rename = "BPE")]
    Bpe(HfBpeModel),
    #[serde(rename = "WordPiece")]
    WordPiece(HfWordPieceModel),
    #[serde(rename = "Unigram")]
    Unigram(HfUnigramModel),
    #[serde(rename = "WordLevel")]
    WordLevel(HfWordLevelModel),
}

impl From<HfModelTagged> for HfModel {
    fn from(tagged: HfModelTagged) -> Self {
        match tagged {
            HfModelTagged::Bpe(m) => Self::Bpe(m),
            HfModelTagged::WordPiece(m) => Self::WordPiece(m),
            HfModelTagged::Unigram(m) => Self::Unigram(m),
            HfModelTagged::WordLevel(m) => Self::WordLevel(m),
        }
    }
}

impl<'de> Deserialize<'de> for HfModel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        // Materialise the whole `model` node into a `serde_json::Value`
        // so we can peek at the `"type"` field before choosing which
        // typed shape to route through. This dual-path deserialisation
        // is the whole point of the wrapper — a plain derive cannot
        // combine `tag = "type"` dispatch with a typeless fallback.
        //
        // The `Value` round-trip does couple this impl to
        // `serde_json`, which is acceptable because every caller of
        // this crate's HF loader reaches it through [`parse_tokenizer_json`]
        // (which itself is `serde_json::from_str`). No other
        // deserialiser is in scope for this type in practice.
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("type").is_some() {
            // Modern shape — dispatch on the tag. Every misspelled or
            // unrecognised tag surfaces the tagged-enum's native
            // "unknown variant" error, preserving the diagnostic
            // callers had before this fallback landed.
            let tagged: HfModelTagged = serde_json::from_value(value).map_err(D::Error::custom)?;
            return Ok(tagged.into());
        }

        // Typeless shape — three real HF checkpoints ship this: the
        // GPT-2 v1.0 blob (BPE), `FacebookAI/xlm-roberta-base`
        // (Unigram), and `google-bert/bert-base-multilingual-cased`
        // (WordPiece). We disambiguate on the JSON type of `vocab`
        // and the presence of `merges` / `unk_token`:
        //
        // * `vocab: {surface → id, ...}` (JSON object) + `merges: [...]`
        //   → BPE. The `merges` requirement is what rules out a
        //   typeless WordPiece config (same object-shaped `vocab` but
        //   no `merges`).
        // * `vocab: [[surface, score], ...]` (JSON array of 2-tuples)
        //   → Unigram. The scores are `f64`; `unk_id` is optional per
        //   the type (real xlm-roberta ships it).
        // * `vocab: {surface → id, ...}` (JSON object), no `merges`,
        //   *and* a string `unk_token` → WordPiece. The order matters:
        //   this branch runs after the BPE branch, so a config with
        //   both `merges` and `unk_token` still routes to BPE (BPE
        //   permits an optional `unk_token`). The `unk_token`
        //   requirement is what distinguishes an mBERT-shape config
        //   from a corrupt BPE that dropped its `merges` by author
        //   error — WordPiece's spec makes `unk_token` mandatory, so
        //   its absence on a merges-less object-vocab config is
        //   diagnostic of a broken shape rather than of WordPiece.
        //
        // Anything else (a bare `{vocab: {...}}` with neither `merges`
        // nor `unk_token`; a config missing `vocab` entirely; a
        // `vocab` value of some other JSON type) is rejected — the
        // fallback deliberately covers only the shapes real HF
        // checkpoints publish untagged, not every conceivable typeless
        // config.
        let vocab = value.get("vocab");
        let looks_like_bpe = vocab.is_some_and(serde_json::Value::is_object)
            && value.get("merges").is_some_and(serde_json::Value::is_array);
        if looks_like_bpe {
            let bpe: HfBpeModel = serde_json::from_value(value).map_err(D::Error::custom)?;
            return Ok(Self::Bpe(bpe));
        }
        let looks_like_unigram = vocab.is_some_and(serde_json::Value::is_array);
        if looks_like_unigram {
            let uni: HfUnigramModel = serde_json::from_value(value).map_err(D::Error::custom)?;
            return Ok(Self::Unigram(uni));
        }
        let looks_like_wordpiece = vocab.is_some_and(serde_json::Value::is_object)
            && value.get("merges").is_none()
            && value
                .get("unk_token")
                .is_some_and(serde_json::Value::is_string);
        if looks_like_wordpiece {
            let wp: HfWordPieceModel = serde_json::from_value(value).map_err(D::Error::custom)?;
            return Ok(Self::WordPiece(wp));
        }

        Err(D::Error::custom(
            "invalid `model` block: missing `type` field and does not \
             match a known typeless shape (BPE requires both an object \
             `vocab` and an array `merges`; Unigram requires an array \
             `vocab` of `[surface, score]` pairs; WordPiece requires an \
             object `vocab` and a string `unk_token` and no `merges`)",
        ))
    }
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

/// The `WordLevel`-specific fields of a `model` block.
///
/// `WordLevel` is HF's plain whole-word vocabulary-lookup model: no
/// merges, no subword decomposition, one vocab entry per word.
/// [`Self::unk_token`] is the surface string of the unknown token; it
/// must map to some id in [`Self::vocab`], otherwise
/// [`to_wordlevel_tokenizer`] surfaces
/// [`HfConversionError::WordLevelUnkNotInVocab`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfWordLevelModel {
    /// The surface-string ↔ id map. Every word the tokenizer will
    /// ever encode must appear here; anything else maps to
    /// [`Self::unk_token`].
    pub vocab: BTreeMap<String, TokenId>,
    /// The surface string of the unknown token. Required — HF's own
    /// spec makes this field mandatory on a `WordLevel` model.
    pub unk_token: String,
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
/// Every field except [`Self::vocab`], [`Self::merges`], and
/// [`Self::byte_fallback`] is optional and captured only so callers
/// can inspect what the source config declared. [`to_bpe_tokenizer`]
/// honours [`Self::byte_fallback`] (see [`BpeTokenizer::with_byte_fallback`])
/// and otherwise matches the "MVP-with-ignore" contract documented
/// at the module level.
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
    /// Whether byte-fallback is enabled — the `SentencePiece`
    /// mechanism that emits the UTF-8 bytes of an out-of-vocab
    /// character as a run of `<0xXX>` tokens (256 reserved ids) instead
    /// of failing. When [`to_bpe_tokenizer`] materialises a config
    /// with `Some(true)` here it scans the vocabulary for the 256
    /// `<0x00>`..`<0xFF>` surface strings and enables the fallback on
    /// the produced [`BpeTokenizer`] via
    /// [`BpeTokenizer::with_byte_fallback`]. A config that turns byte-
    /// fallback on but is missing any of the 256 byte tokens is
    /// rejected at conversion with
    /// [`HfConversionError::ByteFallbackTokensMissing`]. This is the
    /// same mechanism the Unigram side honours via
    /// [`HfUnigramModel::byte_fallback`]; real Llama-2 / Mistral / Qwen
    /// checkpoints ship as `model.type == "BPE"` (not `"Unigram"`) with
    /// the same 256 reserved tokens embedded, so both landings are
    /// required in practice.
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
    /// Whether byte-fallback is enabled — the `SentencePiece`
    /// mechanism that emits the UTF-8 bytes of an out-of-vocab
    /// character as a run of `<0xXX>` tokens (256 reserved ids) instead
    /// of failing. When [`to_unigram_tokenizer`] materialises a config
    /// with `Some(true)` here it scans the vocabulary for the 256
    /// `<0x00>`..`<0xFF>` surface strings and enables the fallback on
    /// the produced [`UnigramTokenizer`]. A config that turns byte-
    /// fallback on but is missing any of the 256 byte tokens is
    /// rejected at conversion with
    /// [`HfConversionError::ByteFallbackTokensMissing`].
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
    /// [`to_runtime_metaspace`] into a runtime [`Metaspace`].
    /// Both [`to_bpe_tokenizer`] (Mistral-7B-v0.1 layout) and
    /// [`to_unigram_tokenizer`] accept this shape and wire it through
    /// [`PreTokenizerSequence`]. `WordPiece` and `WordLevel` have no
    /// composition rule for Metaspace and still reject it.
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
/// The variants named here are honoured at conversion time and
/// materialise into a runtime [`Decoder`]; every other tag string
/// falls through to [`Self::Other`] (raw JSON preserved for caller
/// inspection, no runtime decoder attached). See
/// [`HfTokenizerConfig::decoder`] for the field's role and
/// [`to_bpe_tokenizer`] for the wiring.
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
    /// Compose several decoders left-to-right. Every child materialises
    /// into a runtime [`Decoder`] arm at conversion time, producing a
    /// [`Decoder::Sequence`] whose stages match this block's `decoders`
    /// array in order. Llama-2 ships as `Sequence[Replace(▁→ ),
    /// ByteFallback, Fuse, Strip(" ",1,0)]` — see
    /// [`Decoder::Sequence`] for the semantics.
    Sequence {
        /// The child decoders. HF calls this field `"decoders"` on
        /// disk (matching every other `Sequence`-shape's plural naming
        /// in the spec).
        decoders: Vec<HfDecoder>,
    },
    /// Per-token literal or regex replace. Only [`HfPattern::String`]
    /// is honoured at conversion — the runtime [`Decoder::Replace`] is
    /// literal-only. An [`HfPattern::Regex`] here surfaces
    /// [`HfConversionError::UnsupportedDecoder`] with the offending
    /// variant name; no shipped HF checkpoint's decoder-side `Replace`
    /// uses a regex, so this restriction is a real-world no-op.
    Replace {
        /// The literal-or-regex pattern block.
        pattern: HfPattern,
        /// The replacement string.
        content: String,
    },
    /// Concatenate every token string into a single-entry list
    /// (empty-string separator). Materialises into
    /// [`Decoder::Fuse`]. Ships in the Llama-2 chain as the third
    /// stage, after `Replace` and `ByteFallback` have run and before
    /// `Strip` trims the surviving leading space.
    Fuse,
    /// Strip up to `start` leading / `stop` trailing occurrences of
    /// [`Self::Strip::content`] from each token. HF stores `content`
    /// as a JSON string; at conversion time we validate it is exactly
    /// one Unicode scalar (which every shipped checkpoint's decoder
    /// satisfies — Llama-2 uses `" "`) and materialise it into
    /// [`Decoder::Strip`]. A multi-character `content` surfaces
    /// [`HfConversionError::UnsupportedDecoder`].
    Strip {
        /// The character to strip — on disk a JSON string of length 1.
        content: String,
        /// Maximum leading occurrences to remove.
        #[serde(default)]
        start: usize,
        /// Maximum trailing occurrences to remove.
        #[serde(default)]
        stop: usize,
    },
    /// `SentencePiece` byte-fallback reassembly at the decoder-chain
    /// level. Materialises into [`Decoder::ByteFallback`]. Distinct
    /// from the model-side `byte_fallback` flag under
    /// [`HfBpeModel::byte_fallback`] / [`HfUnigramModel::byte_fallback`]:
    /// the model-side flag controls the *encode* path (how OOV chars
    /// become ids); the decoder-side stage controls the *decode* path
    /// (how `<0xXX>` surface strings reassemble into UTF-8 chars).
    /// Both stages typically ship together in real checkpoints — Llama-2
    /// enables the model-side flag *and* wires `ByteFallback` into
    /// its decoder chain, and this crate honours both.
    ByteFallback,
    /// Any other decoder (`WordPiece`, `Metaspace`, `BPEDecoder`,
    /// ...). Serde's `#[serde(other)]` catches every tag string that
    /// does not match the variants listed above; the payload beyond
    /// the tag is discarded (callers who need to inspect a rejected
    /// decoder can re-parse the raw JSON themselves — the original
    /// `tokenizer.json` byte string is the authoritative source).
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
/// Deferred variants (`Nmt`, custom callables) surface at conversion
/// as [`HfConversionError::UnsupportedNormalizer`] with the offending
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
    /// String or regex substitution. [`HfPattern::String`] materialises
    /// into [`Normalizer::Replace`] (literal search-and-replace);
    /// [`HfPattern::Regex`] materialises into
    /// [`Normalizer::ReplaceRegex`] (compiled `fancy-regex` pattern
    /// applied at every [`normalize`](crate::normalizer::normalize)
    /// call — see that variant for the compile-per-call caveat).
    Replace {
        /// The pattern block. Both [`HfPattern::String`] and
        /// [`HfPattern::Regex`] are honoured.
        pattern: HfPattern,
        /// The replacement string. Used verbatim — regex capture
        /// group back-references (`$1`, `${name}`) are not interpreted
        /// even when `pattern` is a regex.
        content: String,
    },
    /// Trim whitespace from one or both sides.
    ///
    /// HF's on-disk `tokenizer.json` shape names the two toggles
    /// `strip_left` and `strip_right` (matching upstream's
    /// [`tokenizers-rs`][hf-strip] source); those are the primary
    /// serde names. The bare `left` / `right` spellings are accepted
    /// as aliases so any legacy blob written by tools that use the
    /// short names still parses.
    ///
    /// [hf-strip]: https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/normalizers/strip.rs
    Strip {
        /// If `true`, strip leading whitespace.
        #[serde(rename = "strip_left", alias = "left", default = "default_true")]
        strip_left: bool,
        /// If `true`, strip trailing whitespace.
        #[serde(rename = "strip_right", alias = "right", default = "default_true")]
        strip_right: bool,
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
/// (Llama / BERT shape), [`Self::BertProcessing`] (stock BERT shape),
/// [`Self::RobertaProcessing`] (`XLM-RoBERTa` / `RoBERTa`),
/// [`Self::ByteLevel`] (GPT-2 shape), and [`Self::Sequence`]
/// (composition of nested post-processors). Every other tag string
/// falls through to [`Self::Other`] via serde's `#[serde(other)]` and
/// surfaces [`HfConversionError::UnsupportedPostProcessor`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum HfPostProcessor {
    /// The Llama-shape template that injects BOS/EOS around the
    /// primary encoding. See [`TemplateProcessing`] for the semantics.
    TemplateProcessing(HfTemplateProcessing),
    /// The stock BERT `BertProcessing` shape — a fixed
    /// `[CLS] $A [SEP]` splice with no byte-level flags. Both `cls`
    /// and `sep` are the on-disk `[surface_string, id]` two-element
    /// tuples HF stores. See [`BertProcessing`] for the runtime
    /// semantics.
    BertProcessing(HfBertProcessing),
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
    /// Composition of nested post-processors. HF's on-disk shape is
    /// `{"type": "Sequence", "processors": [...]}` — the `processors`
    /// field carries the ordered child list, which is threaded through
    /// each child's `apply` at process time. See [`PostProcessor::Sequence`]
    /// for the runtime semantics; nested `Sequence` values are permitted.
    Sequence {
        /// The ordered child post-processors. HF's on-disk field
        /// name is `processors` — mirrored verbatim here.
        #[serde(default)]
        processors: Vec<HfPostProcessor>,
    },
    /// Any other post-processor tag string. Rejected at conversion time.
    #[serde(other)]
    Other,
}

/// The typed shape of a `BertProcessing` post-processor.
///
/// Field names mirror HF's on-disk layout verbatim. `sep` and `cls`
/// are the two-element `[surface_string, id]` tuples HF writes. Unlike
/// [`HfRobertaProcessing`] there are no `trim_offsets` or
/// `add_prefix_space` fields — HF's own `BertProcessing` type has
/// none.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfBertProcessing {
    /// The `[surface_string, id]` tuple emitted at the SEP slot after
    /// the primary encoding.
    pub sep: HfBertSpecial,
    /// The `[surface_string, id]` tuple emitted at the CLS slot before
    /// the primary encoding.
    pub cls: HfBertSpecial,
}

/// The `[surface_string, id]` pair HF stores under `sep` / `cls` in a
/// `BertProcessing` block. Deserialised via a serde two-element-tuple
/// newtype so `["[CLS]", 101]` parses directly. Structurally identical
/// to [`HfRobertaSpecial`] but kept separate for documentation
/// clarity.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HfBertSpecial(pub String, pub TokenId);

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

/// The `"truncation"` block of a `tokenizer.json` file.
///
/// Field names and defaults mirror HF's `TruncationParams` on-disk
/// shape verbatim — `direction` defaults to `"Right"`, `strategy`
/// defaults to `"LongestFirst"`, and `stride` defaults to `0`. Every
/// field is preserved on [`HfTokenizerConfig::truncation`]; a
/// non-`None` value is applied to the runtime tokenizer via
/// [`crate::BpeTokenizer::with_truncation`] (and its
/// `WordPiece`/`WordLevel`/`Unigram` siblings) at conversion time, so
/// [`stringcheese_tokenizer::Tokenizer::encode`] applies truncation
/// automatically.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfTruncationParams {
    /// Maximum length in tokens.
    pub max_length: usize,
    /// Which side of the encoding to drop from. Defaults to
    /// [`HfTruncationDirection::Right`].
    #[serde(default)]
    pub direction: HfTruncationDirection,
    /// How to trim a pair of encodings. Defaults to
    /// [`HfTruncationStrategy::LongestFirst`].
    #[serde(default)]
    pub strategy: HfTruncationStrategy,
    /// Overlap window HF carries between adjacent chunks when a long
    /// input is chunked into multiple encodings. Preserved on the
    /// runtime config for round-trip fidelity but not consumed by this
    /// crate's truncation module — chunking is a caller-side concern.
    #[serde(default)]
    pub stride: usize,
}

/// HF's on-disk truncation direction tag.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
pub enum HfTruncationDirection {
    /// Drop from the tail (HF default).
    #[default]
    Right,
    /// Drop from the head.
    Left,
}

impl From<HfTruncationDirection> for stringcheese_tokenizer::truncation::TruncationDirection {
    fn from(v: HfTruncationDirection) -> Self {
        match v {
            HfTruncationDirection::Right => Self::Right,
            HfTruncationDirection::Left => Self::Left,
        }
    }
}

/// HF's on-disk truncation strategy tag.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
pub enum HfTruncationStrategy {
    /// Alternately drop from whichever side is longer (HF default).
    #[default]
    LongestFirst,
    /// Only trim the first side.
    OnlyFirst,
    /// Only trim the second side.
    OnlySecond,
    /// Do not truncate.
    DoNotTruncate,
}

impl From<HfTruncationStrategy> for stringcheese_tokenizer::truncation::TruncationStrategy {
    fn from(v: HfTruncationStrategy) -> Self {
        match v {
            HfTruncationStrategy::LongestFirst => Self::LongestFirst,
            HfTruncationStrategy::OnlyFirst => Self::OnlyFirst,
            HfTruncationStrategy::OnlySecond => Self::OnlySecond,
            HfTruncationStrategy::DoNotTruncate => Self::DoNotTruncate,
        }
    }
}

impl From<HfTruncationParams> for stringcheese_tokenizer::truncation::TruncationConfig {
    fn from(v: HfTruncationParams) -> Self {
        Self {
            max_length: v.max_length,
            strategy: v.strategy.into(),
            direction: v.direction.into(),
            stride: v.stride,
        }
    }
}

/// The `"padding"` block of a `tokenizer.json` file.
///
/// Field names and defaults mirror HF's `PaddingParams` on-disk
/// shape verbatim.
///
/// * `strategy` is HF's `"BatchLongest"` or
///   `{"Fixed": <usize>}` — routed to the runtime
///   [`stringcheese_tokenizer::padding::PaddingStrategy`].
/// * `direction` defaults to `"Right"`.
/// * `pad_id` / `pad_type_id` are numeric; `pad_token` is the
///   surface string (preserved but not consumed by the runtime — the
///   runtime pads by id only).
/// * `pad_to_multiple_of` is preserved verbatim but not consumed today
///   (a niche feature that no shipped tokenizer.json we've inspected
///   sets to a non-null value).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct HfPaddingParams {
    /// How to compute the target length.
    pub strategy: HfPaddingStrategy,
    /// Which side of the encoding to append pad tokens to. Defaults to
    /// [`HfPaddingDirection::Right`].
    #[serde(default)]
    pub direction: HfPaddingDirection,
    /// The pad token id.
    pub pad_id: TokenId,
    /// The pad token's `type_id`. Defaults to `0`.
    #[serde(default)]
    pub pad_type_id: u32,
    /// The pad token's surface string. Preserved for round-trip
    /// fidelity but not consumed by the runtime.
    #[serde(default)]
    pub pad_token: String,
    /// HF's "pad every encoding up to a multiple of N" niche. Preserved
    /// verbatim; not consumed.
    #[serde(default)]
    pub pad_to_multiple_of: Option<usize>,
}

/// HF's on-disk padding strategy tag.
///
/// The `BatchLongest` variant is a bare string (`"BatchLongest"`); the
/// `Fixed` variant is a tagged object (`{"Fixed": 512}`).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum HfPaddingStrategy {
    /// The `"BatchLongest"` bare-string form.
    Named(HfPaddingStrategyNamed),
    /// The `{"Fixed": N}` tagged-object form.
    Tagged(HfPaddingStrategyTagged),
}

/// The bare-string form of [`HfPaddingStrategy`].
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum HfPaddingStrategyNamed {
    /// Pad to the longest encoding in the batch.
    BatchLongest,
}

/// The tagged-object form of [`HfPaddingStrategy`].
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum HfPaddingStrategyTagged {
    /// Pad every encoding to the given fixed length.
    Fixed(usize),
}

impl From<HfPaddingStrategy> for stringcheese_tokenizer::padding::PaddingStrategy {
    fn from(v: HfPaddingStrategy) -> Self {
        match v {
            HfPaddingStrategy::Named(HfPaddingStrategyNamed::BatchLongest) => Self::BatchLongest,
            HfPaddingStrategy::Tagged(HfPaddingStrategyTagged::Fixed(n)) => Self::Fixed(n),
        }
    }
}

/// HF's on-disk padding direction tag.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
pub enum HfPaddingDirection {
    /// Append pad tokens on the right (HF default).
    #[default]
    Right,
    /// Prepend pad tokens on the left.
    Left,
}

impl From<HfPaddingDirection> for stringcheese_tokenizer::padding::PaddingDirection {
    fn from(v: HfPaddingDirection) -> Self {
        match v {
            HfPaddingDirection::Right => Self::Right,
            HfPaddingDirection::Left => Self::Left,
        }
    }
}

impl From<HfPaddingParams> for stringcheese_tokenizer::padding::PaddingConfig<TokenId> {
    fn from(v: HfPaddingParams) -> Self {
        Self {
            strategy: v.strategy.into(),
            pad_id: v.pad_id,
            pad_type_id: v.pad_type_id,
            direction: v.direction.into(),
        }
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
    /// `model.type` is one this crate does not materialise. Every
    /// currently-shipped variant (`BPE`, `WordPiece`, `Unigram`,
    /// `WordLevel`) has its own conversion path, so this variant is
    /// currently unreachable from the loader — kept for
    /// forward-compatibility if HF adds a fifth model type. Carries
    /// the specific type name.
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
    /// [`to_wordlevel_tokenizer`] was called on a config whose
    /// `model.type` is not `"WordLevel"`. Carries the specific type
    /// name.
    UnsupportedModelForWordLevel {
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
    /// A `WordLevel` config's `unk_token` surface string is not in
    /// the vocab. Encoding an out-of-vocab word would emit an id the
    /// caller cannot decode.
    WordLevelUnkNotInVocab {
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
    /// materialise yet (`Nmt` or a custom callable). `Precompiled` is
    /// honoured as a passthrough and does not surface here; `Replace`
    /// with either a literal or regex pattern is honoured.
    UnsupportedNormalizer {
        /// The `normalizer.type` string, or a short synthesised name
        /// for a nested rejection.
        type_name: String,
    },
    /// The `post_processor` block used a tag string this crate does
    /// not materialise (any variant not in the honoured set —
    /// `TemplateProcessing`, `BertProcessing`, `RobertaProcessing`,
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
    /// A `Unigram` or `BPE` config set `byte_fallback: true` but the
    /// vocabulary is missing coverage for one or more of the 256
    /// bytes the mechanism requires. Every byte value from `0x00` to
    /// `0xFF` needs a resolvable id in the vocab, in one of two
    /// shapes: either the reserved surface string `<0x00>`, `<0x01>`,
    /// ..., `<0xFF>`, *or* a literal single-byte token whose surface
    /// is exactly that byte (e.g. byte `0x09` → a vocab entry keyed by
    /// the literal tab character `"\t"`). Without either shape,
    /// encoding an out-of-vocab character has no well-defined fallback
    /// path. Surfaced by both [`to_unigram_tokenizer`] and
    /// [`to_bpe_tokenizer`] — real Llama-2 / Mistral / Qwen
    /// `tokenizer.json` blobs ship as BPE with this flag set and all
    /// 256 reserved surfaces; XLM-RoBERTa-style `SentencePiece`
    /// checkpoints ship as Unigram; Gemma-family checkpoints ship 255
    /// reserved surfaces plus one literal single-byte token.
    ByteFallbackTokensMissing {
        /// How many bytes are missing coverage: the count of bytes
        /// for which *both* the reserved `<0xXX>` surface *and* a
        /// literal single-byte fallback token are absent from the
        /// vocabulary.
        missing_count: usize,
        /// The lowest byte value with no resolvable coverage — enough
        /// to give the caller a starting point without dumping all
        /// 256 possible values.
        first_missing_byte: u8,
    },
    /// The `decoder` block used a tag or field this crate cannot
    /// materialise. Currently surfaces for
    /// [`HfDecoder::Replace`] with an [`HfPattern::Regex`] (the
    /// runtime is literal-only), for [`HfDecoder::Strip`] whose
    /// `content` string is not exactly one Unicode scalar, and for
    /// [`HfDecoder::Other`] (unrecognised tag names such as
    /// `WordPieceDecoder`, `Metaspace`, `BPEDecoder`, ...).
    UnsupportedDecoder {
        /// A short label naming the offending shape.
        reason: String,
    },
}

impl fmt::Display for HfConversionError {
    // One arm per variant of a wide, non-exhaustive error enum — every
    // arm is a single `write!` call, so the length is proportional to
    // the variant count rather than the algorithmic complexity of the
    // function. Splitting it into per-variant helpers would obscure
    // the shape reviewers actually want to see (one arm per
    // `HfConversionError` variant, all in one place).
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedModel { type_name } => write!(
                f,
                "unsupported HF model type {type_name:?} \
                 (this crate materialises \"BPE\", \"WordPiece\", \"Unigram\", \
                 and \"WordLevel\"; every other tag is unrecognised)"
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
            Self::UnsupportedModelForWordLevel { type_name } => write!(
                f,
                "to_wordlevel_tokenizer called on non-WordLevel model type {type_name:?}; \
                 use to_bpe_tokenizer, to_wordpiece_tokenizer, to_unigram_tokenizer, \
                 or to_tokenizer instead"
            ),
            Self::UnigramUnkIdOutOfRange { unk_id, vocab_size } => write!(
                f,
                "Unigram model's unk_id {unk_id} is out of range for a vocabulary of size {vocab_size}"
            ),
            Self::WordPieceUnkNotInVocab { unk_token } => write!(
                f,
                "WordPiece model's unk_token {unk_token:?} is not present in the vocabulary"
            ),
            Self::WordLevelUnkNotInVocab { unk_token } => write!(
                f,
                "WordLevel model's unk_token {unk_token:?} is not present in the vocabulary"
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
                 Replace(String), Replace(Regex), Strip, Prepend, \
                 BertNormalizer, Precompiled (passthrough), and their \
                 Sequence composition"
            ),
            Self::UnsupportedPostProcessor { type_name } => write!(
                f,
                "unsupported HF post_processor type {type_name:?}: \
                 this crate materialises \"TemplateProcessing\", \
                 \"BertProcessing\", \"RobertaProcessing\", \"ByteLevel\", \
                 and \"Sequence\""
            ),
            Self::TemplateSpecialTokenNotDeclared { name } => write!(
                f,
                "TemplateProcessing template references special-token name \
                 {name:?} that is missing from its own \"special_tokens\" map"
            ),
            Self::ByteFallbackTokensMissing {
                missing_count,
                first_missing_byte,
            } => write!(
                f,
                "model sets byte_fallback: true but its vocabulary \
                 is missing coverage for {missing_count} of the 256 \
                 byte-fallback bytes — every byte needs either its \
                 reserved <0xXX> surface or a literal single-byte token \
                 in the vocab (first uncovered byte: 0x{first_missing_byte:02X}, \
                 checked for both `<0x{first_missing_byte:02X}>` and a \
                 literal one-byte surface)"
            ),
            Self::UnsupportedDecoder { reason } => {
                write!(f, "unsupported HF decoder configuration: {reason}")
            }
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

/// Scan a BPE vocabulary for the 256 reserved `<0x00>`..`<0xFF>`
/// byte-fallback tokens and return the resolved `byte → id` array.
///
/// Mirrors the Unigram-side scan in
/// [`UnigramTokenizer::with_byte_fallback`] byte for byte: the first
/// occurrence of each surface wins, uppercase and lowercase hex both
/// count.
///
/// When the reserved `<0xXX>` surface for a byte is *absent*, the scan
/// falls back to searching the vocab for a **literal single-byte token**
/// whose surface is exactly that byte (e.g. byte `0x09` → a vocab entry
/// keyed by the literal tab character `"\t"`). This second shape covers
/// checkpoints such as `google/gemma-2b` which ship
/// `byte_fallback: true` alongside 255 of 256 `<0xXX>` surfaces and
/// substitute a literal single-byte token for the remaining slot.
/// Because a `String` cannot hold a lone `0x80..=0xFF` byte, only ASCII
/// bytes (`0x00..=0x7F`) can be covered by this second shape — that is
/// the same set every real-world checkpoint hits.
///
/// Only if *both* the reserved surface *and* the literal single-byte
/// fallback are missing for a byte does the scan surface
/// [`HfConversionError::ByteFallbackTokensMissing`] with the count of
/// unresolved bytes plus the first such byte value.
fn resolve_bpe_byte_fallback_ids(
    vocab: &BTreeMap<String, TokenId>,
) -> Result<[TokenId; 256], HfConversionError> {
    let mut ids: [Option<TokenId>; 256] = [None; 256];
    // Second-pass fallback: id of a literal single-byte surface for
    // byte `b`, if any. Only bytes `0x00..=0x7F` can appear as a
    // one-byte `String` key (bytes `0x80..=0xFF` on their own are not
    // valid UTF-8), but the array is written over the full 0..256 so
    // the resolution logic below stays uniform.
    let mut literal_ids: [Option<TokenId>; 256] = [None; 256];
    for (surface, &id) in vocab {
        if let Some(b) = parse_byte_fallback_surface(surface) {
            // First occurrence wins — matches the tokenizer's own
            // vocab-lookup direction and the Unigram-side scan.
            if ids[b as usize].is_none() {
                ids[b as usize] = Some(id);
            }
        } else if surface.len() == 1 {
            let b = surface.as_bytes()[0];
            if literal_ids[b as usize].is_none() {
                literal_ids[b as usize] = Some(id);
            }
        }
    }
    let mut resolved: [TokenId; 256] = [0u32; 256];
    let mut missing_count = 0usize;
    let mut first_missing: Option<u8> = None;
    for b in 0usize..256 {
        // Prefer the reserved `<0xXX>` surface; fall back to a literal
        // single-byte token when it is absent. Both shapes are
        // equivalent under the byte-fallback contract: encode emits
        // whichever id ends up in the table for a byte that survives
        // the merge loop, and decode's reverse-lookup maps the id back
        // to the byte for the flush-run path.
        if let Some(id) = ids[b].or(literal_ids[b]) {
            resolved[b] = id;
        } else {
            missing_count += 1;
            if first_missing.is_none() {
                // `b` iterates 0..256 so the cast is exact;
                // `try_from(...).ok()` sidesteps clippy's truncation
                // lint the same way the Unigram-side scan does.
                first_missing = u8::try_from(b).ok();
            }
        }
    }
    if missing_count != 0 {
        return Err(HfConversionError::ByteFallbackTokensMissing {
            missing_count,
            first_missing_byte: first_missing.unwrap_or(0),
        });
    }
    Ok(resolved)
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
/// single-space-joined string. Additionally, a `model.byte_fallback:
/// true` config whose vocabulary is missing any of the 256 reserved
/// `<0xXX>` byte tokens surfaces
/// [`HfConversionError::ByteFallbackTokensMissing`].
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
            return Err(HfConversionError::UnsupportedModelForBpe {
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
    // a Sequence combining one Split(Regex) with one ByteLevel, a
    // SentencePiece Metaspace shape (bare or inside a Sequence — the
    // Mistral-7B-v0.1 layout), or nothing at all. Metaspace routes
    // through `extract_pre_tokenizer_sequence` and is attached separately
    // via `BpeTokenizer::with_pre_tokenizer_sequence`; the other shapes
    // stay on the regex/byte-level pipeline through `extract_pre_tokenizer`.
    // See both helpers for the exact rules.
    let mut sequence_pre_tokenizer: Option<PreTokenizerSequence> = None;
    let pipeline = match &config.pre_tokenizer {
        None => PreTokPipeline::None,
        Some(pt) if pre_tokenizer_uses_metaspace(pt) => {
            sequence_pre_tokenizer = Some(extract_pre_tokenizer_sequence(pt)?);
            PreTokPipeline::None
        }
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

    // Byte-fallback — the `SentencePiece` mechanism that reroutes the
    // OOV path from `unk` to the 256 reserved `<0xXX>` tokens. Enable
    // it early (after the vocab is assembled but before the ancillary
    // pipeline pieces) so a missing-tokens error surfaces before any
    // pre-tokenizer / normalizer / post-processor conversion runs.
    // Mirrors the Unigram-side landing in `to_unigram_tokenizer`.
    if bpe.byte_fallback == Some(true) {
        tok = tok.with_byte_fallback(resolve_bpe_byte_fallback_ids(&bpe.vocab)?);
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

    // Metaspace pre-tokenizer (SentencePiece — Mistral / Llama-family
    // character-BPE checkpoints that place `▁` marking on the
    // pre-tokenizer side rather than the normalizer side). Attach after
    // the regex/byte-level pipeline so the two are mutually exclusive
    // by construction: `pre_tokenizer_uses_metaspace` above steered
    // Metaspace-shaped configs to `PreTokPipeline::None`.
    if let Some(seq) = sequence_pre_tokenizer {
        tok = tok.with_pre_tokenizer_sequence(seq);
    }

    // Decoder — every honoured HfDecoder variant materialises into a
    // runtime `Decoder` (ByteLevel keeps its byte-buffer legacy path;
    // Sequence / Replace / Fuse / Strip / ByteFallback take the chain
    // path). Unrecognised tags fall through to the tokenizer's default
    // decoder — see [`to_runtime_decoder`] for the soft-fail
    // rationale.
    if let Some(hd) = &config.decoder {
        if let Some(dec) = to_runtime_decoder(hd)? {
            tok = tok.with_decoder(dec);
        }
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
    if let Some(t) = &config.truncation {
        tok = tok.with_truncation(t.clone().into());
    }
    if let Some(p) = &config.padding {
        tok = tok.with_padding(p.clone().into());
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
/// * [`HfTokenizer::WordLevel`] wraps a
///   [`crate::wordlevel::WordLevelTokenizer`] — every
///   `model.type == "WordLevel"` config (specialised BERT-family and
///   fixed-vocabulary checkpoints) lands here.
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
    /// A [`crate::wordlevel::WordLevelTokenizer`] materialised from a
    /// `WordLevel` model.
    WordLevel(crate::wordlevel::WordLevelTokenizer),
}

/// Materialise an [`HfTokenizerConfig`] as a runnable [`HfTokenizer`].
///
/// Dispatches on `model.type`: `BPE` produces
/// [`HfTokenizer::Bpe`]; `WordPiece` produces
/// [`HfTokenizer::WordPiece`]; `Unigram` produces
/// [`HfTokenizer::Unigram`]; `WordLevel` produces
/// [`HfTokenizer::WordLevel`]. Every currently-shipped HF model type
/// is covered; a fifth type added upstream would surface
/// [`HfConversionError::UnsupportedModel`] via the tagged-enum's
/// "unknown variant" error at deserialisation time before ever
/// reaching this dispatch.
///
/// # Errors
///
/// Returns any [`HfConversionError`] the underlying
/// [`to_bpe_tokenizer`] / [`to_wordpiece_tokenizer`] /
/// [`to_unigram_tokenizer`] / [`to_wordlevel_tokenizer`] would.
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
        HfModel::WordLevel(_) => Ok(HfTokenizer::WordLevel(to_wordlevel_tokenizer(config)?)),
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
///   `Replace(String)` / `Replace(Regex)` / their `Sequence`
///   composition are honoured the same way; deferred variants
///   (`Nmt`, ...) surface
///   [`HfConversionError::UnsupportedNormalizer`].
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

    // Special tokens — `added_tokens[*]` entries with `special == true`
    // must be pre-extracted from raw input before the Bert
    // pre-tokenizer's punctuation split otherwise splits `[CLS]` into
    // `[`, `CLS`, `]`. Mirrors `to_bpe_tokenizer`'s specials wiring.
    // HF's own schema treats `special` as `false` when absent, and
    // some checkpoints ship added_tokens with `special: false`
    // (`##`-continuations added by tokenizer training loops, etc.);
    // filtering to `special == true` is the same rule
    // `to_bpe_tokenizer` uses.
    let mut wp_specials: BTreeMap<String, TokenId> = BTreeMap::new();
    for at in &config.added_tokens {
        if at.special {
            wp_specials.insert(at.content.clone(), at.id);
        }
    }
    if !wp_specials.is_empty() {
        tok = tok.with_special_tokens(wp_specials);
    }

    // Pre-tokenizer routing. `WordPiece` cares only about the
    // whitespace / punctuation split; the shape carried in a
    // `Sequence` around one of the supported entries is unwrapped.
    let pre = extract_wordpiece_pre_tokenizer(config.pre_tokenizer.as_ref())?;
    tok = tok.with_pre_tokenizer(pre);

    // Normalizer — runs before the pre-tokenizer at encode time. The
    // shared `to_runtime_normalizer` honours NFC / NFD / NFKC / NFKD /
    // Lowercase / Strip / Prepend / Replace(String) / Replace(Regex) /
    // BertNormalizer / their Sequence composition; everything else
    // surfaces as `UnsupportedNormalizer`. Mirrors `to_bpe_tokenizer`'s
    // wiring — the runtime `WordPieceTokenizer` now carries a
    // normalizer slot, so the parsed value is attached and applied
    // end-to-end.
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

    // Decoder — honour any HF decoder chain the config carries. The
    // WordPiece runtime's default decode path already glues subwords
    // with the continuing-subword prefix and inserts spaces between
    // full words, so a checkpoint that ships without a decoder block
    // — or with a shape this crate doesn't materialise (BERT's own
    // `WordPieceDecoder` falls through to `Ok(None)` per
    // [`to_runtime_decoder`]'s soft-fail rationale) — keeps that
    // behaviour.
    if let Some(hd) = &config.decoder {
        if let Some(dec) = to_runtime_decoder(hd)? {
            tok = tok.with_decoder(dec);
        }
    }

    if let Some(t) = &config.truncation {
        tok = tok.with_truncation(t.clone().into());
    }
    if let Some(p) = &config.padding {
        tok = tok.with_padding(p.clone().into());
    }

    Ok(tok)
}

/// Materialise an [`HfTokenizerConfig`] as a runnable
/// [`crate::wordlevel::WordLevelTokenizer`].
///
/// The config's `model.type` must be `"WordLevel"`; any other type
/// (including `"BPE"`) surfaces
/// [`HfConversionError::UnsupportedModelForWordLevel`].
///
/// Supported ancillary features today:
///
/// * `pre_tokenizer.type ∈ {"WhitespaceSplit", "Whitespace",
///   "BertPreTokenizer"}` — routes through the corresponding
///   [`crate::wordlevel::WordLevelPreTokenizer`] variant. A missing
///   `pre_tokenizer` block defaults to
///   [`crate::wordlevel::WordLevelPreTokenizer::WhitespaceSplit`] —
///   the shape real HF `WordLevel` checkpoints ship.
/// * The `WordLevel` model's `unk_token` field is resolved to an id
///   via the vocab; a missing entry surfaces
///   [`HfConversionError::WordLevelUnkNotInVocab`].
/// * `normalizer` — every variant this crate's internal normalizer
///   materialiser accepts (see [`HfNormalizer`] for the shipped tags)
///   is attached to the produced tokenizer via
///   [`crate::wordlevel::WordLevelTokenizer::with_normalizer`], so
///   `encode` runs `normalize -> pre-tokenize -> lookup -> post-process`
///   end to end.
/// * `post_processor` — every variant this crate's internal
///   post-processor materialiser accepts (see [`HfPostProcessor`] for
///   the shipped tags) is attached via
///   [`crate::wordlevel::WordLevelTokenizer::with_post_processor`].
///
/// **Deferred** ancillary features (parse but reject at conversion):
///
/// * `decoder` — the raw config is preserved on
///   [`HfTokenizerConfig::decoder`] for caller inspection but not
///   applied; [`crate::wordlevel::WordLevelTokenizer::decode`] joins
///   surface strings with single ASCII spaces regardless of what the
///   config declares (the only shape a real `WordLevel` checkpoint
///   ships).
///
/// # Errors
///
/// * [`HfConversionError::UnsupportedModelForWordLevel`] — the
///   config's `model.type` is not `"WordLevel"`.
/// * [`HfConversionError::WordLevelUnkNotInVocab`] — the config's
///   `unk_token` surface string is not present in the vocabulary.
/// * [`HfConversionError::UnsupportedNormalizer`] /
///   [`HfConversionError::UnsupportedPreTokenizer`] /
///   [`HfConversionError::UnsupportedPostProcessor`] — the config
///   references an ancillary shape this crate does not materialise.
/// * [`HfConversionError::Vocabulary`] — the vocabulary is
///   internally inconsistent (duplicate id or duplicate surface).
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_hf::hf::{parse_tokenizer_json, to_wordlevel_tokenizer};
///
/// let json = r#"{
///     "added_tokens": [],
///     "model": {
///         "type": "WordLevel",
///         "vocab": {"[UNK]": 0, "hello": 1, "world": 2},
///         "unk_token": "[UNK]"
///     }
/// }"#;
/// let config = parse_tokenizer_json(json).unwrap();
/// let tok = to_wordlevel_tokenizer(&config).unwrap();
/// assert_eq!(tok.encode("hello world").unwrap(), vec![1, 2]);
/// assert_eq!(tok.encode("unknown").unwrap(), vec![0]);
/// ```
pub fn to_wordlevel_tokenizer(
    config: &HfTokenizerConfig,
) -> Result<crate::wordlevel::WordLevelTokenizer, HfConversionError> {
    let wl = match &config.model {
        HfModel::WordLevel(wl) => wl,
        HfModel::Bpe(_) => {
            return Err(HfConversionError::UnsupportedModelForWordLevel {
                type_name: "BPE".to_string(),
            });
        }
        HfModel::WordPiece(_) => {
            return Err(HfConversionError::UnsupportedModelForWordLevel {
                type_name: "WordPiece".to_string(),
            });
        }
        HfModel::Unigram(_) => {
            return Err(HfConversionError::UnsupportedModelForWordLevel {
                type_name: "Unigram".to_string(),
            });
        }
    };

    // Resolve unk_token surface string to an id via the vocab.
    let Some(&unk_id) = wl.vocab.get(&wl.unk_token) else {
        return Err(HfConversionError::WordLevelUnkNotInVocab {
            unk_token: wl.unk_token.clone(),
        });
    };

    // Fold added_tokens into the vocabulary so callers who inspect
    // added specials find them under the same lookup as the model
    // vocab. Overlapping (id, surface) pairs are idempotent; a
    // conflicting one surfaces as a vocabulary-builder error.
    let mut vocab: BTreeMap<String, TokenId> = wl.vocab.clone();
    for at in &config.added_tokens {
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

    // Assemble via `from_parts`; propagate its build errors as
    // targeted HF-conversion errors so callers get a specific
    // diagnostic.
    let mut tok = crate::wordlevel::WordLevelTokenizer::from_parts(vocab, Some(unk_id)).map_err(
        |e| match e {
            crate::wordlevel::WordLevelBuildError::UnkNotInVocab(_) => {
                HfConversionError::WordLevelUnkNotInVocab {
                    unk_token: wl.unk_token.clone(),
                }
            }
            crate::wordlevel::WordLevelBuildError::Vocabulary(v) => {
                HfConversionError::Vocabulary(v)
            }
        },
    )?;

    // Pre-tokenizer routing. `WordLevel` cares only about the
    // whitespace / punctuation split; a `Sequence` around one of the
    // supported entries is unwrapped.
    let pre = extract_wordlevel_pre_tokenizer(config.pre_tokenizer.as_ref())?;
    tok = tok.with_pre_tokenizer(pre);

    // Normalizer — runs before the pre-tokenizer at encode time.
    if let Some(hn) = &config.normalizer {
        let n = to_runtime_normalizer(hn)?;
        tok = tok.with_normalizer(n);
    }

    // Post-processor — runs on the finished encoding before it
    // leaves `encode`.
    if let Some(hp) = &config.post_processor {
        let pp = to_runtime_post_processor(hp)?;
        tok = tok.with_post_processor(pp);
    }
    if let Some(t) = &config.truncation {
        tok = tok.with_truncation(t.clone().into());
    }
    if let Some(p) = &config.padding {
        tok = tok.with_padding(p.clone().into());
    }

    Ok(tok)
}

/// Reduce an [`HfPreTokenizer`] (or its absence) to a
/// [`crate::wordlevel::WordLevelPreTokenizer`], following the
/// `WordLevel` routing rules documented on [`to_wordlevel_tokenizer`].
fn extract_wordlevel_pre_tokenizer(
    pt: Option<&HfPreTokenizer>,
) -> Result<crate::wordlevel::WordLevelPreTokenizer, HfConversionError> {
    use crate::wordlevel::WordLevelPreTokenizer;
    let Some(pt) = pt else {
        // Missing pre_tokenizer block: fall back to the WhitespaceSplit
        // default (what real WordLevel checkpoints ship).
        return Ok(WordLevelPreTokenizer::WhitespaceSplit);
    };
    match pt {
        HfPreTokenizer::WhitespaceSplit(_) => Ok(WordLevelPreTokenizer::WhitespaceSplit),
        HfPreTokenizer::Whitespace(_) => Ok(WordLevelPreTokenizer::Whitespace),
        HfPreTokenizer::BertPreTokenizer(_) => Ok(WordLevelPreTokenizer::Bert),
        HfPreTokenizer::Sequence { pretokenizers } => {
            if pretokenizers.is_empty() {
                return Ok(WordLevelPreTokenizer::WhitespaceSplit);
            }
            if pretokenizers.len() == 1 {
                return extract_wordlevel_pre_tokenizer(Some(&pretokenizers[0]));
            }
            Err(HfConversionError::AmbiguousSequencePreTokenizer {
                child_count: pretokenizers.len(),
            })
        }
        HfPreTokenizer::Split(_) => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "Split".to_string(),
            reason: "Split pre-tokenizers are for BPE; WordLevel uses whitespace / punctuation",
        }),
        HfPreTokenizer::ByteLevel(_) => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "ByteLevel".to_string(),
            reason: "ByteLevel pre-tokenizers are for byte-level BPE, not WordLevel",
        }),
        other => {
            if let Some(err) = deferred_pre_tokenizer_reason(other) {
                Err(err)
            } else {
                Err(HfConversionError::UnsupportedPreTokenizer {
                    type_name: "unknown".to_string(),
                    reason: "unhandled pre_tokenizer variant on WordLevel path",
                })
            }
        }
    }
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
/// reachable position, the runtime tries three strategies in order:
///
/// 1. **Byte-fallback** — if [`Self::with_byte_fallback`] was called
///    (typically because the source `tokenizer.json` set
///    `byte_fallback: true`), the offending character's UTF-8 bytes
///    are emitted as a run of `<0xXX>` reserved tokens (one id per
///    byte, so a 2-byte UTF-8 char produces two ids, a 3-byte char
///    produces three, and so on). Byte-fallback is preferred over
///    `unk` when both are configured — this matches upstream
///    `SentencePiece`, where `unk` on a byte-fallback-enabled vocab
///    is the "genuinely nothing left" path.
/// 2. **`unk` fallback** — if [`Self::unk_id`] is `Some` and byte-
///    fallback did not fire, the runtime takes a single-character
///    `unk` transition from `i - 1` to `i`, scored by the `unk`
///    token's own log probability minus a fixed penalty. The penalty
///    is chosen large enough that the fallback is only ever preferred
///    when no vocab-only path exists.
/// 3. **Hard error** — if neither is configured,
///    [`Self::encode`] returns
///    [`UnigramEncodeError::UntokenizableChar`] pointing at the
///    offending character.
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
    /// Optional pre-tokenizer sequence applied to the normalized text.
    /// When set, each piece produced by [`PreTokenizerSequence::apply`]
    /// is fed through the Viterbi loop independently, and the resulting
    /// ids are concatenated in order. When `None` the Viterbi loop runs
    /// on the whole normalized string as one piece — matching the
    /// pre-composition behaviour this type shipped with.
    ///
    /// The sequence shape (as opposed to the pre-composition bare
    /// [`Metaspace`]) is what makes composed configurations such as
    /// xlm-roberta-base's `Sequence[WhitespaceSplit, Metaspace]`
    /// materialise correctly: `WhitespaceSplit` collapses runs of
    /// whitespace before Metaspace inserts its `▁` markers, so a
    /// three-space run does not become three consecutive `▁` pieces.
    /// Backward compat is preserved via `From<Metaspace>`, so callers
    /// who pass a bare `Metaspace` to [`Self::with_pre_tokenizer`]
    /// still get the pre-composition single-stage behaviour.
    pre_tokenizer: Option<PreTokenizerSequence>,
    /// Optional post-processor applied to the finished [`Encoding`]
    /// before it leaves [`Self::encode`]. Mirrors
    /// [`BpeTokenizer::with_post_processor`]; the default
    /// [`PostProcessor::None`] is a pass-through so callers who never
    /// configure one see the raw Viterbi output.
    post_processor: PostProcessor,
    /// Special-token surface strings that must be pre-extracted from
    /// raw (post-normalized) input before the Metaspace pre-tokenizer
    /// and the Viterbi loop see it. Mirrors
    /// [`BpeTokenizer::with_special_tokens`] /
    /// [`crate::wordpiece::WordPieceTokenizer::with_special_tokens`]:
    /// registered surfaces are longest-match extracted, each occurrence
    /// emits its pre-assigned id, and the between-specials chunks feed
    /// the pre-tokenizer + Viterbi pipeline. A default-empty map
    /// preserves the pre-Wave-14 behaviour where a literal `"<s>"` in
    /// the input goes through Viterbi as regular text.
    special_tokens: BTreeMap<String, TokenId>,
    /// Optional byte-fallback table: `byte_fallback[b]` is the vocab
    /// id of the `<0xBB>` token reserved for byte value `b`. Populated
    /// by [`Self::with_byte_fallback`] after scanning the vocab for
    /// the 256 reserved surfaces; `None` disables the fallback and
    /// leaves the encode path on its previous `unk`-only behaviour.
    /// Boxed so a clone of `UnigramTokenizer` avoids a 2 KiB memcpy
    /// on the common (byte-fallback-off) path.
    byte_fallback: Option<alloc::boxed::Box<[usize; 256]>>,
    /// Optional decoder chain applied at [`Self::decode`] time. When
    /// set, ids are converted to per-token surface strings (via the
    /// vocab) and threaded through the chain — mirroring HF's own
    /// `Decoder::decode_chain` — instead of running the crate's
    /// default Metaspace-reversing decoder. Only the "chain" variants
    /// of [`Decoder`] participate here; [`Decoder::Passthrough`] and
    /// [`Decoder::ByteLevel`] are kept in the enum for API-shape
    /// parity with [`BpeTokenizer::decoder`] and behave as identity
    /// on the Unigram path.
    decoder: Option<Decoder>,
    /// Optional truncation configuration; see
    /// [`crate::BpeTokenizer::with_truncation`] for the semantics.
    truncation: Option<stringcheese_tokenizer::truncation::TruncationConfig>,
    /// Optional padding configuration; see
    /// [`crate::BpeTokenizer::with_padding`] for the semantics.
    padding: Option<stringcheese_tokenizer::padding::PaddingConfig<TokenId>>,
}

/// One backtracking record in the Viterbi trellis: either a single
/// emitted id (the vocab-only and `unk` paths) or a run of
/// byte-fallback ids (up to four — UTF-8's maximum encoded length).
///
/// Held inline in `best_prev` so a byte-fallback transition never
/// allocates: the four-slot buffer is populated only up to `len` on
/// each `Bytes` transition, and backtracking walks it in reverse.
#[derive(Debug, Clone)]
enum UnigramTransition {
    /// One vocab entry (either a normal match or an `unk` fallback).
    Single(usize),
    /// A byte-fallback run: `buf[..len]` holds the byte-token ids to
    /// emit in forward order.
    Bytes {
        /// Buffer of at most 4 byte-token ids (UTF-8 max encoded
        /// length). Only `buf[..len]` is meaningful.
        buf: [usize; 4],
        /// Number of meaningful entries in `buf` (1..=4).
        len: usize,
    },
}

/// Parse a `<0xXX>` byte-fallback surface string and return the byte
/// value it maps to. Returns `None` for anything else — normal vocab
/// entries flow through untouched.
///
/// The surface has a fixed shape: exactly six ASCII bytes,
/// `<`, `0`, `x`, two hex digits, `>`. Uppercase and lowercase hex
/// digits are both accepted — `<0xff>` and `<0xFF>` both map to
/// byte `0xFF`. HF's own on-disk convention is uppercase.
///
/// Shared between the Unigram [`Self::with_byte_fallback`] scan and
/// the BPE-side [`to_bpe_tokenizer`] scan — both walk their vocabs
/// looking for the same 256 reserved surface strings and want the
/// same tolerant parse.
pub(crate) fn parse_byte_fallback_surface(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 6 {
        return None;
    }
    if bytes[0] != b'<' || bytes[1] != b'0' || bytes[2] != b'x' || bytes[5] != b'>' {
        return None;
    }
    let hi = decode_hex_digit(bytes[3])?;
    let lo = decode_hex_digit(bytes[4])?;
    Some((hi << 4) | lo)
}

/// Sort a Unigram special-token map into `(surface, id)` pairs,
/// longest surface first, with lexical order breaking ties. Mirrors
/// [`BpeTokenizer::sorted_specials`] /
/// [`crate::wordpiece`]'s helper so `<|im_start|>` matches before
/// `<|im|>` if both are registered.
fn sorted_unigram_special_tokens(specials: &BTreeMap<String, TokenId>) -> Vec<(String, TokenId)> {
    let mut v: Vec<(String, TokenId)> = specials.iter().map(|(k, v)| (k.clone(), *v)).collect();
    v.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Decode one ASCII hex digit (`0-9`, `A-F`, `a-f`) to its 0..=15
/// value; `None` for anything else.
const fn decode_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
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
            byte_fallback: None,
            special_tokens: BTreeMap::new(),
            decoder: None,
            truncation: None,
            padding: None,
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

    /// Attach (or replace) the pre-tokenizer.
    ///
    /// When set, [`Self::encode`] splits the normalized input into
    /// pieces via [`PreTokenizerSequence::apply`] and runs the Viterbi
    /// loop on each piece independently. This is what makes
    /// XLM-RoBERTa / Llama / Mistral / T5 encode identically to
    /// upstream `tokenizers-rs`: the `▁` mark inserted by the
    /// Metaspace stage carries word-initial position information into
    /// the vocab lookup, and any preceding `WhitespaceSplit` stage
    /// collapses runs of whitespace before that substitution happens.
    ///
    /// # Backward compatibility
    ///
    /// The parameter is `impl Into<PreTokenizerSequence>`, so callers
    /// can keep passing a bare [`Metaspace`] and it will be wrapped in
    /// a single-stage sequence with identical semantics to the
    /// pre-composition shape:
    ///
    /// ```ignore
    /// // Pre-composition: still compiles, still works.
    /// let tok = tok.with_pre_tokenizer(Metaspace::new());
    /// // Composed sequence: new xlm-roberta-shape support.
    /// let seq = PreTokenizerSequence::new(vec![
    ///     PreTokenizer::WhitespaceSplit,
    ///     PreTokenizer::Metaspace(Metaspace::new()),
    /// ]);
    /// let tok = tok.with_pre_tokenizer(seq);
    /// ```
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pre_tokenizer: impl Into<PreTokenizerSequence>) -> Self {
        self.pre_tokenizer = Some(pre_tokenizer.into());
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

    /// Attach (or replace) the decoder chain applied at
    /// [`Self::decode`] time.
    ///
    /// When set, `decode` walks each id to its per-token surface
    /// string (via the vocab) and threads the list through the chain
    /// (see [`Decoder`] for the shape). This bypasses the default
    /// Metaspace-reversal decode path — callers who want that
    /// behaviour must leave the decoder unset.
    ///
    /// Only the "chain" variants of [`Decoder`] have semantic effect
    /// here; [`Decoder::Passthrough`] and [`Decoder::ByteLevel`] are
    /// treated as identity because the Unigram runtime's byte-level
    /// interpretation lives on the pre-tokenizer / normalizer side.
    #[must_use]
    pub fn with_decoder(mut self, decoder: Decoder) -> Self {
        self.decoder = Some(decoder);
        self
    }

    /// Attach (or replace) the truncation configuration; see
    /// [`crate::BpeTokenizer::with_truncation`] for the semantics.
    #[must_use]
    pub fn with_truncation(
        mut self,
        truncation: stringcheese_tokenizer::truncation::TruncationConfig,
    ) -> Self {
        self.truncation = Some(truncation);
        self
    }

    /// Attach (or replace) the padding configuration; see
    /// [`crate::BpeTokenizer::with_padding`] for the semantics.
    #[must_use]
    pub fn with_padding(
        mut self,
        padding: stringcheese_tokenizer::padding::PaddingConfig<TokenId>,
    ) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Read-only access to the configured decoder chain, if any.
    #[must_use]
    pub fn decoder(&self) -> Option<&Decoder> {
        self.decoder.as_ref()
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

    /// Attach (or replace) the special-token map.
    ///
    /// Registered surfaces are pre-extracted from the normalized input
    /// (longest match first, ties broken lexically) *before* the
    /// Metaspace pre-tokenizer and Viterbi loop run; each occurrence
    /// emits its pre-assigned id directly and the between-specials
    /// chunks flow through the normal pipeline. Mirrors
    /// [`BpeTokenizer::with_special_tokens`] /
    /// [`crate::wordpiece::WordPieceTokenizer::with_special_tokens`].
    /// A default-empty map preserves the previous behaviour where a
    /// literal `"<s>"` in the input flowed through Viterbi as regular
    /// text.
    ///
    /// This is what makes xlm-roberta-base tokenize a literal `"<s>"`
    /// in the input as the CLS id (matching
    /// `transformers.AutoTokenizer`) instead of decomposing it into
    /// unrelated Metaspace pieces.
    #[must_use]
    pub fn with_special_tokens(mut self, special_tokens: BTreeMap<String, TokenId>) -> Self {
        self.special_tokens = special_tokens;
        self
    }

    /// Enable `SentencePiece`'s byte-fallback mechanism on this
    /// tokenizer.
    ///
    /// Scans the vocabulary for the 256 reserved `<0x00>`..`<0xFF>`
    /// surface strings and, if all 256 are present, stores a
    /// `byte → id` map on the tokenizer so [`Self::encode`] can emit
    /// a run of byte tokens whenever a character has no vocab-only
    /// path. [`Self::decode`] is likewise updated to reassemble runs
    /// of byte-fallback ids back into their UTF-8 characters.
    ///
    /// [`to_unigram_tokenizer`] calls this automatically when the
    /// source config's `model.byte_fallback` field is `Some(true)`;
    /// callers who assemble a tokenizer manually via
    /// [`Self::from_parts`] can call it themselves.
    ///
    /// # Scan strategy
    ///
    /// For each byte value the scan first prefers the reserved
    /// `<0xXX>` surface (uppercase and lowercase hex both accepted);
    /// when that surface is *absent* it falls back to a **literal
    /// single-byte token** whose surface is exactly that byte (e.g.
    /// byte `0x09` → a vocab entry keyed by the literal tab character
    /// `"\t"`). This second shape covers checkpoints that ship
    /// `byte_fallback: true` alongside 255 of 256 `<0xXX>` surfaces —
    /// see the BPE-side [`to_bpe_tokenizer`] scan for the same
    /// relaxation and the Gemma-family motivating case. Only bytes
    /// `0x00..=0x7F` can be represented as a one-byte `String` key
    /// (bytes `0x80..=0xFF` on their own are not valid UTF-8), which
    /// is the same set every real-world checkpoint hits.
    ///
    /// # Errors
    ///
    /// Returns [`HfConversionError::ByteFallbackTokensMissing`] if any
    /// byte lacks *both* the reserved `<0xXX>` surface and a literal
    /// single-byte token in the vocabulary.
    pub fn with_byte_fallback(mut self) -> Result<Self, HfConversionError> {
        let mut ids: [Option<usize>; 256] = [None; 256];
        // Second-pass fallback: id of a literal single-byte surface
        // for byte `b`, if any. See the BPE-side comment for the same
        // shape and why the array is written over the full 0..256.
        let mut literal_ids: [Option<usize>; 256] = [None; 256];
        for (id, (surface, _)) in self.vocab.iter().enumerate() {
            if let Some(b) = parse_byte_fallback_surface(surface) {
                // First occurrence wins — same policy as `lookup`
                // (a well-formed vocab has no duplicates anyway).
                if ids[b as usize].is_none() {
                    ids[b as usize] = Some(id);
                }
            } else if surface.len() == 1 {
                let b = surface.as_bytes()[0];
                if literal_ids[b as usize].is_none() {
                    literal_ids[b as usize] = Some(id);
                }
            }
        }
        let mut resolved = alloc::boxed::Box::new([0usize; 256]);
        let mut missing_count = 0usize;
        let mut first_missing: Option<u8> = None;
        for b in 0usize..256 {
            // Prefer the reserved `<0xXX>` surface; fall back to a
            // literal single-byte token when it is absent. Mirrors
            // the BPE-side scan byte for byte.
            if let Some(id) = ids[b].or(literal_ids[b]) {
                resolved[b] = id;
            } else {
                missing_count += 1;
                if first_missing.is_none() {
                    // `b` iterates 0..256 so the cast is exact; use
                    // `try_from` so clippy's truncation lint does not
                    // fire.
                    first_missing = u8::try_from(b).ok();
                }
            }
        }
        if missing_count != 0 {
            return Err(HfConversionError::ByteFallbackTokensMissing {
                missing_count,
                first_missing_byte: first_missing.unwrap_or(0),
            });
        }
        self.byte_fallback = Some(resolved);
        Ok(self)
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

    /// Read-only access to the configured pre-tokenizer sequence, if
    /// any.
    ///
    /// Returns a [`PreTokenizerSequence`] rather than a bare
    /// [`Metaspace`] so composed shapes such as xlm-roberta-base's
    /// `Sequence[WhitespaceSplit, Metaspace]` are visible in full.
    /// Callers that only need the Metaspace stage (the pre-composition
    /// shape) can go through [`PreTokenizerSequence::metaspace`].
    #[must_use]
    pub fn pre_tokenizer(&self) -> Option<&PreTokenizerSequence> {
        self.pre_tokenizer.as_ref()
    }

    /// Read-only access to the configured post-processor.
    #[must_use]
    pub fn post_processor(&self) -> &PostProcessor {
        &self.post_processor
    }

    /// Read-only access to the registered special tokens.
    #[must_use]
    pub fn special_tokens(&self) -> &BTreeMap<String, TokenId> {
        &self.special_tokens
    }

    /// `true` when [`Self::with_byte_fallback`] has been called on
    /// this tokenizer and the byte-fallback path is active.
    #[must_use]
    pub const fn byte_fallback_enabled(&self) -> bool {
        self.byte_fallback.is_some()
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
        if self.special_tokens.is_empty() {
            // Fast path — no specials to pre-extract. Normalize the
            // whole input in one shot; there is no between-specials
            // boundary that would ask for per-region normalization.
            let normalized: alloc::borrow::Cow<'_, str> = match &self.normalizer {
                Some(n) => alloc::borrow::Cow::Owned(crate::normalizer::normalize(input, n)),
                None => alloc::borrow::Cow::Borrowed(input),
            };
            return self.encode_regions(normalized.as_ref());
        }

        // Slow path: pre-extract registered special-token surfaces
        // from the RAW input first, then normalize (and pre-tokenize)
        // each between-specials region independently. This matches
        // HF's `added_vocabulary::extract_and_normalize` ordering: the
        // added-tokens split runs against the raw string so a special
        // surface like `[CLS]` is not lowercased away by a BERT
        // normalizer's `lowercase: true` before the specials matcher
        // sees it, and DeBERTa-v3's `Strip { strip_right: true }`
        // trims each region's trailing whitespace BEFORE Metaspace
        // fires — so a between-specials region such as `" hello "`
        // normalises to `" hello"` and Metaspace with `always` emits
        // `["▁hello"]` rather than `["▁hello", "▁"]`.
        //
        // Mirrors [`BpeTokenizer::encode_pieces_with_policy`] /
        // [`crate::wordpiece::WordPieceTokenizer::encode_ids_raw`].
        let sorted_specials = sorted_unigram_special_tokens(&self.special_tokens);
        let mut ids = Vec::new();
        let mut cursor = 0usize;
        while cursor < input.len() {
            let remaining = &input[cursor..];
            // Try to match a special at the current cursor.
            let mut matched: Option<(usize, usize)> = None;
            for (surface, id) in &sorted_specials {
                if remaining.starts_with(surface.as_str()) {
                    matched = Some((*id as usize, surface.len()));
                    break;
                }
            }
            if let Some((id, len)) = matched {
                ids.push(id);
                cursor += len;
                continue;
            }
            // No special match — take the region up to the next match
            // (or end-of-input) and hand it to the normal pipeline.
            let mut next_rel = remaining.len();
            for (surface, _) in &sorted_specials {
                if let Some(rel) = remaining.find(surface.as_str()) {
                    if rel < next_rel {
                        next_rel = rel;
                    }
                }
            }
            let region = &remaining[..next_rel];
            if !region.is_empty() {
                // Per-region normalize, then pre-tokenize + Viterbi.
                // The normalizer is applied to the region in
                // isolation — matching HF's per-segment normalization
                // shape.
                let region_normalized: alloc::borrow::Cow<'_, str> = match &self.normalizer {
                    Some(n) => alloc::borrow::Cow::Owned(crate::normalizer::normalize(region, n)),
                    None => alloc::borrow::Cow::Borrowed(region),
                };
                let region_ids = self.encode_regions(region_normalized.as_ref())?;
                ids.extend(region_ids);
            }
            cursor += next_rel;
        }
        Ok(ids)
    }

    /// Run the pre-tokenize + Viterbi pipeline on `text`, treating it
    /// as one already-normalized region without any special-token
    /// extraction. Shared by [`Self::encode`]'s fast and slow paths.
    /// Uses `self.pre_tokenizer` (a `PreTokenizerSequence`) when
    /// configured, else runs on the whole string.
    fn encode_regions(&self, text: &str) -> Result<Vec<usize>, UnigramEncodeError> {
        let mut ids = Vec::new();
        if let Some(seq) = &self.pre_tokenizer {
            for piece in seq.apply(text) {
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
        // Chain-decoder fast path: when a decoder chain is configured,
        // walk each id to its vocab surface string and thread the list
        // through the chain — mirroring HF's own `Decoder::decode_chain`
        // — instead of the default Metaspace-reversal path below.
        //
        // The model-side byte-fallback reassembly is skipped in this
        // branch for the same reason as the BPE side: the chain's own
        // `ByteFallback` stage (when present) is what reassembles the
        // `<0xXX>` runs on the token surface strings; running both
        // would double-decode.
        if let Some(dec) = self.decoder.as_ref().filter(|d| d.is_chain()) {
            let mut token_strs: Vec<String> = Vec::with_capacity(tokens.len());
            for &id in tokens {
                if id >= self.vocab.len() {
                    return Err(UnigramDecodeError::UnknownId(id));
                }
                let (surface, _) = &self.vocab[id];
                token_strs.push(surface.clone());
            }
            let out = dec.apply_chain(token_strs);
            let mut buf = String::new();
            for s in &out {
                buf.push_str(s);
            }
            return Ok(buf);
        }

        let mut buf = String::new();
        // Accumulator for a run of byte-fallback tokens. Bytes are
        // pushed here while consecutive byte-fallback ids arrive; the
        // run is flushed as `String::from_utf8_lossy` when a non-
        // byte-fallback token is seen or at the end of the stream.
        // Lossy decoding maps invalid UTF-8 to U+FFFD, matching what
        // upstream `SentencePiece` does when the emitted byte stream
        // happens not to be well-formed UTF-8 (e.g. an id-list
        // constructed by hand with a stray byte).
        let mut byte_run: Vec<u8> = Vec::new();
        for &id in tokens {
            if id >= self.vocab.len() {
                return Err(UnigramDecodeError::UnknownId(id));
            }
            if let Some(b) = self.byte_fallback_byte_for(id) {
                byte_run.push(b);
                continue;
            }
            if !byte_run.is_empty() {
                buf.push_str(&alloc::string::String::from_utf8_lossy(&byte_run));
                byte_run.clear();
            }
            let (surface, _) = &self.vocab[id];
            buf.push_str(surface);
        }
        if !byte_run.is_empty() {
            buf.push_str(&alloc::string::String::from_utf8_lossy(&byte_run));
        }
        // If a Metaspace stage is configured (either standalone or
        // inside a composed sequence), reverse its substitution:
        // `replacement` -> ' ', and drop the single leading space
        // that the prepend-scheme mark inserted (Always / First for
        // an unmarked input both prepend one).
        if let Some(ms) = self
            .pre_tokenizer
            .as_ref()
            .and_then(PreTokenizerSequence::metaspace)
        {
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
        // best_prev[i] = (previous char position, transition kind) for
        // the winning transition into `i`. Sentinel for i=0 is unused.
        let mut best_prev: Vec<(usize, UnigramTransition)> =
            alloc::vec![(0, UnigramTransition::Single(0)); n + 1];
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
                        best_prev[i] = (j, UnigramTransition::Single(id));
                    }
                }
            }
            // Fallback: if `i` is unreachable and we have either
            // byte-fallback or an `unk` token, take a single-character
            // transition from `i - 1` (if that itself is reachable).
            // Byte-fallback is preferred over `unk` when both are
            // configured — matches upstream `SentencePiece`, where
            // `unk` on a byte-fallback-enabled vocab is the
            // "genuinely nothing left" path.
            if !best_score[i].is_finite() && best_score[i - 1].is_finite() {
                if let Some(bf) = &self.byte_fallback {
                    let char_bytes = &input.as_bytes()[boundaries[i - 1]..boundaries[i]];
                    // UTF-8 encodes any scalar in at most 4 bytes; a
                    // small stack buffer keeps every real transition
                    // heap-free.
                    let mut buf = [0usize; 4];
                    let len = char_bytes.len();
                    debug_assert!(len <= 4);
                    for (k, &b) in char_bytes.iter().enumerate() {
                        buf[k] = bf[b as usize];
                    }
                    // Score with the same `unk_penalty` used for the
                    // unk fallback so a vocab-only path always wins
                    // when one is available. The absolute magnitude
                    // does not matter — only the relative ordering
                    // against vocab-only paths does.
                    best_score[i] = best_score[i - 1] - self.unk_penalty;
                    best_prev[i] = (i - 1, UnigramTransition::Bytes { buf, len });
                } else if let Some(u) = self.unk_id {
                    let (_, unk_score) = self.vocab[u];
                    let candidate = best_score[i - 1] + unk_score - self.unk_penalty;
                    best_score[i] = candidate;
                    best_prev[i] = (i - 1, UnigramTransition::Single(u));
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

        // Backtrack from n to 0, collecting emitted ids in reverse
        // order. `Single` contributes one id; `Bytes` contributes its
        // `len` ids in reverse. Consecutive `unk_id` transitions are
        // fused into a single UNK emission — this is SentencePiece's
        // `fuse_unk = true` default (also HF's Unigram default), which
        // makes an input like `"\u{200d}\u{1f52c}"` (ZWJ + microscope,
        // neither in vocab) surface as one `[UNK]` id rather than two.
        // Byte-fallback runs are not fused (they emit real bytes that
        // the decoder later reassembles).
        //
        // A final `reverse()` restores forward order.
        let mut ids = Vec::new();
        let mut pos = n;
        let mut prev_was_unk = false;
        while pos > 0 {
            let (prev, trans) = &best_prev[pos];
            match trans {
                UnigramTransition::Single(id) => {
                    let is_unk = self.unk_id == Some(*id);
                    if is_unk && prev_was_unk {
                        // Fuse: skip pushing a second consecutive UNK.
                    } else {
                        ids.push(*id);
                    }
                    prev_was_unk = is_unk;
                }
                UnigramTransition::Bytes { buf, len } => {
                    for k in (0..*len).rev() {
                        ids.push(buf[k]);
                    }
                    // Byte-fallback bytes are not UNKs; a following
                    // UNK must not fuse with them.
                    prev_was_unk = false;
                }
            }
            pos = *prev;
        }
        ids.reverse();
        Ok(ids)
    }

    /// Reverse-lookup: if `id` is one of the 256 byte-fallback tokens,
    /// return its associated byte value. `None` when byte-fallback is
    /// disabled or `id` is a regular vocab entry.
    ///
    /// A linear scan of a 256-entry array is cheap enough that
    /// building a reverse `id → byte` map on construction would be
    /// premature optimisation — decode is not a hot path here.
    fn byte_fallback_byte_for(&self, id: usize) -> Option<u8> {
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
        // top and apply the truncation config. The inherent method
        // returns `Vec<usize>`; the trait's `Encoding<TokenId>` uses
        // `u32`. Cast at the boundary — every id from a real
        // SentencePiece vocab fits.
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
        let mut out = if matches!(self.post_processor, PostProcessor::None) {
            enc
        } else {
            self.post_processor.apply(&enc, true)
        };
        if let Some(cfg) = &self.truncation {
            stringcheese_tokenizer::truncation::truncate(&mut out, cfg);
        }
        Ok(out)
    }

    fn encode_batch(
        &self,
        inputs: &[&str],
    ) -> Result<
        alloc::vec::Vec<stringcheese_tokenizer::Encoding<Self::Token>>,
        stringcheese_tokenizer::TokenizerError,
    > {
        let mut out: alloc::vec::Vec<stringcheese_tokenizer::Encoding<Self::Token>> =
            alloc::vec::Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(<Self as stringcheese_tokenizer::Tokenizer>::encode(
                self, input,
            )?);
        }
        if let Some(cfg) = &self.padding {
            stringcheese_tokenizer::padding::pad_batch(&mut out, cfg);
        }
        Ok(out)
    }

    fn encode_pair(
        &self,
        a: &str,
        b: &str,
    ) -> Result<stringcheese_tokenizer::Encoding<Self::Token>, stringcheese_tokenizer::TokenizerError>
    {
        // Encode both sides through the raw Viterbi pipeline (no
        // post-processor), truncate the pair together, then splice the
        // pair template.
        let raw_a = Self::encode(self, a).map_err(|e| {
            stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!("{e}"))
        })?;
        let raw_b = Self::encode(self, b).map_err(|e| {
            stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!("{e}"))
        })?;
        let mut ea: stringcheese_tokenizer::Encoding<TokenId> =
            stringcheese_tokenizer::Encoding::new();
        ea.ids.reserve(raw_a.len());
        for id in raw_a {
            let tid = TokenId::try_from(id).map_err(|_| {
                stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!(
                    "Unigram id {id} does not fit in TokenId (u32)"
                ))
            })?;
            ea.ids.push(tid);
        }
        let mut eb: stringcheese_tokenizer::Encoding<TokenId> =
            stringcheese_tokenizer::Encoding::new();
        eb.ids.reserve(raw_b.len());
        for id in raw_b {
            let tid = TokenId::try_from(id).map_err(|_| {
                stringcheese_tokenizer::TokenizerError::UnknownToken(alloc::format!(
                    "Unigram id {id} does not fit in TokenId (u32)"
                ))
            })?;
            eb.ids.push(tid);
        }
        if let Some(cfg) = &self.truncation {
            stringcheese_tokenizer::truncation::truncate_pair(&mut ea, &mut eb, cfg);
        }
        Ok(self.post_processor.apply_pair(&ea, &eb, true))
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
/// * [`HfConversionError::ByteFallbackTokensMissing`] — the config
///   sets `byte_fallback: true` but the vocabulary is missing one or
///   more of the 256 reserved `<0x00>`..`<0xFF>` tokens the mechanism
///   requires.
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

    // Byte-fallback — a `SentencePiece` mechanism that reroutes the
    // OOV path from `unk` to the 256 reserved `<0xXX>` tokens. Enable
    // it early so a missing-tokens error surfaces before any of the
    // ancillary pipeline pieces (normalizer / pre-tokenizer /
    // post-processor) are materialised. The scan runs on the raw
    // vocab, so its result is independent of what comes after.
    if uni.byte_fallback == Some(true) {
        tok = tok.with_byte_fallback()?;
    }

    // Normalizer — runs before the pre-tokenizer at encode time.
    if let Some(hn) = &config.normalizer {
        let n = to_runtime_normalizer(hn)?;
        tok = tok.with_normalizer(n);
    }

    // Pre-tokenizer — Metaspace (bare or inside a Sequence, possibly
    // preceded by WhitespaceSplit as xlm-roberta-base ships) is the
    // shape SentencePiece Unigram checkpoints commonly wear.
    if let Some(pt) = &config.pre_tokenizer {
        let seq = extract_pre_tokenizer_sequence(pt)?;
        tok = tok.with_pre_tokenizer(seq);
    }

    // Post-processor — runs on the finished Encoding before the trait
    // `Tokenizer::encode` returns it.
    if let Some(hp) = &config.post_processor {
        let pp = to_runtime_post_processor(hp)?;
        tok = tok.with_post_processor(pp);
    }

    // Special tokens — `added_tokens[*]` entries with `special == true`
    // must be pre-extracted from the (normalized) input before the
    // Metaspace pre-tokenizer and Viterbi loop see them. Otherwise a
    // literal `"<s>"` in the input would flow through Viterbi as
    // regular text and (on xlm-r) come out as `▁<`, `s`, `>` pieces
    // instead of the CLS id. Same filter rule as
    // [`to_bpe_tokenizer`] / [`to_wordpiece_tokenizer`].
    let mut uni_specials: BTreeMap<String, TokenId> = BTreeMap::new();
    for at in &config.added_tokens {
        if at.special {
            uni_specials.insert(at.content.clone(), at.id);
        }
    }
    if !uni_specials.is_empty() {
        tok = tok.with_special_tokens(uni_specials);
    }

    // Decoder — honour any HF decoder chain the config carries. The
    // Unigram runtime's default decode path already reverses the
    // Metaspace substitution unconditionally, so a checkpoint that
    // ships without a decoder block — or with a decoder shape this
    // crate doesn't materialise (real xlm-roberta-base ships
    // `Metaspace` here, which falls through to `Ok(None)` — see
    // [`to_runtime_decoder`] for the soft-fail rationale) — keeps
    // that Metaspace-reversal behaviour.
    if let Some(hd) = &config.decoder {
        if let Some(dec) = to_runtime_decoder(hd)? {
            tok = tok.with_decoder(dec);
        }
    }

    if let Some(t) = &config.truncation {
        tok = tok.with_truncation(t.clone().into());
    }
    if let Some(p) = &config.padding {
        tok = tok.with_padding(p.clone().into());
    }

    Ok(tok)
}

/// Reduce an [`HfPreTokenizer`] to a runtime [`PreTokenizerSequence`].
///
/// Shared between [`to_unigram_tokenizer`] and [`to_bpe_tokenizer`] —
/// both wire this shape onto the SentencePiece-descended checkpoints
/// they load. The Unigram runtime uses it to feed the Viterbi loop;
/// the BPE runtime (Mistral-family character-BPE with `byte_fallback`)
/// uses it to insert the `▁` markers before the character-level merge
/// loop runs.
///
/// Accepted on-disk shapes:
///
/// * A bare `Metaspace` block — wraps into a single-stage sequence
///   containing that Metaspace.
/// * A bare `WhitespaceSplit` block — wraps into a single-stage
///   sequence containing a `WhitespaceSplit`.
/// * A `Sequence` whose children are any combination of the two above.
///   In particular the composed shape xlm-roberta-base ships
///   (`Sequence[WhitespaceSplit, Metaspace]`) materialises into a
///   two-stage sequence with the same order.
///
/// Every other pre-tokenizer variant surfaces
/// [`HfConversionError::UnsupportedPreTokenizer`]; ambiguous mixes
/// (e.g. a Sequence containing a `Split(Regex)` sibling that neither
/// runtime can compose with) surface
/// [`HfConversionError::AmbiguousSequencePreTokenizer`].
fn extract_pre_tokenizer_sequence(
    pt: &HfPreTokenizer,
) -> Result<PreTokenizerSequence, HfConversionError> {
    match pt {
        HfPreTokenizer::Metaspace { .. } => {
            let ms = to_runtime_metaspace(pt)?;
            Ok(PreTokenizerSequence::from(ms))
        }
        HfPreTokenizer::WhitespaceSplit(_) => {
            Ok(PreTokenizerSequence::from(PreTokenizer::WhitespaceSplit))
        }
        HfPreTokenizer::Sequence { pretokenizers } => {
            sequence_to_pre_tokenizer_sequence(pretokenizers)
        }
        _ => Err(HfConversionError::UnsupportedPreTokenizer {
            type_name: "non-Metaspace".to_string(),
            reason: "SentencePiece-shape tokenizers only accept Metaspace and/or \
                     WhitespaceSplit pre-tokenizer stages here",
        }),
    }
}

/// Materialise a `Sequence[...]` block into a runtime
/// [`PreTokenizerSequence`], applying the acceptance rules documented
/// on [`extract_pre_tokenizer_sequence`].
///
/// Nested Sequences are permitted and flatten into the enclosing
/// sequence's stage list; every non-Metaspace / non-WhitespaceSplit
/// child surfaces the same error the sibling `extract_*` helpers do.
fn sequence_to_pre_tokenizer_sequence(
    children: &[HfPreTokenizer],
) -> Result<PreTokenizerSequence, HfConversionError> {
    let mut stages: Vec<PreTokenizer> = Vec::with_capacity(children.len());
    for child in children {
        match child {
            HfPreTokenizer::Metaspace { .. } => {
                let ms = to_runtime_metaspace(child)?;
                stages.push(PreTokenizer::Metaspace(ms));
            }
            HfPreTokenizer::WhitespaceSplit(_) => {
                stages.push(PreTokenizer::WhitespaceSplit);
            }
            HfPreTokenizer::Sequence { pretokenizers } => {
                // Flatten nested Sequences so the runtime always sees a
                // flat stage list. Order is preserved.
                let nested = sequence_to_pre_tokenizer_sequence(pretokenizers)?;
                stages.extend(nested.stages().iter().cloned());
            }
            _ => {
                // Any other child (Split, ByteLevel, Punctuation, ...)
                // is ambiguous inside a SentencePiece-oriented Sequence:
                // neither the Unigram Viterbi loop nor the BPE merge
                // loop has a composition rule for it. Surface the
                // ambiguity so callers see exactly what tripped.
                return Err(HfConversionError::AmbiguousSequencePreTokenizer {
                    child_count: children.len(),
                });
            }
        }
    }
    Ok(PreTokenizerSequence::new(stages))
}

/// `true` iff `pt` is (or contains, at any nesting depth in a `Sequence`)
/// a `SentencePiece` `Metaspace` block — the shape that routes the BPE and
/// Unigram loaders through [`extract_pre_tokenizer_sequence`] instead of
/// the regex/byte-level pipeline. A bare `WhitespaceSplit` alone is
/// *not* considered a Metaspace shape — its current callers (`WordPiece`,
/// `WordLevel`) have their own pipelines that consume it directly.
fn pre_tokenizer_uses_metaspace(pt: &HfPreTokenizer) -> bool {
    match pt {
        HfPreTokenizer::Metaspace { .. } => true,
        HfPreTokenizer::Sequence { pretokenizers } => {
            pretokenizers.iter().any(pre_tokenizer_uses_metaspace)
        }
        _ => false,
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
            "SentencePiece Metaspace is not supported on the current model \
             pipeline; the BPE and Unigram loaders wire Metaspace through \
             `PreTokenizerSequence` (BPE: `with_pre_tokenizer_sequence`, \
             Unigram: `with_pre_tokenizer`) — WordPiece and WordLevel have \
             no composition rule for it",
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
            HfPattern::Regex(p) => Ok(Normalizer::ReplaceRegex {
                pattern: p.clone(),
                content: content.clone(),
            }),
        },
        HfNormalizer::Strip {
            strip_left,
            strip_right,
        } => Ok(Normalizer::Strip {
            left: *strip_left,
            right: *strip_right,
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
/// exact semantics. It is wired into both [`to_bpe_tokenizer`]
/// (Mistral-7B-v0.1 ships Metaspace on the pre-tokenizer side) and
/// [`to_unigram_tokenizer`] via the shared [`PreTokenizerSequence`]
/// path; callers who want the raw typed runtime value (rather than
/// letting the loaders wire it) can obtain it here and drive it
/// against the produced tokenizer themselves.
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
        HfPostProcessor::BertProcessing(bp) => Ok(PostProcessor::BertProcessing(BertProcessing {
            sep: (bp.sep.0.clone(), bp.sep.1),
            cls: (bp.cls.0.clone(), bp.cls.1),
        })),
        HfPostProcessor::RobertaProcessing(rp) => {
            Ok(PostProcessor::RobertaProcessing(RobertaProcessing {
                sep: (rp.sep.0.clone(), rp.sep.1),
                cls: (rp.cls.0.clone(), rp.cls.1),
                trim_offsets: rp.trim_offsets,
                add_prefix_space: rp.add_prefix_space,
            }))
        }
        HfPostProcessor::Sequence { processors } => {
            // Recursively materialise each child. Nested Sequence
            // configs are permitted — the recursion re-enters this
            // same function.
            let mut children = Vec::with_capacity(processors.len());
            for child in processors {
                children.push(to_runtime_post_processor(child)?);
            }
            Ok(PostProcessor::Sequence(children))
        }
        HfPostProcessor::Other => Err(HfConversionError::UnsupportedPostProcessor {
            type_name: "Other".to_string(),
        }),
    }
}

/// Materialise a parsed [`HfDecoder`] block into a runtime [`Decoder`],
/// or `None` if the block references a tag this crate does not
/// materialise.
///
/// Called by [`to_bpe_tokenizer`] / [`to_unigram_tokenizer`] /
/// [`to_wordpiece_tokenizer`] whenever `config.decoder` is
/// `Some(...)`. When this function returns `Ok(None)`, the caller
/// leaves the tokenizer's decoder slot unset — the runtime's
/// per-family default decode wins (byte-buffer passthrough for BPE,
/// Metaspace-reversal for Unigram, continuing-subword-prefix for
/// `WordPiece`).
///
/// Every honoured variant maps to exactly one runtime [`Decoder`] arm:
///
/// * [`HfDecoder::ByteLevel`] → `Some(Decoder::ByteLevel)`. The
///   `add_prefix_space` / `trim_offsets` / `use_regex` fields are
///   preserved on [`HfTokenizerConfig::decoder`] for caller inspection
///   but not read here — the runtime decoder is the byte↔char inverse
///   mapping and takes no other config.
/// * [`HfDecoder::Sequence`] → `Some(Decoder::Sequence)`. Recursively
///   materialises each child; nested `Sequence` values are permitted.
///   A child that materialises to `None` is silently dropped from the
///   sequence (mirrors HF's forward-compat behaviour when a decoder
///   tag is unrecognised — the surrounding pipeline still runs). An
///   all-unmaterialised child list therefore produces
///   `Some(Decoder::Sequence(vec![]))`, i.e. identity, which matches
///   HF's own "empty Sequence == identity" semantics.
/// * [`HfDecoder::Replace`] → `Some(Decoder::Replace)`. Requires
///   `pattern` to be an [`HfPattern::String`] (literal); an
///   [`HfPattern::Regex`] surfaces
///   [`HfConversionError::UnsupportedDecoder`].
/// * [`HfDecoder::Fuse`] → `Some(Decoder::Fuse)`.
/// * [`HfDecoder::Strip`] → `Some(Decoder::Strip)`. Requires `content`
///   to be exactly one Unicode scalar (as HF's own type demands); a
///   multi-character `content` surfaces
///   [`HfConversionError::UnsupportedDecoder`].
/// * [`HfDecoder::ByteFallback`] → `Some(Decoder::ByteFallback)`.
/// * [`HfDecoder::Other`] → `Ok(None)`. Every unrecognised tag string
///   (`Metaspace`, `WordPiece`, `BPEDecoder`, ...) falls here at
///   parse time; a soft-fail keeps loaders that ship such decoders
///   (real xlm-roberta-base ships `Metaspace`) working end-to-end,
///   with the runtime's own per-family default decode taking over.
///
/// # Errors
///
/// [`HfConversionError::UnsupportedDecoder`] fires only for a shape
/// that would silently produce wrong output — a `Replace` with a
/// regex pattern or a `Strip` whose `content` is not a single Unicode
/// scalar. Every tag string this crate does not know about lands in
/// [`HfDecoder::Other`] and produces `Ok(None)` instead, so the
/// forward-compat guarantee ("loading a config never fails on an
/// unrecognised decoder tag") holds.
fn to_runtime_decoder(hd: &HfDecoder) -> Result<Option<Decoder>, HfConversionError> {
    match hd {
        HfDecoder::ByteLevel { .. } => Ok(Some(Decoder::ByteLevel)),
        HfDecoder::Sequence { decoders } => {
            let mut children: Vec<Decoder> = Vec::with_capacity(decoders.len());
            for child in decoders {
                if let Some(c) = to_runtime_decoder(child)? {
                    children.push(c);
                }
            }
            Ok(Some(Decoder::Sequence(children)))
        }
        HfDecoder::Replace { pattern, content } => match pattern {
            HfPattern::String(s) => Ok(Some(Decoder::Replace {
                pattern: s.clone(),
                content: content.clone(),
            })),
            HfPattern::Regex(_) => Err(HfConversionError::UnsupportedDecoder {
                reason: "Replace decoder with Regex pattern is not supported \
                         (runtime is literal-only)"
                    .to_string(),
            }),
        },
        HfDecoder::Fuse => Ok(Some(Decoder::Fuse)),
        HfDecoder::Strip {
            content,
            start,
            stop,
        } => {
            let mut chars = content.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(HfConversionError::UnsupportedDecoder {
                    reason: alloc::format!(
                        "Strip decoder `content` must be exactly one Unicode scalar; \
                         got {content:?}"
                    ),
                });
            };
            Ok(Some(Decoder::Strip {
                content: c,
                start: *start,
                stop: *stop,
            }))
        }
        HfDecoder::ByteFallback => Ok(Some(Decoder::ByteFallback)),
        HfDecoder::Other => Ok(None),
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
        assert_eq!(tok.decoder(), &crate::Decoder::ByteLevel);
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
    fn wordlevel_model_rejected_by_to_bpe_tokenizer() {
        // WordLevel is materialised via `to_wordlevel_tokenizer` /
        // `to_tokenizer`, not `to_bpe_tokenizer`. Backwards-compat:
        // `to_bpe_tokenizer` errors with `UnsupportedModelForBpe` so
        // callers can dispatch on the specific model type.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordLevel",
                "vocab": {"[UNK]": 0, "a": 1},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModelForBpe { type_name } => {
                assert_eq!(type_name, "WordLevel");
            }
            other => panic!("expected UnsupportedModelForBpe(WordLevel), got {other:?}"),
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
        assert_eq!(tok.decoder(), &Decoder::Passthrough);
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
        assert_eq!(tok.decoder(), &Decoder::ByteLevel);
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
            other => panic!("bare ByteLevel decoder parsed as {other:?}"),
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
    fn normalizer_replace_regex_pattern_is_honoured() {
        // The `pattern: {"Regex": "..."}` shape now materialises into
        // `Normalizer::ReplaceRegex` (DeBERTa-v3 and mDeBERTa-v3 both
        // ship one — see `to_runtime_normalizer` for the routing).
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Replace",
                "pattern": {"Regex": " {2,}"},
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
        // Runs of two-or-more ASCII spaces collapse to one; the BPE
        // whitespace fallback then discards the surviving space, so
        // "a    b" encodes to just [a, b].
        let ids = tok.encode("a    b").unwrap().ids;
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn normalizer_deberta_v3_shape_loads_end_to_end() {
        // Inline mini deberta-v3-shape tokenizer.json: the normalizer
        // is the exact `Sequence[Prepend, Replace(Regex), Replace(String)]`
        // stack the upstream `AutoTokenizer(...).save_pretrained(...)`
        // conversion writes for microsoft/deberta-v3-base. This test
        // asserts the config now converts without error — before the
        // Replace(Regex) landing, `to_tokenizer` bailed with
        // `UnsupportedNormalizer{type_name:"Replace(Regex)"}` and
        // deberta-v3 / mdeberta-v3 could not be loaded at all.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Sequence",
                "normalizers": [
                    {"type": "Prepend", "prepend": "▁"},
                    {"type": "Replace", "pattern": {"Regex": " {2,}"}, "content": " "},
                    {"type": "Replace", "pattern": {"String": "▁"}, "content": " "}
                ]
            },
            "model": {
                "type": "Unigram",
                "vocab": [["<unk>", 0.0], ["▁", -1.0], ["a", -2.0], ["b", -3.0]],
                "unk_id": 0
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        // Before this landing the next call errored out with
        // UnsupportedNormalizer(Replace(Regex)); assert it now
        // succeeds. The Unigram tokenizer built from the tiny vocab
        // is not exercised further here — the conformance corpus is
        // the right home for encode-side parity assertions.
        let _tok = to_tokenizer(&config).unwrap();
    }

    #[test]
    fn normalizer_deberta_v3_real_stack_loads_and_normalizes() {
        // The exact `Sequence[Replace(Regex), NFC, Strip{strip_right}]`
        // stack that microsoft/deberta-v3-base and
        // microsoft/mdeberta-v3-base ship on disk (transformers==5.14.1
        // conversion output). Two things are being pinned here:
        //
        //   1. Sequence composes Replace(Regex) + NFC + Strip in order
        //      through the runtime `normalize` fold — earlier waves
        //      shipped Replace(String) inside Sequence but not
        //      Replace(Regex) or the Strip variant with HF's on-disk
        //      `strip_left`/`strip_right` field names.
        //   2. `Strip { strip_left: false, strip_right: true }`
        //      deserialises with the HF-canonical field names (before
        //      this landing the loader used the short names `left` /
        //      `right`, which meant HF's `strip_left`/`strip_right`
        //      were silently dropped and defaults applied — mDeBERTa's
        //      trailing-whitespace normalisation would silently swap
        //      to two-sided strip).
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Sequence",
                "normalizers": [
                    {"type": "Replace", "pattern": {"Regex": "\\s{2,}|[\\n\\r\\t]"}, "content": " "},
                    {"type": "NFC"},
                    {"type": "Strip", "strip_left": false, "strip_right": true}
                ]
            },
            "model": {
                "type": "Unigram",
                "vocab": [["<unk>", 0.0], ["a", -1.0], ["b", -2.0], [" ", -3.0]],
                "unk_id": 0
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        // Assert the Strip child parsed with the HF field names,
        // *not* the alias defaults — before this landing the loader
        // silently dropped `strip_left`/`strip_right` and applied
        // `left: true, right: true` via serde defaults.
        match config.normalizer.as_ref().unwrap() {
            HfNormalizer::Sequence { normalizers } => {
                assert_eq!(normalizers.len(), 3);
                match &normalizers[2] {
                    HfNormalizer::Strip {
                        strip_left,
                        strip_right,
                    } => {
                        assert!(!*strip_left, "strip_left must parse as false");
                        assert!(*strip_right, "strip_right must parse as true");
                    }
                    other => panic!("expected Strip, got {other:?}"),
                }
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
        let tok = to_tokenizer(&config).unwrap();
        // Sanity: the runtime normalizer is a Sequence of three.
        let n = match &tok {
            HfTokenizer::Unigram(u) => u.normalizer(),
            other => panic!("expected Unigram tokenizer, got {other:?}"),
        };
        match n {
            Some(crate::normalizer::Normalizer::Sequence(children)) => {
                assert_eq!(children.len(), 3);
                assert!(matches!(
                    children[2],
                    crate::normalizer::Normalizer::Strip {
                        left: false,
                        right: true,
                    }
                ));
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn normalizer_strip_accepts_short_field_alias() {
        // Defensive: any legacy tokenizer.json blob that uses the
        // short `left` / `right` field names (mirroring the runtime
        // type's fields, not HF's on-disk shape) still parses via the
        // serde alias.
        let n: HfNormalizer =
            serde_json::from_str(r#"{"type": "Strip", "left": false, "right": true}"#).unwrap();
        match n {
            HfNormalizer::Strip {
                strip_left,
                strip_right,
            } => {
                assert!(!strip_left);
                assert!(strip_right);
            }
            other => panic!("expected Strip, got {other:?}"),
        }
    }

    #[test]
    fn normalizer_nested_sequence_loads_and_flattens_at_runtime() {
        // A nested Sequence — outer contains an inner Sequence plus a
        // sibling — must parse (nested `HfNormalizer::Sequence` is
        // valid on the wire) and materialise into a runtime Sequence
        // whose fold is left-to-right through both layers. This
        // pins the associativity behaviour tested on the runtime side
        // (`sequence_nesting_is_associative`) for the loader path too.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {
                "type": "Sequence",
                "normalizers": [
                    {
                        "type": "Sequence",
                        "normalizers": [
                            {"type": "NFD"},
                            {"type": "Lowercase"}
                        ]
                    },
                    {"type": "Strip", "strip_left": true, "strip_right": true}
                ]
            },
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        match tok.normalizer() {
            Some(crate::normalizer::Normalizer::Sequence(children)) => {
                assert_eq!(children.len(), 2);
                match &children[0] {
                    crate::normalizer::Normalizer::Sequence(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert!(matches!(inner[0], crate::normalizer::Normalizer::Nfd));
                        assert!(matches!(inner[1], crate::normalizer::Normalizer::Lowercase));
                    }
                    other => panic!("expected inner Sequence, got {other:?}"),
                }
                assert!(matches!(
                    children[1],
                    crate::normalizer::Normalizer::Strip {
                        left: true,
                        right: true,
                    }
                ));
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn normalizer_empty_sequence_loads_as_identity() {
        // An empty Sequence must parse and materialise into a runtime
        // Sequence with zero children — which the runtime `normalize`
        // fold treats as the identity function.
        let json = r#"{
            "added_tokens": [],
            "normalizer": {"type": "Sequence", "normalizers": []},
            "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        match tok.normalizer() {
            Some(crate::normalizer::Normalizer::Sequence(children)) => {
                assert!(children.is_empty());
            }
            other => panic!("expected empty Sequence, got {other:?}"),
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
    fn bert_post_processor_parses_and_materialises_on_bpe_loader() {
        // Regression: BertProcessing now materialises (it used to be
        // deferred and surfaced HfConversionError::UnsupportedPostProcessor
        // on any loader). The BPE loader route through
        // `to_runtime_post_processor` must attach the runtime variant.
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
            Some(HfPostProcessor::BertProcessing(_))
        ));
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(matches!(
            tok.post_processor(),
            crate::post_processor::PostProcessor::BertProcessing(_)
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
    fn to_tokenizer_dispatches_to_wordlevel_enum_variant() {
        // WordLevel now routes through `to_tokenizer` and produces the
        // `HfTokenizer::WordLevel` variant. `[UNK]` at id 0 is the
        // fallback for the OOV "unknown" word.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordLevel",
                "vocab": {"[UNK]": 0, "hello": 1, "world": 2, "foo": 3, "bar": 4},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::WordLevel(wl) => {
                assert_eq!(
                    wl.encode("hello world foo unknown").unwrap(),
                    vec![1, 2, 3, 0]
                );
            }
            other => panic!("expected HfTokenizer::WordLevel, got {other:?}"),
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

    // -----------------------------------------------------------------
    // Untagged `model` fallback (GPT-2 v1.0 shape).
    // -----------------------------------------------------------------

    #[test]
    fn typeless_model_with_vocab_and_merges_deserialises_as_bpe() {
        // Shape mirrors `openai-community/gpt2/tokenizer.json` (v1.0):
        // the `model` node ships without a `"type"` field, relying on
        // the consumer to autodetect BPE from the `{vocab, merges}`
        // presence.
        let json = r#"{
            "version": "1.0",
            "added_tokens": [],
            "model": {
                "dropout": null,
                "unk_token": null,
                "continuing_subword_prefix": null,
                "end_of_word_suffix": null,
                "fuse_unk": false,
                "vocab": {"a": 0, "b": 1, "ab": 2},
                "merges": [["a", "b"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.model {
            HfModel::Bpe(bpe) => {
                assert_eq!(bpe.vocab.len(), 3);
                assert_eq!(bpe.merges.len(), 1);
                // Optional fields captured verbatim from the source JSON.
                assert_eq!(bpe.dropout, None);
                assert_eq!(bpe.fuse_unk, Some(false));
            }
            other => panic!("expected typeless BPE fallback; got {other:?}"),
        }
        // And it converts end-to-end.
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("ab").unwrap().ids, vec![2]);
    }

    #[test]
    fn typeless_model_still_routes_tagged_form_unchanged() {
        // Belt-and-braces: the fallback path must never intercept a
        // well-formed tagged config. This is the same MINIMAL_JSON
        // that `parse_minimal_config` uses; if the wrapper's tagged
        // branch regresses, this test is the first alarm.
        let config = parse_tokenizer_json(MINIMAL_JSON).unwrap();
        assert!(matches!(&config.model, HfModel::Bpe(_)));
    }

    #[test]
    fn typeless_unigram_shape_deserialises_as_unigram() {
        // Shape mirrors `FacebookAI/xlm-roberta-base/tokenizer.json`:
        // the `model` node omits `"type"` and carries a `vocab` array
        // of `[surface, score]` pairs plus an `unk_id`. Real xlm-r
        // ships 250 002 entries; this inline blob picks the same
        // shape at three entries so the loader's disambiguation
        // (JSON-type of `vocab`) is exercised end-to-end without
        // pulling real vocab bytes into the crate.
        let json = r#"{
            "version": "1.0",
            "added_tokens": [],
            "model": {
                "unk_id": 0,
                "vocab": [
                    ["<unk>", 0.0],
                    ["a", -1.5],
                    ["b", -2.0]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.model {
            HfModel::Unigram(uni) => {
                assert_eq!(uni.vocab.len(), 3);
                assert_eq!(uni.unk_id, Some(0));
                assert_eq!(uni.vocab[0].0, "<unk>");
                assert!((uni.vocab[1].1 - -1.5).abs() < f64::EPSILON);
            }
            other => panic!("expected typeless Unigram fallback; got {other:?}"),
        }
    }

    #[test]
    fn typeless_model_missing_merges_and_unk_token_is_rejected() {
        // Vocab present as an object but no `merges` and no
        // `unk_token` — the config matches neither typeless BPE
        // (needs `merges`), typeless Unigram (needs an array `vocab`),
        // nor typeless WordPiece (needs `unk_token`). Rejecting is
        // the whole point of gating each fallback on a distinctive
        // key. The mBERT-shape typeless-WordPiece landing widened the
        // former "missing merges" rejection to allow the WordPiece
        // shape through — this test now pins the *narrower*
        // rejection: the config must also lack `unk_token` to reject.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "vocab": {"a": 0, "b": 1}
            }
        }"#;
        let err = parse_tokenizer_json(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("BPE requires")
                || msg.contains("Unigram requires")
                || msg.contains("WordPiece requires")
                || msg.contains("missing `type`"),
            "expected typeless-shape rejection, got: {msg}"
        );
    }

    #[test]
    fn typeless_wordpiece_shape_deserialises_as_wordpiece() {
        // Shape mirrors `google-bert/bert-base-multilingual-cased/tokenizer.json`
        // (and similarly-shaped typeless-WordPiece configs on the
        // Hub): the `model` node omits `"type"` and carries a
        // `vocab` object plus the mandatory `unk_token`, `continuing_subword_prefix`,
        // and `max_input_chars_per_word` fields. The disambiguation
        // key vs. typeless-BPE is the absence of `merges` combined
        // with the presence of `unk_token` — a bare `{vocab: {...}}`
        // (no `unk_token`) would still reject, distinguishing the
        // WordPiece shape from a corrupt BPE that dropped its merges.
        let json = r###"{
            "version": "1.0",
            "added_tokens": [],
            "model": {
                "vocab": {"[UNK]": 0, "hello": 1, "##ing": 2, "world": 3},
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100
            }
        }"###;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.model {
            HfModel::WordPiece(wp) => {
                assert_eq!(wp.vocab.len(), 4);
                assert_eq!(wp.unk_token, "[UNK]");
                assert_eq!(wp.continuing_subword_prefix, "##");
                assert_eq!(wp.max_input_chars_per_word, 100);
            }
            other => panic!("expected typeless WordPiece fallback; got {other:?}"),
        }
    }

    #[test]
    fn typeless_wordpiece_shape_applies_serde_defaults() {
        // Same disambiguation as the canonical typeless-WordPiece
        // test above, but exercises the serde defaults on
        // `continuing_subword_prefix` and `max_input_chars_per_word`
        // — a real Hub config *may* omit them and expect BERT-canonical
        // values. This pins that behaviour to the untagged fallback
        // path (the tagged path is already covered elsewhere).
        let json = r#"{
            "version": "1.0",
            "added_tokens": [],
            "model": {
                "vocab": {"[UNK]": 0, "hello": 1},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        match &config.model {
            HfModel::WordPiece(wp) => {
                assert_eq!(wp.continuing_subword_prefix, "##");
                assert_eq!(wp.max_input_chars_per_word, 100);
            }
            other => panic!("expected typeless WordPiece fallback; got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // BertProcessing loader
    // -----------------------------------------------------------------

    /// Inline minimal BERT-shape tokenizer JSON that uses the
    /// `BertProcessing` post-processor tag (the stock BERT shape,
    /// distinct from the `TemplateProcessing` shape [`BERT_JSON`]
    /// uses).
    const MINIMAL_BERT_PROCESSING_JSON: &str = r###"{
        "version": "1.0",
        "added_tokens": [
            {"id": 100, "content": "[UNK]", "special": true},
            {"id": 101, "content": "[CLS]", "special": true},
            {"id": 102, "content": "[SEP]", "special": true}
        ],
        "pre_tokenizer": {"type": "BertPreTokenizer"},
        "post_processor": {
            "type": "BertProcessing",
            "sep": ["[SEP]", 102],
            "cls": ["[CLS]", 101]
        },
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[UNK]": 100,
                "[CLS]": 101,
                "[SEP]": 102,
                "cat": 203,
                "dog": 204
            }
        }
    }"###;

    #[test]
    fn bert_processing_parses_typed() {
        let config = parse_tokenizer_json(MINIMAL_BERT_PROCESSING_JSON).unwrap();
        match config
            .post_processor
            .as_ref()
            .expect("post_processor present")
        {
            HfPostProcessor::BertProcessing(bp) => {
                assert_eq!(bp.sep.0, "[SEP]");
                assert_eq!(bp.sep.1, 102);
                assert_eq!(bp.cls.0, "[CLS]");
                assert_eq!(bp.cls.1, 101);
            }
            other => panic!("expected BertProcessing, got {other:?}"),
        }
    }

    #[test]
    fn bert_processing_wired_end_to_end_via_wordpiece_loader() {
        let config = parse_tokenizer_json(MINIMAL_BERT_PROCESSING_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "cat" (203), "dog" (204). BertProcessing splices [CLS]=101
        // before and [SEP]=102 after.
        assert_eq!(tok.encode("cat dog"), vec![101, 203, 204, 102]);
        // `count` must agree with `encode(text)?.ids.len()`.
        assert_eq!(
            stringcheese_tokenizer::Tokenizer::count(&tok, "cat dog").unwrap(),
            4
        );
    }

    #[test]
    fn bert_processing_runtime_variant_wraps_bertprocessing() {
        let config = parse_tokenizer_json(MINIMAL_BERT_PROCESSING_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert!(matches!(
            tok.post_processor(),
            crate::post_processor::PostProcessor::BertProcessing(_)
        ));
    }

    // -----------------------------------------------------------------
    // Sequence loader
    // -----------------------------------------------------------------

    /// Inline minimal `WordPiece` config with a `Sequence` post-processor
    /// composing two `BertProcessing` children. Contrived but exercises
    /// the composition path end-to-end.
    const SEQUENCE_POST_PROCESSOR_JSON: &str = r###"{
        "added_tokens": [
            {"id": 100, "content": "[UNK]", "special": true},
            {"id": 101, "content": "[CLS]", "special": true},
            {"id": 102, "content": "[SEP]", "special": true},
            {"id": 103, "content": "[CLS2]", "special": true},
            {"id": 104, "content": "[SEP2]", "special": true}
        ],
        "pre_tokenizer": {"type": "BertPreTokenizer"},
        "post_processor": {
            "type": "Sequence",
            "processors": [
                {
                    "type": "BertProcessing",
                    "sep": ["[SEP]", 102],
                    "cls": ["[CLS]", 101]
                },
                {
                    "type": "BertProcessing",
                    "sep": ["[SEP2]", 104],
                    "cls": ["[CLS2]", 103]
                }
            ]
        },
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[UNK]": 100,
                "[CLS]": 101,
                "[SEP]": 102,
                "[CLS2]": 103,
                "[SEP2]": 104,
                "cat": 203,
                "dog": 204
            }
        }
    }"###;

    #[test]
    fn sequence_post_processor_parses_typed() {
        let config = parse_tokenizer_json(SEQUENCE_POST_PROCESSOR_JSON).unwrap();
        match config
            .post_processor
            .as_ref()
            .expect("post_processor present")
        {
            HfPostProcessor::Sequence { processors } => {
                assert_eq!(processors.len(), 2);
                assert!(matches!(processors[0], HfPostProcessor::BertProcessing(_)));
                assert!(matches!(processors[1], HfPostProcessor::BertProcessing(_)));
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn sequence_post_processor_wired_end_to_end() {
        let config = parse_tokenizer_json(SEQUENCE_POST_PROCESSOR_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // "cat dog" primary: [203, 204].
        // First BertProcessing wraps: [101, 203, 204, 102].
        // Second BertProcessing wraps that: [103, 101, 203, 204, 102, 104].
        assert_eq!(tok.encode("cat dog"), vec![103, 101, 203, 204, 102, 104]);
        // `count` must agree with `encode(text)?.ids.len()`.
        assert_eq!(
            stringcheese_tokenizer::Tokenizer::count(&tok, "cat dog").unwrap(),
            6
        );
    }

    #[test]
    fn sequence_post_processor_runtime_variant_shape() {
        let config = parse_tokenizer_json(SEQUENCE_POST_PROCESSOR_JSON).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        match tok.post_processor() {
            crate::post_processor::PostProcessor::Sequence(children) => {
                assert_eq!(children.len(), 2);
                for child in children {
                    assert!(matches!(
                        child,
                        crate::post_processor::PostProcessor::BertProcessing(_)
                    ));
                }
            }
            other => panic!("expected PostProcessor::Sequence, got {other:?}"),
        }
    }

    #[test]
    fn empty_sequence_post_processor_is_identity() {
        // A `Sequence` with an empty processors array — HF's schema
        // permits it (default = empty vec) and the runtime treats it
        // as identity.
        let json = r###"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "post_processor": {
                "type": "Sequence",
                "processors": []
            },
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100,
                "vocab": {"[UNK]": 100, "cat": 203, "dog": 204}
            }
        }"###;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert_eq!(tok.encode("cat dog"), vec![203, 204]);
    }

    #[test]
    fn nested_sequence_post_processor_composes_recursively() {
        // Sequence inside Sequence — the outer Sequence's second child
        // is itself a Sequence containing a single BertProcessing. The
        // materialiser must recurse.
        let json = r###"{
            "added_tokens": [
                {"id": 100, "content": "[UNK]", "special": true},
                {"id": 101, "content": "[CLS]", "special": true},
                {"id": 102, "content": "[SEP]", "special": true}
            ],
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "post_processor": {
                "type": "Sequence",
                "processors": [
                    {
                        "type": "Sequence",
                        "processors": [
                            {
                                "type": "BertProcessing",
                                "sep": ["[SEP]", 102],
                                "cls": ["[CLS]", 101]
                            }
                        ]
                    }
                ]
            },
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100,
                "vocab": {"[UNK]": 100, "[CLS]": 101, "[SEP]": 102, "cat": 203}
            }
        }"###;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // Primary: [203]. Inner Sequence -> BertProcessing wraps:
        // [101, 203, 102]. Outer Sequence over that single-child
        // result: same output.
        assert_eq!(tok.encode("cat"), vec![101, 203, 102]);
    }

    // -----------------------------------------------------------------
    // WordLevel loader
    // -----------------------------------------------------------------

    /// Inline minimal `WordLevel` `tokenizer.json`: 5-word vocab with
    /// `[UNK]` at id 0. No `pre_tokenizer` block — the loader falls
    /// back to `WhitespaceSplit` per HF's real `WordLevel` shipping
    /// convention.
    const MINIMAL_WORDLEVEL_JSON: &str = r#"{
        "version": "1.0",
        "added_tokens": [],
        "model": {
            "type": "WordLevel",
            "vocab": {
                "[UNK]": 0,
                "hello": 1,
                "world": 2,
                "foo": 3,
                "bar": 4
            },
            "unk_token": "[UNK]"
        }
    }"#;

    #[test]
    fn wordlevel_model_parses_typed() {
        let config = parse_tokenizer_json(MINIMAL_WORDLEVEL_JSON).unwrap();
        match &config.model {
            HfModel::WordLevel(wl) => {
                assert_eq!(wl.unk_token, "[UNK]");
                assert_eq!(wl.vocab.len(), 5);
                assert!(wl.vocab.contains_key("hello"));
                assert_eq!(wl.vocab.get("[UNK]"), Some(&0));
            }
            other => panic!("expected WordLevel model, got {other:?}"),
        }
    }

    #[test]
    fn to_wordlevel_tokenizer_encodes_and_emits_unk() {
        // HF-loader test the task asks for: inline WordLevel-shape
        // tokenizer.json, verify `to_tokenizer` returns
        // `HfTokenizer::WordLevel`, encode "hello world foo unknown"
        // and check the unk id (0) appears in the "unknown" slot.
        let config = parse_tokenizer_json(MINIMAL_WORDLEVEL_JSON).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        let wl = match tok {
            HfTokenizer::WordLevel(wl) => wl,
            other => panic!("expected HfTokenizer::WordLevel, got {other:?}"),
        };
        let ids = wl.encode("hello world foo unknown").unwrap();
        assert_eq!(ids, vec![1, 2, 3, 0]);
        // Round-trip through decode joins the surface strings with
        // single ASCII spaces; `[UNK]` decodes to its literal
        // registered surface.
        assert_eq!(wl.decode(&ids).unwrap(), "hello world foo [UNK]");
    }

    #[test]
    fn to_wordlevel_tokenizer_defaults_pre_tokenizer_to_whitespace_split() {
        let config = parse_tokenizer_json(MINIMAL_WORDLEVEL_JSON).unwrap();
        let tok = to_wordlevel_tokenizer(&config).unwrap();
        assert_eq!(
            tok.pre_tokenizer(),
            crate::wordlevel::WordLevelPreTokenizer::WhitespaceSplit
        );
    }

    #[test]
    fn to_wordlevel_tokenizer_rejects_unk_missing_from_vocab() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "WordLevel",
                "vocab": {"cat": 0},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_wordlevel_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::WordLevelUnkNotInVocab { unk_token } => {
                assert_eq!(unk_token, "[UNK]");
            }
            other => panic!("expected WordLevelUnkNotInVocab, got {other:?}"),
        }
    }

    #[test]
    fn to_wordlevel_tokenizer_rejects_bpe_model() {
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": [["a", "b"]]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_wordlevel_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::UnsupportedModelForWordLevel { type_name } => {
                assert_eq!(type_name, "BPE");
            }
            other => panic!("expected UnsupportedModelForWordLevel(BPE), got {other:?}"),
        }
    }

    #[test]
    fn to_wordlevel_tokenizer_routes_whitespace_pre_tokenizer() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "Whitespace"},
            "model": {
                "type": "WordLevel",
                "vocab": {"[UNK]": 0, "hello": 1, ",": 2, "world": 3},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordlevel_tokenizer(&config).unwrap();
        assert_eq!(
            tok.pre_tokenizer(),
            crate::wordlevel::WordLevelPreTokenizer::Whitespace
        );
        // "hello, world" via whitespace + punctuation split →
        // ["hello", ",", "world"] → [1, 2, 3].
        assert_eq!(tok.encode("hello, world").unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn to_wordlevel_tokenizer_routes_bert_pre_tokenizer() {
        let json = r#"{
            "added_tokens": [],
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "model": {
                "type": "WordLevel",
                "vocab": {"[UNK]": 0, "hello": 1, ",": 2, "world": 3},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordlevel_tokenizer(&config).unwrap();
        assert_eq!(
            tok.pre_tokenizer(),
            crate::wordlevel::WordLevelPreTokenizer::Bert
        );
    }

    #[test]
    fn to_wordlevel_tokenizer_wires_normalizer_and_post_processor_end_to_end() {
        // BertNormalizer (lowercase) + WhitespaceSplit + TemplateProcessing
        // ([CLS] .. [SEP]).
        let json = r#"{
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
                "type": "WordLevel",
                "vocab": {
                    "[UNK]": 0,
                    "[CLS]": 1,
                    "[SEP]": 2,
                    "hello": 3,
                    "world": 4
                },
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        let wl = match tok {
            HfTokenizer::WordLevel(wl) => wl,
            other => panic!("expected HfTokenizer::WordLevel, got {other:?}"),
        };
        assert!(matches!(
            wl.normalizer(),
            Some(crate::normalizer::Normalizer::Bert { .. })
        ));
        assert!(matches!(
            wl.post_processor(),
            crate::post_processor::PostProcessor::TemplateProcessing(_)
        ));
        // "Hello WORLD" -> normalizer lowercases -> "hello world" ->
        // WhitespaceSplit -> ["hello", "world"] -> [3, 4] ->
        // template splice -> [1, 3, 4, 2].
        assert_eq!(wl.encode("Hello WORLD").unwrap(), vec![1, 3, 4, 2]);
    }

    // -----------------------------------------------------------------
    // Special-token pre-extraction (BERT-family + XLM-R literal-in-text
    // parity) — WordPiece and Unigram loaders.
    // -----------------------------------------------------------------

    #[test]
    fn to_wordpiece_tokenizer_pre_extracts_registered_special_tokens() {
        // BERT-style config: `[CLS]` (101) and `[SEP]` (102) are
        // registered as `special: true` in `added_tokens`, and the
        // default pre-tokenizer (BertPreTokenizer) would otherwise
        // split `[CLS]` into `[`, `CLS`, `]`. With special-token
        // pre-extraction wired up on the loader, a literal `[CLS]` in
        // the input emits id 101 directly — matching
        // `transformers.AutoTokenizer`.
        //
        // The BERT-style TemplateProcessing here splices `[CLS]`/`[SEP]`
        // as the outer wrapping; the inner ids come from the pre-
        // extraction of the literal specials plus the WordPiece encode
        // of `"hello"`.
        let json = r#"{
            "added_tokens": [
                {"id": 101, "content": "[CLS]", "special": true},
                {"id": 102, "content": "[SEP]", "special": true}
            ],
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "post_processor": {
                "type": "TemplateProcessing",
                "single": [
                    {"SpecialToken": {"id": "[CLS]", "type_id": 0}},
                    {"Sequence":     {"id": "A",     "type_id": 0}},
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
                "vocab": {
                    "[UNK]": 100,
                    "[CLS]": 101,
                    "[SEP]": 102,
                    "hello": 7592
                }
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // The tokenizer must carry the two specials pre-extracted from
        // added_tokens (not the whole vocab).
        assert_eq!(tok.special_tokens().len(), 2);
        assert_eq!(tok.special_tokens().get("[CLS]"), Some(&101));
        assert_eq!(tok.special_tokens().get("[SEP]"), Some(&102));
        // "[CLS] hello [SEP]" — expected (matching HF's shape):
        //   outer CLS (101, from template) + literal CLS (101) +
        //   "hello" (7592) + literal SEP (102) + outer SEP (102).
        let ids = tok.encode("[CLS] hello [SEP]");
        assert_eq!(ids, vec![101, 101, 7592, 102, 102]);
    }

    #[test]
    fn to_wordpiece_tokenizer_filters_added_tokens_by_special_flag() {
        // Confirm the `special: false` bit does what HF's schema says:
        // added_tokens with `special == false` land in the vocab (they
        // affect WordPiece lookups) but are NOT pre-extracted. Only
        // `special == true` entries reach the special_tokens map.
        //
        // Example: `[NORMAL]` is `special: false`; `[SPECIAL]` is
        // `special: true`. Only `[SPECIAL]` shows up in
        // `special_tokens()`.
        let json = r#"{
            "added_tokens": [
                {"id": 200, "content": "[NORMAL]",  "special": false},
                {"id": 201, "content": "[SPECIAL]", "special": true}
            ],
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {"[UNK]": 0}
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        assert_eq!(tok.special_tokens().len(), 1);
        assert!(tok.special_tokens().contains_key("[SPECIAL]"));
        assert!(!tok.special_tokens().contains_key("[NORMAL]"));
        // Both surfaces land in the vocab regardless of the flag.
        assert!(tok.vocab().contains_key("[NORMAL]"));
        assert!(tok.vocab().contains_key("[SPECIAL]"));
    }

    #[test]
    fn to_unigram_tokenizer_pre_extracts_registered_special_tokens() {
        // XLM-R style: `<s>` and `</s>` are `special: true` in
        // added_tokens. Without pre-extraction the Metaspace
        // pre-tokenizer would fold `▁` onto the `<` and the Viterbi
        // loop would decompose `<s>` into unrelated pieces (or fall
        // back to `unk`/byte-fallback). With pre-extraction, a literal
        // `<s>` in the input emits its registered id directly.
        //
        // The vocab is inline (no real xlm-r bytes) so the test can
        // ship without a materialised tokenizer.json — the ids for
        // `▁hello` and `hello` are hand-picked.
        let json = r#"{
            "added_tokens": [
                {"id": 0, "content": "<s>",   "special": true},
                {"id": 2, "content": "</s>",  "special": true}
            ],
            "pre_tokenizer": {
                "type": "Metaspace",
                "replacement": "▁",
                "prepend_scheme": "always",
                "split": true
            },
            "model": {
                "type": "Unigram",
                "unk_id": 1,
                "vocab": [
                    ["<s>",     0.0],
                    ["<unk>",   -100.0],
                    ["</s>",    0.0],
                    ["▁hello",  -1.0],
                    ["▁world",  -2.0]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_unigram_tokenizer(&config).unwrap();
        // The loader must have populated special_tokens from
        // added_tokens with `special == true`.
        assert_eq!(tok.special_tokens().len(), 2);
        assert_eq!(tok.special_tokens().get("<s>"), Some(&0));
        assert_eq!(tok.special_tokens().get("</s>"), Some(&2));
        // "<s> hello </s>" — expected: [0, id("▁hello"), 2].
        //   pre-extraction: [0], region " hello " -> Metaspace ->
        //     ["▁", "▁hello", "▁"] -> Viterbi -> [id_for_▁hello]
        //     (leading/trailing bare `▁` may or may not be in the
        //     vocab; on this hand-crafted vocab it isn't, so they fall
        //     back to `unk` id 1). So the full sequence is:
        //     [<s>, ...region..., </s>].
        let ids = tok.encode("<s> hello </s>").unwrap();
        // Check the anchors: first id must be `<s>`, last must be
        // `</s>`, and `▁hello` (id 3) must appear somewhere in the
        // middle. We don't over-constrain the exact region encoding
        // because the Metaspace + Viterbi interaction on the bare `▁`
        // is not the subject of this test.
        assert_eq!(ids.first(), Some(&0));
        assert_eq!(ids.last(), Some(&2));
        assert!(ids.contains(&3), "expected `▁hello` (id 3) in {ids:?}");
    }

    #[test]
    fn unigram_between_specials_region_metaspace_does_not_double_prepend() {
        // DeBERTa-v3-shape regression: pre-extract specials, then the
        // between-specials region `" hello "` normalises to `" hello"`
        // via `Strip { strip_right: true }` and Metaspace `always`
        // emits `["▁hello"]` — a single id, not `["▁", "▁hello", "▁"]`.
        // Before the specials-then-normalize + Metaspace-Always fix,
        // this pinned as `[<special>, "▁", "▁hello", "▁", <special>]`
        // and broke DeBERTa-v3 cases[32] / [33].
        let json = r#"{
            "added_tokens": [
                {"id": 0, "content": "[CLS]", "special": true},
                {"id": 1, "content": "[SEP]", "special": true}
            ],
            "normalizer": {
                "type": "Sequence",
                "normalizers": [
                    {"type": "Replace", "pattern": {"Regex": "\\s{2,}|[\\n\\r\\t]"}, "content": " "},
                    {"type": "NFC"},
                    {"type": "Strip", "strip_left": false, "strip_right": true}
                ]
            },
            "pre_tokenizer": {
                "type": "Metaspace",
                "replacement": "▁",
                "prepend_scheme": "always",
                "split": true
            },
            "model": {
                "type": "Unigram",
                "unk_id": 2,
                "vocab": [
                    ["[CLS]",  0.0],
                    ["[SEP]",  0.0],
                    ["<unk>", -100.0],
                    ["▁hello", -1.0],
                    ["▁",      -50.0]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_unigram_tokenizer(&config).unwrap();
        // `[CLS] hello [SEP]` -> [[CLS], ▁hello, [SEP]] = [0, 3, 1].
        // The stray `▁` (id 4) MUST NOT appear in the encoded output —
        // that would mean Metaspace-Always double-prepended on the
        // region, or the normalizer's `strip_right` failed to fire on
        // the between-specials region.
        let ids = tok.encode("[CLS] hello [SEP]").unwrap();
        assert_eq!(
            ids,
            alloc::vec![0, 3, 1],
            "expected [CLS, ▁hello, SEP] but got {ids:?}"
        );
    }

    #[test]
    fn wordpiece_between_specials_region_normalizer_runs_per_region() {
        // BERT-uncased-shape regression: the specials `[CLS]` / `[SEP]`
        // must be extracted from the RAW input before a lowercasing
        // `BertNormalizer` folds them to `[cls]` / `[sep]`. This test
        // uses a hand-crafted config that would previously (whole-input
        // normalize -> specials on the normalized text) have failed to
        // match `[CLS]` because the normalizer had already produced
        // `[cls]`, leaving the specials matcher blind.
        let json = r#"{
            "added_tokens": [
                {"id": 0, "content": "[CLS]", "special": true},
                {"id": 1, "content": "[SEP]", "special": true},
                {"id": 2, "content": "[UNK]", "special": true}
            ],
            "normalizer": {"type": "Lowercase"},
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "vocab": {
                    "[UNK]": 2,
                    "hello": 3
                }
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordpiece_tokenizer(&config).unwrap();
        // `[CLS] Hello [SEP]` -> [CLS, hello, SEP] = [0, 3, 1].
        // The `Hello` region is normalised to `hello` per-region, so
        // WordPiece finds it in the vocab. Before the fix, whole-input
        // lowercasing produced `[cls] hello [sep]`, the specials
        // matcher missed `[cls]`/`[sep]` (the vocab has neither), and
        // every bracketed piece decomposed to UNKs.
        let ids = tok.encode("[CLS] Hello [SEP]");
        assert_eq!(
            ids,
            alloc::vec![0, 3, 1],
            "expected [CLS, hello, SEP] but got {ids:?}"
        );
    }

    #[test]
    fn to_unigram_tokenizer_filters_added_tokens_by_special_flag() {
        // Same guarantee as the WordPiece test: only `special: true`
        // entries land in `special_tokens()`.
        let json = r#"{
            "added_tokens": [
                {"id": 4, "content": "<normal>",  "special": false},
                {"id": 5, "content": "<special>", "special": true}
            ],
            "model": {
                "type": "Unigram",
                "unk_id": 0,
                "vocab": [
                    ["<unk>",     -100.0],
                    ["hello",     -1.0],
                    ["world",     -2.0],
                    ["<extra>",   -3.0],
                    ["<normal>",  -4.0],
                    ["<special>", -5.0]
                ]
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_unigram_tokenizer(&config).unwrap();
        assert_eq!(tok.special_tokens().len(), 1);
        assert!(tok.special_tokens().contains_key("<special>"));
        assert!(!tok.special_tokens().contains_key("<normal>"));
    }

    // -----------------------------------------------------------------
    // Inline unit tests for the special-token surface API on
    // UnigramTokenizer (mirrors the WordPiece coverage in
    // `src/wordpiece.rs`).
    // -----------------------------------------------------------------

    #[test]
    fn unigram_encode_extracts_registered_special_tokens() {
        // Hand-built Unigram tokenizer with a small vocab, no
        // Metaspace, and two registered specials.
        let vocab = vec![
            ("<unk>".to_string(), -100.0),
            ("hello".to_string(), -1.0),
            ("world".to_string(), -2.0),
        ];
        let tok = UnigramTokenizer::from_parts(vocab, Some(0)).unwrap();
        let mut specials = BTreeMap::new();
        specials.insert("<s>".to_string(), 10u32);
        specials.insert("</s>".to_string(), 11u32);
        let tok = tok.with_special_tokens(specials);
        // "<s>hello</s>" — expected: [10, id("hello")=1, 11].
        let ids = tok.encode("<s>hello</s>").unwrap();
        assert_eq!(ids, vec![10, 1, 11]);
    }

    #[test]
    fn unigram_encode_special_tokens_prefer_longest_match_first() {
        let vocab = vec![("<unk>".to_string(), -100.0), ("hello".to_string(), -1.0)];
        let tok = UnigramTokenizer::from_parts(vocab, Some(0)).unwrap();
        let mut specials = BTreeMap::new();
        specials.insert("<|im|>".to_string(), 42u32);
        specials.insert("<|im_start|>".to_string(), 43u32);
        let tok = tok.with_special_tokens(specials);
        let ids = tok.encode("<|im_start|>").unwrap();
        assert_eq!(ids, vec![43]);
    }

    #[test]
    fn to_wordlevel_tokenizer_added_specials_land_in_vocab() {
        // added_tokens with unique surface strings are folded into
        // the vocab; conflicting duplicates surface a vocabulary
        // builder error.
        let json = r#"{
            "added_tokens": [
                {"id": 5, "content": "[BOS]", "special": true}
            ],
            "model": {
                "type": "WordLevel",
                "vocab": {"[UNK]": 0, "cat": 1},
                "unk_token": "[UNK]"
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_wordlevel_tokenizer(&config).unwrap();
        // The added special is in the vocab: encoding it produces id 5.
        assert_eq!(tok.encode("[BOS] cat").unwrap(), vec![5, 1]);
    }

    // ---------------------------------------------------------------------
    // BPE-side byte-fallback (SentencePiece `byte_fallback: true` on a
    // `BPE` model — Llama-2 / Mistral / Qwen shape).
    // ---------------------------------------------------------------------

    /// Build a Llama-style BPE tokenizer.json blob: three fake
    /// specials at ids 0..=2, the 256 reserved `<0x00>`..`<0xFF>`
    /// byte-fallback tokens at ids 3..=258, plus the single-character
    /// entries listed in `extra_chars` at successive ids from 259.
    /// Every extra char also becomes a vocab entry keyed by its raw
    /// UTF-8 bytes — matches how real Llama-2 vocabs ship (character-
    /// oriented BPE, not byte-level). Returns `(json, byte_id_base)`
    /// where `byte_id_base + XX` is the id of `<0xXX>`.
    fn bpe_byte_fallback_vocab_json(
        extra_chars: &[&str],
        merges: &[(&str, &str)],
    ) -> (String, u32) {
        let mut entries: Vec<String> = Vec::new();
        entries.push(r#""<unk>": 0"#.to_string());
        entries.push(r#""<s>": 1"#.to_string());
        entries.push(r#""</s>": 2"#.to_string());
        let byte_id_base: u32 = 3;
        for b in 0u32..=255 {
            entries.push(format!(r#""<0x{b:02X}>": {}"#, byte_id_base + b));
        }
        for (next, w) in (byte_id_base + 256..).zip(extra_chars.iter()) {
            entries.push(format!("\"{w}\": {next}"));
        }
        let merges_json: Vec<String> = merges
            .iter()
            .map(|(l, r)| format!("[\"{l}\", \"{r}\"]"))
            .collect();
        let json = format!(
            r#"{{
                "added_tokens": [],
                "model": {{
                    "type": "BPE",
                    "vocab": {{{}}},
                    "merges": [{}],
                    "byte_fallback": true
                }}
            }}"#,
            entries.join(","),
            merges_json.join(","),
        );
        (json, byte_id_base)
    }

    #[test]
    fn bpe_byte_fallback_construction_detects_all_256_tokens() {
        let (json, _base) = bpe_byte_fallback_vocab_json(&[], &[]);
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(tok.byte_fallback_enabled());
    }

    #[test]
    fn bpe_byte_fallback_missing_tokens_are_rejected() {
        // A BPE vocab with byte_fallback: true but only two `<0xXX>`
        // tokens present — construction must surface the specific
        // missing-tokens error rather than silently degrading.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {
                    "<unk>": 0,
                    "<0x00>": 1,
                    "<0x01>": 2
                },
                "merges": [],
                "byte_fallback": true
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::ByteFallbackTokensMissing {
                missing_count,
                first_missing_byte,
            } => {
                assert_eq!(missing_count, 254);
                assert_eq!(first_missing_byte, 0x02);
            }
            other => panic!("expected ByteFallbackTokensMissing, got {other:?}"),
        }
    }

    #[test]
    fn bpe_byte_fallback_disabled_leaves_encoder_on_unknown_error_path() {
        // A BPE config with byte_fallback: false (or absent) must not
        // enable the byte-fallback path — an OOV char still surfaces
        // `UnknownToken` under the crate's previous contract.
        let json = r#"{
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"h": 0, "i": 1},
                "merges": [],
                "byte_fallback": false
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(!tok.byte_fallback_enabled());
        // "hi" encodes cleanly, "?" errors out.
        assert!(Tokenizer::encode(&tok, "hi").is_ok());
        assert!(matches!(
            Tokenizer::encode(&tok, "?"),
            Err(stringcheese_tokenizer::TokenizerError::UnknownToken(_))
        ));
    }

    #[test]
    fn bpe_byte_fallback_accepts_lowercase_hex_surface() {
        // HF's on-disk convention is uppercase, but the scan should
        // accept lowercase hex too — parity with the Unigram-side scan.
        let mut entries: Vec<String> = Vec::new();
        entries.push(r#""<unk>": 0"#.to_string());
        for b in 0u32..=255 {
            entries.push(format!(r#""<0x{b:02x}>": {}"#, 3 + b));
        }
        let json = format!(
            r#"{{
                "added_tokens": [],
                "model": {{
                    "type": "BPE",
                    "vocab": {{{}}},
                    "merges": [],
                    "byte_fallback": true
                }}
            }}"#,
            entries.join(","),
        );
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(tok.byte_fallback_enabled());
    }

    #[test]
    fn bpe_byte_fallback_hf_loader_end_to_end() {
        // A minimal Llama-shape BPE config: byte_fallback + a couple
        // of single-character entries + one merge. Loader routes
        // through `to_tokenizer` and the returned BPE tokenizer
        // encodes an OOV char via the byte-fallback path.
        let (json, base) = bpe_byte_fallback_vocab_json(&["h", "i", "hi"], &[("h", "i")]);
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_tokenizer(&config).unwrap();
        match tok {
            HfTokenizer::Bpe(bpe) => {
                assert!(bpe.byte_fallback_enabled());
                // ASCII "?" (0x3F) is not in the vocab — fires
                // byte-fallback.
                let ids = Tokenizer::encode(bpe.as_ref(), "?").unwrap().ids;
                assert_eq!(ids, vec![base + 0x3F]);
                // Two vocab-covered chars merge into a whole-word id;
                // the merged surface "hi" sits at id base + 256 + 2
                // (the third `extra_chars` entry after "h", "i").
                let hi_id: TokenId = base + 256 + 2;
                let ids = Tokenizer::encode(bpe.as_ref(), "hi").unwrap().ids;
                assert_eq!(ids, vec![hi_id]);
                // Mixed: whole-word + byte-fallback for OOV char.
                let ids = Tokenizer::encode(bpe.as_ref(), "hi?").unwrap().ids;
                assert_eq!(ids, vec![hi_id, base + 0x3F]);
            }
            other => panic!("expected HfTokenizer::Bpe, got {other:?}"),
        }
    }

    #[test]
    fn bpe_byte_fallback_multibyte_utf8_char_fans_out_in_byte_order() {
        // A 4-byte emoji not in the vocab — every UTF-8 byte becomes
        // its reserved `<0xXX>` id in forward byte order. This is the
        // key correctness check that motivated the whole landing.
        let (json, base) = bpe_byte_fallback_vocab_json(&[], &[]);
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let ids = Tokenizer::encode(&tok, "😀").unwrap().ids;
        let expected: Vec<TokenId> = "😀".bytes().map(|b| base + u32::from(b)).collect();
        assert_eq!(ids, expected);
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn bpe_byte_fallback_round_trip_through_decode() {
        // Round-trip a mix of vocab-covered and byte-fallback inputs:
        // decode must reassemble the original UTF-8 for each.
        let (json, _base) = bpe_byte_fallback_vocab_json(&["h", "i", "hi"], &[("h", "i")]);
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        for text in ["hi", "?", "😀", "hi😀", "😀hi", "?!hi"] {
            let enc = Tokenizer::encode(&tok, text).unwrap();
            let round = Tokenizer::decode(&tok, &enc.ids).unwrap();
            assert_eq!(round, text, "round-trip failed on {text:?}");
        }
    }

    #[test]
    fn bpe_byte_fallback_accepts_literal_single_byte_surface_when_reserved_is_missing() {
        // Gemma-2b's shape: `byte_fallback: true` alongside 255 of 256
        // reserved `<0xXX>` surfaces plus a literal single-byte token
        // for the 256th (`google/gemma-2b` omits `<0x09>` in favour of
        // a literal tab). Loader construction must succeed and the
        // byte-fallback table must resolve byte 0x09 to the literal
        // tab token's id — verified via the decode reverse-lookup so
        // this test does not have to plumb a Metaspace pre-tokenizer
        // (the encoding side is exercised by the `conformance_gemma_2b`
        // fixture, which runs against the real gemma-2b pipeline).
        let mut entries: Vec<String> = Vec::new();
        entries.push(r#""<unk>": 0"#.to_string());
        entries.push(r#""<s>": 1"#.to_string());
        entries.push(r#""</s>": 2"#.to_string());
        let byte_id_base: u32 = 3;
        for b in 0u32..=255 {
            if b == 0x09 {
                // Skip the reserved <0x09> surface. Every other id in
                // the 3..=258 window still corresponds to `<0xXX>` for
                // its byte; the byte-fallback scan resolves 0x09 to
                // the literal-tab id below.
                continue;
            }
            entries.push(format!(r#""<0x{b:02X}>": {}"#, byte_id_base + b));
        }
        // Literal tab token — the byte-fallback scan's second pass
        // resolves byte 0x09 to this id.
        let literal_tab_id: u32 = 259;
        entries.push(format!(r#""\t": {literal_tab_id}"#));
        let json = format!(
            r#"{{
                "added_tokens": [],
                "model": {{
                    "type": "BPE",
                    "vocab": {{{}}},
                    "merges": [],
                    "byte_fallback": true
                }}
            }}"#,
            entries.join(",")
        );
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // Loader accepted the 255-reserved + 1-literal shape.
        assert!(tok.byte_fallback_enabled());

        // Decode-side proof that the literal-tab id occupies the byte
        // 0x09 slot in the byte-fallback table: decoding [259] routes
        // through the byte-run flush path (`byte_fallback_byte_for(259)
        // == Some(0x09)`) and produces "\t".
        let decoded = Tokenizer::decode(&tok, &[literal_tab_id]).unwrap();
        assert_eq!(decoded, "\t");

        // Every *other* byte still resolves via its reserved surface —
        // encoding a non-vocab ASCII char (`?`, 0x3F) fires the byte-
        // fallback path and emits the reserved id at `base + 0x3F`.
        let enc = Tokenizer::encode(&tok, "?").unwrap();
        assert_eq!(enc.ids, vec![byte_id_base + 0x3F]);
    }

    #[test]
    fn bpe_byte_fallback_rejects_when_both_reserved_and_literal_are_missing() {
        // Regression: even with the literal-single-byte fallback in
        // play, a byte with neither its reserved `<0xXX>` surface nor
        // a literal single-byte token in the vocab is still surfaced
        // as `ByteFallbackTokensMissing`. Build a vocab with 254
        // reserved surfaces (skipping 0x09 and 0x41) plus a literal
        // `A` (single-byte surface for 0x41) — byte 0x09 is uncovered
        // on both shapes and must error.
        let mut entries: Vec<String> = Vec::new();
        entries.push(r#""<unk>": 0"#.to_string());
        entries.push(r#""<s>": 1"#.to_string());
        entries.push(r#""</s>": 2"#.to_string());
        let byte_id_base: u32 = 3;
        for b in 0u32..=255 {
            if b == 0x09 || b == 0x41 {
                continue;
            }
            entries.push(format!(r#""<0x{b:02X}>": {}"#, byte_id_base + b));
        }
        // Literal `A` covers byte 0x41 but not 0x09.
        entries.push(r#""A": 259"#.to_string());
        let json = format!(
            r#"{{
                "added_tokens": [],
                "model": {{
                    "type": "BPE",
                    "vocab": {{{}}},
                    "merges": [],
                    "byte_fallback": true
                }}
            }}"#,
            entries.join(",")
        );
        let config = parse_tokenizer_json(&json).unwrap();
        let err = to_bpe_tokenizer(&config).unwrap_err();
        match err {
            HfConversionError::ByteFallbackTokensMissing {
                missing_count,
                first_missing_byte,
            } => {
                assert_eq!(missing_count, 1);
                assert_eq!(first_missing_byte, 0x09);
            }
            other => panic!("expected ByteFallbackTokensMissing, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // truncation / padding — HF loader landing.
    // -----------------------------------------------------------------

    #[test]
    fn hf_loader_parses_truncation_block_and_applies_it() {
        // A BPE config with a `truncation` block. Every character is
        // one token under a byte-alphabet-shape mini-vocab (no merges
        // fire), so a 4-char input encoded under `max_length: 2` must
        // trim to 2 tokens.
        let json = r#"{
            "version": "1.0",
            "truncation": {"max_length": 2, "direction": "Right",
                           "strategy": "LongestFirst", "stride": 0},
            "padding": null,
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, "c": 2, "d": 3},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(config.truncation.is_some());
        assert_eq!(config.truncation.as_ref().unwrap().max_length, 2);
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(tok.truncation().is_some());
        let enc = Tokenizer::encode(&tok, "abcd").unwrap();
        assert_eq!(enc.ids.len(), 2);
        assert_eq!(enc.ids, vec![0, 1]);
    }

    #[test]
    fn hf_loader_parses_padding_block_and_applies_it_on_batch() {
        // A BPE config with a `padding` block. `pad_id: 0` and
        // `BatchLongest` direction=Right — a batch of "a" and "abc"
        // must pad the first to length 3.
        let json = r#"{
            "version": "1.0",
            "truncation": null,
            "padding": {"strategy": "BatchLongest", "direction": "Right",
                        "pad_id": 0, "pad_type_id": 0, "pad_token": "[PAD]"},
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1, "c": 2},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(config.padding.is_some());
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(tok.padding().is_some());
        let batch = <BpeTokenizer as Tokenizer>::encode_batch(&tok, &["a", "abc"]).unwrap();
        assert_eq!(batch[0].ids, vec![0, 0, 0]);
        assert_eq!(batch[0].attention_mask, vec![true, false, false]);
        assert_eq!(batch[1].ids, vec![0, 1, 2]);
        assert_eq!(batch[1].attention_mask, vec![true, true, true]);
    }

    #[test]
    fn hf_loader_parses_fixed_padding_strategy() {
        // Tagged-object form of the padding strategy: `{"Fixed": 5}`.
        // Verify the parse dispatch and that the runtime pads to the
        // fixed length even for a single-encoding batch.
        let json = r#"{
            "version": "1.0",
            "truncation": null,
            "padding": {"strategy": {"Fixed": 5}, "direction": "Right",
                        "pad_id": 0, "pad_type_id": 0, "pad_token": "[PAD]"},
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        let batch = <BpeTokenizer as Tokenizer>::encode_batch(&tok, &["ab"]).unwrap();
        assert_eq!(batch[0].ids.len(), 5);
        assert_eq!(batch[0].ids, vec![0, 1, 0, 0, 0]);
    }

    #[test]
    fn hf_loader_null_truncation_and_padding_are_no_op() {
        // Explicit nulls (as every real Hub-shipped config carries when
        // the model does not preconfigure them) don't attach any
        // truncation/padding to the runtime.
        let json = r#"{
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "model": {
                "type": "BPE",
                "vocab": {"a": 0, "b": 1},
                "merges": []
            }
        }"#;
        let config = parse_tokenizer_json(json).unwrap();
        assert!(config.truncation.is_none());
        assert!(config.padding.is_none());
        let tok = to_bpe_tokenizer(&config).unwrap();
        assert!(tok.truncation().is_none());
        assert!(tok.padding().is_none());
    }

    // ---------------------------------------------------------------------
    // Decoder-chain loader tests — the Llama-2 shape and the
    // per-variant parse-then-materialise plumbing.
    // ---------------------------------------------------------------------

    /// The full Llama-2 decoder block, exactly as it appears in
    /// `NousResearch/Llama-2-7b-hf/tokenizer.json`. Kept as a constant
    /// so the same JSON drives both the parse-shape and
    /// convert-shape tests below.
    const LLAMA2_DECODER_JSON: &str = r#"{
        "type": "Sequence",
        "decoders": [
            {"type": "Replace", "pattern": {"String": "▁"}, "content": " "},
            {"type": "ByteFallback"},
            {"type": "Fuse"},
            {"type": "Strip", "content": " ", "start": 1, "stop": 0}
        ]
    }"#;

    #[test]
    fn hf_decoder_llama2_shape_parses_into_typed_sequence() {
        let dec: HfDecoder = serde_json::from_str(LLAMA2_DECODER_JSON).unwrap();
        let HfDecoder::Sequence { decoders } = dec else {
            panic!("expected Sequence variant, got a different shape");
        };
        assert_eq!(decoders.len(), 4);
        match &decoders[0] {
            HfDecoder::Replace { pattern, content } => {
                assert_eq!(*pattern, HfPattern::String(String::from("\u{2581}")));
                assert_eq!(content, " ");
            }
            other => panic!("stage 0: expected Replace, got {other:?}"),
        }
        assert!(matches!(decoders[1], HfDecoder::ByteFallback));
        assert!(matches!(decoders[2], HfDecoder::Fuse));
        match &decoders[3] {
            HfDecoder::Strip {
                content,
                start,
                stop,
            } => {
                assert_eq!(content, " ");
                assert_eq!(*start, 1);
                assert_eq!(*stop, 0);
            }
            other => panic!("stage 3: expected Strip, got {other:?}"),
        }
    }

    #[test]
    fn hf_decoder_llama2_shape_materialises_into_runtime_sequence() {
        // Wrap the block in a minimal BPE-shape config and load it end
        // to end. The runtime `Decoder` must be a 4-stage Sequence
        // whose arms match Llama-2's shape byte-for-byte.
        let mut json = String::from(
            r#"{
                "added_tokens": [],
                "decoder": "#,
        );
        json.push_str(LLAMA2_DECODER_JSON);
        json.push_str(
            r#",
                "model": {
                    "type": "BPE",
                    "vocab": {"a": 0, "b": 1},
                    "merges": []
                }
            }"#,
        );
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        match tok.decoder() {
            Decoder::Sequence(children) => {
                assert_eq!(children.len(), 4);
                match &children[0] {
                    Decoder::Replace { pattern, content } => {
                        assert_eq!(pattern, "\u{2581}");
                        assert_eq!(content, " ");
                    }
                    other => panic!("stage 0: {other:?}"),
                }
                assert!(matches!(children[1], Decoder::ByteFallback));
                assert!(matches!(children[2], Decoder::Fuse));
                match &children[3] {
                    Decoder::Strip {
                        content,
                        start,
                        stop,
                    } => {
                        assert_eq!(*content, ' ');
                        assert_eq!(*start, 1);
                        assert_eq!(*stop, 0);
                    }
                    other => panic!("stage 3: {other:?}"),
                }
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn hf_decoder_bytefallback_bare_parses_and_materialises() {
        let json = r#"{"type": "ByteFallback"}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        assert!(matches!(dec, HfDecoder::ByteFallback));
        assert!(matches!(
            to_runtime_decoder(&dec).unwrap(),
            Some(Decoder::ByteFallback)
        ));
    }

    #[test]
    fn hf_decoder_fuse_bare_parses_and_materialises() {
        let dec: HfDecoder = serde_json::from_str(r#"{"type": "Fuse"}"#).unwrap();
        assert!(matches!(dec, HfDecoder::Fuse));
        assert!(matches!(
            to_runtime_decoder(&dec).unwrap(),
            Some(Decoder::Fuse)
        ));
    }

    #[test]
    fn hf_decoder_strip_multichar_content_surfaces_error() {
        let json = r#"{"type": "Strip", "content": "  ", "start": 1, "stop": 0}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        assert!(matches!(
            to_runtime_decoder(&dec).unwrap_err(),
            HfConversionError::UnsupportedDecoder { .. }
        ));
    }

    #[test]
    fn hf_decoder_replace_regex_pattern_surfaces_error() {
        let json = r#"{"type": "Replace", "pattern": {"Regex": "a+"}, "content": "b"}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        assert!(matches!(
            to_runtime_decoder(&dec).unwrap_err(),
            HfConversionError::UnsupportedDecoder { .. }
        ));
    }

    #[test]
    fn hf_decoder_unknown_tag_falls_through_as_soft_fail() {
        // A `Metaspace` decoder falls to `HfDecoder::Other`; the loader
        // must not attach a runtime decoder (so the per-family default
        // decode wins). Real xlm-roberta-base ships this shape.
        let json = r#"{"type": "Metaspace"}"#;
        let dec: HfDecoder = serde_json::from_str(json).unwrap();
        assert!(matches!(dec, HfDecoder::Other));
        assert!(to_runtime_decoder(&dec).unwrap().is_none());
    }

    #[test]
    fn hf_decoder_llama2_shape_end_to_end_decode() {
        // Wire the Llama-2 chain end-to-end: build a BPE tokenizer
        // with a Llama-style vocab (character-BPE with `<0xXX>`
        // byte-fallback tokens and a `▁hi` piece), attach the chain,
        // and decode a synthetic id sequence.
        let mut json = String::from(
            r#"{
                "added_tokens": [],
                "decoder": "#,
        );
        json.push_str(LLAMA2_DECODER_JSON);
        json.push_str(
            r#",
                "model": {
                    "type": "BPE",
                    "vocab": {
                        "▁hi": 0,
                        "<0xF0>": 1,
                        "<0x9F>": 2,
                        "<0x98>": 3,
                        "<0x80>": 4
                    },
                    "merges": []
                }
            }"#,
        );
        let config = parse_tokenizer_json(&json).unwrap();
        let tok = to_bpe_tokenizer(&config).unwrap();
        // Ids: ▁hi, then the emoji byte-fallback run 😀 (F0 9F 98 80).
        // Expected decode: "hi😀" (the ▁ became ' ', which the Strip
        // stripped from the head, and the byte-fallback run reassembled
        // into 😀).
        let decoded = <BpeTokenizer as Tokenizer>::decode(&tok, &[0, 1, 2, 3, 4]).unwrap();
        assert_eq!(decoded, "hi😀");
    }
}
