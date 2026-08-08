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
//! * [`Normalizer::Replace`] — literal-string substitution. HF's spec
//!   also supports regex patterns; the regex form is deferred (see
//!   the module-level "Deferred" list).
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
//! # Deferred variants
//!
//! * `Precompiled` — `SentencePiece`'s compiled char-mapping table.
//!   Requires shipping the (many-megabyte) precompiled binary format;
//!   out of scope for the tokenizer.json BPE landing.
//! * `Nmt` — the Marian NMT normalizer.
//! * `Replace` with a `Regex` pattern (rather than a literal). Callers
//!   who need this can approximate with a `Sequence` of literal
//!   `Replace` steps for now.
//! * Custom callable normalizers.
//!
//! Deferred variants surface at [`crate::hf::to_bpe_tokenizer`] time as
//! [`crate::hf::HfConversionError::UnsupportedNormalizer`] with the
//! offending type name in the message.

use alloc::string::String;
use alloc::vec::Vec;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

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
    ///    replacement character (`U+FFFD`), and every other Unicode
    ///    "control" scalar *except* tab / newline / carriage return;
    ///    then replace every remaining whitespace scalar with a single
    ///    ASCII space (which lets the subsequent BERT pre-tokenizer
    ///    handle any run of whitespace uniformly).
    /// 2. **`handle_chinese_chars`** — pad every CJK / Han codepoint
    ///    with an ASCII space on each side, so the downstream
    ///    pre-tokenizer treats each Han character as its own "word".
    ///    The ranges checked are the classic HF set: CJK Unified
    ///    Ideographs (`U+4E00..=U+9FFF`), Extensions A / B / C / D / E,
    ///    CJK Compatibility Ideographs, and CJK Compatibility
    ///    Ideographs Supplement.
    /// 3. **`strip_accents`** — NFD-decompose the input, then drop
    ///    every combining mark (checked via
    ///    [`unicode_normalization::char::canonical_combining_class`]
    ///    `!= 0`, which is the closest zero-table proxy for HF's own
    ///    `Mn`-only filter and coincides on every Latin-script accent
    ///    a typical caller cares about). When `strip_accents` is
    ///    `None`, HF defaults it to the value of `lowercase` — the
    ///    same behaviour is reproduced here.
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
/// use stringcheese_tokenizer_bpe::normalizer::{normalize, Normalizer};
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

/// BERT's `clean_text` pass.
///
/// Drops NUL (`U+0000`), the Unicode replacement character (`U+FFFD`),
/// and every other Unicode "control" scalar except tab, newline, and
/// carriage return (which are treated as whitespace, not controls);
/// every remaining whitespace scalar is replaced with a single ASCII
/// space.
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
fn is_bert_control(c: char) -> bool {
    if c == '\t' || c == '\n' || c == '\r' {
        return false;
    }
    // `char::is_control` covers Cc (the C0 and C1 control blocks).
    // HF's own predicate is more permissive (it also catches Cf / Co /
    // Cs), but every non-Cc scalar we might see in practice is either
    // handled by `is_bert_whitespace` below (Zs) or is a normal
    // letter/mark. Cc coverage is sufficient for round-tripping the
    // BERT-canonical inputs.
    c.is_control()
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
/// every combining mark (canonical combining class ≠ 0). The result
/// is left in NFD form, matching HF's own behaviour.
fn bert_strip_accents(input: &str) -> String {
    input
        .nfd()
        .filter(|c| canonical_combining_class(*c) == 0)
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
