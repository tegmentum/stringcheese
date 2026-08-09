//! Paragraph-first splitter — treats every run of text between
//! blank lines as one candidate chunk.
//!
//! Paragraphs that fit under the size threshold become one chunk
//! each. Paragraphs that are too big fall through to a nested
//! [`crate::RecursiveSplitter`] with the caller's size / overlap
//! settings.

use alloc::vec::Vec;

use crate::recursive::RecursiveSplitter;
use crate::{Chunk, TextSplitter};

/// Paragraph-oriented splitter.
#[derive(Clone, Debug)]
pub struct ParagraphSplitter {
    chunk_size: usize,
    overlap: usize,
}

impl Default for ParagraphSplitter {
    fn default() -> Self {
        Self::new(1000, 0)
    }
}

impl ParagraphSplitter {
    /// Construct with a chunk-size budget (bytes) and overlap.
    /// Overlap applies only to the [`RecursiveSplitter`] fallback for
    /// oversized paragraphs — normal paragraph boundaries never
    /// overlap.
    ///
    /// # Panics
    ///
    /// Panics on `chunk_size == 0` or `overlap >= chunk_size`.
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
        }
    }
}

impl TextSplitter for ParagraphSplitter {
    fn split(&self, input: &str) -> Vec<Chunk> {
        if input.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<Chunk> = Vec::new();
        let fallback = RecursiveSplitter::new(self.chunk_size, self.overlap);
        let mut cursor = 0usize;
        for (match_start, sep) in input.match_indices("\n\n") {
            let para = &input[cursor..match_start];
            if !para.is_empty() {
                emit_paragraph(para, cursor, self.chunk_size, &fallback, &mut out);
            }
            cursor = match_start + sep.len();
        }
        let tail = &input[cursor..];
        if !tail.is_empty() {
            emit_paragraph(tail, cursor, self.chunk_size, &fallback, &mut out);
        }
        out
    }
}

fn emit_paragraph(
    para: &str,
    base_offset: usize,
    chunk_size: usize,
    fallback: &RecursiveSplitter,
    out: &mut Vec<Chunk>,
) {
    if para.len() <= chunk_size {
        out.push(Chunk {
            text: para.into(),
            start: base_offset,
            end: base_offset + para.len(),
        });
        return;
    }
    // Oversized paragraph → recurse. Adjust offsets to point back
    // into the original input.
    for c in fallback.split(para) {
        out.push(Chunk {
            text: c.text,
            start: base_offset + c.start,
            end: base_offset + c.end,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_each_become_one_chunk() {
        let s = ParagraphSplitter::new(100, 0);
        let input = "para one\n\npara two\n\npara three";
        let chunks = s.split(input);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "para one");
        assert_eq!(chunks[1].text, "para two");
        assert_eq!(chunks[2].text, "para three");
    }

    #[test]
    fn oversized_paragraph_falls_through_to_recursive() {
        // Single long paragraph, no blank-line separators.
        let s = ParagraphSplitter::new(15, 0);
        let input = "one two three four five six seven eight nine ten";
        let chunks = s.split(input);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= 15));
    }

    #[test]
    fn offsets_point_back_into_original_input() {
        let s = ParagraphSplitter::new(100, 0);
        let input = "alpha\n\nbeta\n\ngamma";
        let chunks = s.split(input);
        assert_eq!(&input[chunks[0].start..chunks[0].end], "alpha");
        assert_eq!(&input[chunks[1].start..chunks[1].end], "beta");
        assert_eq!(&input[chunks[2].start..chunks[2].end], "gamma");
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(ParagraphSplitter::new(100, 0).split("").is_empty());
    }
}
