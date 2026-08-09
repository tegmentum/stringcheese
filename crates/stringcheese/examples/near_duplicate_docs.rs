//! # Find near-duplicate documents via `MinHash` + LSH
//!
//! Given a corpus of short documents, return every pair whose
//! estimated Jaccard similarity meets a threshold. The pipeline
//! is the canonical "sketches + LSH candidate generation +
//! exact-rescore" shape a real deduplication or plagiarism-
//! detection service would use.
//!
//! Pipeline:
//!
//! 1. **Grams** — each doc's character 5-grams via
//!    `stringcheese-ngram`. Character grams are language-agnostic
//!    and robust to word-level rearrangements.
//! 2. **Sketch** — each doc's gram set collapses to a 128-permutation
//!    `MinHash` sketch via `stringcheese-minhash`.
//! 3. **Bucket** — each sketch's LSH bands go into a bucket map;
//!    any two docs sharing a bucket are a candidate pair.
//! 4. **Rescore** — every candidate pair's exact `MinHash` Jaccard
//!    estimate is computed, and pairs above the threshold are
//!    returned.
//!
//! Run: `cargo run --example near_duplicate_docs`

use std::collections::HashMap;

use stringcheese_minhash::{Sketch, Sketcher, lsh};

fn main() {
    // Toy corpus — three near-duplicate pairs and one unrelated.
    let docs = [
        ("doc-a1", "The quick brown fox jumps over the lazy dog."),
        ("doc-a2", "The quick brown fox jumped over the lazy dog."),
        (
            "doc-b1",
            "Rust is a language empowering everyone to build reliable software.",
        ),
        (
            "doc-b2",
            "Rust empowers everyone to build reliable and efficient software.",
        ),
        (
            "doc-c",
            "Winnowing is a document fingerprinting scheme from Stanford.",
        ),
    ];

    let sketcher = Sketcher::new(128);
    let width = 128;
    let bands = 16; // 128 bits / 16 bands = 8 rows per band
    let rows = 8;

    // Sketch every document.
    let doc_sketches: Vec<(&str, Sketch)> = docs
        .iter()
        .map(|(name, text)| {
            let grams = stringcheese_ngram::char_ngrams(text, 5);
            (*name, sketcher.sketch(grams))
        })
        .collect();

    // LSH bucket map: (band_index, band_signature) → docs sharing that bucket.
    let mut buckets: HashMap<(usize, u64), Vec<&str>> = HashMap::new();
    for (name, sk) in &doc_sketches {
        let sigs = lsh::band_signatures(sk, bands, rows);
        for (band_idx, sig) in sigs.iter().enumerate() {
            buckets.entry((band_idx, *sig)).or_default().push(name);
        }
    }
    let _ = width; // width is documented for orientation; not read.

    // Collect candidate pairs — any two docs sharing at least one bucket.
    let mut candidates: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();
    for group in buckets.values() {
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let (a, b) = if group[i] < group[j] {
                    (group[i], group[j])
                } else {
                    (group[j], group[i])
                };
                candidates.insert((a, b));
            }
        }
    }

    // Rescore every candidate pair with the full MinHash estimate.
    let sketch_by_name: HashMap<&str, &Sketch> =
        doc_sketches.iter().map(|(n, s)| (*n, s)).collect();
    let threshold = 0.6;

    let mut hits: Vec<(&str, &str, f64)> = candidates
        .into_iter()
        .map(|(a, b)| {
            let sa = sketch_by_name[a];
            let sb = sketch_by_name[b];
            (a, b, sa.jaccard(sb))
        })
        .filter(|(_, _, j)| *j >= threshold)
        .collect();

    // Descending similarity for readable output.
    hits.sort_by(|x, y| y.2.partial_cmp(&x.2).unwrap());

    println!("Near-duplicate pairs (Jaccard ≥ {threshold}):\n");
    for (a, b, j) in &hits {
        println!("  {a} ~ {b}   J≈{j:.3}");
    }
    if hits.is_empty() {
        println!("  (none above threshold)");
    }
}
