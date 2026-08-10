//! Post-encode truncation.
//!
//! Truncation runs *after* a tokenizer's [`encode`](crate::Tokenizer::encode)
//! call produces an [`Encoding`] — it is not baked into any runtime's
//! encode loop, so a caller who wants to bound `max_length` at
//! serving-time can splice this module in without touching the
//! per-runtime code. Batch-serving code typically composes
//! [`truncate`] with [`crate::padding::pad_batch`] to reach a uniform
//! length across the batch.
//!
//! # Single vs pair encodings
//!
//! * [`truncate`] operates on a single [`Encoding`]. When
//!   [`TruncationConfig::strategy`] is [`TruncationStrategy::OnlySecond`]
//!   or [`TruncationStrategy::DoNotTruncate`], the function is a no-op —
//!   the "second" side never exists on a single encoding, and
//!   `DoNotTruncate` is the explicit "off" sentinel.
//! * [`truncate_pair`] operates on two [`Encoding`]s and applies the
//!   strategy per-side:
//!     * [`TruncationStrategy::LongestFirst`] — alternately drops one
//!       token from whichever side is longer, until the combined length
//!       reaches [`TruncationConfig::max_length`].
//!     * [`TruncationStrategy::OnlyFirst`] — trims only the first
//!       encoding.
//!     * [`TruncationStrategy::OnlySecond`] — trims only the second
//!       encoding.
//!     * [`TruncationStrategy::DoNotTruncate`] — no-op.
//!
//! # Direction
//!
//! [`TruncationDirection::Right`] (the HF default) drops tokens from
//! the tail. [`TruncationDirection::Left`] drops tokens from the head
//! — the shape LLM chat pipelines reach for when the *newest* content
//! must be preserved and the oldest history is what gets forgotten.
//!
//! # `stride`
//!
//! HF's `stride` field (an overlap window carried when a very long
//! input is chunked into multiple encodings) is preserved on
//! [`TruncationConfig::stride`] for round-trip fidelity but is not
//! interpreted by [`truncate`] / [`truncate_pair`] — chunking is a
//! caller-side concern in this crate. Set to `0` on the paved path.

use alloc::vec::Vec;

use crate::traits::Encoding;

/// Which side of the encoding to drop from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TruncationDirection {
    /// Drop tokens from the tail (HF default).
    #[default]
    Right,
    /// Drop tokens from the head (LLM chat serving preserves the most
    /// recent tokens by dropping the oldest).
    Left,
}

/// How to trim a pair of encodings when the combined length exceeds
/// [`TruncationConfig::max_length`].
///
/// Single-encoding truncation ignores every variant except
/// [`Self::DoNotTruncate`] and [`Self::OnlySecond`] (both no-ops on a
/// single encoding, matching HF's behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TruncationStrategy {
    /// Alternately drop from whichever encoding is currently longer.
    /// This is HF's default and preserves the pair-side balance a QA
    /// pipeline expects.
    #[default]
    LongestFirst,
    /// Only trim the first encoding. A pair where the first side is
    /// already shorter than the remaining budget is a no-op.
    OnlyFirst,
    /// Only trim the second encoding.
    OnlySecond,
    /// Do not truncate. [`truncate`] and [`truncate_pair`] both
    /// short-circuit on this variant.
    DoNotTruncate,
}

/// Configuration for a truncation invocation.
///
/// Field shape mirrors HF's `TruncationParams` on-disk shape so the HF
/// loader can splice values straight in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncationConfig {
    /// Maximum length in tokens.
    pub max_length: usize,
    /// Which side(s) of a pair to trim.
    pub strategy: TruncationStrategy,
    /// End of the encoding to drop from.
    pub direction: TruncationDirection,
    /// Overlap window preserved for round-trip fidelity with HF; not
    /// consumed by this crate's [`truncate`] / [`truncate_pair`].
    pub stride: usize,
}

impl TruncationConfig {
    /// Convenience constructor with HF defaults
    /// (`LongestFirst` + `Right` + `stride: 0`).
    #[must_use]
    pub const fn new(max_length: usize) -> Self {
        Self {
            max_length,
            strategy: TruncationStrategy::LongestFirst,
            direction: TruncationDirection::Right,
            stride: 0,
        }
    }
}

/// Truncate a single encoding to `config.max_length`.
///
/// * `DoNotTruncate` / `OnlySecond` — no-op.
/// * `Right` — drops from the tail.
/// * `Left` — drops from the head.
///
/// Every populated per-token array (`offsets`, `special_mask`,
/// `type_ids`, `attention_mask`) is trimmed in lockstep with `ids`.
pub fn truncate<Token>(encoding: &mut Encoding<Token>, config: &TruncationConfig) {
    if matches!(
        config.strategy,
        TruncationStrategy::DoNotTruncate | TruncationStrategy::OnlySecond
    ) {
        return;
    }
    if encoding.ids.len() <= config.max_length {
        return;
    }
    let drop = encoding.ids.len() - config.max_length;
    match config.direction {
        TruncationDirection::Right => truncate_tail_by(encoding, drop),
        TruncationDirection::Left => truncate_head_by(encoding, drop),
    }
}

/// Truncate a pair of encodings to `config.max_length` combined tokens.
///
/// * `LongestFirst` — alternately drop one token from whichever side
///   is currently longer, breaking ties by trimming the first side.
///   Matches HF's own `truncate_encodings` when its pair path fires.
/// * `OnlyFirst` — drops only from `a`. Once `a` is empty the loop
///   stops and the combined length may still exceed `max_length`
///   (matches HF's behaviour when the caller pinned `OnlyFirst` and
///   the second side alone already exceeds the budget).
/// * `OnlySecond` — mirror of `OnlyFirst`.
/// * `DoNotTruncate` — no-op.
pub fn truncate_pair<Token>(
    a: &mut Encoding<Token>,
    b: &mut Encoding<Token>,
    config: &TruncationConfig,
) {
    if matches!(config.strategy, TruncationStrategy::DoNotTruncate) {
        return;
    }
    let combined = a.ids.len() + b.ids.len();
    if combined <= config.max_length {
        return;
    }
    let mut over = combined - config.max_length;
    match config.strategy {
        TruncationStrategy::DoNotTruncate => {}
        TruncationStrategy::OnlyFirst => {
            let drop = over.min(a.ids.len());
            drop_one_side(a, drop, config.direction);
        }
        TruncationStrategy::OnlySecond => {
            let drop = over.min(b.ids.len());
            drop_one_side(b, drop, config.direction);
        }
        TruncationStrategy::LongestFirst => {
            while over > 0 && (!a.ids.is_empty() || !b.ids.is_empty()) {
                if a.ids.len() >= b.ids.len() && !a.ids.is_empty() {
                    drop_one_side(a, 1, config.direction);
                } else if !b.ids.is_empty() {
                    drop_one_side(b, 1, config.direction);
                } else {
                    break;
                }
                over -= 1;
            }
        }
    }
}

fn drop_one_side<Token>(
    encoding: &mut Encoding<Token>,
    count: usize,
    direction: TruncationDirection,
) {
    if count == 0 {
        return;
    }
    match direction {
        TruncationDirection::Right => truncate_tail_by(encoding, count),
        TruncationDirection::Left => truncate_head_by(encoding, count),
    }
}

fn truncate_tail_by<Token>(encoding: &mut Encoding<Token>, drop: usize) {
    let new_len = encoding.ids.len().saturating_sub(drop);
    encoding.ids.truncate(new_len);
    if !encoding.offsets.is_empty() {
        encoding.offsets.truncate(new_len);
    }
    if !encoding.special_mask.is_empty() {
        encoding.special_mask.truncate(new_len);
    }
    if !encoding.type_ids.is_empty() {
        encoding.type_ids.truncate(new_len);
    }
    if !encoding.attention_mask.is_empty() {
        encoding.attention_mask.truncate(new_len);
    }
}

fn truncate_head_by<Token>(encoding: &mut Encoding<Token>, drop: usize) {
    let drop = drop.min(encoding.ids.len());
    drain_head(&mut encoding.ids, drop);
    if !encoding.offsets.is_empty() {
        drain_head(&mut encoding.offsets, drop);
    }
    if !encoding.special_mask.is_empty() {
        drain_head(&mut encoding.special_mask, drop);
    }
    if !encoding.type_ids.is_empty() {
        drain_head(&mut encoding.type_ids, drop);
    }
    if !encoding.attention_mask.is_empty() {
        drain_head(&mut encoding.attention_mask, drop);
    }
}

fn drain_head<T>(v: &mut Vec<T>, count: usize) {
    if count == 0 {
        return;
    }
    v.drain(..count);
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

    #[test]
    fn truncate_right_drops_tail() {
        let mut e = synth(&(0u32..30).collect::<Vec<_>>());
        truncate(&mut e, &TruncationConfig::new(10));
        assert_eq!(e.ids.len(), 10);
        assert_eq!(e.ids.first().copied(), Some(0));
        assert_eq!(e.ids.last().copied(), Some(9));
        assert_eq!(e.offsets.len(), 10);
        assert_eq!(e.special_mask.len(), 10);
        assert_eq!(e.type_ids.len(), 10);
        assert_eq!(e.attention_mask.len(), 10);
    }

    #[test]
    fn truncate_left_drops_head() {
        let mut e = synth(&(0u32..30).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::LongestFirst,
            direction: TruncationDirection::Left,
            stride: 0,
        };
        truncate(&mut e, &cfg);
        assert_eq!(e.ids.len(), 10);
        assert_eq!(e.ids.first().copied(), Some(20));
        assert_eq!(e.ids.last().copied(), Some(29));
    }

    #[test]
    fn truncate_do_not_truncate_is_noop() {
        let mut e = synth(&(0u32..30).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::DoNotTruncate,
            direction: TruncationDirection::Right,
            stride: 0,
        };
        truncate(&mut e, &cfg);
        assert_eq!(e.ids.len(), 30);
    }

    #[test]
    fn truncate_only_second_is_noop_on_single_encoding() {
        let mut e = synth(&(0u32..30).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::OnlySecond,
            direction: TruncationDirection::Right,
            stride: 0,
        };
        truncate(&mut e, &cfg);
        assert_eq!(e.ids.len(), 30);
    }

    #[test]
    fn truncate_under_max_is_noop() {
        let mut e = synth(&[1, 2, 3]);
        truncate(&mut e, &TruncationConfig::new(10));
        assert_eq!(e.ids, vec![1, 2, 3]);
    }

    #[test]
    fn truncate_pair_longest_first_balances() {
        let mut a = synth(&(0u32..20).collect::<Vec<_>>());
        let mut b = synth(&(0u32..5).collect::<Vec<_>>());
        truncate_pair(&mut a, &mut b, &TruncationConfig::new(10));
        assert_eq!(a.ids.len() + b.ids.len(), 10);
        // 15 tokens over budget; LongestFirst drops from the longer
        // side until they equalise then alternates. a starts at 20, b
        // at 5. After 15 drops we expect a==5 and b==5 (a dropped 15
        // times because it stayed >= b throughout).
        assert_eq!(a.ids.len(), 5);
        assert_eq!(b.ids.len(), 5);
    }

    #[test]
    fn truncate_pair_only_first() {
        let mut a = synth(&(0u32..20).collect::<Vec<_>>());
        let mut b = synth(&(0u32..5).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::OnlyFirst,
            direction: TruncationDirection::Right,
            stride: 0,
        };
        truncate_pair(&mut a, &mut b, &cfg);
        // b untouched, a trimmed by 15 (from 20 to 5).
        assert_eq!(a.ids.len(), 5);
        assert_eq!(b.ids.len(), 5);
    }

    #[test]
    fn truncate_pair_only_second() {
        let mut a = synth(&(0u32..5).collect::<Vec<_>>());
        let mut b = synth(&(0u32..20).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::OnlySecond,
            direction: TruncationDirection::Right,
            stride: 0,
        };
        truncate_pair(&mut a, &mut b, &cfg);
        assert_eq!(a.ids.len(), 5);
        assert_eq!(b.ids.len(), 5);
    }

    #[test]
    fn truncate_pair_do_not_truncate_is_noop() {
        let mut a = synth(&(0u32..20).collect::<Vec<_>>());
        let mut b = synth(&(0u32..20).collect::<Vec<_>>());
        let cfg = TruncationConfig {
            max_length: 10,
            strategy: TruncationStrategy::DoNotTruncate,
            direction: TruncationDirection::Right,
            stride: 0,
        };
        truncate_pair(&mut a, &mut b, &cfg);
        assert_eq!(a.ids.len(), 20);
        assert_eq!(b.ids.len(), 20);
    }
}
