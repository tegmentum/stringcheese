//! # Split a document into RAG-ready chunks with size metadata
//!
//! LLM retrieval-augmented-generation pipelines need chunks small
//! enough to fit the model's context window with room for the
//! prompt scaffolding. This example demonstrates the composition
//! that most pipelines end up wanting:
//!
//! 1. **Split** — `textsplit::RecursiveSplitter` with a target
//!    chunk size + overlap. Splits at paragraph boundaries first,
//!    then lines, then sentence-ish boundaries, then any space,
//!    falling back to a hard char-boundary cut.
//! 2. **Measure** — every chunk carries its byte range in the
//!    original input plus `Lengths::of` gives byte / code-point /
//!    grapheme counts. Use whichever unit your downstream token
//!    counter cares about.
//! 3. **Characterise** — `Ratios::of` reports the alphabetic /
//!    digit / punctuation / whitespace mix, useful for filtering
//!    out code blocks or heavy-boilerplate chunks before
//!    embedding.
//!
//! Run: `cargo run --example text_chunk_for_llm`

use stringcheese::stats::{Lengths, Ratios};
use stringcheese::textsplit::{RecursiveSplitter, TextSplitter};

fn main() {
    let doc = "\
StringCheese is a comprehensive, performance-oriented toolkit for string \
and sequence comparison. It aims to be the canonical Rust library for \
distance, similarity, alignment, phonetic matching, n-gram, \
fingerprinting, and content-defined chunking algorithms.

The project treats correctness, memory transparency, and multilingual \
support as first-class concerns rather than after-the-fact bolt-ons. \
Every subsystem documents its unit choice explicitly — bytes vs code \
points vs graphemes — because silent defaults are how string bugs \
creep in.

The workspace ships around forty crates today: a comparison kernel, a \
segmentation subsystem, three sketch families (MinHash, SimHash, \
winnowing), a diff engine, a pattern-matching subsystem, and a \
language-detection stack that lazy-loads WASM components by script and \
language on demand.
";

    // 200-byte target chunks with 40-byte overlap — modest sizes
    // for demonstration; production sizes are usually 500-2000
    // bytes depending on the embedding model's context window.
    let splitter = RecursiveSplitter::new(200, 40);
    let chunks = splitter.split(doc);

    println!("Split into {} chunk(s):\n", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let lens = Lengths::of(&chunk.text);
        let ratios = Ratios::of(&chunk.text);
        println!("─── chunk {i} ─── [{}..{}) bytes", chunk.start, chunk.end);
        println!(
            "  length: {} bytes / {} code points",
            lens.bytes, lens.code_points,
        );
        println!(
            "  ratios: alpha={:.2} digit={:.2} punct={:.2} whitespace={:.2}",
            ratios.alphabetic, ratios.digit, ratios.punctuation, ratios.whitespace,
        );
        println!("  text: {:?}", chunk.text);
        println!();
    }
}
