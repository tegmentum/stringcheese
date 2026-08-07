//! The BPE core: merge table, vocabulary, and tokenizer types.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
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
/// Maps each merge pair `(left, right)` (both as owned byte strings) to
/// a numeric rank. Lower rank means the merge is applied *earlier* — that
/// is, when two adjacent pairs are both in the table, the one with the
/// lower rank wins. This matches the tiktoken / Hugging Face
/// convention.
#[derive(Debug, Default, Clone)]
pub struct BpeMergeTable {
    ranks: BTreeMap<(Vec<u8>, Vec<u8>), u32>,
}

impl BpeMergeTable {
    /// Constructs an empty merge table. A tokenizer with no merges is
    /// well-defined: it emits one token per input byte.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ranks: BTreeMap::new(),
        }
    }

    /// Inserts a merge with the given rank. Overwrites any prior entry
    /// for the same pair.
    pub fn insert(&mut self, left: Vec<u8>, right: Vec<u8>, rank: u32) {
        self.ranks.insert((left, right), rank);
    }

    /// Returns the rank of `(left, right)`, or `None` if the pair is
    /// not in the table.
    #[must_use]
    pub fn rank(&self, left: &[u8], right: &[u8]) -> Option<u32> {
        // BTreeMap requires an owned key for lookup; use a small local
        // helper to avoid allocation on every scan. Since `Vec<u8>: Borrow<[u8]>`
        // does not extend to tuples, we materialise once.
        let key = (left.to_vec(), right.to_vec());
        self.ranks.get(&key).copied()
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

/// A pre-tokenizer pattern.
///
/// Phase 1 of the BPE algorithm crate only supports a **literal string
/// separator** or **no pattern**. Full regex support — required to match
/// tiktoken's `cl100k_base` pre-tokenizer verbatim — arrives with Phase
/// 2b once the workspace commits to a specific regex backend. See
/// `docs/design/tokenizers.md` § 5.1 for the shape of that machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreTokenizerRegex {
    /// Split at every occurrence of a literal string. Adjacent separator
    /// matches collapse; leading and trailing matches are dropped;
    /// nothing empty is yielded.
    Literal(String),
}

impl PreTokenizerRegex {
    /// Constructs a literal-string pre-tokenizer.
    #[must_use]
    pub fn literal(s: impl Into<String>) -> Self {
        Self::Literal(s.into())
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
/// use stringcheese_tokenizer_bpe::{BpeMergeTable, BpeTokenizer, BpeVocabulary};
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
    pre_tokenizer_pattern: Option<PreTokenizerRegex>,
}

impl BpeTokenizer {
    /// Builds a tokenizer from a merge table and vocabulary.
    #[must_use]
    pub fn from_parts(merges: BpeMergeTable, vocab: BpeVocabulary) -> Self {
        Self {
            merges,
            vocab,
            special_tokens: BTreeMap::new(),
            pre_tokenizer_pattern: None,
        }
    }

    /// Attaches (or replaces) the special-token map.
    ///
    /// Special tokens are matched *literally* in the input (longest
    /// match first) and emitted as their pre-assigned ids without
    /// participating in the BPE merge loop.
    #[must_use]
    pub fn with_special_tokens(mut self, tokens: BTreeMap<String, TokenId>) -> Self {
        self.special_tokens = tokens;
        self
    }

    /// Attaches (or replaces) the pre-tokenizer pattern.
    #[must_use]
    pub fn with_pre_tokenizer(mut self, pattern: PreTokenizerRegex) -> Self {
        self.pre_tokenizer_pattern = Some(pattern);
        self
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

    // ---- internal encoding pipeline ----

    /// Emit `(id, byte_range_in_input)` pairs for `text`, walking the
    /// full encoding pipeline. This is the shared entry point for both
    /// `encode` (which tracks offsets and special-mask) and `count`
    /// (which discards everything but the length).
    fn encode_pieces(
        &self,
        text: &str,
    ) -> Result<Vec<(TokenId, Range<usize>, bool)>, TokenizerError> {
        let mut out = Vec::new();
        // Walk the input, extracting special-token literal matches. The
        // regions between matches are handed to the BPE loop.
        let specials_sorted = self.sorted_specials();
        let bytes = text.as_bytes();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            // Try to match a special token at `cursor`.
            let mut matched = None;
            for (surface, id) in &specials_sorted {
                let sb = surface.as_bytes();
                if bytes[cursor..].starts_with(sb) {
                    matched = Some((surface.clone(), *id, sb.len()));
                    break;
                }
            }
            if let Some((_surface, id, len)) = matched {
                // Emit any accumulated pre-cursor region first: this
                // shouldn't happen because we always flush before we
                // reach the special-match position. (Kept as a defence.)
                out.push((id, cursor..cursor + len, true));
                cursor += len;
                continue;
            }
            // No special match here: consume up to the next special
            // occurrence (or end-of-input) and BPE-encode that region.
            let mut next_special = bytes.len();
            for (surface, _id) in &specials_sorted {
                let sb = surface.as_bytes();
                if let Some(rel) = find_subslice(&bytes[cursor..], sb) {
                    let abs = cursor + rel;
                    if abs < next_special {
                        next_special = abs;
                    }
                }
            }
            let region = &text[cursor..next_special];
            self.encode_region_bpe(region, cursor, &mut out)?;
            cursor = next_special;
        }
        Ok(out)
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

    /// BPE-encode a substring of the input; `region_offset` is the byte
    /// offset of `region` within the original input, used to compute
    /// output offsets.
    fn encode_region_bpe(
        &self,
        region: &str,
        region_offset: usize,
        out: &mut Vec<(TokenId, Range<usize>, bool)>,
    ) -> Result<(), TokenizerError> {
        if region.is_empty() {
            return Ok(());
        }

        // Pre-tokenize into "words" (or one word for the whole region).
        let words = pre_tokenize(region, self.pre_tokenizer_pattern.as_ref());
        for word in words {
            let word_start_in_region = word.offset;
            let word_bytes = word.text.as_bytes();

            // Seed pieces: one per byte.
            let mut pieces: Vec<PieceRef> = Vec::with_capacity(word_bytes.len());
            for (i, &b) in word_bytes.iter().enumerate() {
                pieces.push(PieceRef {
                    bytes: alloc::vec![b],
                    start: i,
                    len: 1,
                });
            }

            self.merge_loop(&mut pieces);

            for p in pieces {
                let Some(id) = self.vocab.id(&p.bytes) else {
                    return Err(TokenizerError::UnknownToken(format_bytes_literal(&p.bytes)));
                };
                let abs_start = region_offset + word_start_in_region + p.start;
                let abs_end = abs_start + p.len;
                out.push((id, abs_start..abs_end, false));
            }
        }
        Ok(())
    }

    /// Iteratively merge the pair with the lowest rank until no
    /// mergeable pair remains. `O(n²)`; see the crate docs for the
    /// planned linked-list-plus-heap optimisation.
    fn merge_loop(&self, pieces: &mut Vec<PieceRef>) {
        loop {
            if pieces.len() < 2 {
                return;
            }
            // Find the adjacent pair with the lowest rank in the merge table.
            let mut best_idx: Option<usize> = None;
            let mut best_rank: u32 = u32::MAX;
            for i in 0..pieces.len() - 1 {
                if let Some(r) = self.merges.rank(&pieces[i].bytes, &pieces[i + 1].bytes) {
                    if r < best_rank {
                        best_rank = r;
                        best_idx = Some(i);
                    }
                }
            }
            let Some(i) = best_idx else {
                return;
            };
            // Merge pieces[i] and pieces[i+1].
            let mut merged_bytes = pieces[i].bytes.clone();
            merged_bytes.extend_from_slice(&pieces[i + 1].bytes);
            let merged = PieceRef {
                bytes: merged_bytes,
                start: pieces[i].start,
                len: pieces[i].len + pieces[i + 1].len,
            };
            pieces[i] = merged;
            pieces.remove(i + 1);
        }
    }
}

impl Tokenizer for BpeTokenizer {
    type Token = TokenId;

    fn encode(&self, text: &str) -> Result<Encoding<Self::Token>, TokenizerError> {
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
        Ok(enc)
    }

    fn decode(&self, tokens: &[Self::Token]) -> Result<String, TokenizerError> {
        let mut buf: Vec<u8> = Vec::new();
        for &id in tokens {
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
        String::from_utf8(buf).map_err(|_| TokenizerError::InvalidUtf8)
    }

    fn count(&self, text: &str) -> Result<usize, TokenizerError> {
        Ok(self.encode_pieces(text)?.len())
    }
}

// ---- internal helpers ----

/// A single merge-loop piece: its current byte string, and the byte
/// range within its enclosing word.
#[derive(Debug, Clone)]
struct PieceRef {
    bytes: Vec<u8>,
    start: usize,
    len: usize,
}

/// Text plus (byte) offset within its enclosing region.
struct Word<'a> {
    offset: usize,
    text: &'a str,
}

fn pre_tokenize<'a>(text: &'a str, pattern: Option<&PreTokenizerRegex>) -> Vec<Word<'a>> {
    let mut out = Vec::new();
    match pattern {
        Some(PreTokenizerRegex::Literal(sep)) if !sep.is_empty() => {
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
                        text: &text[start..cursor],
                    });
                }
            }
        }
        _ => {
            // No pattern (or empty separator): fall back to whitespace
            // splitting. This matches the design doc's "fall through to
            // whitespace" behaviour.
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
                        text: &text[start..cursor],
                    });
                }
            }
            // If the text contained NO non-whitespace at all we still
            // emit nothing; if the text was pure non-whitespace we
            // emit one word.
            if out.is_empty() && !text.is_empty() && text.chars().any(|c| !c.is_whitespace()) {
                out.push(Word { offset: 0, text });
            }
        }
    }
    // Fallback: if no pattern *and* whitespace splitting yielded nothing
    // and the input is entirely non-whitespace, we've already handled
    // that above. If the input is entirely whitespace, `out` stays
    // empty and we emit nothing — which is what tiktoken does too.
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
}
