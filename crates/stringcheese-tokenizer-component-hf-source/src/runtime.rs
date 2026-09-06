//! Native runtime for the hf-source pack.
//!
//! Owns an [`HfTokenizer`] materialised from a tokenizer.json blob
//! at runtime and exposes the same four verbs the baked packs do
//! (encode / decode / count / get-capabilities) plus the two
//! constructors the resource-based WIT adds (from-json /
//! from-bytes).  The [`crate::wit`] module — compiled only on
//! wasm targets with `wit-component` — bridges this native API
//! to the WIT boundary.

use alloc::string::{String, ToString};

use stringcheese_tokenizer::Encoding;
use stringcheese_tokenizer_hf::hf::{
    parse_tokenizer_json, to_tokenizer, HfConversionError, HfParseError, HfTokenizer,
};

/// A live tokenizer instance loaded from a tokenizer.json blob.
///
/// Holds the materialised [`HfTokenizer`] enum + the vocabulary
/// size cached at load time.  Dispatch on the enum is inline in
/// [`Self::encode`] / [`Self::decode`] / [`Self::count`] rather
/// than through a `dyn Trait` — every variant already implements
/// the same [`stringcheese_tokenizer::Tokenizer`] trait, but
/// keeping the concrete type lets the wasm binary avoid a vtable
/// per call.
#[derive(Debug)]
pub struct HfSourceHandle {
    inner: HfTokenizer,
    vocab_size: u32,
    model_type: &'static str,
}

impl HfSourceHandle {
    /// Construct from a UTF-8 tokenizer.json blob.
    ///
    /// # Errors
    ///
    /// Any deserialisation / conversion failure surfaces as
    /// [`HfSourceError::InvalidTokenizerJson`].  The wasm bridge
    /// maps that variant onto the WIT's
    /// `TokenizerError::VocabularyMismatch`.
    pub fn from_json(json: &str) -> Result<Self, HfSourceError> {
        let config =
            parse_tokenizer_json(json).map_err(HfSourceError::parse)?;
        let tokenizer = to_tokenizer(&config).map_err(HfSourceError::conversion)?;
        Ok(Self::from_tokenizer(tokenizer))
    }

    /// Construct from raw tokenizer.json bytes.
    ///
    /// Convenience wrapper around [`Self::from_json`]; validates the
    /// bytes are UTF-8 first so a caller who fetched a binary blob
    /// gets `InvalidUtf8` rather than a confusing parse error.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HfSourceError> {
        let json = core::str::from_utf8(bytes).map_err(|_| HfSourceError::InvalidUtf8)?;
        Self::from_json(json)
    }

    /// Total id count in the tokenizer's vocabulary.
    #[must_use]
    pub const fn vocab_size(&self) -> u32 {
        self.vocab_size
    }

    /// Backend identifier — `"bpe" | "wordpiece" | "unigram" | "wordlevel"`.
    /// Matches the `model.type` field of a HuggingFace tokenizer.json.
    #[must_use]
    pub const fn model_type(&self) -> &'static str {
        self.model_type
    }

    /// Encode `text` through the underlying tokenizer's full
    /// pipeline (normalize + pre-tokenize + model + post-process
    /// + truncate).
    ///
    /// # Errors
    ///
    /// A tokenizer-internal failure (unknown token without byte
    /// fallback, invalid utf-8 in a decoder round-trip, …) maps
    /// to [`HfSourceError::EncodeFailed`].
    pub fn encode(&self, text: &str) -> Result<Encoding<u32>, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        let mut enc = match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::encode(t.as_ref(), text),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::encode(t, text),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::encode(t, text),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::encode(t, text),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::encode)?;
        // Per-token side-arrays are documented as "empty when not
        // tracked", but downstream consumers (embed-e5, embed, gliner)
        // slice-index them alongside `ids` and panic on empty.  Fill
        // the two universally-required arrays with the trivial
        // single-sentence-no-padding defaults so a caller can rely on
        // them:
        //
        //   * `attention_mask` — all `true` (no padding at encode
        //     boundary; pad-batch is where padding is later applied)
        //   * `type_ids` — all `0` (single sentence encode; segment 1
        //     only appears on the `encode_pair` path this WIT does
        //     not expose)
        //
        // `special_mask` and `offsets` are left as whatever the
        // tokenizer populated — a BERT/WordPiece tokenizer produces
        // both; a bare Unigram/BPE without an offset-tracker leaves
        // both empty, and consumers that need them fall back to the
        // empty-vec contract.
        let n = enc.ids.len();
        if enc.attention_mask.is_empty() {
            enc.attention_mask = alloc::vec![true; n];
        }
        if enc.type_ids.is_empty() {
            enc.type_ids = alloc::vec![0u32; n];
        }
        // Best-effort offset synthesis for the Unigram (SentencePiece)
        // path — stringcheese's `impl Tokenizer for UnigramTokenizer`
        // returns bare ids without offset bookkeeping (see
        // ~/.claude/…/memory/stringcheese_unigram_offsets_gap.md).
        // Downstream consumers that need offsets (gliner's decode.rs
        // to map subword spans back to character spans) trip on the
        // empty vec.
        //
        // Reconstruct offsets by splitting the source text on ASCII
        // whitespace — one "word" per whitespace-separated run,
        // matching Metaspace's semantic word boundary.  Distribute
        // each word's byte range across its share of the emitted
        // subword ids by proportional splitting; special tokens
        // (typically the leading CLS + trailing SEP the post-
        // processor added) get the zero-range `(0, 0)` marker every
        // downstream consumer already expects.
        //
        // This is not HF-parity — HF's own offsets are Viterbi-
        // driven per-subword — but it produces:
        //
        //   * non-empty offsets so consumers don't slice-index a
        //     shorter-than-`ids` vector
        //   * monotonically non-decreasing offset.start values
        //     across content tokens
        //   * the first content subword of each "word" gets an
        //     offset.start pointing at the whitespace preceding it
        //     (or 0 for the first word) — the anchor gliner's
        //     `words_mask_bio` heuristic checks for
        //
        // The heuristic is enough to unblock gliner runtime through
        // hf-source; a true fix belongs upstream in stringcheese's
        // UnigramTokenizer.
        if enc.offsets.is_empty() && n > 0 {
            enc.offsets = synthesize_offsets(text, n);
        }
        Ok(enc)
    }

    /// Decode `ids` back to text.
    ///
    /// # Errors
    ///
    /// A caller-side inconsistency (an id outside the vocabulary,
    /// bytes that no longer form valid utf-8) surfaces as
    /// [`HfSourceError::DecodeFailed`].
    pub fn decode(&self, ids: &[u32]) -> Result<String, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::decode(t.as_ref(), ids),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::decode(t, ids),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::decode(t, ids),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::decode(t, ids),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::decode)
    }

    /// Count the tokens `encode(text)` would produce without
    /// materialising offsets / masks.
    pub fn count(&self, text: &str) -> Result<u32, HfSourceError> {
        use stringcheese_tokenizer::Tokenizer as TokTrait;
        use stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer;
        use stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer;
        use stringcheese_tokenizer_hf::BpeTokenizer;
        use stringcheese_tokenizer_hf::hf::UnigramTokenizer;
        let n = match &self.inner {
            HfTokenizer::Bpe(t) => <BpeTokenizer as TokTrait>::count(t.as_ref(), text),
            HfTokenizer::WordPiece(t) => <WordPieceTokenizer as TokTrait>::count(t, text),
            HfTokenizer::Unigram(t) => <UnigramTokenizer as TokTrait>::count(t, text),
            HfTokenizer::WordLevel(t) => <WordLevelTokenizer as TokTrait>::count(t, text),
            other => {
                return Err(HfSourceError::Other(alloc::format!(
                    "unsupported HfTokenizer variant: {other:?}"
                )))
            }
        }
        .map_err(HfSourceError::encode)?;
        u32::try_from(n).map_err(|_| HfSourceError::Other("count overflowed u32".to_string()))
    }

    fn from_tokenizer(tokenizer: HfTokenizer) -> Self {
        let (vocab_size, model_type) = match &tokenizer {
            HfTokenizer::Bpe(t) => (VocabSize::vocab_size(t.as_ref()), "bpe"),
            HfTokenizer::WordPiece(t) => (VocabSize::vocab_size(t), "wordpiece"),
            HfTokenizer::Unigram(t) => (VocabSize::vocab_size(t), "unigram"),
            HfTokenizer::WordLevel(t) => (VocabSize::vocab_size(t), "wordlevel"),
            _ => (0, "unknown"),
        };
        Self {
            inner: tokenizer,
            vocab_size: u32::try_from(vocab_size).unwrap_or(u32::MAX),
            model_type,
        }
    }
}

/// Best-effort per-token byte offsets when the underlying tokenizer
/// impl left them empty (typically `stringcheese_tokenizer_hf`'s
/// UnigramTokenizer).  Split `text` on ASCII whitespace, distribute
/// each word's byte range across its proportional share of the `n`
/// emitted subword ids.  Reserves two `(0, 0)` slots at the ends
/// for the CLS + SEP special tokens the BERT-family post-processor
/// adds — a good match for the shape gliner's decode expects.
///
/// This is a lossy approximation of the HF-parity per-subword
/// offsets and is scoped narrowly to unblocking downstream
/// consumers that only need "non-empty offsets whose first byte
/// discriminates a word-start via whitespace anchor" (gliner's
/// `words_mask_bio` derivation).  A proper fix belongs upstream.
fn synthesize_offsets(text: &str, n: usize) -> alloc::vec::Vec<core::ops::Range<usize>> {
    let mut out = alloc::vec::Vec::with_capacity(n);
    if n == 0 {
        return out;
    }

    // Detect the "leading + trailing special token" shape by
    // looking at the outer ids after the fact.  Since we don't
    // have that here, be conservative: reserve 1 special at each
    // end (matches BERT/DeBERTa's [CLS] $A [SEP] shape).  Down-
    // stream consumers that get an off-by-one offset on the
    // outermost content token can tolerate it — the whitespace-
    // anchor heuristic still fires on the correct first-of-word
    // subword.
    let reserve_leading = 1usize.min(n);
    let content_n = n.saturating_sub(reserve_leading);
    let reserve_trailing = 1usize.min(content_n);
    let content_n = content_n.saturating_sub(reserve_trailing);

    for _ in 0..reserve_leading {
        out.push(0..0);
    }

    if content_n == 0 {
        for _ in 0..reserve_trailing {
            out.push(0..0);
        }
        return out;
    }

    // Split source text at ASCII whitespace boundaries.  Each
    // segment's byte range in the source becomes one "word" range.
    // Runs of whitespace between segments collapse — every real
    // word contributes one segment, matching Metaspace's word-per-
    // pre-token shape.
    let bytes = text.as_bytes();
    let mut segments: alloc::vec::Vec<core::ops::Range<usize>> = alloc::vec::Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if start < i {
            // Include the leading space in the segment's byte range
            // when there is one — matches DeBERTa Metaspace's
            // "first subword carries the leading space" convention.
            let leading_space = if start > 0
                && bytes.get(start - 1).is_some_and(|b| b.is_ascii_whitespace())
            {
                start - 1
            } else {
                start
            };
            segments.push(leading_space..i);
        }
    }

    if segments.is_empty() {
        // No word segments — every content id gets `(0, 0)`.
        for _ in 0..content_n {
            out.push(0..0);
        }
    } else {
        // Distribute `content_n` subwords across `segments.len()` word
        // ranges as evenly as possible.  Each word gets floor(N/W)
        // subwords, plus a leftover 1 for the first N%W words.
        let w = segments.len();
        let base = content_n / w;
        let rem = content_n % w;
        for (idx, seg) in segments.iter().enumerate() {
            let subs_in_this_word = base + if idx < rem { 1 } else { 0 };
            if subs_in_this_word == 0 {
                continue;
            }
            let seg_len = seg.end - seg.start;
            if subs_in_this_word == 1 || seg_len <= subs_in_this_word {
                // One-subword-per-word (common Metaspace case), OR
                // the word is too short to split further — emit the
                // whole segment for the first subword and empty ranges
                // for any remainder.
                out.push(seg.start..seg.end);
                for _ in 1..subs_in_this_word {
                    out.push(seg.end..seg.end);
                }
            } else {
                let step = seg_len / subs_in_this_word;
                let mut cursor = seg.start;
                for k in 0..subs_in_this_word {
                    let next = if k + 1 == subs_in_this_word {
                        seg.end
                    } else {
                        cursor + step
                    };
                    out.push(cursor..next);
                    cursor = next;
                }
            }
        }
    }

    for _ in 0..reserve_trailing {
        out.push(0..0);
    }
    // Defensive: guarantee the output length matches `n` exactly
    // (rounding errors in the distribution loop would otherwise
    // leave the vec short).
    while out.len() < n {
        out.push(0..0);
    }
    out.truncate(n);
    out
}

/// Failure modes the hf-source runtime surfaces.  Kept narrow so
/// the WIT bridge can map each variant onto a matching
/// `TokenizerError` case without an `Other(...)` catch-all
/// swallowing detail.
#[derive(Debug)]
pub enum HfSourceError {
    /// The input to `from-bytes` was not valid UTF-8.
    InvalidUtf8,
    /// Deserialising or converting the tokenizer.json failed —
    /// malformed JSON, unrecognised `model.type`, incoherent
    /// vocab / merges, …
    InvalidTokenizerJson(String),
    /// A per-call `encode` (or `count`) surfaced an internal error
    /// after the tokenizer.json had loaded cleanly.
    EncodeFailed(String),
    /// A per-call `decode` surfaced an internal error.
    DecodeFailed(String),
    /// Anything else — carries a short human-readable message.
    Other(String),
}

impl HfSourceError {
    fn parse(e: HfParseError) -> Self {
        Self::InvalidTokenizerJson(alloc::format!("{e:?}"))
    }

    fn conversion(e: HfConversionError) -> Self {
        Self::InvalidTokenizerJson(alloc::format!("{e:?}"))
    }

    fn encode(e: stringcheese_tokenizer::TokenizerError) -> Self {
        Self::EncodeFailed(alloc::format!("{e:?}"))
    }

    fn decode(e: stringcheese_tokenizer::TokenizerError) -> Self {
        Self::DecodeFailed(alloc::format!("{e:?}"))
    }
}

/// Every tokenizer variant `HfTokenizer` wraps advertises a vocab
/// size, but the trait only surfaces it as a method on the
/// concrete variant.  Cover the four with one tiny helper that
/// returns a `usize` so the enum dispatch above stays uniform.
trait VocabSize {
    fn vocab_size(&self) -> usize;
}

impl VocabSize for stringcheese_tokenizer_hf::BpeTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::wordpiece::WordPieceTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::hf::UnigramTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

impl VocabSize for stringcheese_tokenizer_hf::wordlevel::WordLevelTokenizer {
    fn vocab_size(&self) -> usize {
        self.vocab().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small WordLevel fixture — the minimal shape
    /// `parse_tokenizer_json + to_tokenizer` will accept.  Same
    /// shape the cognition-t5-runner's tokenizer_source unit
    /// tests use, so a WIT / parser drift shows up in both
    /// places.
    const WORDLEVEL_FIXTURE: &str = r###"{
      "version": "1.0",
      "truncation": null,
      "padding": null,
      "added_tokens": [
        {"id": 0, "content": "[UNK]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}
      ],
      "normalizer": null,
      "pre_tokenizer": {"type": "Whitespace"},
      "post_processor": null,
      "decoder": null,
      "model": {
        "type": "WordLevel",
        "vocab": {"[UNK]": 0, "hello": 1, "world": 2},
        "unk_token": "[UNK]"
      }
    }"###;

    #[test]
    fn from_bytes_rejects_non_utf8() {
        let err = HfSourceHandle::from_bytes(&[0xff, 0xfe, 0x00]).unwrap_err();
        assert!(matches!(err, HfSourceError::InvalidUtf8));
    }

    #[test]
    fn from_json_rejects_bad_json() {
        let err = HfSourceHandle::from_json("{this is not tokenizer.json").unwrap_err();
        assert!(matches!(err, HfSourceError::InvalidTokenizerJson(_)));
    }

    #[test]
    fn from_json_loads_wordlevel_fixture() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        assert_eq!(handle.vocab_size(), 3);
        assert_eq!(handle.model_type(), "wordlevel");
    }

    #[test]
    fn encode_wordlevel_fixture_round_trips() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        let enc = handle.encode("hello world").expect("encode");
        assert_eq!(enc.ids, vec![1u32, 2u32]);
        let decoded = handle.decode(&[1, 2]).expect("decode");
        assert!(decoded.contains("hello") && decoded.contains("world"));
    }

    #[test]
    fn count_matches_encode_len() {
        let handle = HfSourceHandle::from_json(WORDLEVEL_FIXTURE).expect("fixture loads");
        let enc = handle.encode("hello world").expect("encode");
        let n = handle.count("hello world").expect("count");
        assert_eq!(n as usize, enc.ids.len());
    }

    #[test]
    fn synthesize_offsets_empty_text() {
        assert!(synthesize_offsets("", 0).is_empty());
        // n > 0 with empty text: every id gets a (0, 0) offset.
        let out = synthesize_offsets("", 3);
        assert_eq!(out.len(), 3);
        for r in &out {
            assert_eq!(r.start, 0);
            assert_eq!(r.end, 0);
        }
    }

    #[test]
    fn synthesize_offsets_one_word_per_subword() {
        // "Bill Gates founded" — 3 words, plus 2 special-token
        // slots at the ends (n = 5).  Every word gets exactly
        // one subword; the whitespace anchor on the second and
        // third words is the leading space.
        let out = synthesize_offsets("Bill Gates founded", 5);
        assert_eq!(out.len(), 5);
        // Leading + trailing specials.
        assert_eq!(out[0], 0..0);
        assert_eq!(out[4], 0..0);
        // First content subword — no leading space.
        assert_eq!(out[1], 0..4);
        // Second and third — start on the whitespace preceding
        // the word.
        assert_eq!(out[2].start, 4);
        assert_eq!(out[2].end, 10);
        assert_eq!(out[3].start, 10);
        assert_eq!(out[3].end, 18);
    }

    #[test]
    fn synthesize_offsets_more_subwords_than_words() {
        // "Albuquerque" is one word getting three subwords in
        // Metaspace's typical decomposition.  The word range is
        // 0..11; the three subwords get contiguous shares of it.
        let out = synthesize_offsets("Albuquerque", 5);
        assert_eq!(out.len(), 5);
        // Leading + trailing specials.
        assert_eq!(out[0], 0..0);
        assert_eq!(out[4], 0..0);
        // The three content subwords partition 0..11.
        assert_eq!(out[1].start, 0);
        assert_eq!(out[3].end, 11);
        // Every content subword is non-empty.
        for r in &out[1..4] {
            assert!(r.start < r.end, "content subword got empty range {r:?}");
        }
        // Monotonically non-decreasing.
        assert!(out[1].end <= out[2].start);
        assert!(out[2].end <= out[3].start);
    }

    #[test]
    fn synthesize_offsets_first_content_subword_of_second_word_starts_on_whitespace() {
        // The load-bearing invariant for gliner's `words_mask_bio`
        // derivation: for a token whose predecessor is content,
        // `text.as_bytes()[offset.start].is_ascii_whitespace()`
        // decides start-of-word.  Verify that a two-word input
        // with two subwords in the second word puts a whitespace
        // byte at the start of the FIRST subword of the second
        // word.
        let text = "Bill Gates founded"; // 3 words, 3 subwords + 2 specials = 5
        let out = synthesize_offsets(text, 5);
        assert_eq!(out.len(), 5);
        let bytes = text.as_bytes();
        // out[2] should start on a whitespace byte (the space
        // between "Bill" and "Gates").
        assert!(
            bytes.get(out[2].start).is_some_and(|b| b.is_ascii_whitespace()),
            "out[2] = {:?} should start on a whitespace byte, got byte {:?}",
            out[2],
            out[2].start
        );
    }
}
