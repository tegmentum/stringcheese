//! # Segment a string with the built-in tokenizers
//!
//! The `stringcheese::tokenizer` subsystem exposes the [`Segmenter`]
//! trait plus a family of built-in segmenters (whitespace,
//! delimiter, identifier, grapheme, n-gram). This example runs a
//! sample sentence through two of them and prints the segment text
//! and byte offset for each.
//!
//! The heavier subword tokenizers (HF-compatible BPE / `WordPiece` /
//! `Unigram`, tiktoken) live in the opt-in `stringcheese-tokenizer-hf`
//! and `stringcheese-tokenizer-tiktoken` crates and are not
//! re-exported by the umbrella. See their crate docs for the
//! `tokenizer.json` loader and the tiktoken model packs.
//!
//! Run: `cargo run --example tokenizer_encode -p stringcheese`

use stringcheese::tokenizer::{
    IdentifierMode, IdentifierTokenizer, Segmenter, WhitespaceTokenizer,
};

fn main() {
    let input = "hello, world! parseXMLDocument v2";

    println!("input: {input:?}\n");

    println!("WhitespaceTokenizer:");
    for seg in WhitespaceTokenizer.segment(input) {
        println!("  [{:>3}]  {:?}", seg.offset, seg.text);
    }

    println!("\nIdentifierTokenizer (Auto mode — camelCase splitting on last token):");
    let ident = IdentifierTokenizer::new(IdentifierMode::Auto);
    // Feed the single identifier through the camelCase-aware splitter.
    for seg in ident.segment("parseXMLDocument") {
        println!("  [{:>3}]  {:?}", seg.offset, seg.text);
    }
}
