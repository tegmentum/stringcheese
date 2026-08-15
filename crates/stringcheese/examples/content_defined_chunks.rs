//! # `FastCDC` content-defined chunking
//!
//! Chunk a byte stream at content-defined boundaries using `FastCDC`
//! (Xia et al. 2016). The boundaries depend on the *content* of the
//! stream, not on absolute byte positions — inserting or deleting
//! a byte affects only nearby chunks, which is what makes CDC the
//! workhorse of deduplication (backup, sync, dedup storage, git-lfs
//! style content-addressed blob stores).
//!
//! The `default_8k` config produces chunks with a target average of
//! 8 KiB. The example uses a small config so the output fits on
//! screen — the pattern is identical at any size.
//!
//! Run: `cargo run --example content_defined_chunks -p stringcheese`

use stringcheese::cdc::{FastCdc, FastCdcConfig};

fn main() {
    // Small pseudo-random-ish payload so several boundaries fire.
    // In real use the config would be `default_8k` / `default_16k`;
    // here we shrink the sizes so the example's output is readable.
    let mut payload = Vec::with_capacity(4096);
    for i in 0..4096u32 {
        // A simple LCG-style byte stream — enough entropy for FastCDC
        // to fire, deterministic across runs. The `>> 8` selects a
        // higher-entropy byte from the multiplied word; truncation to
        // `u8` is what we actually want.
        #[allow(clippy::cast_possible_truncation)]
        let b = ((i.wrapping_mul(2_654_435_761)) >> 8) as u8;
        payload.push(b);
    }

    let config = FastCdcConfig {
        min_size: 64,
        avg_size: 256,
        max_size: 1024,
        // Small-size configs published in the FastCDC paper for a
        // 256-byte target average.
        mask_small: 0x0000_5907_0353_0000,
        mask_large: 0x0000_1900_0353_0000,
    };
    let chunker = FastCdc::new(config);

    let boundaries: Vec<_> = chunker.chunk_boundaries(&payload).collect();
    println!(
        "Chunked {} bytes into {} chunk(s):\n",
        payload.len(),
        boundaries.len()
    );
    println!("{:>4}  {:>6}  {:>6}  {:>6}", "idx", "start", "end", "size");
    println!("{:>4}  {:>6}  {:>6}  {:>6}", "---", "-----", "---", "----");
    for (i, b) in boundaries.iter().enumerate() {
        println!("{:>4}  {:>6}  {:>6}  {:>6}", i, b.start(), b.end(), b.size);
    }

    let total: usize = boundaries.iter().map(|b| b.size).sum();
    println!("\nsum of chunk sizes = {total}   (should equal input size)");
    assert_eq!(total, payload.len());
}
