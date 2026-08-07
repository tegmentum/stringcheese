//! [`IdentifierTokenizer`] — split code-shaped identifiers.
//!
//! A segmenter for code-adjacent workloads (searching for identifier
//! matches, comparing symbol names across languages, computing edit
//! distances between renames). Splits identifiers along the boundaries
//! programmers use to compose them:
//!
//! * `camelCase` — split on lower→upper transitions.
//! * `PascalCase` — same as camelCase; the first character being upper
//!   does not change the boundary rules.
//! * `snake_case`, `kebab-case`, `dotted.path` — split on the separator.
//! * `SCREAMING_SNAKE` — split on `_`; the parts stay uppercase.
//! * `XMLHttpRequest` — split at the boundary where a run of uppercase
//!   characters gives way to a Title-case-then-lowercase run (i.e., the
//!   last upper in `XMLH` is actually the start of `Http`).
//!
//! [`IdentifierMode`] controls the strategy: pick a specific split rule,
//! or use [`IdentifierMode::Auto`] to detect and apply whichever
//! separator character the input contains before falling back to the
//! camelCase / acronym rules.

use alloc::vec::Vec;

use crate::traits::{Segment, Segmenter};

/// The identifier-splitting strategy an [`IdentifierTokenizer`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierMode {
    /// Split on `_`. Preserves case within each segment (so both
    /// `snake_case` and `SCREAMING_SNAKE` produce the expected pieces).
    SnakeCase,
    /// Split on `-`. Case is preserved.
    KebabCase,
    /// Split on `.`. Case is preserved.
    DottedPath,
    /// Split on case transitions (`camelCase`, `PascalCase`,
    /// `XMLHttpRequest`). See the crate-level explanation for the
    /// acronym rule.
    CamelCase,
    /// Auto-detect: split on whichever separator character (`_`, `-`,
    /// `.`) is present; if none is present, apply the camelCase rules.
    /// A mix of separators splits on all present separators.
    Auto,
}

/// Splits code-shaped identifiers along programmer-familiar boundaries.
///
/// See [`IdentifierMode`] for the available strategies and the
/// module-level documentation for the acronym-handling rule that
/// distinguishes `XMLHttpRequest` from `Xmlhttprequest`.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{IdentifierMode, IdentifierTokenizer, Segmenter};
///
/// let seg = IdentifierTokenizer::new(IdentifierMode::CamelCase);
/// let parts: Vec<_> = seg.segment("XMLHttpRequest").map(|s| s.text).collect();
/// assert_eq!(parts, ["XML", "Http", "Request"]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct IdentifierTokenizer {
    /// Which splitting strategy to apply.
    pub mode: IdentifierMode,
}

impl IdentifierTokenizer {
    /// Constructs a tokenizer that applies `mode`.
    #[must_use]
    pub const fn new(mode: IdentifierMode) -> Self {
        Self { mode }
    }
}

/// Iterator yielded by [`IdentifierTokenizer::segment`].
///
/// Split points are computed eagerly into a `Vec<(offset, len)>` once —
/// identifier tokenization is a rare fast path and the added `O(n)`
/// scan pays for the substantially simpler traversal logic. If a hot
/// loop ever emerges we can revisit.
#[derive(Debug)]
pub struct IdentifierSegments<'a> {
    input: &'a str,
    spans: Vec<(usize, usize)>,
    cursor: usize,
}

impl<'a> Iterator for IdentifierSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.spans.len() {
            return None;
        }
        let (start, end) = self.spans[self.cursor];
        self.cursor += 1;
        Some(Segment::new(start, &self.input[start..end]))
    }
}

impl Segmenter for IdentifierTokenizer {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = IdentifierSegments<'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        let spans = compute_spans(self.mode, text);
        IdentifierSegments {
            input: text,
            spans,
            cursor: 0,
        }
    }
}

/// Whether `ch` is one of the recognised separator characters.
#[inline]
fn is_any_separator(ch: char) -> bool {
    matches!(ch, '_' | '-' | '.')
}

/// Compute the (byte-start, byte-end) spans for `mode` over `input`.
fn compute_spans(mode: IdentifierMode, input: &str) -> Vec<(usize, usize)> {
    if input.is_empty() {
        return Vec::new();
    }

    // Resolve `Auto`.
    let effective = if mode == IdentifierMode::Auto {
        let has_sep = input.chars().any(is_any_separator);
        if has_sep {
            IdentifierMode::Auto // handled below with per-char check
        } else {
            IdentifierMode::CamelCase
        }
    } else {
        mode
    };

    match effective {
        IdentifierMode::SnakeCase => split_on(input, |c| c == '_'),
        IdentifierMode::KebabCase => split_on(input, |c| c == '-'),
        IdentifierMode::DottedPath => split_on(input, |c| c == '.'),
        IdentifierMode::CamelCase => split_camel(input),
        // Auto with separators present: split on any separator; do NOT
        // apply camel splits, since a caller who wrote `snake_case_name`
        // reasonably expects three parts, not more.
        IdentifierMode::Auto => split_on(input, is_any_separator),
    }
}

/// Split on any character for which `pred` returns true.
fn split_on<P: FnMut(char) -> bool>(input: &str, mut pred: P) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    // Skip leading separators.
    while cursor < input.len() {
        let Some(ch) = input[cursor..].chars().next() else {
            break;
        };
        if pred(ch) {
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    while cursor < input.len() {
        let start = cursor;
        while cursor < input.len() {
            let Some(ch) = input[cursor..].chars().next() else {
                break;
            };
            if pred(ch) {
                break;
            }
            cursor += ch.len_utf8();
        }
        if cursor > start {
            out.push((start, cursor));
        }
        // Skip consecutive separators.
        while cursor < input.len() {
            let Some(ch) = input[cursor..].chars().next() else {
                break;
            };
            if pred(ch) {
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    out
}

/// Split on case transitions, using the acronym rule:
///
/// * lower → upper starts a new segment (`camelCase` → `camel|Case`).
/// * A run of uppers followed by a lower ends the *previous* segment
///   one upper before the transition: `XMLHttpRequest` → `XML|Http|Request`.
/// * A digit-to-letter or letter-to-digit boundary starts a new segment
///   (`utf8Encoder` → `utf|8|Encoder`). This is the tightest rule that
///   still keeps `parse2` as `parse|2` while producing what most tools
///   emit.
fn split_camel(input: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();

    // We look at char-boundary triples of (prev, curr, next).
    // Collect positions of char boundaries (as byte offsets) so we can
    // peek two chars ahead cleanly. `char_indices` gives us `(pos, ch)`.
    let indices: Vec<(usize, char)> = input.char_indices().collect();
    if indices.is_empty() {
        return out;
    }

    let mut seg_start = indices[0].0;
    for i in 1..indices.len() {
        let (pos, curr) = indices[i];
        let (_ppos, prev) = indices[i - 1];

        let mut boundary = false;

        // lower → upper (camelCase boundary)
        if is_lower(prev) && is_upper(curr) {
            boundary = true;
        }

        // digit ↔ letter
        if (prev.is_ascii_digit() && curr.is_alphabetic())
            || (prev.is_alphabetic() && curr.is_ascii_digit())
        {
            boundary = true;
        }

        // Acronym rule: upper → upper, where the *next* char is lower.
        // Prefer inserting the boundary at `curr` (so the previous segment
        // ends before curr).
        if is_upper(prev) && is_upper(curr) {
            if let Some(&(_, next)) = indices.get(i + 1) {
                if is_lower(next) {
                    boundary = true;
                }
            }
        }

        if boundary {
            if pos > seg_start {
                out.push((seg_start, pos));
            }
            seg_start = pos;
        }
    }
    // Final segment.
    if seg_start < input.len() {
        out.push((seg_start, input.len()));
    }
    out
}

#[inline]
fn is_upper(ch: char) -> bool {
    ch.is_uppercase()
}

#[inline]
fn is_lower(ch: char) -> bool {
    ch.is_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::string::ToString;

    fn split(mode: IdentifierMode, input: &str) -> Vec<String> {
        IdentifierTokenizer::new(mode)
            .segment(input)
            .map(|s| s.text.to_string())
            .collect()
    }

    #[test]
    fn camel_case_two_parts() {
        assert_eq!(
            split(IdentifierMode::CamelCase, "camelCase"),
            ["camel", "Case"]
        );
    }

    #[test]
    fn pascal_case_two_parts() {
        assert_eq!(
            split(IdentifierMode::CamelCase, "PascalCase"),
            ["Pascal", "Case"]
        );
    }

    #[test]
    fn xml_http_request_acronym_rule() {
        assert_eq!(
            split(IdentifierMode::CamelCase, "XMLHttpRequest"),
            ["XML", "Http", "Request"]
        );
    }

    #[test]
    fn snake_case_basic() {
        assert_eq!(
            split(IdentifierMode::SnakeCase, "snake_case"),
            ["snake", "case"]
        );
    }

    #[test]
    fn screaming_snake_case_preserves_case() {
        assert_eq!(
            split(IdentifierMode::SnakeCase, "SCREAMING_SNAKE"),
            ["SCREAMING", "SNAKE"]
        );
    }

    #[test]
    fn kebab_case_basic() {
        assert_eq!(
            split(IdentifierMode::KebabCase, "kebab-case-name"),
            ["kebab", "case", "name"]
        );
    }

    #[test]
    fn dotted_path_basic() {
        assert_eq!(
            split(IdentifierMode::DottedPath, "com.example.pkg"),
            ["com", "example", "pkg"]
        );
    }

    #[test]
    fn snake_case_consecutive_underscores_collapse() {
        assert_eq!(
            split(IdentifierMode::SnakeCase, "a__b___c"),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn snake_case_leading_trailing_underscores_dropped() {
        assert_eq!(split(IdentifierMode::SnakeCase, "__x_y__"), ["x", "y"]);
    }

    #[test]
    fn camel_case_single_word_no_split() {
        assert_eq!(split(IdentifierMode::CamelCase, "lowercase"), ["lowercase"]);
        assert_eq!(split(IdentifierMode::CamelCase, "UPPERCASE"), ["UPPERCASE"]);
    }

    #[test]
    fn camel_case_digits() {
        // digit-letter boundary splits.
        assert_eq!(
            split(IdentifierMode::CamelCase, "utf8Encoder"),
            ["utf", "8", "Encoder"]
        );
        assert_eq!(split(IdentifierMode::CamelCase, "parse2"), ["parse", "2"]);
    }

    #[test]
    fn camel_case_acronym_at_end_no_split() {
        // "TokenID" → ["Token", "ID"] — capital I after capital n, next
        // char is capital D (no lower follows), so acronym rule does
        // not fire on the ID→ end. The lower→upper transition inside
        // "Token" gives us the split.
        assert_eq!(split(IdentifierMode::CamelCase, "TokenID"), ["Token", "ID"]);
    }

    #[test]
    fn camel_case_pure_acronym_all_upper() {
        assert_eq!(split(IdentifierMode::CamelCase, "IOError"), ["IO", "Error"]);
    }

    #[test]
    fn auto_detects_snake_case() {
        assert_eq!(
            split(IdentifierMode::Auto, "snake_case_name"),
            ["snake", "case", "name"]
        );
    }

    #[test]
    fn auto_detects_kebab_case() {
        assert_eq!(
            split(IdentifierMode::Auto, "kebab-case-name"),
            ["kebab", "case", "name"]
        );
    }

    #[test]
    fn auto_detects_dotted_path() {
        assert_eq!(split(IdentifierMode::Auto, "a.b.c"), ["a", "b", "c"]);
    }

    #[test]
    fn auto_falls_back_to_camel_case() {
        assert_eq!(split(IdentifierMode::Auto, "camelCase"), ["camel", "Case"]);
    }

    #[test]
    fn auto_mixed_separators() {
        assert_eq!(split(IdentifierMode::Auto, "a_b-c.d"), ["a", "b", "c", "d"]);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(split(IdentifierMode::CamelCase, "").is_empty());
        assert!(split(IdentifierMode::SnakeCase, "").is_empty());
        assert!(split(IdentifierMode::Auto, "").is_empty());
    }

    #[test]
    fn offsets_are_byte_positions() {
        let seg = IdentifierTokenizer::new(IdentifierMode::SnakeCase);
        let out: Vec<_> = seg.segment("hello_world").collect();
        assert_eq!(out[0], Segment::new(0, "hello"));
        assert_eq!(out[1], Segment::new(6, "world"));
    }

    #[test]
    fn camel_case_offsets() {
        let seg = IdentifierTokenizer::new(IdentifierMode::CamelCase);
        let out: Vec<_> = seg.segment("camelCase").collect();
        assert_eq!(out[0], Segment::new(0, "camel"));
        assert_eq!(out[1], Segment::new(5, "Case"));
    }

    #[test]
    fn camel_case_unicode_letters() {
        // Non-ASCII lowercase followed by non-ASCII uppercase.
        let parts = split(IdentifierMode::CamelCase, "αΒ");
        assert_eq!(parts, ["α", "Β"]);
    }
}
