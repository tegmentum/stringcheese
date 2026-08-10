//! Post-encode, batch-scoped padding.
//!
//! Padding runs *after* a tokenizer's [`encode`](crate::Tokenizer::encode)
//! call produces an [`Encoding`] (or a batch of them). Batch-serving
//! code typically composes [`pad_batch`] with
//! [`crate::truncation::truncate`] — truncate over-long inputs down to
//! `max_length` and then pad every short input up to the batch's max
//! length so the downstream tensor library gets a uniform matrix.
//!
//! # Strategies
//!
//! * [`PaddingStrategy::BatchLongest`] — pad every encoding in a batch
//!   to the length of the longest encoding in that batch. This is the
//!   HF default and the common serving-side shape.
//! * [`PaddingStrategy::Fixed`] — pad every encoding to the given
//!   length. Encodings already at or above that length are left alone.
//!
//! # Direction
//!
//! [`PaddingDirection::Right`] (the HF default) appends pad tokens on
//! the right. [`PaddingDirection::Left`] prepends them — the shape
//! LLM causal-attention pipelines reach for.
//!
//! # `attention_mask` and per-token arrays
//!
//! [`pad_batch`] populates [`Encoding::attention_mask`] on every
//! padded encoding (real tokens `true`, pad tokens `false`) and
//! grows every already-populated per-token array in lockstep with
//! `ids`:
//!
//! * `offsets` — pad slots receive an empty range `0..0`.
//! * `special_mask` — pad slots receive `false` (they are not
//!   registered specials in the tokenizer's sense).
//! * `type_ids` — pad slots receive [`PaddingConfig::pad_type_id`].
//!
//! An encoding that arrives with an empty per-token array (e.g. a
//! `WordPiece` encoding without offsets) leaves that array empty; the
//! padding function does not synthesise arrays that were opted out
//! upstream — the exception is `attention_mask`, which is populated
//! for every encoding because it is the load-bearing signal for
//! downstream tensor code.

use alloc::vec::Vec;

use crate::traits::Encoding;

/// Which side of the encoding to append pad tokens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaddingDirection {
    /// Append pad tokens on the right (HF default).
    #[default]
    Right,
    /// Prepend pad tokens on the left.
    Left,
}

/// How to choose the target length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaddingStrategy {
    /// Pad every encoding in the batch to the length of the longest
    /// encoding in the same batch. HF's default.
    #[default]
    BatchLongest,
    /// Pad every encoding to the given fixed length. Encodings
    /// already at or above the target are left alone.
    Fixed(usize),
}

/// Configuration for a padding invocation.
///
/// The generic `Token` parameter is the tokenizer's token type — the
/// `pad_id` must be storable in the same type. Field shape mirrors HF's
/// `PaddingParams` on-disk shape so the HF loader can splice values
/// straight in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaddingConfig<Token> {
    /// How to compute the target length.
    pub strategy: PaddingStrategy,
    /// The pad token id — appended (or prepended) to reach the target
    /// length. Must be valid for the tokenizer whose encoding this
    /// pads; the padding function does not validate the id against a
    /// vocabulary.
    pub pad_id: Token,
    /// The pad token's `type_id` — used to fill new slots on
    /// [`Encoding::type_ids`] when that array is populated.
    pub pad_type_id: u32,
    /// End of the encoding to append pad tokens to.
    pub direction: PaddingDirection,
}

impl<Token: Default> Default for PaddingConfig<Token> {
    fn default() -> Self {
        Self {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: Token::default(),
            pad_type_id: 0,
            direction: PaddingDirection::Right,
        }
    }
}

/// Pad every encoding in a batch to a common target length.
///
/// Empty batches are a no-op. Every populated per-token array grows
/// in lockstep with `ids`; [`Encoding::attention_mask`] is populated
/// for every encoding (real tokens `true`, pad tokens `false`) even
/// when it started empty.
pub fn pad_batch<Token: Clone>(encodings: &mut [Encoding<Token>], config: &PaddingConfig<Token>) {
    if encodings.is_empty() {
        return;
    }
    let target = match config.strategy {
        PaddingStrategy::BatchLongest => encodings.iter().map(|e| e.ids.len()).max().unwrap_or(0),
        PaddingStrategy::Fixed(n) => n,
    };
    for enc in encodings.iter_mut() {
        pad(enc, target, config);
    }
}

/// Pad a single encoding to the given target length.
///
/// Called by [`pad_batch`]; exposed as a top-level function so callers
/// who want to pad one encoding at a time (single-input serving path)
/// do not have to wrap it in a slice.
pub fn pad<Token: Clone>(
    encoding: &mut Encoding<Token>,
    target_len: usize,
    config: &PaddingConfig<Token>,
) {
    // Ensure attention_mask is populated for the real tokens even
    // when the encoding arrived without one — downstream tensor code
    // requires this array either way.
    if encoding.attention_mask.is_empty() && !encoding.ids.is_empty() {
        encoding.attention_mask.resize(encoding.ids.len(), true);
    }
    if encoding.ids.len() >= target_len {
        return;
    }
    let missing = target_len - encoding.ids.len();
    let has_offsets = !encoding.offsets.is_empty();
    let has_mask = !encoding.special_mask.is_empty();
    let has_type_ids = !encoding.type_ids.is_empty();
    // attention_mask is always populated after the resize above.
    match config.direction {
        PaddingDirection::Right => {
            append_pad(
                encoding,
                missing,
                config,
                has_offsets,
                has_mask,
                has_type_ids,
            );
        }
        PaddingDirection::Left => {
            prepend_pad(
                encoding,
                missing,
                config,
                has_offsets,
                has_mask,
                has_type_ids,
            );
        }
    }
}

fn append_pad<Token: Clone>(
    encoding: &mut Encoding<Token>,
    missing: usize,
    config: &PaddingConfig<Token>,
    has_offsets: bool,
    has_mask: bool,
    has_type_ids: bool,
) {
    encoding.ids.reserve(missing);
    encoding.attention_mask.reserve(missing);
    if has_offsets {
        encoding.offsets.reserve(missing);
    }
    if has_mask {
        encoding.special_mask.reserve(missing);
    }
    if has_type_ids {
        encoding.type_ids.reserve(missing);
    }
    for _ in 0..missing {
        encoding.ids.push(config.pad_id.clone());
        encoding.attention_mask.push(false);
        if has_offsets {
            encoding.offsets.push(0..0);
        }
        if has_mask {
            encoding.special_mask.push(false);
        }
        if has_type_ids {
            encoding.type_ids.push(config.pad_type_id);
        }
    }
}

fn prepend_pad<Token: Clone>(
    encoding: &mut Encoding<Token>,
    missing: usize,
    config: &PaddingConfig<Token>,
    has_offsets: bool,
    has_mask: bool,
    has_type_ids: bool,
) {
    let mut new_ids: Vec<Token> = Vec::with_capacity(missing + encoding.ids.len());
    let mut new_attn: Vec<bool> = Vec::with_capacity(missing + encoding.attention_mask.len());
    let mut new_offsets = if has_offsets {
        Vec::with_capacity(missing + encoding.offsets.len())
    } else {
        Vec::new()
    };
    let mut new_mask = if has_mask {
        Vec::with_capacity(missing + encoding.special_mask.len())
    } else {
        Vec::new()
    };
    let mut new_type_ids = if has_type_ids {
        Vec::with_capacity(missing + encoding.type_ids.len())
    } else {
        Vec::new()
    };
    for _ in 0..missing {
        new_ids.push(config.pad_id.clone());
        new_attn.push(false);
        if has_offsets {
            new_offsets.push(0..0);
        }
        if has_mask {
            new_mask.push(false);
        }
        if has_type_ids {
            new_type_ids.push(config.pad_type_id);
        }
    }
    new_ids.extend(core::mem::take(&mut encoding.ids));
    new_attn.extend(core::mem::take(&mut encoding.attention_mask));
    if has_offsets {
        new_offsets.extend(core::mem::take(&mut encoding.offsets));
    }
    if has_mask {
        new_mask.extend(core::mem::take(&mut encoding.special_mask));
    }
    if has_type_ids {
        new_type_ids.extend(core::mem::take(&mut encoding.type_ids));
    }
    encoding.ids = new_ids;
    encoding.attention_mask = new_attn;
    if has_offsets {
        encoding.offsets = new_offsets;
    }
    if has_mask {
        encoding.special_mask = new_mask;
    }
    if has_type_ids {
        encoding.type_ids = new_type_ids;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn synth(ids: &[u32]) -> Encoding<u32> {
        let mut e = Encoding::<u32>::new();
        for (i, &id) in ids.iter().enumerate() {
            e.ids.push(id);
            e.offsets.push(i..i + 1);
            e.special_mask.push(false);
            e.type_ids.push(0);
            e.attention_mask.push(true);
        }
        e
    }

    fn ids_only(ids: &[u32]) -> Encoding<u32> {
        let mut e = Encoding::<u32>::new();
        e.ids = ids.to_vec();
        e
    }

    #[test]
    fn pad_batch_longest_pads_all_to_max() {
        let mut batch = vec![synth(&[1, 2, 3]), synth(&[10, 11]), synth(&[100])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: 0,
            pad_type_id: 0,
            direction: PaddingDirection::Right,
        };
        pad_batch(&mut batch, &cfg);
        for enc in &batch {
            assert_eq!(enc.ids.len(), 3);
            assert_eq!(enc.attention_mask.len(), 3);
        }
        assert_eq!(batch[0].ids, vec![1, 2, 3]);
        assert_eq!(batch[0].attention_mask, vec![true, true, true]);
        assert_eq!(batch[1].ids, vec![10, 11, 0]);
        assert_eq!(batch[1].attention_mask, vec![true, true, false]);
        assert_eq!(batch[2].ids, vec![100, 0, 0]);
        assert_eq!(batch[2].attention_mask, vec![true, false, false]);
    }

    #[test]
    fn pad_batch_fixed_pads_all_to_target() {
        let mut batch = vec![synth(&[1]), synth(&[2, 3])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::Fixed(5),
            pad_id: 0,
            pad_type_id: 0,
            direction: PaddingDirection::Right,
        };
        pad_batch(&mut batch, &cfg);
        assert_eq!(batch[0].ids, vec![1, 0, 0, 0, 0]);
        assert_eq!(
            batch[0].attention_mask,
            vec![true, false, false, false, false]
        );
        assert_eq!(batch[1].ids, vec![2, 3, 0, 0, 0]);
    }

    #[test]
    fn pad_batch_left_prepends() {
        let mut batch = vec![synth(&[1, 2, 3]), synth(&[10])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: 0,
            pad_type_id: 0,
            direction: PaddingDirection::Left,
        };
        pad_batch(&mut batch, &cfg);
        assert_eq!(batch[1].ids, vec![0, 0, 10]);
        assert_eq!(batch[1].attention_mask, vec![false, false, true]);
    }

    #[test]
    fn pad_synthesises_attention_mask_when_missing() {
        let mut batch = vec![ids_only(&[1, 2, 3]), ids_only(&[10])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: 0,
            pad_type_id: 0,
            direction: PaddingDirection::Right,
        };
        pad_batch(&mut batch, &cfg);
        assert_eq!(batch[0].attention_mask, vec![true, true, true]);
        assert_eq!(batch[1].attention_mask, vec![true, false, false]);
        // Optional arrays that were empty stay empty (padding does not
        // synthesise offsets/special_mask/type_ids that were never
        // tracked).
        assert!(batch[0].offsets.is_empty());
        assert!(batch[1].special_mask.is_empty());
    }

    #[test]
    fn pad_batch_empty_is_noop() {
        let mut batch: Vec<Encoding<u32>> = Vec::new();
        let cfg = PaddingConfig::<u32>::default();
        pad_batch(&mut batch, &cfg);
        assert!(batch.is_empty());
    }

    #[test]
    fn pad_fixed_target_below_length_is_noop() {
        let mut batch = vec![synth(&[1, 2, 3, 4, 5])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::Fixed(2),
            pad_id: 0,
            pad_type_id: 0,
            direction: PaddingDirection::Right,
        };
        pad_batch(&mut batch, &cfg);
        assert_eq!(batch[0].ids, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn pad_carries_pad_type_id_into_type_ids() {
        let mut batch = vec![synth(&[1, 2, 3]), synth(&[10])];
        let cfg = PaddingConfig::<u32> {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: 0,
            pad_type_id: 7,
            direction: PaddingDirection::Right,
        };
        pad_batch(&mut batch, &cfg);
        // First encoding wasn't padded — its type_ids stay [0, 0, 0].
        assert_eq!(batch[0].type_ids, vec![0, 0, 0]);
        // Second encoding was padded — the two new slots get pad_type_id.
        assert_eq!(batch[1].type_ids, vec![0, 7, 7]);
    }
}
