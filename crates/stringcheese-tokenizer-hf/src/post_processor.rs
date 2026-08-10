//! Post-processing layer for the BPE tokenizer.
//!
//! Runs after the BPE encode loop and before the caller sees the
//! [`Encoding`]. Today the module ships:
//!
//! * `TemplateProcessing` — the shape every Llama-family checkpoint
//!   uses to inject `<s>` / `</s>` (or `<|begin_of_text|>` /
//!   `<|eot_id|>`) around a primary encoding.
//! * `BertProcessing` — the fixed `[CLS] $A [SEP]` splice every stock
//!   BERT / `DistilBERT` / `MobileBERT` / `ALBERT` checkpoint ships.
//! * `RobertaProcessing` — the `XLM-RoBERTa` / `RoBERTa` / `CamemBERT`
//!   CLS-and-SEP splice, distinct from `BertProcessing` in that it
//!   also carries the two flags HF preserves for byte-level
//!   round-tripping (`trim_offsets`, `add_prefix_space`).
//! * `ByteLevel` — HF's `ByteLevel` post-processor. Ships in every
//!   GPT-2-family `tokenizer.json`. See the variant's docs for the
//!   no-op-on-offsets policy this crate applies.
//! * `Sequence` — composition of nested post-processors, applied left
//!   to right. Threads a single [`Encoding`] through each child's
//!   [`PostProcessor::apply`] in order and returns the final result.
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
//! # `RobertaProcessing` semantics
//!
//! [`PostProcessor::RobertaProcessing`] carries a `(surface, id)` pair
//! for the CLS token and another for the SEP token, plus the two
//! HF-visible flags [`RobertaProcessing::trim_offsets`] and
//! [`RobertaProcessing::add_prefix_space`]. On the single-input encode
//! path this crate exercises, `apply` splices the CLS id on the left
//! of the primary encoding and the SEP id on the right — the shape
//! every `XLM-RoBERTa` / `RoBERTa` / `CamemBERT` / `XLM-V` checkpoint
//! uses today. Both flags are preserved verbatim on the runtime value
//! but not otherwise consumed here: `trim_offsets` targets
//! `ByteLevel`-space offset accounting that the Unigram consumer (the
//! only tokenizer that ships this variant in practice) does not
//! track, and `add_prefix_space` is meaningful only for the
//! `ByteLevel` pre-tokenizer that never composes with a
//! `SentencePiece` model. Both remain accessible so callers who
//! round-trip through the parsed config can inspect them.
//!
//! # `BertProcessing` semantics
//!
//! [`PostProcessor::BertProcessing`] carries a `(surface, id)` pair for
//! the SEP token and another for the CLS token. On the single-input
//! encode path this crate exercises, `apply` splices the CLS id on the
//! left of the primary encoding and the SEP id on the right — the
//! shape every stock BERT / `DistilBERT` / `MobileBERT` / `ALBERT`
//! checkpoint uses. Unlike [`RobertaProcessing`] the variant carries
//! no `trim_offsets` / `add_prefix_space` flags — HF's own
//! `BertProcessing` shape has none.
//!
//! # `Sequence` semantics
//!
//! [`PostProcessor::Sequence`] composes multiple post-processors: the
//! primary encoding is threaded through each child's [`PostProcessor::apply`]
//! call in order, and the final encoding is returned. Nested
//! `Sequence` values are permitted (each recursive `apply` re-enters
//! the top-level dispatch); depth is bounded in practice by the shape
//! of the source config. When `add_special_tokens` is `false`, the
//! top-level early return preserves the input unchanged — no child is
//! applied.
//!
//! # Deferred variants
//!
//! All HF `PostProcessor` types this module ships are now materialised
//! at conversion time. The only remaining exotic variant HF's own
//! crate defines is a `WordPieceProcessing`-style tag which no shipped
//! checkpoint uses in practice; if a real config ships one, add a
//! variant here alongside a matching [`crate::hf::HfPostProcessor`]
//! tag and route it through [`crate::hf::to_bpe_tokenizer`].

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

/// The `RobertaProcessing` post-processor.
///
/// See the module-level documentation for the full semantics. The
/// field names mirror Hugging Face's on-disk shape verbatim so the
/// JSON loader can splice values straight in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobertaProcessing {
    /// The `(surface_string, id)` pair emitted at the SEP slot after
    /// the primary encoding. HF's canonical value for `XLM-RoBERTa`
    /// is `("</s>", 2)`.
    pub sep: (String, TokenId),
    /// The `(surface_string, id)` pair emitted at the CLS slot before
    /// the primary encoding. HF's canonical value for `XLM-RoBERTa`
    /// is `("<s>", 0)`.
    pub cls: (String, TokenId),
    /// Whether HF's own runtime would trim `Ġ`-prefixed offsets. The
    /// crate does not track byte-level offsets on the Unigram output
    /// (`SentencePiece` is character-space, not byte-encoded space) so
    /// this flag is preserved verbatim but not consumed by
    /// [`PostProcessor::apply`]. Callers who round-trip through the
    /// parsed config can still inspect it.
    pub trim_offsets: bool,
    /// Whether HF's own runtime would insert a leading space before
    /// the primary text. Only meaningful for the `ByteLevel`
    /// pre-tokenizer this processor never composes with in practice;
    /// preserved verbatim for the same round-trip reason as
    /// [`Self::trim_offsets`].
    pub add_prefix_space: bool,
}

/// The `BertProcessing` post-processor.
///
/// See the module-level documentation for the full semantics. The
/// field names mirror Hugging Face's on-disk shape verbatim
/// (`{"type": "BertProcessing", "sep": ["[SEP]", 102], "cls":
/// ["[CLS]", 101]}`) so the JSON loader can splice values straight
/// in. Unlike [`RobertaProcessing`] there are no `trim_offsets` or
/// `add_prefix_space` fields — HF's own `BertProcessing` type has
/// none either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BertProcessing {
    /// The `(surface_string, id)` pair emitted at the SEP slot after
    /// the primary encoding. HF's canonical value for BERT-base is
    /// `("[SEP]", 102)`.
    pub sep: (String, TokenId),
    /// The `(surface_string, id)` pair emitted at the CLS slot before
    /// the primary encoding. HF's canonical value for BERT-base is
    /// `("[CLS]", 101)`.
    pub cls: (String, TokenId),
}

/// The post-processor variant.
///
/// [`Self::None`] is the "identity" pass-through used when the config
/// has no `post_processor` slot or when the caller explicitly opts
/// out. [`Self::TemplateProcessing`] carries the Llama-shape template.
/// [`Self::BertProcessing`] carries the stock-BERT `[CLS] $A [SEP]`
/// splice. [`Self::RobertaProcessing`] carries the `XLM-RoBERTa` /
/// `RoBERTa` CLS-and-SEP splice (with the two byte-level flags HF
/// preserves). [`Self::ByteLevel`] carries the GPT-2-shape
/// offset-trim config (see the variant's docs for why it is a no-op
/// on the encoding this crate ships). [`Self::Sequence`] composes
/// nested post-processors and threads the encoding through them left
/// to right.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PostProcessor {
    /// No post-processing.
    #[default]
    None,
    /// The `TemplateProcessing` variant. See [`TemplateProcessing`].
    TemplateProcessing(TemplateProcessing),
    /// The `BertProcessing` variant. See [`BertProcessing`].
    BertProcessing(BertProcessing),
    /// The `RobertaProcessing` variant. See [`RobertaProcessing`].
    RobertaProcessing(RobertaProcessing),
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
    /// Compose several post-processors, applied left to right against
    /// the encoding. See the module-level `Sequence` semantics section.
    ///
    /// The primary encoding is passed to the first child's
    /// [`PostProcessor::apply`], and the result becomes the input to the next
    /// child. Nested `Sequence` values are permitted — each recursive
    /// `apply` re-enters the top-level dispatch. HF's own on-disk
    /// shape is `{"type": "Sequence", "processors": [...]}` — the
    /// field name preserved verbatim on
    /// [`crate::hf::HfPostProcessor::Sequence`].
    Sequence(Vec<PostProcessor>),
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
            Self::BertProcessing(bp) => apply_bert(bp, encoding),
            Self::RobertaProcessing(rp) => apply_roberta(rp, encoding),
            Self::Sequence(children) => {
                // Thread the incoming encoding through each child in
                // order. The first child sees the caller's encoding;
                // every subsequent child sees the previous child's
                // output. Nested `Sequence` values re-enter this same
                // `apply` dispatch — depth is bounded in practice by
                // the shape of the source config.
                //
                // Empty sequence is a no-op (matches HF's own
                // behaviour of returning the input unchanged).
                if children.is_empty() {
                    return encoding.clone();
                }
                let mut current = children[0].apply(encoding, add_special_tokens);
                for child in &children[1..] {
                    current = child.apply(&current, add_special_tokens);
                }
                current
            }
        }
    }
}

/// Splice the `cls` id on the left of `encoding` and the `sep` id on
/// the right. Same shape as [`apply_roberta`] — offsets and
/// special-mask carry through unchanged for the primary body; the
/// spliced ids get an empty offset range and `special_mask == true`.
fn apply_bert(bp: &BertProcessing, encoding: &Encoding<TokenId>) -> Encoding<TokenId> {
    let primary_len = encoding.ids.len();
    let mut out = Encoding::<TokenId>::new();
    out.ids.reserve(primary_len + 2);
    out.offsets.reserve(primary_len + 2);
    out.special_mask.reserve(primary_len + 2);

    let has_offsets = !encoding.offsets.is_empty();
    let has_mask = !encoding.special_mask.is_empty();

    // Left splice: <cls>.
    out.ids.push(bp.cls.1);
    if has_offsets {
        out.offsets.push(0..0);
    }
    if has_mask {
        out.special_mask.push(true);
    }
    // Primary body.
    for (i, &tid) in encoding.ids.iter().enumerate() {
        out.ids.push(tid);
        if has_offsets {
            out.offsets.push(encoding.offsets[i].clone());
        }
        if has_mask {
            out.special_mask.push(encoding.special_mask[i]);
        }
    }
    // Right splice: <sep>.
    out.ids.push(bp.sep.1);
    if has_offsets {
        out.offsets.push(0..0);
    }
    if has_mask {
        out.special_mask.push(true);
    }
    out
}

/// Splice the `cls` id on the left of `encoding` and the `sep` id on
/// the right. Offsets and special-mask carry through unchanged for
/// the primary body; the spliced ids get an empty offset range and
/// `special_mask == true` (matching the `TemplateProcessing`
/// [`apply_template`] convention above).
fn apply_roberta(rp: &RobertaProcessing, encoding: &Encoding<TokenId>) -> Encoding<TokenId> {
    let primary_len = encoding.ids.len();
    let mut out = Encoding::<TokenId>::new();
    out.ids.reserve(primary_len + 2);
    out.offsets.reserve(primary_len + 2);
    out.special_mask.reserve(primary_len + 2);

    let has_offsets = !encoding.offsets.is_empty();
    let has_mask = !encoding.special_mask.is_empty();

    // Left splice: <cls>.
    out.ids.push(rp.cls.1);
    if has_offsets {
        out.offsets.push(0..0);
    }
    if has_mask {
        out.special_mask.push(true);
    }
    // Primary body.
    for (i, &tid) in encoding.ids.iter().enumerate() {
        out.ids.push(tid);
        if has_offsets {
            out.offsets.push(encoding.offsets[i].clone());
        }
        if has_mask {
            out.special_mask.push(encoding.special_mask[i]);
        }
    }
    // Right splice: <sep>.
    out.ids.push(rp.sep.1);
    if has_offsets {
        out.offsets.push(0..0);
    }
    if has_mask {
        out.special_mask.push(true);
    }
    out
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

    // -----------------------------------------------------------------
    // RobertaProcessing
    // -----------------------------------------------------------------

    fn xlm_roberta_processor() -> PostProcessor {
        PostProcessor::RobertaProcessing(RobertaProcessing {
            sep: ("</s>".to_string(), 2),
            cls: ("<s>".to_string(), 0),
            trim_offsets: true,
            add_prefix_space: true,
        })
    }

    #[test]
    fn roberta_processing_wraps_primary_with_cls_and_sep() {
        let pp = xlm_roberta_processor();
        let enc = primary(&[10, 11, 12]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![0, 10, 11, 12, 2]);
        assert_eq!(out.special_mask, vec![true, false, false, false, true]);
        assert_eq!(out.offsets, vec![0..0, 0..1, 1..2, 2..3, 0..0]);
    }

    #[test]
    fn roberta_processing_empty_primary_yields_cls_sep_only() {
        let pp = xlm_roberta_processor();
        let enc = Encoding::<TokenId>::new();
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![0, 2]);
        assert!(out.offsets.is_empty());
        assert!(out.special_mask.is_empty());
    }

    #[test]
    fn roberta_processing_add_special_tokens_false_is_identity() {
        let pp = xlm_roberta_processor();
        let enc = primary(&[10, 11]);
        let out = pp.apply(&enc, false);
        assert_eq!(out.ids, vec![10, 11]);
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

    // -----------------------------------------------------------------
    // BertProcessing
    // -----------------------------------------------------------------

    fn bert_processor() -> PostProcessor {
        PostProcessor::BertProcessing(BertProcessing {
            sep: ("[SEP]".to_string(), 102),
            cls: ("[CLS]".to_string(), 101),
        })
    }

    #[test]
    fn bert_processing_wraps_primary_with_cls_and_sep() {
        let pp = bert_processor();
        let enc = primary(&[10, 11, 12]);
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![101, 10, 11, 12, 102]);
        assert_eq!(out.special_mask, vec![true, false, false, false, true]);
        assert_eq!(out.offsets, vec![0..0, 0..1, 1..2, 2..3, 0..0]);
    }

    #[test]
    fn bert_processing_empty_primary_yields_cls_sep_only() {
        let pp = bert_processor();
        let enc = Encoding::<TokenId>::new();
        let out = pp.apply(&enc, true);
        assert_eq!(out.ids, vec![101, 102]);
        assert!(out.offsets.is_empty());
        assert!(out.special_mask.is_empty());
    }

    #[test]
    fn bert_processing_add_special_tokens_false_is_identity() {
        let pp = bert_processor();
        let enc = primary(&[10, 11]);
        let out = pp.apply(&enc, false);
        assert_eq!(out.ids, vec![10, 11]);
    }

    // -----------------------------------------------------------------
    // Sequence
    // -----------------------------------------------------------------

    fn tp_with_only_cls(cls_id: TokenId) -> PostProcessor {
        let mut specials = BTreeMap::new();
        specials.insert(
            "<cls>".to_string(),
            SpecialTokenInfo {
                ids: vec![cls_id],
                tokens: vec!["<cls>".to_string()],
            },
        );
        PostProcessor::TemplateProcessing(TemplateProcessing {
            single: vec![
                TemplatePiece::SpecialToken {
                    id: "<cls>".to_string(),
                    type_id: 0,
                },
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: specials,
        })
    }

    fn tp_with_only_sep(sep_id: TokenId) -> PostProcessor {
        let mut specials = BTreeMap::new();
        specials.insert(
            "<sep>".to_string(),
            SpecialTokenInfo {
                ids: vec![sep_id],
                tokens: vec!["<sep>".to_string()],
            },
        );
        PostProcessor::TemplateProcessing(TemplateProcessing {
            single: vec![
                TemplatePiece::Sequence {
                    id: "A".to_string(),
                    type_id: 0,
                },
                TemplatePiece::SpecialToken {
                    id: "<sep>".to_string(),
                    type_id: 0,
                },
            ],
            pair: vec![],
            special_tokens: specials,
        })
    }

    #[test]
    fn sequence_threads_encoding_through_children_in_order() {
        // Two TemplateProcessing children: the first prepends id 100,
        // the second appends id 200. The second child must see the
        // first child's output as its primary encoding.
        let seq = PostProcessor::Sequence(vec![tp_with_only_cls(100), tp_with_only_sep(200)]);
        let enc = primary(&[10, 11]);
        let out = seq.apply(&enc, true);
        assert_eq!(out.ids, vec![100, 10, 11, 200]);
        // Both spliced ids are marked special.
        assert_eq!(out.special_mask, vec![true, false, false, true]);
    }

    #[test]
    fn sequence_empty_is_identity() {
        let seq = PostProcessor::Sequence(Vec::new());
        let enc = primary(&[10, 11]);
        let out = seq.apply(&enc, true);
        assert_eq!(out.ids, vec![10, 11]);
        assert_eq!(out.special_mask, vec![false, false]);
    }

    #[test]
    fn sequence_add_special_tokens_false_short_circuits_at_top_level() {
        // The top-level early return fires before any child runs — so
        // even a Sequence with side-effect children returns the input
        // unchanged when add_special_tokens is false.
        let seq = PostProcessor::Sequence(vec![tp_with_only_cls(100), tp_with_only_sep(200)]);
        let enc = primary(&[10, 11]);
        let out = seq.apply(&enc, false);
        assert_eq!(out.ids, vec![10, 11]);
        assert_eq!(out.special_mask, vec![false, false]);
    }

    #[test]
    fn sequence_containing_sequence_composes_correctly() {
        // Sequence-inside-Sequence: the inner sequence prepends 100
        // and appends 200; the outer sequence then applies a
        // BertProcessing that wraps the result in cls/sep.
        let inner = PostProcessor::Sequence(vec![tp_with_only_cls(100), tp_with_only_sep(200)]);
        let outer = PostProcessor::Sequence(vec![inner, bert_processor()]);
        let enc = primary(&[10, 11]);
        let out = outer.apply(&enc, true);
        // Inner: [100, 10, 11, 200]. Outer BertProcessing wraps that:
        // [101, 100, 10, 11, 200, 102].
        assert_eq!(out.ids, vec![101, 100, 10, 11, 200, 102]);
    }

    #[test]
    fn sequence_of_byte_level_and_bert_matches_bert_alone() {
        // ByteLevel is a documented no-op on the encoding this crate
        // ships, so composing it with BertProcessing must produce
        // the same output as BertProcessing alone.
        let seq = PostProcessor::Sequence(vec![
            PostProcessor::ByteLevel {
                add_prefix_space: false,
                trim_offsets: true,
                use_regex: true,
            },
            bert_processor(),
        ]);
        let enc = primary(&[10, 11, 12]);
        let out_seq = seq.apply(&enc, true);
        let out_bare = bert_processor().apply(&enc, true);
        assert_eq!(out_seq.ids, out_bare.ids);
        assert_eq!(out_seq.special_mask, out_bare.special_mask);
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
