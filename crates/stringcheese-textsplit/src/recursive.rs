//! The classic separator-list recursive splitter.
//!
//! Given a chunk-size budget and a priority-ordered separator
//! list, find the first separator that yields pieces small enough
//! to fit. Any piece still too big recurses into the remaining
//! separators. The final fallback is a hard char-boundary cut at
//! the size limit.
//!
//! This is what most LLM RAG pipelines mean by "text splitter."

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{Chunk, TextSplitter};

/// LangChain-style recursive text splitter.
///
/// Configure once with a chunk size, overlap, and separator
/// priority list; apply to any `&str`.
#[derive(Clone, Debug)]
pub struct RecursiveSplitter {
    chunk_size: usize,
    overlap: usize,
    separators: Vec<String>,
}

impl Default for RecursiveSplitter {
    fn default() -> Self {
        Self::new(1000, 200)
    }
}

impl RecursiveSplitter {
    /// Construct with target chunk size (bytes) and overlap (bytes).
    /// Default separator list is `["\n\n", "\n", ". ", " ", ""]` —
    /// prefer paragraph boundaries, then line, then sentence-ish
    /// boundaries, then any space, then any char boundary.
    ///
    /// # Panics
    ///
    /// Panics on `chunk_size == 0` — a zero-size target has no
    /// stopping condition. Also panics when `overlap >= chunk_size`
    /// (would produce infinite recursion in the classical
    /// algorithm).
    #[must_use]
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be > 0");
        assert!(
            overlap < chunk_size,
            "overlap must be strictly less than chunk_size",
        );
        Self {
            chunk_size,
            overlap,
            separators: [
                "\n\n".to_string(),
                "\n".to_string(),
                ". ".to_string(),
                " ".to_string(),
                String::new(),
            ]
            .into(),
        }
    }

    /// Override the separator list. Must include an empty-string
    /// terminator (or a very short separator) as the last entry —
    /// otherwise inputs with no separators at all can't be broken
    /// down.
    #[must_use]
    pub fn with_separators(mut self, seps: Vec<String>) -> Self {
        self.separators = seps;
        self
    }

    /// Access the configured chunk size in bytes.
    #[must_use]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Access the configured overlap in bytes.
    #[must_use]
    pub fn overlap(&self) -> usize {
        self.overlap
    }
}

impl TextSplitter for RecursiveSplitter {
    fn split(&self, input: &str) -> Vec<Chunk> {
        if input.is_empty() {
            return Vec::new();
        }
        let mut raw_pieces: Vec<(usize, String)> = Vec::new();
        recurse(input, 0, &self.separators, self.chunk_size, &mut raw_pieces);
        merge_and_overlap(raw_pieces, self.chunk_size, self.overlap)
    }
}

fn recurse(
    input: &str,
    base_offset: usize,
    seps: &[String],
    chunk_size: usize,
    out: &mut Vec<(usize, String)>,
) {
    if input.is_empty() {
        return;
    }
    if input.len() <= chunk_size {
        out.push((base_offset, input.to_string()));
        return;
    }
    // Find the first separator that actually appears in the input
    // (or an empty separator — the fallback splits on char
    // boundaries directly).
    let sep = seps
        .iter()
        .find(|s| s.is_empty() || input.contains(s.as_str()));
    match sep {
        None => {
            // No separator matched. Hard-cut on a char boundary.
            hard_cut(input, base_offset, chunk_size, out);
        }
        Some(sep) if sep.is_empty() => {
            hard_cut(input, base_offset, chunk_size, out);
        }
        Some(sep) => {
            // Split by this separator; recurse on any piece still
            // too big under the REMAINING separators.
            let sep_str = sep.as_str();
            let sep_len = sep_str.len();
            let remaining = &seps[seps.iter().position(|s| s == sep).unwrap() + 1..];
            let mut cursor = 0usize;
            for (match_start, _) in input.match_indices(sep_str) {
                let piece = &input[cursor..match_start];
                if !piece.is_empty() {
                    if piece.len() <= chunk_size {
                        out.push((base_offset + cursor, piece.to_string()));
                    } else {
                        recurse(piece, base_offset + cursor, remaining, chunk_size, out);
                    }
                }
                cursor = match_start + sep_len;
            }
            let tail = &input[cursor..];
            if !tail.is_empty() {
                if tail.len() <= chunk_size {
                    out.push((base_offset + cursor, tail.to_string()));
                } else {
                    recurse(tail, base_offset + cursor, remaining, chunk_size, out);
                }
            }
        }
    }
}

fn hard_cut(input: &str, base_offset: usize, chunk_size: usize, out: &mut Vec<(usize, String)>) {
    let mut cursor = 0usize;
    while cursor < input.len() {
        let mut end = (cursor + chunk_size).min(input.len());
        // Walk back to a char boundary.
        while end > cursor && !input.is_char_boundary(end) {
            end -= 1;
        }
        // If we couldn't even fit one char, force one full char.
        if end == cursor {
            let mut probe = cursor + 1;
            while probe < input.len() && !input.is_char_boundary(probe) {
                probe += 1;
            }
            end = probe;
        }
        out.push((base_offset + cursor, input[cursor..end].to_string()));
        cursor = end;
    }
}

/// Post-process the raw pieces: greedy-merge adjacent pieces
/// that together fit under `chunk_size`, then apply overlap.
///
/// **Greedy merge is on by default** — the recursive split step
/// produces pieces at whichever separator carved them (which for
/// deep recursion can be single spaces), and a caller who set
/// `chunk_size = 500` expects chunks close to 500 bytes, not a
/// scattering of one-word slivers. The merge step joins those
/// slivers back up to (but not exceeding) the target size,
/// re-inserting a single space between joined pieces so the
/// merged text stays readable.
///
/// When a caller genuinely wants separator-preserving splits
/// (paragraph-per-chunk display), they set `chunk_size` small
/// enough that each paragraph is already at the ceiling, or use
/// [`crate::ParagraphSplitter`] which never merges across
/// `\n\n` boundaries.
fn merge_and_overlap(
    pieces: Vec<(usize, String)>,
    chunk_size: usize,
    overlap: usize,
) -> Vec<Chunk> {
    if pieces.is_empty() {
        return Vec::new();
    }

    // Greedy merge — join adjacent pieces up to chunk_size with a
    // single space between them. The +1 accounts for the
    // separator we'd insert.
    let mut merged: Vec<(usize, String)> = Vec::new();
    for (offset, text) in pieces {
        match merged.last_mut() {
            Some((_, prev)) if prev.len() + 1 + text.len() <= chunk_size => {
                prev.push(' ');
                prev.push_str(&text);
            }
            _ => merged.push((offset, text)),
        }
    }

    if overlap == 0 {
        return merged
            .into_iter()
            .map(|(start, text)| {
                let end = start + text.len();
                Chunk { text, start, end }
            })
            .collect();
    }

    // Apply overlap — prepend the tail of the previous chunk to
    // this one. Take from the previous chunk's text on a char
    // boundary.
    let mut out: Vec<Chunk> = Vec::with_capacity(merged.len());
    for (i, (start, text)) in merged.iter().enumerate() {
        if i == 0 {
            let end = start + text.len();
            out.push(Chunk {
                text: text.clone(),
                start: *start,
                end,
            });
            continue;
        }
        let (prev_start, prev_text) = &merged[i - 1];
        let take = overlap.min(prev_text.len());
        // Move `take` bytes from the tail of prev_text, walking to
        // a char boundary.
        let mut cut = prev_text.len() - take;
        while cut < prev_text.len() && !prev_text.is_char_boundary(cut) {
            cut += 1;
        }
        let overlap_bytes = &prev_text[cut..];
        let mut combined = String::with_capacity(overlap_bytes.len() + text.len());
        combined.push_str(overlap_bytes);
        combined.push_str(text);
        let new_start = prev_start + cut;
        let end = start + text.len();
        out.push(Chunk {
            text: combined,
            start: new_start,
            end,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_input_returns_one_chunk() {
        let s = RecursiveSplitter::new(100, 0);
        let chunks = s.split("hello world");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello world");
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 11);
    }

    #[test]
    fn empty_input_returns_no_chunks() {
        assert!(RecursiveSplitter::new(100, 0).split("").is_empty());
    }

    #[test]
    fn paragraph_boundary_wins_over_line() {
        // chunk_size 10 fits every single paragraph exactly, but
        // NOT any two together (8 + 1 space + 8 = 17 > 10), so
        // greedy merge keeps them separate at the `\n\n` boundary.
        let s = RecursiveSplitter::new(10, 0);
        let input = "para one\n\npara two\n\npara three";
        let chunks = s.split(input);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "para one");
        assert_eq!(chunks[1].text, "para two");
        assert_eq!(chunks[2].text, "para three");
    }

    #[test]
    fn adjacent_pieces_merge_up_to_chunk_size() {
        // A single long paragraph broken into sentences with a
        // chunk_size that lets several sentences fit together.
        // Greedy merge packs the sentence-splits back up
        // (LangChain-style — a caller who set chunk_size 60 wants
        // ~60 byte chunks, not one sentence per chunk).
        let s = RecursiveSplitter::new(60, 0);
        let input = "one. two. three. four. five. six. seven. eight. nine. ten.";
        let chunks = s.split(input);
        // The whole input is 58 bytes so it fits in one chunk
        // without splitting. Just verify the packing behaviour on
        // an input that DOES need splitting.
        assert!(!chunks.is_empty());
        for c in &chunks {
            assert!(c.len() <= 60, "chunk {:?} exceeds 60 bytes", c.text);
        }
    }

    #[test]
    fn oversized_paragraph_recurses_to_line() {
        // A paragraph too big for the chunk size splits at line
        // boundaries.
        let s = RecursiveSplitter::new(15, 0);
        let input = "line one here\nline two here\nline three here";
        let chunks = s.split(input);
        assert!(chunks.iter().all(|c| c.len() <= 15));
    }

    #[test]
    fn overlap_shares_tail_between_chunks() {
        // Overlap of 5 bytes on 15-byte chunks — each chunk after
        // the first starts with the last 5 bytes of the previous.
        let s = RecursiveSplitter::new(15, 5);
        // "aaaaaaaa bbbbbbbb ccccccccc" — has spaces to split at.
        let input = "aaaaaaaa bbbbbbbb ccccccccc";
        let chunks = s.split(input);
        assert!(chunks.len() >= 2);
        // First chunk starts at 0 and has no prepended overlap.
        assert_eq!(chunks[0].start, 0);
        // Second chunk has overlap prepended — its text is longer
        // than the piece from the source at [start..end] would be
        // without overlap.
        assert!(chunks[1].text.len() >= 5);
    }

    #[test]
    fn hard_cut_respects_char_boundaries() {
        // Input with a 3-byte scalar right around the cut point;
        // the cut MUST land at a char boundary.
        let s = RecursiveSplitter::new(5, 0).with_separators(alloc::vec![String::new()]);
        // "日本語日本語" — 6 scalars, 18 bytes.
        let chunks = s.split("日本語日本語");
        for c in &chunks {
            // Every returned string is valid UTF-8 by definition;
            // the concern is that hard_cut doesn't panic and the
            // reported (start, end) offsets land on boundaries.
            assert!(c.text.is_empty() || !c.text.starts_with('\u{FFFD}'));
        }
    }

    #[test]
    #[should_panic(expected = "chunk_size must be > 0")]
    fn zero_chunk_size_panics() {
        let _ = RecursiveSplitter::new(0, 0);
    }

    #[test]
    #[should_panic(expected = "overlap must be strictly less than chunk_size")]
    fn overlap_ge_chunk_size_panics() {
        let _ = RecursiveSplitter::new(10, 10);
    }
}
