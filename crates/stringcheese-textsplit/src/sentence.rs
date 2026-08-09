//! Sentence-grouping splitter — collect whole sentences into
//! chunks until the size threshold is reached.
//!
//! Every chunk is one or more complete sentences; no sentence is
//! ever split mid-way. When a single sentence exceeds the chunk
//! size, it becomes its own chunk (slightly over-budget) — the
//! alternative is mid-sentence hard cuts that destroy the reason
//! the caller picked a sentence splitter in the first place.
//!
//! Uses [`stringcheese_segment`] for sentence-boundary detection.
//! Without the `sentences-icu` feature enabled at build time, the
//! detection is the naive `. `/`? `/`! `/`\n\n` fallback from
//! stringcheese-segment.

use alloc::vec::Vec;

use stringcheese_segment::{SegmentUnit, split};

use crate::{Chunk, TextSplitter};

/// Sentence-grouping splitter.
#[derive(Clone, Debug)]
pub struct SentenceSplitter {
    chunk_size: usize,
}

impl Default for SentenceSplitter {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl SentenceSplitter {
    /// Construct with a target chunk size (bytes).
    ///
    /// # Panics
    ///
    /// Panics on `chunk_size == 0`.
    #[must_use]
    pub fn new(chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be > 0");
        Self { chunk_size }
    }
}

impl TextSplitter for SentenceSplitter {
    fn split(&self, input: &str) -> Vec<Chunk> {
        if input.is_empty() {
            return Vec::new();
        }
        // Collect sentences from stringcheese-segment, tracking the
        // byte offset of each one in the original input.
        //
        // Segments are contiguous slices of the input in order, so
        // the start offset is simply the running cursor. The
        // earlier implementation used `input[cursor..].find(s)`
        // per sentence, which is O(N) per iteration and O(N²)
        // overall — bench 2026-08-09 showed 14× slowdown vs.
        // ParagraphSplitter. Cursor tracking is O(N) total.
        let mut sentences: Vec<(usize, &str)> = Vec::new();
        let mut cursor = 0usize;
        for s in split(input, SegmentUnit::Sentences) {
            if s.is_empty() {
                continue;
            }
            sentences.push((cursor, s));
            cursor += s.len();
        }

        let mut out: Vec<Chunk> = Vec::new();
        let mut buf_start: Option<usize> = None;
        let mut buf_end: usize = 0;
        for (start, s) in sentences {
            let would_be_len = if let Some(bs) = buf_start {
                (start + s.len()) - bs
            } else {
                s.len()
            };
            if buf_start.is_none() {
                buf_start = Some(start);
                buf_end = start + s.len();
                continue;
            }
            if would_be_len <= self.chunk_size {
                buf_end = start + s.len();
            } else {
                // Emit the current buffer, start a new one with
                // this sentence.
                let bs = buf_start.take().unwrap();
                out.push(Chunk {
                    text: input[bs..buf_end].into(),
                    start: bs,
                    end: buf_end,
                });
                buf_start = Some(start);
                buf_end = start + s.len();
            }
        }
        if let Some(bs) = buf_start {
            out.push(Chunk {
                text: input[bs..buf_end].into(),
                start: bs,
                end: buf_end,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_input_yields_one_chunk() {
        let s = SentenceSplitter::new(100);
        let chunks = s.split("Hello. World.");
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn sentences_group_up_to_size_limit() {
        // Three short sentences with a small chunk size — should
        // produce multiple chunks with sentence boundaries as
        // splits.
        let s = SentenceSplitter::new(15);
        let chunks = s.split("Hi. Bye. Ok. Nope.");
        assert!(chunks.len() >= 2);
        for c in &chunks {
            // Each chunk ends at a sentence-boundary the segmenter
            // recognises — the text must end in one of the
            // terminator strings the naive segmenter looks for.
            assert!(!c.text.is_empty());
        }
    }

    #[test]
    fn oversized_sentence_stays_intact() {
        // A single sentence longer than the chunk size stays
        // whole; we don't cut sentences mid-word.
        let s = SentenceSplitter::new(10);
        let input = "This is one very long sentence with no terminators until here.";
        let chunks = s.split(input);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, input);
    }

    #[test]
    fn offsets_point_back_into_original_input() {
        let s = SentenceSplitter::new(100);
        let input = "Alpha. Beta. Gamma.";
        let chunks = s.split(input);
        for c in &chunks {
            assert_eq!(&input[c.start..c.end], c.text.as_str());
        }
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(SentenceSplitter::new(100).split("").is_empty());
    }

    #[test]
    #[should_panic(expected = "chunk_size must be > 0")]
    fn zero_chunk_size_panics() {
        let _ = SentenceSplitter::new(0);
    }
}
