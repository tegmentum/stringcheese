//! Unicode normalization layer for the BPE tokenizer.
//!
//! This module implements the `normalizer` slice of Hugging Face's
//! `tokenizer.json` spec: an optional text-in / text-out transform that
//! runs *before* the pre-tokenizer and the BPE merge loop. The layer is
//! small on purpose — HF's own list is closed and this crate only
//! materialises the shapes downstream models actually ship (Llama,
//! Mistral, Qwen, `DeepSeek`, Phi, ...): the four Unicode normalization
//! forms, lower-casing, literal replace, whitespace strip, and their
//! `Sequence` composition.
//!
//! # Semantics
//!
//! Every variant is a pure `&str -> String` function of its input; the
//! offset space of the resulting encoding is therefore into the
//! *normalized* text, not the raw input. This is the same lossy trade
//! Hugging Face's own `tokenizers-rs` makes; recovering original-input
//! offsets requires the on-the-side `NormalizedString` accounting that
//! this crate does not ship.
//!
//! # Supported variants
//!
//! * [`Normalizer::Nfc`], [`Normalizer::Nfd`], [`Normalizer::Nfkc`],
//!   [`Normalizer::Nfkd`] — Unicode Standard Annex #15 canonical /
//!   compatibility (de)composition. Delegated to
//!   [`unicode_normalization::UnicodeNormalization`].
//! * [`Normalizer::Lowercase`] — Unicode-aware lower-casing via
//!   [`str::to_lowercase`] (which walks each scalar's
//!   `Lowercase_Mapping` and is what HF's Rust normalizer calls into).
//! * [`Normalizer::Replace`] — literal-string substitution.
//! * [`Normalizer::ReplaceRegex`] — regex-driven substitution.
//!   Pattern is a `fancy-regex` source string, compiled on each
//!   [`normalize`] call. Materialises `Replace { pattern: Regex(...) }`
//!   entries from `tokenizer.json` blobs (the SentencePiece-conversion
//!   pipeline used by DeBERTa-v3 / mDeBERTa-v3 ships one).
//! * [`Normalizer::Strip`] — trims leading and/or trailing whitespace,
//!   independently controllable on each side.
//! * [`Normalizer::Prepend`] — prepends a fixed literal to the input.
//!   The `SentencePiece` "`▁`" prefix pattern lives here.
//! * [`Normalizer::Sequence`] — apply a list of normalizers left to
//!   right. The `Vec` is followed order-preserving; each entry sees
//!   the output of every preceding one.
//! * [`Normalizer::Bert`] — the classic BERT normalizer used by
//!   BERT / `DistilBERT` / `ELECTRA` and their `WordPiece`
//!   siblings. Composes four independently-toggleable passes:
//!   control-character cleanup, CJK spacing, accent stripping, and
//!   lower-casing. See the variant's doc-comment for the exact order
//!   and the semantics of each pass.
//!
//! # Partially-supported variants
//!
//! * [`Normalizer::Precompiled`] — `SentencePiece`'s compiled
//!   char-mapping table (used by every Llama, Mistral, T5, and
//!   XLM-RoBERTa checkpoint). The base64-encoded charsmap is decoded
//!   into a Darts / `DoubleArray` trie plus replacement-string table
//!   and applied at runtime — see [`precompiled`] for the wire format
//!   and algorithm. When the raw payload fails to decode or parse
//!   (malformed base64, truncated header, ...) the variant falls back
//!   to a passthrough so callers with unusual inputs still round-trip.
//!
//! # Deferred variants
//!
//! * `Nmt` — the Marian NMT normalizer.
//! * Custom callable normalizers.
//!
//! Deferred variants surface at [`crate::hf::to_bpe_tokenizer`] time as
//! [`crate::hf::HfConversionError::UnsupportedNormalizer`] with the
//! offending type name in the message.

use alloc::string::String;
use alloc::vec::Vec;

use fancy_regex::Regex;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

pub mod precompiled;

pub use precompiled::{PrecompiledError, PrecompiledNormalizer};

/// A Unicode normalization step to apply before the pre-tokenizer.
///
/// See the module-level documentation for the full support matrix and
/// the semantics of each variant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Normalizer {
    /// Canonical composition (Normalization Form C).
    Nfc,
    /// Canonical decomposition (Normalization Form D).
    Nfd,
    /// Compatibility composition (Normalization Form KC).
    Nfkc,
    /// Compatibility decomposition (Normalization Form KD).
    Nfkd,
    /// Unicode-aware lower-casing.
    Lowercase,
    /// Literal-string replace: every non-overlapping occurrence of
    /// [`Self::Replace::pattern`] is replaced with
    /// [`Self::Replace::content`]. The pattern is matched *literally*
    /// (no regex); an empty pattern is a no-op.
    Replace {
        /// The substring to search for. Matched literally, left to
        /// right, non-overlapping.
        pattern: String,
        /// The string to substitute for every match.
        content: String,
    },
    /// Regex-driven substitution: every non-overlapping match of
    /// [`Self::ReplaceRegex::pattern`] (compiled as a `fancy-regex`
    /// regular expression) is replaced with
    /// [`Self::ReplaceRegex::content`].
    ///
    /// # Runtime behaviour
    ///
    /// The pattern is compiled on every call to [`normalize`]. That
    /// is convenient for a variant whose value is typically constructed
    /// from a `tokenizer.json` blob (compile-once amortises poorly when
    /// the runtime `Normalizer` value is dropped after a single encode
    /// or when the caller ships several distinct patterns); callers who
    /// want the compile cost hoisted out of a hot encode loop should
    /// build a `fancy_regex::Regex` themselves and drive it directly.
    ///
    /// A pattern that fails to compile causes [`normalize`] to fall
    /// back to passthrough — the same "infallible runtime" contract
    /// [`Self::Precompiled`] uses for a malformed base64 payload.
    /// Callers that want to fail fast on a bad pattern should validate
    /// via `fancy_regex::Regex::new` before materialising the variant.
    ///
    /// The `content` string is used verbatim; `fancy-regex` capture
    /// group references (`$1`, `${name}`) are **not** interpreted —
    /// this matches Hugging Face's own `Replace` normalizer, which
    /// also does a literal substitution of the whole matched region.
    /// An empty pattern (`""`) matches at every position and would
    /// interleave `content` between every character; this is what
    /// the underlying regex engine reports, and no special-cased
    /// early return is taken for it.
    ReplaceRegex {
        /// The regex source, in `fancy-regex` dialect. Recompiled on
        /// every call to [`normalize`].
        pattern: String,
        /// The literal string to substitute for every match. Capture
        /// group back-references are not interpreted.
        content: String,
    },
    /// Trim runs of ASCII / Unicode whitespace from the input.
    ///
    /// Delegates to [`str::trim_start`] / [`str::trim_end`], which
    /// walks each leading/trailing scalar's `White_Space` property —
    /// the same rule HF's own strip normalizer uses.
    Strip {
        /// If `true`, trim leading whitespace.
        left: bool,
        /// If `true`, trim trailing whitespace.
        right: bool,
    },
    /// Prepend a fixed literal to the input.
    ///
    /// `SentencePiece`'s canonical `▁` prefix (informally, the "space
    /// bar" mark) is expressed with `Prepend { prepend: "▁".into() }`.
    Prepend {
        /// The literal string to prepend.
        prepend: String,
    },
    /// Compose several normalizers, left to right.
    ///
    /// Each child sees the output of every preceding child. An empty
    /// sequence is a no-op.
    Sequence(Vec<Normalizer>),
    /// The classic BERT normalizer — the shape shipped by BERT,
    /// `DistilBERT`, `ELECTRA`, and every other `WordPiece`-family
    /// tokenizer.json that predates the current `Sequence`-of-atoms
    /// idiom.
    ///
    /// Composes four independently-toggleable passes, applied in this
    /// order:
    ///
    /// 1. **`clean_text`** — drop NUL (`U+0000`), the Unicode
    ///    replacement character (`U+FFFD`), and every scalar whose
    ///    Unicode `General_Category` starts with `C` (i.e. `Cc`
    ///    control, `Cf` format, `Co` private-use, `Cn` unassigned —
    ///    `Cs` surrogate is unreachable in a Rust `char`) *except*
    ///    tab / newline / carriage return; then replace every
    ///    remaining whitespace scalar with a single ASCII space (which
    ///    lets the subsequent BERT pre-tokenizer handle any run of
    ///    whitespace uniformly). Matches HF's
    ///    [`is_control`][hf-is-control] predicate exactly — the `Cf`
    ///    coverage is what strips zero-width joiner / non-joiner /
    ///    space (`U+200B..=U+200D`, `U+FEFF`) and the soft-hyphen
    ///    (`U+00AD`), all of which the reference tokenizer drops.
    ///
    ///    [hf-is-control]: https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/normalizers/bert.rs
    /// 2. **`handle_chinese_chars`** — pad every CJK / Han codepoint
    ///    with an ASCII space on each side, so the downstream
    ///    pre-tokenizer treats each Han character as its own "word".
    ///    The ranges checked are the classic HF set: CJK Unified
    ///    Ideographs (`U+4E00..=U+9FFF`), Extensions A / B / C / D / E,
    ///    CJK Compatibility Ideographs, and CJK Compatibility
    ///    Ideographs Supplement.
    /// 3. **`strip_accents`** — NFD-decompose the input, then drop
    ///    every scalar whose `General_Category` is `Mn` (Non-Spacing
    ///    Mark). Matches HF's reference `strip_accents`, which calls
    ///    [`unicode_categories::UnicodeCategories::is_mark_nonspacing`][hf-strip]
    ///    on the same NFD stream. The filter is deliberately not a
    ///    "Latin accents only" whitelist and not a
    ///    canonical-combining-class filter — the vocabularies of
    ///    every BERT-family checkpoint (bert-base-uncased,
    ///    distilbert-base-uncased, ...) are built against the exact
    ///    Mn-stripped surface HF emits, so any narrower filter
    ///    silently misses vocab entries on non-Latin scripts (e.g.
    ///    Devanagari matras U+094D / U+0947 / U+0941 are all Mn and
    ///    must be dropped; U+093F / U+093E are Mc and must survive).
    ///    When `strip_accents` is `None`, HF defaults it to the value
    ///    of `lowercase` — the same behaviour is reproduced here.
    ///
    ///    [hf-strip]: https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/normalizers/bert.rs
    /// 4. **`lowercase`** — Unicode-aware lower-casing via
    ///    [`str::to_lowercase`], matching [`Normalizer::Lowercase`].
    ///
    /// Every field defaults to `true` when materialised from a
    /// tokenizer.json blob, matching HF's own on-disk defaults.
    Bert {
        /// Enable the control-character / whitespace cleanup pass.
        clean_text: bool,
        /// Enable the CJK-character spacing pass.
        handle_chinese_chars: bool,
        /// Enable the accent-stripping pass. `None` defers to
        /// [`Self::Bert::lowercase`] — HF's own default rule.
        strip_accents: Option<bool>,
        /// Enable the lower-casing pass.
        lowercase: bool,
    },
    /// `SentencePiece`'s "Precompiled" charsmap normalizer — the shape
    /// used by every Llama, Mistral, T5, and XLM-RoBERTa
    /// `tokenizer.json` we have seen. The wrapped
    /// [`Self::Precompiled::charsmap_base64`] is the base64-encoded
    /// serialized character-remapping trie as `SentencePiece` stores it
    /// on disk.
    ///
    /// # Runtime behaviour
    ///
    /// The base64 payload is decoded on each call to [`normalize`]
    /// and applied via [`precompiled::PrecompiledNormalizer`] —
    /// the Darts / `DoubleArray` trie is walked left-to-right against
    /// the input, and every longest prefix match is replaced with the
    /// leaf's NUL-terminated replacement string. Unmatched positions
    /// pass one UTF-8 scalar through verbatim. Callers that hit this
    /// path repeatedly should build a [`PrecompiledNormalizer`]
    /// directly and cache it — the per-call decode is quadratic in
    /// the number of calls, not in the input length.
    ///
    /// If the payload does not decode as valid base64 or does not
    /// conform to the charsmap wire format (truncated header, trie
    /// size not a multiple of 4, trie overrun) the runtime falls back
    /// to passthrough — the pre-existing landing shipped placeholder
    /// base64 strings in its tests, and downstream callers who feed
    /// literal `Normalizer::Precompiled` values (rather than routing
    /// through the HF loader) rely on the runtime being infallible.
    /// Callers that want to fail fast on a bad payload should validate
    /// via [`PrecompiledNormalizer::from_base64_charsmap`] before
    /// materialising the variant.
    Precompiled {
        /// The raw base64 payload from the source `tokenizer.json`,
        /// preserved verbatim for round-trip inspection. Decoded on
        /// demand by [`normalize`].
        charsmap_base64: String,
    },
}

/// Apply `normalizer` to `text`, returning the normalized output.
///
/// This is a pure function of its inputs; every call with the same
/// arguments returns the same [`String`]. The output owns its
/// contents — normalization never returns a borrowed slice because
/// even the "no-op" variants may be composed with a mutating sibling
/// in a [`Normalizer::Sequence`].
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_hf::normalizer::{normalize, Normalizer};
///
/// // Precomposed "é" (U+00E9) and decomposed "é" (U+0065 U+0301)
/// // both normalize to the same NFC output.
/// let composed = "\u{00E9}";
/// let decomposed = "e\u{0301}";
/// assert_eq!(normalize(composed, &Normalizer::Nfc), normalize(decomposed, &Normalizer::Nfc));
/// ```
#[must_use]
pub fn normalize(text: &str, normalizer: &Normalizer) -> String {
    match normalizer {
        Normalizer::Nfc => text.nfc().collect(),
        Normalizer::Nfd => text.nfd().collect(),
        Normalizer::Nfkc => text.nfkc().collect(),
        Normalizer::Nfkd => text.nfkd().collect(),
        Normalizer::Lowercase => text.to_lowercase(),
        Normalizer::Replace { pattern, content } => {
            if pattern.is_empty() {
                return text.into();
            }
            let mut out = String::with_capacity(text.len());
            let mut rest = text;
            while let Some(pos) = rest.find(pattern.as_str()) {
                out.push_str(&rest[..pos]);
                out.push_str(content);
                rest = &rest[pos + pattern.len()..];
            }
            out.push_str(rest);
            out
        }
        Normalizer::ReplaceRegex { pattern, content } => replace_regex(text, pattern, content),
        Normalizer::Strip { left, right } => match (*left, *right) {
            (true, true) => text.trim().into(),
            (true, false) => text.trim_start().into(),
            (false, true) => text.trim_end().into(),
            (false, false) => text.into(),
        },
        Normalizer::Prepend { prepend } => {
            let mut out = String::with_capacity(prepend.len() + text.len());
            out.push_str(prepend);
            out.push_str(text);
            out
        }
        Normalizer::Sequence(children) => {
            // Fold left-to-right. Avoid one clone per no-op child by
            // treating the head specially — the first child runs on
            // the caller's `&str`, every later child on the produced
            // `String`.
            let mut it = children.iter();
            let Some(first) = it.next() else {
                return text.into();
            };
            let mut cur = normalize(text, first);
            for step in it {
                cur = normalize(&cur, step);
            }
            cur
        }
        Normalizer::Precompiled { charsmap_base64 } => {
            // Decode-per-call because the variant only stores the
            // raw base64 (kept as-is so existing typed-value
            // roundtrips through `PartialEq` and callers who need
            // introspection continue to work). Callers on a hot path
            // should build a `PrecompiledNormalizer` up front and
            // apply it directly. When the payload is malformed
            // (invalid base64, truncated header, ...) fall back to
            // passthrough — legacy tests and downstream loaders
            // predating this landing rely on this being infallible.
            match PrecompiledNormalizer::from_base64_charsmap(charsmap_base64) {
                Ok(pc) => pc.normalize(text),
                Err(_) => text.into(),
            }
        }
        Normalizer::Bert {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        } => {
            // Apply each pass in the HF-canonical order:
            // clean_text → handle_chinese_chars → strip_accents →
            // lowercase. Every pass is independently toggleable and
            // owns its output — the passes below allocate a fresh
            // `String` even for the no-op fall-through so the return
            // type stays uniform.
            let mut cur: String = text.into();
            if *clean_text {
                cur = bert_clean_text(&cur);
            }
            if *handle_chinese_chars {
                cur = bert_handle_chinese_chars(&cur);
            }
            // HF's rule: when `strip_accents` is not set, default it to
            // `lowercase`. This mirrors upstream exactly.
            let do_strip = strip_accents.unwrap_or(*lowercase);
            if do_strip {
                cur = bert_strip_accents(&cur);
            }
            if *lowercase {
                cur = cur.to_lowercase();
            }
            cur
        }
    }
}

/// Apply a `fancy-regex` pattern to `text`, replacing every
/// non-overlapping match with `content`.
///
/// The pattern is compiled on every call — see
/// [`Normalizer::ReplaceRegex`] for the rationale. A pattern that
/// fails to compile, or a runtime match error mid-iteration, produces
/// a passthrough (the accumulated output up to the failure, plus the
/// remainder of the input verbatim). This matches the "infallible
/// runtime" contract [`Normalizer::Precompiled`] uses for a malformed
/// charsmap payload.
fn replace_regex(text: &str, pattern: &str, content: &str) -> String {
    let Ok(re) = Regex::new(pattern) else {
        return text.into();
    };
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for m in re.find_iter(text) {
        let Ok(m) = m else {
            // Mid-iteration engine error: fall back to appending the
            // rest of the input verbatim, preserving whatever
            // substitutions ran cleanly before the failure.
            break;
        };
        // `fancy-regex` can report zero-length matches (e.g. from
        // patterns like `""` or `\b`). Advance past them by one byte
        // so the loop terminates; append the raw content once at the
        // match position to mirror upstream's substitute-then-advance
        // behaviour on a zero-width hit.
        let start = m.start();
        let end = m.end();
        if start < cursor {
            // Overlapping match — `fancy-regex` should never emit
            // one, but if it does, skip rather than double-copy.
            continue;
        }
        out.push_str(&text[cursor..start]);
        out.push_str(content);
        cursor = end;
        if end == start {
            // Zero-width match: step one Unicode scalar forward so
            // the driver makes progress. Copy the character we
            // stepped over into the output so the input's characters
            // survive.
            if let Some(ch) = text[cursor..].chars().next() {
                out.push(ch);
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    out.push_str(&text[cursor..]);
    out
}

/// BERT's `clean_text` pass.
///
/// Drops NUL (`U+0000`), the Unicode replacement character (`U+FFFD`),
/// and every other Unicode "control" scalar (see [`is_bert_control`])
/// except tab, newline, and carriage return (which are treated as
/// whitespace, not controls); every remaining whitespace scalar is
/// replaced with a single ASCII space.
fn bert_clean_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == '\0' || c == '\u{FFFD}' || is_bert_control(c) {
            continue;
        }
        if is_bert_whitespace(c) {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// HF's "is a control character for BERT" predicate. Tab, newline,
/// and carriage return are explicitly *not* controls here — they are
/// treated as whitespace by [`bert_clean_text`].
///
/// Matches HF's reference implementation, which filters every scalar
/// whose `General_Category` starts with `C` (via
/// `unicode_categories::UnicodeCategories::is_other`): `Cc` (Control),
/// `Cf` (Format), `Co` (Private Use), and `Cn` (Unassigned). `Cs`
/// (Surrogate) is technically in the set but is unreachable in a Rust
/// [`char`]. The `Cf` coverage is what strips the zero-width scalars
/// (`U+200B..=U+200D`, `U+FEFF`) and the soft-hyphen `U+00AD` that
/// downstream vocabularies were built to ignore.
fn is_bert_control(c: char) -> bool {
    if c == '\t' || c == '\n' || c == '\r' {
        return false;
    }
    matches!(
        get_general_category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
    )
}

/// HF's "is whitespace for BERT" predicate. Tab / newline / CR are
/// whitespace; every general-category-Zs scalar (and any other
/// scalar `char::is_whitespace` recognises) is whitespace.
fn is_bert_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r') || c.is_whitespace()
}

/// BERT's `handle_chinese_chars` pass.
///
/// Pads every CJK / Han codepoint with an ASCII space on each side so
/// the downstream pre-tokenizer treats each Han character as its own
/// "word".
fn bert_handle_chinese_chars(input: &str) -> String {
    // Worst-case output is `3 * input.len()` (one three-byte char
    // becoming " {char} "), but the common case adds only one or two
    // padding bytes per Han char — start at `input.len()` and grow.
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if is_chinese_char(c) {
            out.push(' ');
            out.push(c);
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// HF's `is_chinese_char` predicate — the eight Han-script Unicode
/// ranges the reference `BertNormalizer` treats as "Chinese".
fn is_chinese_char(c: char) -> bool {
    let cp = u32::from(c);
    matches!(cp,
        0x4E00..=0x9FFF
        | 0x3400..=0x4DBF
        | 0x2_0000..=0x2_A6DF
        | 0x2_A700..=0x2_B73F
        | 0x2_B740..=0x2_B81F
        | 0x2_B820..=0x2_CEAF
        | 0xF900..=0xFAFF
        | 0x2_F800..=0x2_FA1F
    )
}

/// BERT's `strip_accents` pass. NFD-decomposes the input then drops
/// every scalar whose `General_Category` is `Mn` (Non-Spacing Mark).
/// The result is left in NFD form, matching HF's own behaviour.
///
/// The filter is deliberately `Mn` and not
/// [`unicode_normalization::char::canonical_combining_class`] `!= 0`:
/// the two sets diverge on Indic-script matras (Devanagari VOWEL SIGN
/// U `U+0941` and VOWEL SIGN E `U+0947` are both `Mn` but have
/// `ccc == 0`), and the BERT-family vocabularies were built against
/// HF's exact `Mn`-stripped surface. A `ccc`-based filter silently
/// misses those matras, causing `WordPiece` lookups on Devanagari /
/// Bengali / Thai / ... to fall off vocab and collapse to `[UNK]`.
/// See HF's reference:
/// <https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/normalizers/bert.rs>.
///
/// `Mc` (Spacing Combining Mark) scalars — e.g. Devanagari VOWEL SIGN
/// I `U+093F` and VOWEL SIGN AA `U+093E` — are preserved, matching
/// HF's `is_mark_nonspacing`-only filter.
fn bert_strip_accents(input: &str) -> String {
    input
        .nfd()
        .filter(|c| !matches!(get_general_category(*c), GeneralCategory::NonspacingMark))
        .collect()
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    /// Precomposed "café" (`c a f é` with U+00E9 for the é).
    const CAFE_PRECOMPOSED: &str = "caf\u{00E9}";
    /// Decomposed "café" (`c a f e` + combining acute U+0301).
    const CAFE_DECOMPOSED: &str = "cafe\u{0301}";

    #[test]
    fn nfc_maps_decomposed_to_precomposed() {
        assert_eq!(
            normalize(CAFE_DECOMPOSED, &Normalizer::Nfc),
            CAFE_PRECOMPOSED
        );
    }

    #[test]
    fn nfd_maps_precomposed_to_decomposed() {
        assert_eq!(
            normalize(CAFE_PRECOMPOSED, &Normalizer::Nfd),
            CAFE_DECOMPOSED
        );
    }

    #[test]
    fn nfc_is_idempotent_on_already_composed_input() {
        let out = normalize(CAFE_PRECOMPOSED, &Normalizer::Nfc);
        assert_eq!(out, CAFE_PRECOMPOSED);
        assert_eq!(normalize(&out, &Normalizer::Nfc), out);
    }

    #[test]
    fn nfd_is_idempotent_on_already_decomposed_input() {
        let out = normalize(CAFE_DECOMPOSED, &Normalizer::Nfd);
        assert_eq!(out, CAFE_DECOMPOSED);
        assert_eq!(normalize(&out, &Normalizer::Nfd), out);
    }

    #[test]
    fn nfc_and_nfd_agree_after_re_composition() {
        // Round-trip: NFD then NFC recovers NFC.
        let via_nfd = normalize(CAFE_PRECOMPOSED, &Normalizer::Nfd);
        assert_eq!(normalize(&via_nfd, &Normalizer::Nfc), CAFE_PRECOMPOSED);
    }

    #[test]
    fn nfkc_normalizes_compatibility_ligature() {
        // U+FB01 LATIN SMALL LIGATURE FI decomposes under NFKC to "fi".
        let ligature = "\u{FB01}";
        assert_eq!(normalize(ligature, &Normalizer::Nfkc), "fi");
    }

    #[test]
    fn nfkd_normalizes_compatibility_ligature() {
        // Same input decomposes under NFKD to "fi" (two ASCII scalars).
        let ligature = "\u{FB01}";
        assert_eq!(normalize(ligature, &Normalizer::Nfkd), "fi");
    }

    #[test]
    fn nfkc_maps_full_width_digit_to_ascii() {
        // U+FF11 FULLWIDTH DIGIT ONE → ASCII "1" under compatibility.
        let full_width = "\u{FF11}";
        assert_eq!(normalize(full_width, &Normalizer::Nfkc), "1");
        // NFC does *not* touch compatibility variants.
        assert_eq!(normalize(full_width, &Normalizer::Nfc), full_width);
    }

    #[test]
    fn nfd_expands_composed_hangul_syllable() {
        // U+AC00 HANGUL SYLLABLE GA decomposes into two jamo:
        // U+1100 CHOSEONG KIYEOK + U+1161 JUNGSEONG A. Verified against
        // the Unicode normalisation reference tables.
        let ga = "\u{AC00}";
        assert_eq!(normalize(ga, &Normalizer::Nfd), "\u{1100}\u{1161}");
        // And NFC recomposes.
        let decomposed = "\u{1100}\u{1161}";
        assert_eq!(normalize(decomposed, &Normalizer::Nfc), ga);
    }

    #[test]
    fn lowercase_maps_ascii_and_extended() {
        assert_eq!(normalize("Hello", &Normalizer::Lowercase), "hello");
        // German capital eszett (`ß`) lower-cases via to_lowercase to
        // itself. Sharp-S upper-case ("ẞ" U+1E9E) exists but is off the
        // common path — verify a representative sample.
        assert_eq!(normalize("CAFÉ", &Normalizer::Lowercase), "café");
    }

    #[test]
    fn replace_substitutes_all_non_overlapping_matches() {
        let n = Normalizer::Replace {
            pattern: "a".to_string(),
            content: "AB".to_string(),
        };
        assert_eq!(normalize("banana", &n), "bABnABnAB");
    }

    #[test]
    fn replace_with_empty_pattern_is_noop() {
        let n = Normalizer::Replace {
            pattern: String::new(),
            content: "X".to_string(),
        };
        assert_eq!(normalize("abc", &n), "abc");
    }

    #[test]
    fn replace_regex_collapses_whitespace_runs() {
        // Canonical simple use — collapse any run of Unicode
        // whitespace to a single ASCII space.
        let n = Normalizer::ReplaceRegex {
            pattern: r"\s+".to_string(),
            content: " ".to_string(),
        };
        assert_eq!(normalize("a  b\t\tc\n\nd", &n), "a b c d");
    }

    #[test]
    fn replace_regex_deberta_v3_double_space_pattern() {
        // The exact SentencePiece-conversion pattern DeBERTa-v3 and
        // mDeBERTa-v3 ship: replace any run of two or more ASCII
        // spaces with a single ASCII space. Verified against the
        // upstream `transformers.convert_slow_tokenizer` output.
        let n = Normalizer::ReplaceRegex {
            pattern: r" {2,}".to_string(),
            content: " ".to_string(),
        };
        assert_eq!(normalize("hello    world", &n), "hello world");
        // Runs of length one are left alone.
        assert_eq!(normalize("hello world", &n), "hello world");
        // Interior and terminal runs both collapse.
        assert_eq!(normalize("a   b   ", &n), "a b ");
    }

    #[test]
    fn replace_regex_with_no_matches_passes_through() {
        let n = Normalizer::ReplaceRegex {
            pattern: r"\d+".to_string(),
            content: "N".to_string(),
        };
        assert_eq!(normalize("hello world", &n), "hello world");
    }

    #[test]
    fn replace_regex_replaces_multiple_matches() {
        let n = Normalizer::ReplaceRegex {
            pattern: r"\d+".to_string(),
            content: "N".to_string(),
        };
        assert_eq!(normalize("a12b34c5", &n), "aNbNcN");
    }

    #[test]
    fn replace_regex_content_is_literal_not_a_template() {
        // Capture-group back-references (`$1`) are not interpreted —
        // they are emitted verbatim, matching HF's own `Replace`
        // behaviour.
        let n = Normalizer::ReplaceRegex {
            pattern: r"(\d)".to_string(),
            content: "<$1>".to_string(),
        };
        assert_eq!(normalize("a1b2", &n), "a<$1>b<$1>");
    }

    #[test]
    fn replace_regex_invalid_pattern_falls_back_to_passthrough() {
        // A syntactically invalid pattern should not panic — the
        // runtime returns the input verbatim, matching the
        // Precompiled variant's malformed-payload contract.
        let n = Normalizer::ReplaceRegex {
            pattern: r"[unterminated".to_string(),
            content: "X".to_string(),
        };
        assert_eq!(normalize("hello", &n), "hello");
    }

    #[test]
    fn replace_regex_composes_inside_sequence() {
        // The DeBERTa-v3 normalizer stack starts with a `Replace`
        // step; verify the ReplaceRegex variant composes correctly
        // when wrapped in a Sequence with another step.
        let n = Normalizer::Sequence(vec![
            Normalizer::ReplaceRegex {
                pattern: r" {2,}".to_string(),
                content: " ".to_string(),
            },
            Normalizer::Lowercase,
        ]);
        assert_eq!(normalize("HELLO    WORLD", &n), "hello world");
    }

    #[test]
    fn strip_left_only() {
        let n = Normalizer::Strip {
            left: true,
            right: false,
        };
        assert_eq!(normalize("  hi  ", &n), "hi  ");
    }

    #[test]
    fn strip_right_only() {
        let n = Normalizer::Strip {
            left: false,
            right: true,
        };
        assert_eq!(normalize("  hi  ", &n), "  hi");
    }

    #[test]
    fn strip_both_sides() {
        let n = Normalizer::Strip {
            left: true,
            right: true,
        };
        assert_eq!(normalize("  hi  ", &n), "hi");
    }

    #[test]
    fn strip_neither_side_is_noop() {
        let n = Normalizer::Strip {
            left: false,
            right: false,
        };
        assert_eq!(normalize("  hi  ", &n), "  hi  ");
    }

    #[test]
    fn prepend_adds_leading_literal() {
        let n = Normalizer::Prepend {
            prepend: "▁".to_string(),
        };
        assert_eq!(normalize("hello", &n), "▁hello");
    }

    #[test]
    fn sequence_applies_children_left_to_right() {
        // Lowercase then NFC.
        let n = Normalizer::Sequence(vec![Normalizer::Lowercase, Normalizer::Nfc]);
        assert_eq!(normalize("CAFE\u{0301}", &n), "café");
    }

    #[test]
    fn sequence_of_empty_is_noop() {
        let n = Normalizer::Sequence(vec![]);
        assert_eq!(normalize("hello", &n), "hello");
    }

    // ---------------------------------------------------------------
    // BertNormalizer
    // ---------------------------------------------------------------

    /// Convenience — the BERT default (`clean_text: true,
    /// handle_chinese_chars: true, strip_accents: None,
    /// lowercase: true`) matches what a bare
    /// `{"type": "BertNormalizer"}` deserialises to.
    fn bert_default() -> Normalizer {
        Normalizer::Bert {
            clean_text: true,
            handle_chinese_chars: true,
            strip_accents: None,
            lowercase: true,
        }
    }

    #[test]
    fn bert_lowercases_and_strips_accents_by_default() {
        // `strip_accents: None` defers to `lowercase: true`, so the
        // combining acute is dropped and `É` becomes `e`.
        assert_eq!(normalize("café", &bert_default()), "cafe");
        assert_eq!(normalize("CAFÉ", &bert_default()), "cafe");
        // The decomposed form reaches the same output.
        assert_eq!(normalize("cafe\u{0301}", &bert_default()), "cafe");
    }

    #[test]
    fn bert_strip_accents_none_defers_to_lowercase_false() {
        // With `strip_accents: None` and `lowercase: false`, the
        // default resolves to `strip_accents: false` — the accent
        // survives.
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: false,
            strip_accents: None,
            lowercase: false,
        };
        assert_eq!(normalize("café", &n), "café");
    }

    #[test]
    fn bert_strip_accents_explicit_true_overrides_lowercase_false() {
        // Explicit `strip_accents: Some(true)` wins over the default
        // rule even when lowercase is off — `É` decomposes into `E` +
        // combining acute and the acute is dropped, leaving `E`.
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: false,
            strip_accents: Some(true),
            lowercase: false,
        };
        assert_eq!(normalize("CAFÉ", &n), "CAFE");
    }

    #[test]
    fn bert_handle_chinese_chars_pads_han_scalars() {
        // "你" (U+4F60) and "好" (U+597D) are both in the CJK Unified
        // Ideographs block; the world/ASCII characters are not padded.
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: true,
            strip_accents: Some(false),
            lowercase: false,
        };
        assert_eq!(normalize("你好world", &n), " 你  好 world");
    }

    #[test]
    fn bert_clean_text_drops_nul_and_replaces_tab_and_newline() {
        // `\0` and the U+FFFD replacement char are dropped; `\t` /
        // `\n` become a single ASCII space each.
        let n = Normalizer::Bert {
            clean_text: true,
            handle_chinese_chars: false,
            strip_accents: Some(false),
            lowercase: false,
        };
        assert_eq!(normalize("a\0b\tc\nd\u{FFFD}e", &n), "ab c de");
    }

    #[test]
    fn bert_clean_text_drops_other_c0_controls() {
        // `\u{0001}` (SOH) — a C0 control that is neither tab / newline
        // / CR nor NUL / FFFD — is dropped as a control character.
        let n = Normalizer::Bert {
            clean_text: true,
            handle_chinese_chars: false,
            strip_accents: Some(false),
            lowercase: false,
        };
        assert_eq!(normalize("a\u{0001}b", &n), "ab");
    }

    #[test]
    fn bert_clean_text_drops_zero_width_and_soft_hyphen() {
        // Every scalar with `General_Category` starting with `C` is a
        // control for BERT — that includes `Cf` (Format), which covers
        // the zero-width joiner / non-joiner / space and the
        // soft-hyphen. The reference HF tokenizer strips them; this
        // pins the parity.
        let n = Normalizer::Bert {
            clean_text: true,
            handle_chinese_chars: false,
            strip_accents: Some(false),
            lowercase: false,
        };
        // The invisible-chars fixture case from the conformance corpus,
        // condensed: expected output has every U+200B / U+200C /
        // U+200D and the soft-hyphen (U+00AD) removed.
        assert_eq!(
            normalize("he\u{200B}llo\u{200C}world\u{200D}!\u{00AD}end", &n),
            "helloworld!end",
        );
        // U+FEFF (Zero-Width No-Break Space / BOM) is also `Cf` and
        // must be stripped.
        assert_eq!(normalize("\u{FEFF}hello", &n), "hello");
    }

    #[test]
    fn bert_strip_accents_removes_latin_diacritics() {
        // Regression: `caf\u{00E9}` still round-trips through NFD →
        // Mn-filter → NFD as `"cafe"`, matching the historical
        // canonical-combining-class filter's behaviour on Latin scripts.
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: false,
            strip_accents: Some(true),
            lowercase: false,
        };
        assert_eq!(normalize("caf\u{00E9}", &n), "cafe");
        assert_eq!(normalize("CAF\u{00C9}", &n), "CAFE");
        // Already-decomposed form reaches the same output.
        assert_eq!(normalize("cafe\u{0301}", &n), "cafe");
    }

    #[test]
    fn bert_strip_accents_preserves_devanagari_matras() {
        // "नमस्ते दुनिया" — the hindi-devanagari fixture case.
        // HF's Mn-only filter strips VIRAMA (U+094D), VOWEL SIGN E
        // (U+0947), and VOWEL SIGN U (U+0941) — all `Mn` — leaving
        // "नमसत दनिया". VOWEL SIGN I (U+093F) and VOWEL SIGN AA
        // (U+093E) are `Mc` (Spacing Combining Mark) and MUST
        // survive; the bert-base-uncased vocab only has entries for
        // this Mn-stripped surface.
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: false,
            strip_accents: Some(true),
            lowercase: false,
        };
        assert_eq!(
            normalize("\u{0928}\u{092E}\u{0938}\u{094D}\u{0924}\u{0947}", &n),
            "\u{0928}\u{092E}\u{0938}\u{0924}",
        );
        assert_eq!(
            normalize("\u{0926}\u{0941}\u{0928}\u{093F}\u{092F}\u{093E}", &n),
            "\u{0926}\u{0928}\u{093F}\u{092F}\u{093E}",
        );
    }

    #[test]
    fn bert_all_features_disabled_is_identity() {
        let n = Normalizer::Bert {
            clean_text: false,
            handle_chinese_chars: false,
            strip_accents: Some(false),
            lowercase: false,
        };
        assert_eq!(normalize("Hello, café! 你好", &n), "Hello, café! 你好");
    }

    #[test]
    fn precompiled_normalizer_falls_back_on_invalid_charsmap() {
        // "AAAA" decodes to three zero bytes — shorter than the
        // Precompiled charsmap's 4-byte trie-size header. The
        // runtime falls back to passthrough on malformed payloads;
        // this test pins that fallback so downstream callers with
        // legacy placeholder blobs keep working.
        let n = Normalizer::Precompiled {
            charsmap_base64: "AAAA".to_string(),
        };
        assert_eq!(normalize("hello world", &n), "hello world");
        assert_eq!(normalize("café \u{FF11}", &n), "café \u{FF11}");
        assert_eq!(normalize("", &n), "");
    }

    #[test]
    fn precompiled_normalizer_applies_hand_crafted_charsmap() {
        // Build the smallest possible valid charsmap: a single-byte
        // key "a" → replacement "AB". The trie shape and node
        // encoding are documented in the `precompiled` module's
        // hand-crafted trie test. Rebuilt here via the encoder
        // helper to exercise the full base64 -> parse -> apply
        // chain through the outer `normalize()` dispatch.
        let mut trie = vec![0u32; 98];
        trie[0] = 0x400; // root: offset=1
        trie[96] = 0x561; // 'a' transition: label=0x61, has_leaf, offset=1
        trie[97] = 0x8000_0000; // leaf: value=0
        let b64 = precompiled::encode_charsmap_for_tests(&trie, b"AB\0");
        let n = Normalizer::Precompiled {
            charsmap_base64: b64,
        };
        assert_eq!(normalize("banana", &n), "bABnABnAB");
    }

    #[test]
    fn sequence_nesting_is_associative() {
        // Sequence(A, Sequence(B, C)) == Sequence(A, B, C) semantically.
        let inner = Normalizer::Sequence(vec![
            Normalizer::Lowercase,
            Normalizer::Strip {
                left: true,
                right: true,
            },
        ]);
        let flat = Normalizer::Sequence(vec![
            Normalizer::Nfc,
            Normalizer::Lowercase,
            Normalizer::Strip {
                left: true,
                right: true,
            },
        ]);
        let nested = Normalizer::Sequence(vec![Normalizer::Nfc, inner]);
        let input = "  CAFE\u{0301}  ";
        assert_eq!(normalize(input, &nested), normalize(input, &flat));
    }
}
