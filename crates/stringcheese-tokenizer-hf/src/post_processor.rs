//! Post-processing layer for the BPE tokenizer.
//!
//! Runs after the BPE encode loop and before the caller sees the
//! [`Encoding`]. Today the module ships:
//!
//! * `TemplateProcessing` — the shape every Llama-family checkpoint
//!   uses to inject `<s>` / `</s>` (or `<|begin_of_text|>` /
//!   `<|eot_id|>`) around a primary encoding.
//! * `ByteLevel` — HF's `ByteLevel` post-processor. Ships in every
//!   GPT-2-family `tokenizer.json`. See the variant's docs for the
//!   no-op-on-offsets policy this crate applies.
//! * `None` — the identity sentinel used when the config omits the
//!   `post_processor` slot.
//!
//! # `TemplateProcessing` semantics
//!
//! The processor carries two ordered templates:
//!
//! * [`TemplateProcessing::single`] — applied when the caller encoded
//!   a single input string. Each entry is either a
//!   [`TemplatePiece::Sequence`] slot (which the "primary" encoding
//!   fills — matching HF's `"A"` sequence marker) or a
//!   [`TemplatePiece::SpecialToken`] slot (which resolves to a
//!   pre-registered id).
//! * [`TemplateProcessing::pair`] — the shape for two-input encodings.
//!   Not driven by [`crate::BpeTokenizer`]'s `encode` (which encodes
//!   a single string); the field is preserved for callers who want to
//!   inspect it and for the eventual `encode_pair` landing.
//!
//! HF's on-disk template also supports a `"B"` `Sequence` slot for the
//! second input in a pair template — that lives in
//! [`TemplateProcessing::pair`] and never fires under the single-input
//! [`PostProcessor::apply`] path.
//!
//! # Deferred variants
//!
//! Every other HF `PostProcessor` type is out of scope for this
//! landing:
//!
//! * `BertProcessing` — belongs with the `WordPiece` / BERT landing.
//! * `RobertaProcessing` — needs SEP + CLS + offset shifting; wire it
//!   up when a `RoBERTa` checkpoint asks for it.
//! * `Sequence` — composition of multiple post-processors. The
//!   `TemplateProcessing`-only pipeline covers every shipped Llama /
//!   Mistral / Qwen shape today; add composition when a real checkpoint
//!   ships one.
//!
//! Deferred variants surface at [`crate::hf::to_bpe_tokenizer`] time
//! as [`crate::hf::HfConversionError::UnsupportedPostProcessor`] with
//! the offending type name.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_tokenizer::Encoding;

use crate::bpe::TokenId;

/// A single slot in a [`TemplateProcessing`] template.
///
/// Templates are `Vec<TemplatePiece>` and consumed in order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplatePiece {
    /// The "primary" encoding slot — filled by the caller's own
    /// encoded ids when [`PostProcessor::apply`] runs. HF's on-disk
    /// spec uses the string `"A"` for this in single templates and
    /// `"A"` / `"B"` in pair templates; [`PostProcessor::apply`]
    /// substitutes only `"A"`. Every `type_id` recorded here is
    /// preserved on the produced [`Encoding::special_mask`] as
    /// non-special (the primary ids are BPE outputs, not specials).
    Sequence {
        /// Which primary slot this refers to. `"A"` selects the sole
        /// caller-supplied encoding under [`PostProcessor::apply`];
        /// `"B"` slots inside a pair template never fire on the
        /// single-input encode path but are preserved on the parsed
        /// value for caller inspection.
        id: String,
        /// The `type_id` HF assigns to this slot. Preserved verbatim
        /// (this crate does not surface a `type_ids` array on
        /// [`Encoding`] but keeps the field so callers who round-trip
        /// through the parsed config can inspect it).
        type_id: u32,
    },
    /// A pre-registered special-token slot. `id` names the entry in
    /// [`TemplateProcessing::special_tokens`] whose ids are spliced
    /// into the encoding at this position.
    SpecialToken {
        /// The name of the referenced entry in
        /// [`TemplateProcessing::special_tokens`]. Must be present
        /// there or [`crate::hf::to_bpe_tokenizer`] rejects the
        /// config.
        id: String,
        /// The `type_id` HF assigns to this slot. Preserved verbatim.
        type_id: u32,
    },
}

/// Metadata for one entry in [`TemplateProcessing::special_tokens`].
///
/// HF's on-disk shape records both `ids` (the numeric ids emitted at
/// each occurrence) and `tokens` (the string surface forms) for every
/// referenced special token. Both are kept here so callers who
/// inspect the parsed config get the full picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialTokenInfo {
    /// Numeric ids emitted when this special-token slot fires. Most
    /// specials map to a single id (`[<s>_id]`); the field is a `Vec`
    /// because HF's spec permits multi-id specials.
    pub ids: Vec<TokenId>,
    /// Surface strings that correspond to [`Self::ids`]. Parallel to
    /// `ids` — element `i` is the string for id `i`. Preserved for
    /// caller inspection; [`PostProcessor::apply`] itself only
    /// reads [`Self::ids`].
    pub tokens: Vec<String>,
}

/// The `TemplateProcessing` post-processor.
///
/// See the module-level documentation for the full semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateProcessing {
    /// Template for a single-input encoding.
    pub single: Vec<TemplatePiece>,
    /// Template for a pair-input encoding. Preserved verbatim but not
    /// consumed by [`PostProcessor::apply`] under the single-input
    /// encode path.
    pub pair: Vec<TemplatePiece>,
    /// Metadata for every [`TemplatePiece::SpecialToken`] entry
    /// referenced by [`Self::single`] or [`Self::pair`]. Keyed by the
    /// slot's `id`.
    pub special_tokens: alloc::collections::BTreeMap<String, SpecialTokenInfo>,
}

/// The post-processor variant.
///
/// [`Self::None`] is the "identity" pass-through used when the config
/// has no `post_processor` slot or when the caller explicitly opts
/// out. [`Self::TemplateProcessing`] carries the Llama-shape template.
/// [`Self::ByteLevel`] carries the GPT-2-shape offset-trim config
/// (see the variant's docs for why it is a no-op on the encoding
/// this crate ships).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PostProcessor {
    /// No post-processing.
    #[default]
    None,
    /// The `TemplateProcessing` variant. See [`TemplateProcessing`].
    TemplateProcessing(TemplateProcessing),
    /// The `ByteLevel` post-processor — the shape every GPT-2-family
    /// `tokenizer.json` ships.
    ///
    /// # Semantics — and why this is a no-op today
    ///
    /// Hugging Face's own [`ByteLevel::process`] does exactly two
    /// things at post-process time: **(1)** trim leading `Ġ`
    /// characters from every token's *character-space* offset span
    /// when [`Self::ByteLevel::trim_offsets`] is `true`; **(2)** leave
    /// the token ids themselves unchanged. The `add_prefix_space`
    /// field on this variant is preserved on-disk but **is not
    /// consulted at process time**: HF's own pipeline threads the
    /// prefix-space decision through the *pre-tokenizer* (the
    /// `ByteLevel` pre-tokenizer's own `add_prefix_space` field), and
    /// GPT-2's stock config famously carries `false` on the
    /// pre-tokenizer and `true` on the post-processor without any
    /// user-visible effect. The `use_regex` field is likewise inert
    /// on the process-time path.
    ///
    /// [`crate::BpeTokenizer`]'s [`Encoding<TokenId>`](Encoding)
    /// reports `offsets` as **byte** ranges into the (normalised)
    /// input string, not character positions in the byte-encoded
    /// stream — there are no `Ġ` characters to trim in that space.
    /// The `ByteLevel` post-processor is therefore a pure no-op on
    /// both `ids` and `offsets` in this crate's shape, and callers
    /// who load a config with a `ByteLevel` post-processor observe
    /// identical output to callers whose config omits it. The three
    /// fields are preserved verbatim on the parsed value so callers
    /// who round-trip through the config still see the source values.
    ///
    /// This policy — accept the config, encode identically — is what
    /// unblocks GPT-2 `tokenizer.json` loads without imposing HF's
    /// `NormalizedString` bookkeeping on the crate. Future auditors:
    /// if a downstream consumer *does* start tracking character-space
    /// offsets, this arm is where the trim would land.
    ///
    /// [`ByteLevel::process`]: https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/processors/byte_level.rs
    ByteLevel {
        /// Preserved for caller inspection; **not applied at
        /// process time** (HF ignores this field on the post-processor
        /// path — the pre-tokenizer's own `add_prefix_space` is what
        /// governs the prefix behaviour end-to-end).
        add_prefix_space: bool,
        /// Preserved for caller inspection; not applied because this
        /// crate reports byte offsets rather than character offsets,
        /// so there are no `Ġ` characters in the offset space to
        /// trim. See the variant-level docs.
        trim_offsets: bool,
        /// Preserved for caller inspection; not applied — HF's own
        /// `process()` never consults this field either.
        use_regex: bool,
    },
}

impl PostProcessor {
    /// Apply this post-processor to `encoding`, returning a new
    /// [`Encoding`] with the templated tokens spliced in.
    ///
    /// * When `add_special_tokens == false`, returns a clone of
    ///   `encoding` unchanged — matches HF's own behaviour and lets
    ///   callers who want the raw BPE output opt out on a per-encode
    ///   basis.
    /// * When [`Self::None`], returns a clone of `encoding` unchanged.
    /// * When [`Self::TemplateProcessing`], walks the
    ///   [`TemplateProcessing::single`] template: each
    ///   [`TemplatePiece::Sequence`] slot with `id == "A"` splices in
    ///   the caller's encoded ids (offsets and special-mask carried
    ///   through unchanged); each [`TemplatePiece::SpecialToken`]
    ///   emits every id from the referenced
    ///   [`SpecialTokenInfo::ids`], with offsets set to the empty
    ///   range and special-mask flipped to `true`. Slots referencing
    ///   an unknown special-token name are silently dropped —
    ///   [`crate::hf::to_bpe_tokenizer`] validates references at
    ///   construction time so this path is unreachable from the HF
    ///   loader; manually-constructed processors get the same lenient
    ///   handling that HF's own `TemplateProcessing::apply` uses.
    #[must_use]
    pub fn apply(
        &self,
        encoding: &Encoding<TokenId>,
        add_special_tokens: bool,
    ) -> Encoding<TokenId> {
        if !add_special_tokens {
            return encoding.clone();
        }
        match self {
            // ByteLevel is a no-op on the encoding this crate ships;
            // see the [`Self::ByteLevel`] variant docs for the
            // rationale (byte-vs-character offsets, HF's own inert
            // `add_prefix_space` handling on the post-processor path).
            // Merged with the identity arm because the observable
            // behaviour is identical.
            Self::None | Self::ByteLevel { .. } => encoding.clone(),
            Self::TemplateProcessing(tp) => {
                apply_template(&tp.single, &tp.special_tokens, encoding)
            }
        }
    }
}

/// Splice `template`'s slots against `encoding`, resolving
/// [`TemplatePiece::SpecialToken`] entries through `specials`.
fn apply_template(
    template: &[TemplatePiece],
    specials: &alloc::collections::BTreeMap<String, SpecialTokenInfo>,
    encoding: &Encoding<TokenId>,
) -> Encoding<TokenId> {
    let mut out = Encoding::<TokenId>::new();
    // Pre-allocate a lower bound: the primary encoding plus one id per
    // special slot. Under-count is fine; `Vec::push` grows.
    let primary_len = encoding.ids.len();
    let extra = template
        .iter()
        .filter(|p| matches!(p, TemplatePiece::SpecialToken { .. }))
        .count();
    out.ids.reserve(primary_len + extra);
    out.offsets.reserve(primary_len + extra);
    out.special_mask.reserve(primary_len + extra);

    let has_offsets = !encoding.offsets.is_empty();
    let has_mask = !encoding.special_mask.is_empty();

    for piece in template {
        match piece {
            TemplatePiece::Sequence { id, .. } if id == "A" => {
                for (i, &tid) in encoding.ids.iter().enumerate() {
                    out.ids.push(tid);
                    if has_offsets {
                        out.offsets.push(encoding.offsets[i].clone());
                    }
                    if has_mask {
                        out.special_mask.push(encoding.special_mask[i]);
                    }
                }
            }
            TemplatePiece::Sequence { .. } => {
                // A "B" slot inside `single` is ill-formed; HF's own
                // library rejects it. We drop it silently — this path
                // is unreachable via the HF loader (which routes `B`
                // into `pair` only) and gracefully degrades for
                // hand-built processors.
            }
            TemplatePiece::SpecialToken { id, .. } => {
                if let Some(info) = specials.get(id) {
                    for &tid in &info.ids {
                        out.ids.push(tid);
                        if has_offsets {
                            out.offsets.push(0..0);
                        }
                        if has_mask {
                            out.special_mask.push(true);
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;
    use alloc::vec;

    fn primary(ids: &[TokenId]) -> Encoding<TokenId> {
        let mut e = Encoding::<TokenId>::new();
        for (i, &id) in ids.iter().enumerate() {
            e.ids.push(id);
            e.offsets.push(i..i + 1);
            e.special_mask.push(false);
        }
        e
    }

    fn llama_specials() -> BTreeMap<String, SpecialTokenInfo> {
        let mut m = BTreeMap::new();
        m.insert(
            "<s>".to_string(),
            SpecialTokenInfo {
                ids: vec![1],
                tokens: vec!["<s>".to_string()],
            },
        );
        m.insert(
            "</s>".to_string(),
            SpecialTokenInfo {
                ids: vec![2],
                tokens: vec!["</s>".to_string()],
            },
        );
        m
    }

    #[test]
    fn template_processing_wraps_single_encoding_with_bos_eos() {
        let tp = TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "<s>".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
                TemplatePiece::SpecialToken {
                    id: "</s>".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: llama_specials(),
        };
        let pp = PostProcessor::TemplateProcessing(tp);
        let enc = primary(&[10, 11, 12]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![1, 10, 11, 12, 2]);
        assert_eq!(out.special_mask, vec![true, false, false, false, true]);
        assert_eq!(out.offsets, vec![0..0, 0..1, 1..2, 2..3, 0..0]);
    }

    #[test]
    fn template_processing_add_special_tokens_false_returns_input_unchanged() {
        let tp = TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "<s>".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: llama_specials(),
        };
        let pp = PostProcessor::TemplateProcessing(tp);
        let enc = primary(&[10, 11]);
        let out = pp.apply(&enc, false);
        assert_eq!(out.ids, vec![10, 11]);
        assert_eq!(out.special_mask, vec![false, false]);
    }

    #[test]
    fn template_none_is_identity() {
        let pp = PostProcessor::None;
        let enc = primary(&[10, 11]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, enc.ids);
        assert_eq!(out.offsets, enc.offsets);
        assert_eq!(out.special_mask, enc.special_mask);
    }

    #[test]
    fn template_processing_drops_unknown_special_slot() {
        // "<pad>" is not registered; the slot is dropped and the rest
        // of the template still fires.
        let tp = TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "<pad>".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: llama_specials(),
        };
        let pp = PostProcessor::TemplateProcessing(tp);
        let enc = primary(&[7, 8]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![7, 8]);
    }

    #[test]
    fn byte_level_post_processor_is_noop_on_ids() {
        // ByteLevel post-processor is a documented no-op on the ids
        // this crate ships (offsets are byte-space, not char-space).
        // The three fields are preserved on the value but do not
        // touch the encoding at process time.
        let pp = PostProcessor::ByteLevel {
            add_prefix_space: false,
            trim_offsets: true,
            use_regex: true,
        };
        let enc = primary(&[10, 11, 12]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![10, 11, 12]);
        assert_eq!(out.offsets, enc.offsets);
        assert_eq!(out.special_mask, enc.special_mask);
    }

    #[test]
    fn byte_level_post_processor_ignores_add_special_tokens_toggle() {
        // Both true and false must round-trip the encoding identically
        // — the no-op contract does not depend on the flag.
        let pp = PostProcessor::ByteLevel {
            add_prefix_space: true,
            trim_offsets: false,
            use_regex: true,
        };
        let enc = primary(&[7, 8, 9]);
        assert_eq!(pp.apply(&enc, true).ids, vec![7, 8, 9]);
        assert_eq!(pp.apply(&enc, false).ids, vec![7, 8, 9]);
    }

    #[test]
    fn template_processing_multi_id_special_expands_all_ids() {
        // A single SpecialToken slot backed by multiple ids emits
        // every id in order.
        let mut specials = BTreeMap::new();
        specials.insert(
            "<multi>".to_string(),
            SpecialTokenInfo {
                ids: vec![100, 101, 102],
                tokens: vec!["<a>".to_string(), "<b>".to_string(), "<c>".to_string()],
            },
        );
        let tp = TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "<multi>".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: specials,
        };
        let pp = PostProcessor::TemplateProcessing(tp);
        let enc = primary(&[9]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![100, 101, 102, 9]);
        assert_eq!(out.special_mask, vec![true, true, true, false]);
    }
}
