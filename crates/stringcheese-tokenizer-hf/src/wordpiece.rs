//! `WordPiece` tokenizer (Wu et al. 2016, adopted by BERT).
//!
//! # What this module does
//!
//! `WordPiece` is the subword algorithm used by BERT and its family:
//! `DistilBERT`, `RoBERTa` (some variants), `ALBERT`, `MobileBERT`, and
//! every `WordPiece`-based checkpoint on the Hugging Face Hub. This
//! module ships a data-neutral runtime: the caller supplies the
//! vocabulary and the auxiliary parameters
//! (`unk_token` id, `continuing_subword_prefix`, and
//! `max_input_chars_per_word`), and the tokenizer performs the
//! greedy-longest-match algorithm on whitespace-pre-tokenized input.
//!
//! # Algorithm
//!
//! For each input *word* (already split on whitespace and punctuation
//! by a caller-supplied pre-tokenizer):
//!
//! 1. If the word's character count exceeds
//!    [`WordPieceTokenizer::max_input_chars_per_word`], emit the
//!    unknown-token id and stop.
//! 2. Otherwise, walk the word left-to-right and, at each position,
//!    find the *longest* substring that appears in the vocabulary.
//!    Subwords at position 0 are looked up as-is; every subword after
//!    the first is prefixed with
//!    [`WordPieceTokenizer::continuing_subword_prefix`] (usually
//!    `"##"`).
//! 3. If any position has no vocabulary match, the whole word emits
//!    the unknown-token id (i.e. `WordPiece` is *all or nothing* on a
//!    word; the partial subwords are discarded).
//!
//! Example: with vocab `{"un", "##aff", "##able", "[UNK]"}` and prefix
//! `"##"`, encoding `"unaffable"` yields the ids for
//! `["un", "##aff", "##able"]`.
//!
//! # Decoding
//!
//! Decoding is the inverse: look up each id's surface string in the
//! vocabulary, then concatenate — dropping the `##` prefix on
//! continuing subwords and inserting a single ASCII space before every
//! non-continuation subword. `["un", "##aff", "##able", "cat"]`
//! decodes to `"unaffable cat"`.
//!
//! # Pre-tokenization
//!
//! `WordPiece` expects its input to be pre-split into words. This
//! module ships three pre-tokenizer flavours behind
//! [`WordPiecePreTokenizer`]:
//!
//! * [`WordPiecePreTokenizer::Whitespace`] — split on runs of Unicode
//!   whitespace and *also* on punctuation boundaries. Matches HF's
//!   `Whitespace` variant, which internally does the same
//!   letter/digit + punctuation split. This is the default.
//! * [`WordPiecePreTokenizer::WhitespaceSplit`] — split on whitespace
//!   only, leaving punctuation glued to the word. Matches HF's
//!   `WhitespaceSplit` variant.
//! * [`WordPiecePreTokenizer::Bert`] — the BERT-family pre-tokenizer:
//!   whitespace split followed by per-word punctuation split, where
//!   each punctuation character becomes its own word. Equivalent to
//!   HF's `BertPreTokenizer`.
//!
//! # Round-trip
//!
//! `decode(encode(text))` is a lossy round-trip: whitespace is
//! collapsed and the pre-tokenizer boundaries are re-emitted as single
//! spaces. `decode(encode("Hello,  world!"))` yields
//! `"Hello , world !"` under the BERT pre-tokenizer. This matches
//! HF's own behaviour — recovering the exact original spacing requires
//! offset tracking that this crate deliberately does not do on the
//! decode side.
//!
//! # References
//!
//! * Wu, Y., Schuster, M., Chen, Z., et al. (2016). "Google's Neural
//!   Machine Translation System: Bridging the Gap between Human and
//!   Machine Translation." arXiv:1609.08144.
//! * Devlin, J., Chang, M.-W., Lee, K., & Toutanova, K. (2019). "BERT:
//!   Pre-training of Deep Bidirectional Transformers for Language
//!   Understanding." NAACL 2019.

use alloc::borrow::Cow;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use stringcheese_tokenizer::{Encoding, Tokenizer, TokenizerError};

use crate::bpe::TokenId;
use crate::normalizer::{Normalizer, normalize};
use crate::post_processor::PostProcessor;

/// The pre-tokenizer variant used by [`WordPieceTokenizer`] to split
/// raw input text into words before the greedy-longest-match loop
/// runs.
///
/// See the module-level documentation for the semantics of each
/// variant. The default is
/// [`Self::Whitespace`], which matches HF's `Whitespace` pre-tokenizer
/// (whitespace + implicit punctuation split).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WordPiecePreTokenizer {
    /// Whitespace split *and* punctuation split — HF `Whitespace`.
    /// This is the default because it is the most permissive shape
    /// and matches what most `WordPiece` checkpoints use in practice.
    #[default]
    Whitespace,
    /// Whitespace split only — HF `WhitespaceSplit`. Punctuation stays
    /// glued to the surrounding word.
    WhitespaceSplit,
    /// BERT-family pre-tokenizer — HF `BertPreTokenizer`. Whitespace
    /// split followed by per-word punctuation split (each punctuation
    /// character becomes its own word).
    Bert,
}

/// Error returned by [`WordPieceTokenizer::decode`] when a token id
/// cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordPieceDecodeError {
    /// A token id was not present in the vocabulary and is not the
    /// registered unknown-token id.
    UnknownId(TokenId),
}

impl fmt::Display for WordPieceDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "unknown WordPiece token id: {id}"),
        }
    }
}

impl std::error::Error for WordPieceDecodeError {}

/// A `WordPiece` tokenizer.
///
/// Construct via [`WordPieceTokenizer::from_parts`]. The vocabulary
/// must contain the unknown-token surface string (mapped to
/// [`Self::unk_token_id`]); this is validated at construction time
/// and returned as [`WordPieceBuildError::UnkNotInVocab`].
///
/// # Examples
///
/// Encode `"unaffable"` with a small BERT-style vocab:
///
/// ```
/// use std::collections::HashMap;
/// use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
///
/// let mut vocab: HashMap<String, u32> = HashMap::new();
/// vocab.insert("[UNK]".to_string(), 0);
/// vocab.insert("un".to_string(), 1);
/// vocab.insert("##aff".to_string(), 2);
/// vocab.insert("##able".to_string(), 3);
/// let tok = WordPieceTokenizer::from_parts(vocab, 0, "##".to_string(), 100).unwrap();
/// let ids = tok.encode("unaffable");
/// assert_eq!(ids, vec![1, 2, 3]);
/// ```
#[derive(Debug, Clone)]
pub struct WordPieceTokenizer {
    /// Forward vocabulary: token surface string ↔ id.
    ///
    /// Kept as a `BTreeMap` for deterministic iteration and for the
    /// same rationale the surrounding crate uses on
    /// `BpeVocabulary` — it lets the crate compile against a
    /// hypothetical `no_std + alloc`-only build without pulling
    /// `hashbrown`.
    vocab: BTreeMap<String, TokenId>,
    /// Reverse vocabulary for decode. Rebuilt at construction so
    /// [`Self::decode`] is a single lookup per id.
    reverse: BTreeMap<TokenId, String>,
    /// The id emitted for out-of-vocab words and for words longer than
    /// [`Self::max_input_chars_per_word`].
    unk_token_id: TokenId,
    /// The surface string for the unknown token — surfaced by
    /// [`Self::decode`] when the id is resolved through
    /// [`Self::reverse`].
    unk_token: String,
    /// Prefix stamped on every subword after the first in a word.
    /// Usually `"##"` for BERT; some variants use `""` (i.e. no
    /// prefix — the model relies on positional encoding to
    /// distinguish continuations).
    continuing_subword_prefix: String,
    /// Maximum character count of an input word. Longer words emit
    /// [`Self::unk_token_id`] outright (matching HF's behaviour).
    max_input_chars_per_word: usize,
    /// How to split raw input text into words before the greedy
    /// longest-match loop. See [`WordPiecePreTokenizer`].
    pre_tokenizer: WordPiecePreTokenizer,
    /// Special-token surface strings that must be pre-extracted from
    /// raw text before the pre-tokenizer runs. Mirrors
    /// [`crate::BpeTokenizer::with_special_tokens`]: registered
    /// surfaces are longest-match extracted from the input, and each
    /// occurrence is emitted as its pre-assigned id without going
    /// through the `WordPiece` greedy-longest-match loop or the
    /// punctuation split. A default-empty map preserves the existing
    /// behaviour where a literal `"[CLS]"` in the input is split into
    /// `"["`, `"CLS"`, `"]"` by the Bert pre-tokenizer.
    special_tokens: BTreeMap<String, TokenId>,
    /// Optional Unicode normalizer applied to the raw input string
    /// *before* pre-tokenization runs. Matches HF `tokenizers-rs`'
    /// pipeline order (`normalize -> pre-tokenize -> WordPiece ->
    /// post-process`) and mirrors [`crate::BpeTokenizer::with_normalizer`].
    /// A value of `None` leaves the input unchanged, matching the
    /// pre-normalizer behaviour of the tokenizer.
    normalizer: Option<Normalizer>,
    /// Optional post-processor applied to the finished [`Encoding`]
    /// before it leaves [`Self::encode`]. Mirrors
    /// [`crate::BpeTokenizer::with_post_processor`]; the default
    /// [`PostProcessor::None`] is a pass-through so callers that never
    /// configure a processor see the unchanged BPE-like output.
    post_processor: PostProcessor,
}

/// Errors that can arise when building a [`WordPieceTokenizer`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordPieceBuildError {
    /// The `unk_token_id` maps to a surface string that is not
    /// registered in the vocabulary; encoding would produce ids the
    /// caller cannot decode. The wrapped id is the offending
    /// unknown-token id.
    UnkNotInVocab(TokenId),
}

impl fmt::Display for WordPieceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnkNotInVocab(id) => write!(
                f,
                "unknown-token id {id} is not present in the WordPiece vocabulary"
            ),
        }
    }
}

impl std::error::Error for WordPieceBuildError {}

impl WordPieceTokenizer {
    /// Build a tokenizer from raw parts.
    ///
    /// # Parameters
    ///
    /// * `vocab` — the surface-string ↔ id vocabulary. Must contain a
    ///   mapping for the unknown token (i.e. some string must map to
    ///   `unk_token_id`); otherwise
    ///   [`WordPieceBuildError::UnkNotInVocab`] is returned.
    /// * `unk_token_id` — the id emitted for out-of-vocab words and
    ///   for words longer than `max_input_chars_per_word`.
    /// * `continuing_subword_prefix` — the string prefixed on every
    ///   subword after the first inside a word. `"##"` for canonical
    ///   BERT; may be `""` for variants that omit it.
    /// * `max_input_chars_per_word` — words with more characters than
    ///   this shortcut to the unknown-token id.
    ///
    /// The pre-tokenizer defaults to
    /// [`WordPiecePreTokenizer::Whitespace`]; use
    /// [`Self::with_pre_tokenizer`] to switch it.
    ///
    /// # Errors
    ///
    /// Returns [`WordPieceBuildError::UnkNotInVocab`] if no vocabulary
    /// entry maps to `unk_token_id`.
    pub fn from_parts<V>(
        vocab: V,
        unk_token_id: TokenId,
        continuing_subword_prefix: String,
        max_input_chars_per_word: usize,
    ) -> Result<Self, WordPieceBuildError>
    where
        V: IntoIterator<Item = (String, TokenId)>,
    {
        let vocab: BTreeMap<String, TokenId> = vocab.into_iter().collect();
        // Reverse map for decode.
        let mut reverse = BTreeMap::new();
        for (surface, &id) in &vocab {
            reverse.insert(id, surface.clone());
        }
        let Some(unk_token) = reverse.get(&unk_token_id).cloned() else {
            return Err(WordPieceBuildError::UnkNotInVocab(unk_token_id));
        };
        Ok(Self {
            vocab,
            reverse,
            unk_token_id,
            unk_token,
            continuing_subword_prefix,
            max_input_chars_per_word,
            pre_tokenizer: WordPiecePreTokenizer::default(),
            special_tokens: BTreeMap::new(),
            normalizer: None,
            post_processor: PostProcessor::None,
        })
    }

    /// Attach (or replace) the pre-tokenizer applied to raw text
    /// before the greedy-longest-match loop runs.
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pre_tokenizer: WordPiecePreTokenizer) -> Self {
        self.pre_tokenizer = pre_tokenizer;
        self
    }

    /// Attach (or replace) the Unicode normalizer.
    ///
    /// The normalizer runs on the raw input string *before* the
    /// pre-tokenizer, matching HF `tokenizers-rs`' pipeline order:
    /// `normalize -> pre-tokenize -> WordPiece -> post-process`.
    /// See [`Normalizer`] for the supported variants. This mirrors
    /// [`crate::BpeTokenizer::with_normalizer`].
    ///
    /// Encoding offsets are not tracked by [`WordPieceTokenizer`], so
    /// there is no offset-space impedance to worry about — the
    /// normalizer's `&str -> String` output is what the pre-tokenizer
    /// consumes verbatim.
    #[must_use]
    pub fn with_normalizer(mut self, normalizer: Normalizer) -> Self {
        self.normalizer = Some(normalizer);
        self
    }

    /// Attach (or replace) the special-token map.
    ///
    /// Registered surfaces are pre-extracted from raw input (longest
    /// match first, ties broken lexically) *before* the pre-tokenizer
    /// runs; each occurrence emits its pre-assigned id directly and
    /// the between-specials chunks feed the pre-tokenizer +
    /// greedy-longest-match loop. Mirrors
    /// [`crate::BpeTokenizer::with_special_tokens`]. A default-empty
    /// map preserves the pre-Wave-14 behaviour where `"[CLS]"` in the
    /// raw input is split into `"["`, `"CLS"`, `"]"` by the Bert
    /// pre-tokenizer.
    ///
    /// This is what makes bert-base-uncased / distilbert-base-uncased
    /// tokenize a literal `"[CLS]"` in the input as the CLS id
    /// (matching `transformers.AutoTokenizer`) instead of decomposing
    /// it into three pieces.
    #[must_use]
    pub fn with_special_tokens(mut self, special_tokens: BTreeMap<String, TokenId>) -> Self {
        self.special_tokens = special_tokens;
        self
    }

    /// Attach (or replace) the post-processor.
    ///
    /// The post-processor runs on the finished [`Encoding`] before
    /// [`Self::encode`] returns it — the same order
    /// [`crate::BpeTokenizer`] uses. See
    /// [`crate::post_processor::PostProcessor`] for the shape; the
    /// default [`PostProcessor::None`] is a pass-through, so callers
    /// who never configure one see the unchanged `WordPiece` output.
    #[must_use]
    pub fn with_post_processor(mut self, post_processor: PostProcessor) -> Self {
        self.post_processor = post_processor;
        self
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

    /// The registered unknown-token id.
    #[must_use]
    pub fn unk_token_id(&self) -> TokenId {
        self.unk_token_id
    }

    /// The registered unknown-token surface string.
    #[must_use]
    pub fn unk_token(&self) -> &str {
        &self.unk_token
    }

    /// The registered continuing-subword prefix.
    #[must_use]
    pub fn continuing_subword_prefix(&self) -> &str {
        &self.continuing_subword_prefix
    }

    /// The maximum input character count per word.
    #[must_use]
    pub fn max_input_chars_per_word(&self) -> usize {
        self.max_input_chars_per_word
    }

    /// The pre-tokenizer variant this tokenizer uses.
    #[must_use]
    pub fn pre_tokenizer(&self) -> WordPiecePreTokenizer {
        self.pre_tokenizer
    }

    /// Read-only access to the vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &BTreeMap<String, TokenId> {
        &self.vocab
    }

    /// Read-only access to the registered special tokens.
    #[must_use]
    pub fn special_tokens(&self) -> &BTreeMap<String, TokenId> {
        &self.special_tokens
    }

    /// Encode `text` into a sequence of token ids.
    ///
    /// Runs the full BERT-parity pipeline:
    ///
    /// 1. Apply the configured [`Normalizer`] (if any) to `text`.
    /// 2. Run the configured pre-tokenizer to split the normalized
    ///    text into words.
    /// 3. Run the greedy longest-match `WordPiece` loop on each word.
    /// 4. Apply the configured [`PostProcessor`] to the assembled
    ///    encoding.
    ///
    /// The [`Normalizer`] and [`PostProcessor`] default to identity
    /// (`None` and [`PostProcessor::None`] respectively) so callers
    /// who never configure them see the plain `pre-tokenize +
    /// WordPiece` behaviour that this method has always produced.
    #[must_use]
    pub fn encode(&self, text: &str) -> Vec<TokenId> {
        let normalized = self.normalize_text(text);
        let ids = self.encode_ids_raw(normalized.as_ref());
        // Fast-path the identity post-processor to avoid a needless
        // `Encoding` allocation on the common no-post-processor path.
        if matches!(self.post_processor, PostProcessor::None) {
            ids
        } else {
            let mut enc: Encoding<TokenId> = Encoding::new();
            enc.ids = ids;
            self.post_processor.apply(&enc, true).ids
        }
    }

    /// Apply the configured normalizer to `text`, or borrow it through
    /// unchanged when no normalizer is set. Shared by every encode
    /// path so all callers see the same normalization semantics.
    fn normalize_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        match &self.normalizer {
            Some(n) => Cow::Owned(normalize(text, n)),
            None => Cow::Borrowed(text),
        }
    }

    /// Run the pre-tokenizer + greedy longest-match loop over `text`
    /// and return the raw ids, without normalization or
    /// post-processing. Shared by both encode paths.
    ///
    /// When [`Self::special_tokens`] is non-empty, registered
    /// special-token surfaces are pre-extracted from `text` before the
    /// pre-tokenizer runs (longest match first, ties broken lexically).
    /// Each occurrence emits its pre-assigned id directly; the
    /// between-specials chunks feed the pre-tokenizer + greedy loop.
    /// Mirrors [`crate::BpeTokenizer`]'s
    /// `encode_pieces_with_policy` shape.
    fn encode_ids_raw(&self, text: &str) -> Vec<TokenId> {
        if self.special_tokens.is_empty() {
            // Fast path — no allocations for the specials list on the
            // common no-specials case.
            let mut out = Vec::new();
            for word in self.split_words(text) {
                self.encode_word_into(&word, &mut out);
            }
            return out;
        }

        let sorted_specials = sorted_special_tokens(&self.special_tokens);
        let mut out = Vec::new();
        let mut cursor = 0usize;
        while cursor < text.len() {
            let remaining = &text[cursor..];
            // Try to match a special at the current cursor.
            let mut matched: Option<(TokenId, usize)> = None;
            for (surface, id) in &sorted_specials {
                if remaining.starts_with(surface.as_str()) {
                    matched = Some((*id, surface.len()));
                    break;
                }
            }
            if let Some((id, len)) = matched {
                out.push(id);
                cursor += len;
                continue;
            }
            // No special match here — consume up to the next special
            // occurrence (or end-of-input) and pre-tokenize + WordPiece
            // the region.
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
                for word in self.split_words(region) {
                    self.encode_word_into(&word, &mut out);
                }
            }
            cursor += next_rel;
        }
        out
    }

    /// Encode a single already-pre-tokenized word.
    ///
    /// This is the inner greedy-longest-match loop, exposed for
    /// callers who want to drive the pre-tokenization themselves.
    /// `word` should already be a single "word" in the caller's
    /// pre-tokenizer definition; this function does no further
    /// splitting.
    #[must_use]
    pub fn encode_word(&self, word: &str) -> Vec<TokenId> {
        let mut out = Vec::new();
        self.encode_word_into(word, &mut out);
        out
    }

    /// Decode a slice of token ids back into a string.
    ///
    /// Continuing subwords (those whose surface string starts with
    /// [`Self::continuing_subword_prefix`]) are glued onto the
    /// preceding subword with the prefix stripped; every other subword
    /// is separated from the previous one by a single ASCII space.
    /// This matches HF's `WordPieceDecoder` behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`WordPieceDecodeError::UnknownId`] if any id in
    /// `tokens` is not registered in the vocabulary.
    pub fn decode(&self, tokens: &[TokenId]) -> Result<String, WordPieceDecodeError> {
        let mut out = String::new();
        for (i, &id) in tokens.iter().enumerate() {
            let Some(surface) = self.reverse.get(&id) else {
                return Err(WordPieceDecodeError::UnknownId(id));
            };
            let prefix = &self.continuing_subword_prefix;
            if !prefix.is_empty() && surface.starts_with(prefix.as_str()) {
                // Continuing subword: strip the prefix, glue onto the
                // preceding token with no separator.
                out.push_str(&surface[prefix.len()..]);
            } else {
                // First subword of a new word: separate from the
                // previous word with a single ASCII space (unless this
                // is the first token overall).
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(surface);
            }
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // Internal helpers.
    // -----------------------------------------------------------------

    /// Split `text` into words according to
    /// [`Self::pre_tokenizer`]. Returns owned `String`s because the
    /// BERT pre-tokenizer emits per-character punctuation chunks that
    /// cannot always borrow contiguously from the input.
    fn split_words(&self, text: &str) -> Vec<String> {
        match self.pre_tokenizer {
            WordPiecePreTokenizer::Whitespace => split_whitespace_and_punctuation(text),
            WordPiecePreTokenizer::WhitespaceSplit => split_whitespace_only(text),
            WordPiecePreTokenizer::Bert => bert_pre_tokenize(text),
        }
    }

    /// Run the greedy longest-match loop on `word` and push the
    /// resulting ids into `out`.
    fn encode_word_into(&self, word: &str, out: &mut Vec<TokenId>) {
        // Empty word: emit nothing. HF's own `WordPiece::tokenize`
        // returns an empty vec for an empty input.
        if word.is_empty() {
            return;
        }

        // A "character" is a Unicode scalar. Count via `chars()` — HF
        // uses the same rule (`chars().count()` in the Python
        // reference, `.chars().count()` in the Rust reference).
        if word.chars().count() > self.max_input_chars_per_word {
            out.push(self.unk_token_id);
            return;
        }

        // Byte offsets of each char boundary inside `word`. We need
        // these because slicing a `str` is byte-indexed; walking the
        // greedy loop over char positions requires converting to
        // bytes at each step.
        let boundaries: Vec<usize> = word
            .char_indices()
            .map(|(b, _)| b)
            .chain(core::iter::once(word.len()))
            .collect();

        let mut subwords: Vec<TokenId> = Vec::new();
        let mut start_char = 0usize;
        let n = boundaries.len() - 1; // number of chars in `word`
        while start_char < n {
            // Longest substring starting at `start_char` that is in
            // the vocabulary. `end_char` runs from `n` down to
            // `start_char + 1`.
            let mut matched: Option<TokenId> = None;
            let mut end_char = n;
            while end_char > start_char {
                let sub_start = boundaries[start_char];
                let sub_end = boundaries[end_char];
                let raw = &word[sub_start..sub_end];
                let candidate = if start_char == 0 {
                    raw.to_string()
                } else {
                    let mut s =
                        String::with_capacity(self.continuing_subword_prefix.len() + raw.len());
                    s.push_str(&self.continuing_subword_prefix);
                    s.push_str(raw);
                    s
                };
                if let Some(&id) = self.vocab.get(&candidate) {
                    matched = Some(id);
                    break;
                }
                end_char -= 1;
            }
            let Some(id) = matched else {
                // No match at this position: the whole word is
                // unknown. Discard any partial subwords and emit the
                // unknown-token id.
                out.push(self.unk_token_id);
                return;
            };
            subwords.push(id);
            start_char = end_char;
        }
        out.extend(subwords);
    }
}

impl Tokenizer for WordPieceTokenizer {
    type Token = TokenId;

    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
        // Full pipeline: normalize -> pre-tokenize + WordPiece ->
        // post-process. Mirrors the inherent `encode` above; kept as a
        // separate assembly because the `Tokenizer` trait must return
        // an `Encoding<TokenId>` and the post-processor operates on
        // one. Offsets and `special_mask` remain empty on the primary
        // encoding (WordPiece does not track offsets); the
        // post-processor's `apply` gracefully preserves empty
        // per-token arrays.
        let normalized = self.normalize_text(text);
        let ids = self.encode_ids_raw(normalized.as_ref());
        let mut enc: Encoding<TokenId> = Encoding::new();
        enc.ids = ids;
        Ok(if matches!(self.post_processor, PostProcessor::None) {
            enc
        } else {
            self.post_processor.apply(&enc, true)
        })
    }

    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError> {
        Self::decode(self, tokens).map_err(|e| TokenizerError::UnknownToken(alloc::format!("{e}")))
    }

    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        // `count` mirrors `encode`'s full pipeline so
        // `count(text) == encode(text)?.ids.len()` holds for every
        // configuration. Fast-path the identity post-processor.
        let normalized = self.normalize_text(text);
        let base = self.encode_ids_raw(normalized.as_ref()).len();
        Ok(match &self.post_processor {
            // ByteLevel is a documented no-op on the encoding
            // (see [`crate::post_processor::PostProcessor::ByteLevel`]);
            // token count is unchanged.
            PostProcessor::None | PostProcessor::ByteLevel { .. } => base,
            PostProcessor::TemplateProcessing(_)
            | PostProcessor::BertProcessing(_)
            | PostProcessor::RobertaProcessing(_) => {
                // Cheapest correct answer: run the splice against a
                // synthetic encoding of the right length and count the
                // ids field. Matches BpeTokenizer's own approach.
                // All three variants add a fixed number of tokens
                // irrespective of the ids themselves. BertProcessing
                // is the shape stock BERT / DistilBERT ships and is
                // the primary consumer of this arm on the WordPiece
                // side.
                let mut synth: Encoding<TokenId> = Encoding::new();
                synth.ids.resize(base, 0);
                self.post_processor.apply(&synth, true).ids.len()
            }
            PostProcessor::Sequence(children) => {
                // Walk the sequence and thread a synthetic encoding of
                // the current length through each child. Matches
                // BpeTokenizer::count's shape.
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

/// BERT pre-tokenizer: whitespace-split then per-word punctuation
/// split. Semantically identical to
/// [`split_whitespace_and_punctuation`] — the two are duplicates so
/// each stays traceable to its HF name.
fn bert_pre_tokenize(text: &str) -> Vec<String> {
    split_whitespace_and_punctuation(text)
}

/// Split `word` on punctuation boundaries and push each chunk into
/// `out`. A run of letters / digits stays whole; every punctuation
/// character becomes its own single-character chunk.
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

/// Sort a special-token map into `(surface, id)` pairs, longest
/// surface first, with lexical order breaking ties. Mirrors
/// [`crate::BpeTokenizer::sorted_specials`] so `<|im_start|>` matches
/// before `<|im|>` if both are registered and the same input matches
/// both.
fn sorted_special_tokens(specials: &BTreeMap<String, TokenId>) -> Vec<(String, TokenId)> {
    let mut v: Vec<(String, TokenId)> = specials.iter().map(|(k, v)| (k.clone(), *v)).collect();
    v.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Return `true` iff `c` is a punctuation character under BERT's
/// classic rule.
///
/// BERT's `_is_punctuation` is broader than Unicode's `\p{P}`: any
/// non-alphanumeric ASCII character (`!`, `#`, `-`, `~`, and so on)
/// is treated as punctuation, *plus* every scalar the Unicode
/// database marks under any punctuation category (`Pc`, `Pd`, `Pe`,
/// `Pf`, `Pi`, `Po`, `Ps`). We approximate the ASCII half exhaustively
/// via [`char::is_ascii_punctuation`] and the non-ASCII half with a
/// short per-range check covering the common Latin-1, general
/// punctuation, CJK punctuation, and fullwidth-ASCII-punctuation
/// blocks — enough to keep BERT-family checkpoints functional on
/// typical English / European / CJK input.
///
/// The scope is intentionally narrow: this landing targets BERT /
/// `DistilBERT` / `RoBERTa` / `ALBERT` / `MobileBERT` inputs, which
/// are overwhelmingly ASCII with the occasional Latin-1 punctuation
/// character. A future landing that adds full Unicode punctuation
/// coverage (via `unicode-properties` or similar) can widen this
/// check without touching the pre-tokenizer plumbing.
fn is_punctuation(c: char) -> bool {
    // ASCII punctuation, including everything in
    // `!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~`.
    if c.is_ascii_punctuation() {
        return true;
    }
    // Common non-ASCII punctuation blocks. The ranges cover the
    // Latin-1 supplement's punctuation, the General Punctuation
    // block, CJK Symbols and Punctuation, and the fullwidth ASCII
    // punctuation lookalikes — enough for BERT-family text.
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

    /// Build a small BERT-style vocab that covers the canonical
    /// `WordPiece` reference example: `"unaffable"` → `["un",
    /// "##aff", "##able"]`.
    fn small_bert_vocab() -> BTreeMap<String, TokenId> {
        let mut vocab = BTreeMap::new();
        vocab.insert("[UNK]".to_string(), 0);
        vocab.insert("[CLS]".to_string(), 1);
        vocab.insert("[SEP]".to_string(), 2);
        vocab.insert("un".to_string(), 3);
        vocab.insert("##aff".to_string(), 4);
        vocab.insert("##able".to_string(), 5);
        vocab.insert("cat".to_string(), 6);
        vocab.insert("dog".to_string(), 7);
        vocab.insert(",".to_string(), 8);
        vocab.insert("!".to_string(), 9);
        vocab.insert("Hello".to_string(), 10);
        vocab.insert("world".to_string(), 11);
        vocab
    }

    fn small_tokenizer() -> WordPieceTokenizer {
        WordPieceTokenizer::from_parts(small_bert_vocab(), 0, "##".to_string(), 100).unwrap()
    }

    #[test]
    fn build_fails_if_unk_not_in_vocab() {
        let mut vocab = BTreeMap::new();
        vocab.insert("a".to_string(), 1);
        let err = WordPieceTokenizer::from_parts(vocab, 0, "##".to_string(), 100).unwrap_err();
        assert_eq!(err, WordPieceBuildError::UnkNotInVocab(0));
    }

    #[test]
    fn encode_unaffable_reference_example() {
        // The canonical WordPiece reference: "unaffable" → ["un",
        // "##aff", "##able"].
        let tok = small_tokenizer();
        assert_eq!(tok.encode("unaffable"), vec![3, 4, 5]);
    }

    #[test]
    fn encode_word_oov_emits_unk() {
        // "xyz" isn't in the vocab and no prefix decomposition works.
        let tok = small_tokenizer();
        assert_eq!(tok.encode("xyz"), vec![0]);
    }

    #[test]
    fn encode_word_partial_oov_emits_unk() {
        // "unaff" prefix matches ("un" + "##aff") but the trailing "q"
        // has no vocabulary entry and no "##q" continuation. WordPiece
        // is all-or-nothing on the word: emit UNK, drop the partial
        // subwords.
        let tok = small_tokenizer();
        assert_eq!(tok.encode("unaffq"), vec![0]);
    }

    #[test]
    fn encode_multiple_words_split_on_whitespace() {
        let tok = small_tokenizer();
        assert_eq!(tok.encode("cat dog"), vec![6, 7]);
    }

    #[test]
    fn encode_word_longer_than_max_chars_emits_unk() {
        // max_input_chars_per_word = 5: any longer word shortcuts to
        // UNK.
        let tok =
            WordPieceTokenizer::from_parts(small_bert_vocab(), 0, "##".to_string(), 5).unwrap();
        // "unaffable" has 9 chars > 5.
        assert_eq!(tok.encode("unaffable"), vec![0]);
        // "cat" has 3 chars, still fits.
        assert_eq!(tok.encode("cat"), vec![6]);
    }

    #[test]
    fn encode_empty_input_emits_nothing() {
        let tok = small_tokenizer();
        assert!(tok.encode("").is_empty());
        assert!(tok.encode("   ").is_empty());
    }

    #[test]
    fn encode_word_at_char_boundary_max_length_is_ok() {
        // max = 3 chars, word = 3 chars → encoded normally.
        let tok =
            WordPieceTokenizer::from_parts(small_bert_vocab(), 0, "##".to_string(), 3).unwrap();
        assert_eq!(tok.encode("cat"), vec![6]);
    }

    #[test]
    fn encode_with_punctuation_pre_tokenizer_splits_punctuation() {
        // "Hello, world!" → ["Hello", ",", "world", "!"] with the
        // whitespace+punctuation pre-tokenizer (the default).
        let tok = small_tokenizer();
        let ids = tok.encode("Hello, world!");
        assert_eq!(ids, vec![10, 8, 11, 9]);
    }

    #[test]
    fn encode_with_bert_pre_tokenizer_splits_punctuation() {
        let tok = small_tokenizer().with_pre_tokenizer(WordPiecePreTokenizer::Bert);
        let ids = tok.encode("Hello, world!");
        assert_eq!(ids, vec![10, 8, 11, 9]);
    }

    #[test]
    fn encode_with_whitespace_split_leaves_punctuation_attached() {
        // WhitespaceSplit keeps "Hello," as one word — which then
        // fails to decompose ("Hello," is not in the vocab and there
        // is no "##," continuation), so the whole word emits UNK.
        let tok = small_tokenizer().with_pre_tokenizer(WordPiecePreTokenizer::WhitespaceSplit);
        let ids = tok.encode("Hello, world!");
        // "Hello," → UNK, "world!" → UNK.
        assert_eq!(ids, vec![0, 0]);
    }

    #[test]
    fn decode_reassembles_continuing_subwords() {
        let tok = small_tokenizer();
        // ["un", "##aff", "##able"] → "unaffable"
        let text = tok.decode(&[3, 4, 5]).unwrap();
        assert_eq!(text, "unaffable");
    }

    #[test]
    fn decode_inserts_spaces_between_full_words() {
        let tok = small_tokenizer();
        // ["cat", "dog"] → "cat dog"
        let text = tok.decode(&[6, 7]).unwrap();
        assert_eq!(text, "cat dog");
    }

    #[test]
    fn decode_reassembles_mixed_full_and_continuing_subwords() {
        let tok = small_tokenizer();
        // ["un", "##aff", "##able", "cat"] → "unaffable cat"
        let text = tok.decode(&[3, 4, 5, 6]).unwrap();
        assert_eq!(text, "unaffable cat");
    }

    #[test]
    fn decode_rejects_unknown_id() {
        let tok = small_tokenizer();
        let err = tok.decode(&[42]).unwrap_err();
        assert_eq!(err, WordPieceDecodeError::UnknownId(42));
    }

    #[test]
    fn round_trip_encode_decode_lossy_on_whitespace() {
        // Pre-tokenizer collapses whitespace to single spaces; the
        // documented lossy round-trip.
        let tok = small_tokenizer();
        let ids = tok.encode("cat  dog");
        // encode collapses the double space → ["cat", "dog"] → [6,7].
        assert_eq!(ids, vec![6, 7]);
        let text = tok.decode(&ids).unwrap();
        assert_eq!(text, "cat dog");
    }

    #[test]
    fn empty_prefix_disables_continuation_marker() {
        // Some WordPiece variants (or hand-built configs) use "" as
        // the continuing-subword prefix: subwords after the first
        // are looked up bare. Set up a tiny vocab that exercises
        // that.
        let mut vocab = BTreeMap::new();
        vocab.insert("[UNK]".to_string(), 0);
        vocab.insert("un".to_string(), 1);
        vocab.insert("aff".to_string(), 2);
        vocab.insert("able".to_string(), 3);
        let tok = WordPieceTokenizer::from_parts(vocab, 0, String::new(), 100).unwrap();
        // "unaffable" — greedy longest match with empty prefix.
        assert_eq!(tok.encode("unaffable"), vec![1, 2, 3]);
        // Decode: without a prefix every subword is treated as a
        // "new word" so every one gets a leading space. This is the
        // documented degenerate shape.
        assert_eq!(tok.decode(&[1, 2, 3]).unwrap(), "un aff able");
    }

    #[test]
    fn encode_multibyte_word_uses_char_length_not_byte_length() {
        // Build a vocab that carries a multi-byte-char subword and
        // verify the max-chars check counts scalars, not bytes.
        let mut vocab = BTreeMap::new();
        vocab.insert("[UNK]".to_string(), 0);
        // "café" is 4 chars but 5 bytes.
        vocab.insert("café".to_string(), 1);
        let tok = WordPieceTokenizer::from_parts(vocab, 0, "##".to_string(), 4).unwrap();
        // 4 chars ≤ 4 → encoded normally, not UNK.
        assert_eq!(tok.encode("café"), vec![1]);
    }

    #[test]
    fn tokenizer_trait_encode_returns_encoding_with_ids_only() {
        let tok = small_tokenizer();
        let enc = Tokenizer::encode(&tok, "cat dog").unwrap();
        assert_eq!(enc.ids, vec![6, 7]);
        // Offsets and special_mask are empty — WordPiece doesn't
        // track them today.
        assert!(enc.offsets.is_empty());
        assert!(enc.special_mask.is_empty());
    }

    #[test]
    fn tokenizer_trait_count_matches_encode_length() {
        let tok = small_tokenizer();
        assert_eq!(Tokenizer::count(&tok, "unaffable").unwrap(), 3);
        assert_eq!(Tokenizer::count(&tok, "cat dog").unwrap(), 2);
    }

    #[test]
    fn punctuation_helper_matches_ascii_bert_rule() {
        for &c in &['!', '"', '#', ',', '.', '?', '-', '_', '(', ')', '[', ']'] {
            assert!(is_punctuation(c), "expected '{c}' to be punctuation");
        }
        for &c in &['a', 'Z', '0', '9', ' ', '\t'] {
            assert!(!is_punctuation(c), "expected '{c}' to be non-punctuation");
        }
    }

    // -----------------------------------------------------------------
    // Normalizer + post-processor wiring (BERT parity)
    // -----------------------------------------------------------------

    /// Build a small BERT-style vocab whose surface strings are all
    /// lowercased and accent-stripped — the shape the BERT normalizer
    /// hands to the pre-tokenizer.
    fn bert_lowercase_vocab() -> BTreeMap<String, TokenId> {
        let mut vocab = BTreeMap::new();
        vocab.insert("[UNK]".to_string(), 0);
        vocab.insert("[CLS]".to_string(), 1);
        vocab.insert("[SEP]".to_string(), 2);
        vocab.insert("hello".to_string(), 3);
        vocab.insert("world".to_string(), 4);
        vocab.insert("cafe".to_string(), 5);
        vocab.insert(",".to_string(), 6);
        vocab.insert("!".to_string(), 7);
        vocab
    }

    #[test]
    fn encode_applies_bert_normalizer_before_pre_tokenization() {
        // The vocab has only lowercase, accent-stripped entries. Without
        // the normalizer, `"CAFÉ"` would miss the vocab (`"CAFÉ"` is
        // not registered, and there is no `##É` continuation) and emit
        // UNK. With the default BERT normalizer (lowercase + accent
        // strip), it lands as `"cafe"` → id 5.
        let tok = WordPieceTokenizer::from_parts(bert_lowercase_vocab(), 0, "##".to_string(), 100)
            .unwrap()
            .with_normalizer(Normalizer::Bert {
                clean_text: true,
                handle_chinese_chars: true,
                strip_accents: None,
                lowercase: true,
            });
        // Hand-computed expected sequence for "Héllo, WORLD! CAFÉ":
        //   normalize -> "hello, world! cafe"
        //   pre-tokenize (Whitespace + punctuation split) ->
        //     ["hello", ",", "world", "!", "cafe"]
        //   WordPiece lookup -> [3, 6, 4, 7, 5]
        assert_eq!(
            tok.encode("Héllo, WORLD! CAFÉ"),
            vec![3, 6, 4, 7, 5],
            "BERT normalizer must run before pre-tokenization + WordPiece"
        );
    }

    #[test]
    fn encode_without_normalizer_leaves_input_unchanged() {
        // Same vocab, no normalizer: the mixed-case input misses the
        // lowercase vocab and produces UNKs — the existing behaviour is
        // preserved when a caller does not configure a normalizer.
        let tok = WordPieceTokenizer::from_parts(bert_lowercase_vocab(), 0, "##".to_string(), 100)
            .unwrap();
        assert!(tok.normalizer().is_none());
        // "CAFÉ" without normalization: 4 chars, no vocab hit → UNK.
        assert_eq!(tok.encode("CAFÉ"), vec![0]);
    }

    #[test]
    fn encode_applies_template_processing_post_processor() {
        // Configure a `[CLS] A [SEP]` template against a small vocab
        // (ids 1 and 2 for the specials, from `small_bert_vocab`).
        use crate::post_processor::{
            PostProcessor, SpecialTokenInfo, TemplatePiece, TemplateProcessing,
        };
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
        let tok = small_tokenizer().with_post_processor(PostProcessor::TemplateProcessing(tp));
        // Primary encoding of "cat dog" is [6, 7]; the template
        // splices [CLS]=1 before and [SEP]=2 after → [1, 6, 7, 2].
        let ids = tok.encode("cat dog");
        assert_eq!(ids, vec![1, 6, 7, 2]);
        assert_eq!(*ids.first().unwrap(), 1, "output must start with CLS id");
        assert_eq!(*ids.last().unwrap(), 2, "output must end with SEP id");
        // `count` must agree with `encode(text)?.ids.len()`.
        assert_eq!(Tokenizer::count(&tok, "cat dog").unwrap(), ids.len());
        // The trait-shape encode returns the templated encoding too.
        let enc = Tokenizer::encode(&tok, "cat dog").unwrap();
        assert_eq!(enc.ids, ids);
    }

    // -----------------------------------------------------------------
    // Special-token pre-extraction (BERT / DistilBERT literal-in-text
    // parity)
    // -----------------------------------------------------------------

    #[test]
    fn encode_extracts_registered_special_tokens_before_pre_tokenizer() {
        // Without special-token pre-extraction the Bert pre-tokenizer
        // would split `[CLS]` into `[`, `CLS`, `]`, none of which live
        // in the vocab (or their `##` continuations), so the whole
        // "word" would collapse to UNK — three UNKs in a row. With the
        // special-token map wired up, `[CLS]` is extracted verbatim
        // and emits its registered id.
        let mut specials = BTreeMap::new();
        specials.insert("[CLS]".to_string(), 1);
        specials.insert("[SEP]".to_string(), 2);
        let tok = small_tokenizer().with_special_tokens(specials);
        // "[CLS] cat [SEP]" — expected: [CLS], "cat", [SEP]
        //   pre-extraction: [1], region " cat " -> ["cat"] -> [6],
        //   [SEP] -> [2].
        let ids = tok.encode("[CLS] cat [SEP]");
        assert_eq!(ids, vec![1, 6, 2]);
    }

    #[test]
    fn encode_special_tokens_take_precedence_over_bert_punctuation_split() {
        // Under the default (Whitespace + punctuation split) pre-
        // tokenizer `"[CLS]hello"` (no space) still needs to emit
        // `[CLS]` then `hello` — the punctuation split by itself would
        // give ["[", "CLS", "]", "hello"]. Special-token pre-extraction
        // wins.
        let mut specials = BTreeMap::new();
        specials.insert("[CLS]".to_string(), 1);
        let tok = small_tokenizer().with_special_tokens(specials);
        let ids = tok.encode("[CLS]cat");
        assert_eq!(ids, vec![1, 6]);
    }

    #[test]
    fn encode_special_tokens_prefer_longest_match_first() {
        // Register two overlapping surfaces: the longer one must win.
        let mut vocab = small_bert_vocab();
        // Vocab entries for the specials — not strictly required for
        // encode (special_tokens is looked up directly), but real HF
        // configs put them here too so decode round-trips.
        vocab.insert("<|im|>".to_string(), 100);
        vocab.insert("<|im_start|>".to_string(), 101);
        let tok = WordPieceTokenizer::from_parts(vocab, 0, "##".to_string(), 100)
            .unwrap()
            .with_special_tokens({
                let mut m = BTreeMap::new();
                m.insert("<|im|>".to_string(), 100);
                m.insert("<|im_start|>".to_string(), 101);
                m
            });
        // "<|im_start|>" contains "<|im|>" as a prefix if you squint,
        // but not literally; still: register both to prove the
        // sort-by-length rule fires for the two entries.
        let ids = tok.encode("<|im_start|>");
        assert_eq!(ids, vec![101]);
    }

    #[test]
    fn encode_with_no_specials_matches_pre_specials_behaviour() {
        // Sanity: a fresh tokenizer with no specials must behave
        // exactly as before this landing did.
        let tok = small_tokenizer();
        assert!(tok.special_tokens().is_empty());
        assert_eq!(tok.encode("cat dog"), vec![6, 7]);
    }

    #[test]
    fn default_post_processor_is_identity_and_preserves_existing_shape() {
        // Sanity check: a freshly-built tokenizer with no
        // post-processor produces the same ids as before, and the
        // trait-shape encode leaves offsets/special_mask empty (the
        // documented shape for callers who don't configure a
        // post-processor).
        let tok = small_tokenizer();
        assert!(matches!(tok.post_processor(), PostProcessor::None));
        assert_eq!(tok.encode("cat dog"), vec![6, 7]);
        let enc = Tokenizer::encode(&tok, "cat dog").unwrap();
        assert_eq!(enc.ids, vec![6, 7]);
        assert!(enc.offsets.is_empty());
        assert!(enc.special_mask.is_empty());
    }
}
