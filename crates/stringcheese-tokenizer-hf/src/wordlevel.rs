//! `WordLevel` tokenizer — plain whole-word vocabulary lookup.
//!
//! # What this module does
//!
//! `WordLevel` is the simplest of the four Hugging Face `model.type`
//! shapes: input is split into words by a pre-tokenizer, and each word
//! is looked up directly in a vocabulary. There are no merges, no
//! greedy longest-match walk, no subword decomposition. A handful of
//! BERT-family and specialised HF checkpoints ship this shape — usually
//! for tag-based, chord-based, or otherwise fixed-vocabulary tasks
//! where subword splitting would be counter-productive.
//!
//! Any word that is not in the vocabulary emits [`WordLevelTokenizer::unk_token_id`]
//! when one is configured; otherwise [`WordLevelTokenizer::encode`]
//! surfaces [`WordLevelEncodeError::UnknownWord`]. This mirrors HF's
//! own runtime, which requires a `unk_token` on a well-formed
//! `WordLevel` config but leaves the strict-mode option open to callers
//! who assemble the runtime by hand.
//!
//! # Algorithm
//!
//! 1. Apply the configured [`Normalizer`] (if any) to the raw input.
//! 2. Run the configured [`WordLevelPreTokenizer`] over the normalised
//!    string (whitespace-only split by default).
//! 3. For each word, look up its UTF-8 byte string in the vocabulary.
//!    A hit emits the found id; a miss emits `unk_token_id` (or
//!    returns an error when no unk is configured).
//! 4. Apply the configured [`PostProcessor`] to the assembled
//!    [`Encoding`] before it leaves
//!    [`WordLevelTokenizer::encode`].
//!
//! # Decoding
//!
//! Decoding is the inverse: look up each id's surface bytes in the
//! vocabulary and re-emit them joined by a single ASCII space —
//! `WordLevel` is space-separated by construction, so a lossy
//! round-trip through a whitespace-collapsing decoder is the shape
//! every HF `WordLevel` checkpoint uses.
//!
//! # References
//!
//! * Hugging Face `tokenizers` crate, `models/wordlevel/mod.rs`:
//!   <https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/models/wordlevel/mod.rs>

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use stringcheese_tokenizer::{Encoding, Tokenizer, TokenizerError};

use crate::bpe::{BpeVocabulary, TokenId, VocabularyBuilderError};
use crate::normalizer::{Normalizer, normalize};
use crate::post_processor::PostProcessor;

/// The pre-tokenizer variant used by [`WordLevelTokenizer`] to split
/// raw input text into words before the vocabulary lookup runs.
///
/// The default is [`Self::WhitespaceSplit`] — HF's `WhitespaceSplit`
/// pre-tokenizer — because real `WordLevel` checkpoints ship it: a
/// vocab entry like `"cat,"` matches verbatim only when punctuation
/// stays glued to the surrounding word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WordLevelPreTokenizer {
    /// Whitespace split only — HF `WhitespaceSplit`. Punctuation stays
    /// glued to the surrounding word. Default because it is what real
    /// `WordLevel` checkpoints ship.
    #[default]
    WhitespaceSplit,
    /// Whitespace split *and* punctuation split — HF `Whitespace`.
    /// Emits every punctuation character as its own single-character
    /// word.
    Whitespace,
    /// BERT-family pre-tokenizer — HF `BertPreTokenizer`. Whitespace
    /// split followed by per-word punctuation split (each punctuation
    /// character becomes its own word). Semantically identical to
    /// [`Self::Whitespace`] on this crate's implementation but kept
    /// distinct so callers can express intent when the source config
    /// carries the `BertPreTokenizer` tag.
    Bert,
}

/// Errors that can arise when building a [`WordLevelTokenizer`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordLevelBuildError {
    /// The declared `unk_token_id` does not appear in the vocabulary —
    /// encoding an OOV word would emit an id the caller cannot decode.
    /// The wrapped id is the offending unknown-token id.
    UnkNotInVocab(TokenId),
    /// The vocabulary builder rejected an entry (duplicate id or
    /// duplicate surface string).
    Vocabulary(VocabularyBuilderError),
}

impl fmt::Display for WordLevelBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnkNotInVocab(id) => write!(
                f,
                "unknown-token id {id} is not present in the WordLevel vocabulary"
            ),
            Self::Vocabulary(err) => write!(f, "invalid WordLevel vocabulary: {err:?}"),
        }
    }
}

impl std::error::Error for WordLevelBuildError {}

impl From<VocabularyBuilderError> for WordLevelBuildError {
    fn from(err: VocabularyBuilderError) -> Self {
        Self::Vocabulary(err)
    }
}

/// Error returned by [`WordLevelTokenizer::encode`] when a word is
/// not in the vocabulary and no unknown-token id was configured.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordLevelEncodeError {
    /// The wrapped word was not in the vocabulary and no `unk_token_id`
    /// was configured. Encoding was aborted at this point.
    UnknownWord(String),
}

impl fmt::Display for WordLevelEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWord(word) => write!(
                f,
                "WordLevel tokenizer has no entry for input word {word:?} \
                 and no unk_token was configured"
            ),
        }
    }
}

impl std::error::Error for WordLevelEncodeError {}

/// Error returned by [`WordLevelTokenizer::decode`] when a token id
/// cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordLevelDecodeError {
    /// A token id was not present in the vocabulary.
    UnknownId(TokenId),
    /// A token id's stored surface bytes are not valid UTF-8. Every
    /// real HF `WordLevel` checkpoint stores UTF-8 strings, so this
    /// variant only fires on synthetic vocabularies constructed by
    /// hand out of raw bytes.
    InvalidUtf8(TokenId),
}

impl fmt::Display for WordLevelDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "unknown WordLevel token id: {id}"),
            Self::InvalidUtf8(id) => {
                write!(f, "WordLevel token id {id} decodes to non-UTF-8 bytes")
            }
        }
    }
}

impl std::error::Error for WordLevelDecodeError {}

/// A `WordLevel` tokenizer — plain whole-word vocabulary lookup.
///
/// Construct via [`WordLevelTokenizer::from_parts`]. See the
/// module-level documentation for the algorithm and lifetime story.
///
/// # Examples
///
/// Encode `"hello world foo unknown"` against a five-word vocab where
/// `"unknown"` is OOV; the unknown-token id is emitted in its slot.
///
/// ```
/// use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
///
/// let vocab = vec![
///     ("[UNK]".to_string(), 0),
///     ("hello".to_string(), 1),
///     ("world".to_string(), 2),
///     ("foo".to_string(), 3),
///     ("bar".to_string(), 4),
/// ];
/// let tok = WordLevelTokenizer::from_parts(vocab, Some(0)).unwrap();
/// assert_eq!(
///     tok.encode("hello world foo unknown").unwrap(),
///     vec![1, 2, 3, 0]
/// );
/// ```
#[derive(Debug, Clone)]
pub struct WordLevelTokenizer {
    /// Surface bytes ↔ id map. [`BpeVocabulary`] is reused because it
    /// already carries the bidirectional lookup plus duplicate-entry
    /// rejection every subword tokenizer in this crate needs; the
    /// "whole word is one entry" nature of `WordLevel` slots naturally
    /// on top of the same shape.
    vocab: BpeVocabulary,
    /// The id emitted for out-of-vocab words. `None` makes an OOV word
    /// surface [`WordLevelEncodeError::UnknownWord`] at encode time.
    unk_token_id: Option<TokenId>,
    /// How to split raw input text into words before the vocab lookup.
    /// See [`WordLevelPreTokenizer`].
    pre_tokenizer: WordLevelPreTokenizer,
    /// Optional Unicode normalizer applied to the raw input string
    /// *before* the pre-tokenizer runs. Mirrors
    /// [`crate::wordpiece::WordPieceTokenizer::with_normalizer`] and
    /// [`crate::BpeTokenizer::with_normalizer`].
    normalizer: Option<Normalizer>,
    /// Optional post-processor applied to the finished [`Encoding`]
    /// before it leaves [`Self::encode`]. Mirrors
    /// [`crate::BpeTokenizer::with_post_processor`]; the default
    /// [`PostProcessor::None`] is a pass-through, so callers who never
    /// configure one see the raw `WordLevel` output.
    post_processor: PostProcessor,
    /// Optional truncation configuration; see
    /// [`crate::BpeTokenizer::with_truncation`] for the semantics.
    truncation: Option<stringcheese_tokenizer::truncation::TruncationConfig>,
    /// Optional padding configuration; see
    /// [`crate::BpeTokenizer::with_padding`] for the semantics.
    padding: Option<stringcheese_tokenizer::padding::PaddingConfig<TokenId>>,
}

impl WordLevelTokenizer {
    /// Build a tokenizer from raw parts.
    ///
    /// # Parameters
    ///
    /// * `vocab` — the surface-string ↔ id vocabulary. Every entry is
    ///   inserted through [`BpeVocabulary::insert`], which enforces
    ///   the bijection (no duplicate ids or duplicate surface
    ///   strings).
    /// * `unk_token_id` — the id emitted for out-of-vocab words. When
    ///   `Some`, its id must have a corresponding entry in `vocab`;
    ///   otherwise [`WordLevelBuildError::UnkNotInVocab`] is returned.
    ///   `None` makes OOV words surface an error at encode time.
    ///
    /// The pre-tokenizer defaults to
    /// [`WordLevelPreTokenizer::WhitespaceSplit`]; use
    /// [`Self::with_pre_tokenizer`] to switch it.
    ///
    /// # Errors
    ///
    /// * [`WordLevelBuildError::UnkNotInVocab`] — `unk_token_id` is
    ///   `Some` and points at an id not present in the vocabulary.
    /// * [`WordLevelBuildError::Vocabulary`] — the vocabulary builder
    ///   rejected an entry (duplicate id or duplicate surface).
    pub fn from_parts<V>(
        vocab: V,
        unk_token_id: Option<TokenId>,
    ) -> Result<Self, WordLevelBuildError>
    where
        V: IntoIterator<Item = (String, TokenId)>,
    {
        let mut bv = BpeVocabulary::new();
        for (surface, id) in vocab {
            bv.insert(id, surface.into_bytes())?;
        }
        if let Some(id) = unk_token_id {
            if bv.bytes(id).is_none() {
                return Err(WordLevelBuildError::UnkNotInVocab(id));
            }
        }
        Ok(Self {
            vocab: bv,
            unk_token_id,
            pre_tokenizer: WordLevelPreTokenizer::default(),
            normalizer: None,
            post_processor: PostProcessor::None,
            truncation: None,
            padding: None,
        })
    }

    /// Attach (or replace) the pre-tokenizer applied to raw text
    /// before the vocabulary lookup runs.
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pre_tokenizer: WordLevelPreTokenizer) -> Self {
        self.pre_tokenizer = pre_tokenizer;
        self
    }

    /// Attach (or replace) the Unicode normalizer.
    ///
    /// The normalizer runs on the raw input string *before* the
    /// pre-tokenizer, matching HF `tokenizers-rs`' pipeline order:
    /// `normalize -> pre-tokenize -> WordLevel -> post-process`.
    /// See [`Normalizer`] for the supported variants.
    #[must_use]
    pub fn with_normalizer(mut self, normalizer: Normalizer) -> Self {
        self.normalizer = Some(normalizer);
        self
    }

    /// Attach (or replace) the post-processor.
    ///
    /// The post-processor runs on the finished [`Encoding`] before
    /// [`Self::encode`] returns it — the same order the other loaders
    /// in this crate use. See
    /// [`crate::post_processor::PostProcessor`] for the shape.
    #[must_use]
    pub fn with_post_processor(mut self, post_processor: PostProcessor) -> Self {
        self.post_processor = post_processor;
        self
    }

    /// Attach (or replace) the truncation configuration; see
    /// [`crate::BpeTokenizer::with_truncation`] for the shape.
    #[must_use]
    pub fn with_truncation(
        mut self,
        truncation: stringcheese_tokenizer::truncation::TruncationConfig,
    ) -> Self {
        self.truncation = Some(truncation);
        self
    }

    /// Attach (or replace) the padding configuration; see
    /// [`crate::BpeTokenizer::with_padding`] for the shape.
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

    /// Read-only access to the vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &BpeVocabulary {
        &self.vocab
    }

    /// The registered unknown-token id, if any.
    #[must_use]
    pub const fn unk_token_id(&self) -> Option<TokenId> {
        self.unk_token_id
    }

    /// The configured pre-tokenizer variant.
    #[must_use]
    pub fn pre_tokenizer(&self) -> WordLevelPreTokenizer {
        self.pre_tokenizer
    }

    /// Read-only access to the configured normalizer, if any.
    #[must_use]
    pub fn normalizer(&self) -> Option<&Normalizer> {
        self.normalizer.as_ref()
    }

    /// Read-only access to the configured post-processor.
    #[must_use]
    pub fn post_processor(&self) -> &PostProcessor {
        &self.post_processor
    }

    /// Encode `text` into a sequence of token ids.
    ///
    /// Runs the full pipeline: normalise → pre-tokenize → per-word
    /// vocab lookup → post-process. The [`Normalizer`] and
    /// [`PostProcessor`] default to identity (`None` and
    /// [`PostProcessor::None`] respectively) so callers who never
    /// configure them see the plain `pre-tokenize + lookup` behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`WordLevelEncodeError::UnknownWord`] if a word is not
    /// in the vocabulary and no `unk_token_id` was configured on this
    /// tokenizer.
    pub fn encode(&self, text: &str) -> Result<Vec<TokenId>, WordLevelEncodeError> {
        let normalized = self.normalize_text(text);
        let ids = self.encode_ids_raw(normalized.as_ref())?;
        // Fast-path the identity post-processor to avoid a needless
        // `Encoding` allocation on the common no-post-processor path.
        Ok(if matches!(self.post_processor, PostProcessor::None) {
            ids
        } else {
            let mut enc: Encoding<TokenId> = Encoding::new();
            enc.ids = ids;
            self.post_processor.apply(&enc, true).ids
        })
    }

    /// Decode a slice of token ids back into a string.
    ///
    /// `WordLevel` is space-separated by construction, so the surface
    /// strings are joined by a single ASCII space between each. This
    /// matches HF's own `WordLevelDecoder` behaviour.
    ///
    /// # Errors
    ///
    /// * [`WordLevelDecodeError::UnknownId`] — an id in `tokens` is
    ///   not registered in the vocabulary.
    /// * [`WordLevelDecodeError::InvalidUtf8`] — an id's stored bytes
    ///   are not valid UTF-8 (a synthetic vocab shape).
    pub fn decode(&self, tokens: &[TokenId]) -> Result<String, WordLevelDecodeError> {
        let mut out = String::new();
        for (i, &id) in tokens.iter().enumerate() {
            let Some(bytes) = self.vocab.bytes(id) else {
                return Err(WordLevelDecodeError::UnknownId(id));
            };
            let surface =
                core::str::from_utf8(bytes).map_err(|_| WordLevelDecodeError::InvalidUtf8(id))?;
            if i > 0 {
                out.push(' ');
            }
            out.push_str(surface);
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // Internal helpers.
    // -----------------------------------------------------------------

    /// Apply the configured normalizer to `text`, or borrow it through
    /// unchanged when no normalizer is set. Shared by every encode
    /// path so all callers see the same normalisation semantics.
    fn normalize_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        match &self.normalizer {
            Some(n) => Cow::Owned(normalize(text, n)),
            None => Cow::Borrowed(text),
        }
    }

    /// Run the pre-tokenizer + per-word vocab lookup over `text`
    /// and return the raw ids, without post-processing.
    fn encode_ids_raw(&self, text: &str) -> Result<Vec<TokenId>, WordLevelEncodeError> {
        let mut out = Vec::new();
        for word in self.split_words(text) {
            if let Some(id) = self.vocab.id(word.as_bytes()) {
                out.push(id);
            } else if let Some(unk) = self.unk_token_id {
                out.push(unk);
            } else {
                return Err(WordLevelEncodeError::UnknownWord(word));
            }
        }
        Ok(out)
    }

    fn split_words(&self, text: &str) -> Vec<String> {
        match self.pre_tokenizer {
            WordLevelPreTokenizer::WhitespaceSplit => split_whitespace_only(text),
            WordLevelPreTokenizer::Whitespace | WordLevelPreTokenizer::Bert => {
                split_whitespace_and_punctuation(text)
            }
        }
    }
}

impl Tokenizer for WordLevelTokenizer {
    type Token = TokenId;

    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        // Full pipeline: normalize -> pre-tokenize + WordLevel lookup
        // -> post-process -> truncate. Mirrors the inherent `encode`
        // above; kept as a separate assembly because the `Tokenizer`
        // trait must return an `Encoding<TokenId>` and the
        // post-processor operates on one. Offsets and `special_mask`
        // remain empty on the primary encoding (WordLevel does not
        // track offsets); the post-processor's `apply` gracefully
        // preserves empty per-token arrays.
        let normalized = self.normalize_text(text);
        let ids = self
            .encode_ids_raw(normalized.as_ref())
            .map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))?;
        let mut enc: Encoding<TokenId> = Encoding::new();
        enc.ids = ids;
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
        let normalized_a = self.normalize_text(a);
        let normalized_b = self.normalize_text(b);
        let mut ea: Encoding<TokenId> = Encoding::new();
        ea.ids = self
            .encode_ids_raw(normalized_a.as_ref())
            .map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))?;
        let mut eb: Encoding<TokenId> = Encoding::new();
        eb.ids = self
            .encode_ids_raw(normalized_b.as_ref())
            .map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))?;
        if let Some(cfg) = &self.truncation {
            stringcheese_tokenizer::truncation::truncate_pair(&mut ea, &mut eb, cfg);
        }
        Ok(self.post_processor.apply_pair(&ea, &eb, true))
    }

    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError> {
        Self::decode(self, tokens).map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))
    }

    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        // Mirror `encode`'s full pipeline so `count(text) ==
        // encode(text)?.ids.len()` holds for every configuration.
        // Shape follows `BpeTokenizer::count` / `WordPieceTokenizer::count`.
        let normalized = self.normalize_text(text);
        let base = self
            .encode_ids_raw(normalized.as_ref())
            .map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))?
            .len();
        Ok(match &self.post_processor {
            // ByteLevel is a documented no-op on the encoding
            // (see [`crate::post_processor::PostProcessor::ByteLevel`]);
            // the token count is unchanged.
            PostProcessor::None | PostProcessor::ByteLevel { .. } => base,
            PostProcessor::TemplateProcessing(_)
            | PostProcessor::BertProcessing(_)
            | PostProcessor::RobertaProcessing(_) => {
                let mut synth: Encoding<TokenId> = Encoding::new();
                synth.ids.resize(base, 0);
                self.post_processor.apply(&synth, true).ids.len()
            }
            PostProcessor::Sequence(children) => {
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

// ---------------------------------------------------------------------
// Pre-tokenizer helpers.
//
// Duplicated (verbatim from `crate::wordpiece`) so this runtime does
// not reach into a sibling module's private surface — the coupling
// would prevent either from being extracted or re-shaped without
// breaking the other. Keep the two implementations in lockstep if
// either is updated; the shared semantics are documented on the
// public [`WordLevelPreTokenizer`] variants.
// ---------------------------------------------------------------------

/// Split `text` on runs of Unicode whitespace. Empty runs (leading /
/// trailing / consecutive) collapse; no empty word is emitted.
fn split_whitespace_only(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// Split `text` on runs of Unicode whitespace *and* on punctuation
/// boundaries — punctuation characters attach neither to what comes
/// before nor what comes after and are emitted as their own words.
fn split_whitespace_and_punctuation(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        split_on_punctuation_into(word, &mut out);
    }
    out
}

fn split_on_punctuation_into(word: &str, out: &mut Vec<String>) {
    let mut current = String::new();
    for c in word.chars() {
        if is_punctuation(c) {
            if !current.is_empty() {
                out.push(core::mem::take(&mut current));
            }
            let mut s = String::new();
            s.push(c);
            out.push(s);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
}

/// BERT punctuation rule — mirrors
/// [`crate::wordpiece`](crate::wordpiece)'s `is_punctuation` (which is
/// private). See its module doc for the rule's provenance and the
/// deliberate narrow scope (ASCII + Latin-1 + General Punctuation +
/// CJK Symbols + fullwidth ASCII lookalikes).
fn is_punctuation(c: char) -> bool {
    if c.is_ascii_punctuation() {
        return true;
    }
    matches!(
        c,
        '\u{00A1}'
            | '\u{00A7}'
            | '\u{00AB}'
            | '\u{00B6}'
            | '\u{00B7}'
            | '\u{00BB}'
            | '\u{00BF}'
            | '\u{2010}'..='\u{2027}'
            | '\u{2030}'..='\u{205E}'
            | '\u{3000}'..='\u{303F}'
            | '\u{FF01}'..='\u{FF0F}'
            | '\u{FF1A}'..='\u{FF20}'
            | '\u{FF3B}'..='\u{FF40}'
            | '\u{FF5B}'..='\u{FF65}'
    )
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Five-word hand-crafted vocab used by the encode / decode /
    /// round-trip tests. `[UNK]` at id 0; four content words at ids
    /// 1..=4.
    fn small_vocab() -> Vec<(String, TokenId)> {
        vec![
            ("[UNK]".to_string(), 0),
            ("hello".to_string(), 1),
            ("world".to_string(), 2),
            ("foo".to_string(), 3),
            ("bar".to_string(), 4),
        ]
    }

    #[test]
    fn from_parts_rejects_unk_id_missing_from_vocab() {
        let err = WordLevelTokenizer::from_parts(small_vocab(), Some(99)).unwrap_err();
        assert_eq!(err, WordLevelBuildError::UnkNotInVocab(99));
    }

    #[test]
    fn from_parts_accepts_no_unk_token() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), None).unwrap();
        assert_eq!(tok.unk_token_id(), None);
    }

    #[test]
    fn from_parts_rejects_duplicate_ids() {
        let vocab = vec![("a".to_string(), 1), ("b".to_string(), 1)];
        let err = WordLevelTokenizer::from_parts(vocab, None).unwrap_err();
        assert!(matches!(err, WordLevelBuildError::Vocabulary(_)));
    }

    #[test]
    fn encode_maps_words_and_emits_unk_for_oov() {
        // Task-specified canonical example: 5-word vocab, encode
        // "hello world foo unknown" where "unknown" is OOV — unk id
        // appears in its slot.
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        let ids = tok.encode("hello world foo unknown").unwrap();
        assert_eq!(ids, vec![1, 2, 3, 0]);
        assert!(ids.contains(&0), "unk_id (0) must appear in the encoding");
    }

    #[test]
    fn encode_returns_error_when_oov_and_no_unk() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), None).unwrap();
        let err = tok.encode("hello unknown").unwrap_err();
        match err {
            WordLevelEncodeError::UnknownWord(w) => assert_eq!(w, "unknown"),
        }
    }

    #[test]
    fn encode_all_hits_when_input_is_covered() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), None).unwrap();
        let ids = tok.encode("hello world").unwrap();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn encode_empty_input_yields_no_tokens() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        assert!(tok.encode("").unwrap().is_empty());
        assert!(tok.encode("   ").unwrap().is_empty());
    }

    #[test]
    fn decode_roundtrip_joins_words_with_single_space() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        let ids = tok.encode("hello world foo").unwrap();
        assert_eq!(tok.decode(&ids).unwrap(), "hello world foo");
    }

    #[test]
    fn decode_rejects_unknown_id() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        let err = tok.decode(&[42]).unwrap_err();
        assert_eq!(err, WordLevelDecodeError::UnknownId(42));
    }

    #[test]
    fn whitespace_split_default_keeps_punctuation_glued() {
        // Default pre-tokenizer is WhitespaceSplit: "hello," is looked
        // up as one word — missing from the vocab → unk id.
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        assert_eq!(tok.encode("hello,").unwrap(), vec![0]);
    }

    #[test]
    fn whitespace_pretokenizer_splits_punctuation() {
        // Add "," to the vocab so the punctuation-splitter path shows
        // its work: "hello, world" -> ["hello", ",", "world"].
        let mut vocab = small_vocab();
        vocab.push((",".to_string(), 5));
        let tok = WordLevelTokenizer::from_parts(vocab, Some(0))
            .unwrap()
            .with_pre_tokenizer(WordLevelPreTokenizer::Whitespace);
        assert_eq!(tok.encode("hello, world").unwrap(), vec![1, 5, 2]);
    }

    #[test]
    fn bert_pretokenizer_is_alias_of_whitespace() {
        // Bert is semantically identical to Whitespace on this
        // implementation; verify the aliasing.
        let mut vocab = small_vocab();
        vocab.push((",".to_string(), 5));
        let tok = WordLevelTokenizer::from_parts(vocab, Some(0))
            .unwrap()
            .with_pre_tokenizer(WordLevelPreTokenizer::Bert);
        assert_eq!(tok.encode("hello, world").unwrap(), vec![1, 5, 2]);
    }

    #[test]
    fn tokenizer_trait_encode_returns_ids_only_encoding() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        let enc = Tokenizer::encode(&tok, "hello world").unwrap();
        assert_eq!(enc.ids, vec![1, 2]);
        // Offsets and special_mask are empty — WordLevel does not
        // track them.
        assert!(enc.offsets.is_empty());
        assert!(enc.special_mask.is_empty());
    }

    #[test]
    fn tokenizer_trait_count_matches_encode_length() {
        let tok = WordLevelTokenizer::from_parts(small_vocab(), Some(0)).unwrap();
        assert_eq!(Tokenizer::count(&tok, "hello world foo").unwrap(), 3);
    }

    #[test]
    fn tokenizer_trait_encode_surfaces_unk_word_error_as_unknown_token() {
        // With no unk configured, the trait's `encode` must surface
        // the domain error as `TokenizerError::UnknownToken` — the
        // trait's fixed taxonomy has no dedicated variant for the
        // OOV case, and every other loader in this crate uses the
        // same mapping.
        let tok = WordLevelTokenizer::from_parts(small_vocab(), None).unwrap();
        let err = Tokenizer::encode(&tok, "hello unknown").unwrap_err();
        assert!(matches!(err, TokenizerError::UnknownToken(_)));
    }

    // -----------------------------------------------------------------
    // Normalizer + post-processor wiring
    // -----------------------------------------------------------------

    #[test]
    fn normalizer_runs_before_pre_tokenization() {
        // Vocab is lowercase; the input is mixed-case. The Lowercase
        // normalizer must run first so the vocab lookup succeeds.
        let vocab = vec![
            ("[UNK]".to_string(), 0),
            ("hello".to_string(), 1),
            ("world".to_string(), 2),
        ];
        let tok = WordLevelTokenizer::from_parts(vocab, Some(0))
            .unwrap()
            .with_normalizer(Normalizer::Lowercase);
        assert_eq!(tok.encode("Hello WORLD").unwrap(), vec![1, 2]);
    }

    #[test]
    fn template_post_processor_wraps_primary_encoding() {
        use crate::post_processor::{
            PostProcessor, SpecialTokenInfo, TemplatePiece, TemplateProcessing,
        };
        use alloc::collections::BTreeMap;

        let vocab = vec![
            ("[UNK]".to_string(), 0),
            ("[CLS]".to_string(), 1),
            ("[SEP]".to_string(), 2),
            ("hello".to_string(), 3),
            ("world".to_string(), 4),
        ];
        let mut specials = BTreeMap::new();
        specials.insert(
            "[CLS]".to_string(),
            SpecialTokenInfo {
                ids: vec![1],
                tokens: vec!["[CLS]".to_string()],
            },
        );
        specials.insert(
            "[SEP]".to_string(),
            SpecialTokenInfo {
                ids: vec![2],
                tokens: vec!["[SEP]".to_string()],
            },
        );
        let tp = TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "[CLS]".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
                TemplatePiece::SpecialToken {
                    id: "[SEP]".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: specials,
        };
        let tok = WordLevelTokenizer::from_parts(vocab, Some(0))
            .unwrap()
            .with_post_processor(PostProcessor::TemplateProcessing(tp));
        // Primary encoding of "hello world" is [3, 4]; template splices
        // [CLS]=1 before and [SEP]=2 after → [1, 3, 4, 2].
        let ids = tok.encode("hello world").unwrap();
        assert_eq!(ids, vec![1, 3, 4, 2]);
        // `count` must agree with `encode(text)?.ids.len()`.
        assert_eq!(Tokenizer::count(&tok, "hello world").unwrap(), ids.len());
    }
}
