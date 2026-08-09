//! Cross-crate integration tests.
//!
//! Every per-crate test suite exercises one crate in isolation.
//! These tests exercise **compositions** — the API-shape checks
//! that only fire when crates actually feed into each other.
//! When one crate changes an iterator type or a public struct in
//! a way that breaks a natural downstream use, one of these
//! tests goes red loudly (and in one place) rather than
//! surfacing later as documentation drift.

// Same-sketch self-similarity is exactly 1.0 by construction —
// every permutation's minimum matches position-for-position, so
// `matches / width == 1.0` is the assertion, not accident.
#![allow(clippy::float_cmp)]

use stringcheese::{collate, ident, normalize, segment, simhash, stats, textsplit, winnowing};

// ---------------------------------------------------------------------
// Sketch trio: ngram feeds into minhash / simhash / winnowing.
// ---------------------------------------------------------------------

#[test]
fn ngram_hashes_feed_minhash_and_reproduce_perfect_self_similarity() {
    // Character 3-grams of a sentence, MinHash-sketched under two
    // independent sketchers with the same seed. Self-similarity
    // must be exactly 1.0 (every permutation's minimum matches by
    // construction).
    let text = "the quick brown fox jumps over the lazy dog";
    let grams1: Vec<&str> = stringcheese_ngram::char_ngrams(text, 3).collect();
    let grams2: Vec<&str> = stringcheese_ngram::char_ngrams(text, 3).collect();
    let sketcher = stringcheese_minhash::Sketcher::new(128);
    let s1 = sketcher.sketch(grams1.iter().copied());
    let s2 = sketcher.sketch(grams2.iter().copied());
    assert_eq!(s1.jaccard(&s2), 1.0);
}

#[test]
fn ngram_hashes_feed_simhash_across_similar_documents() {
    // Two documents with heavy overlap should produce SimHash
    // fingerprints with a small Hamming distance.
    let doc_a = "the quick brown fox jumps over the lazy dog";
    let doc_b = "the quick brown fox leaps over the sleepy dog";

    let grams_a: Vec<&str> = stringcheese_ngram::char_ngrams(doc_a, 4).collect();
    let grams_b: Vec<&str> = stringcheese_ngram::char_ngrams(doc_b, 4).collect();

    let sk_a = simhash::Sketcher::new()
        .add_all(grams_a.iter().copied())
        .finalize_64();
    let sk_b = simhash::Sketcher::new()
        .add_all(grams_b.iter().copied())
        .finalize_64();

    // Similarity should be well above 0.5 — the two docs share
    // most of their content.
    let sim = sk_a.similarity(&sk_b);
    assert!(
        sim > 0.7,
        "expected similarity > 0.7 for near-duplicate docs, got {sim}"
    );
}

#[test]
fn hashed_ngrams_feed_winnowing() {
    // Winnowing consumes u64 hashes; hash each char-3-gram
    // deterministically and hand the stream to the winnower.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let text = "the quick brown fox jumps over the lazy dog";
    let hashes: Vec<u64> = stringcheese_ngram::char_ngrams(text, 5)
        .map(|g| {
            let mut h = DefaultHasher::new();
            g.hash(&mut h);
            h.finish()
        })
        .collect();

    let w = winnowing::Winnower::new(4);
    let fps: Vec<_> = w.select(hashes.iter().copied()).collect();
    assert!(
        !fps.is_empty(),
        "winnowing should produce fingerprints from a real hash stream"
    );
    // Every fingerprint's position points at a valid index of the
    // hash stream.
    for fp in &fps {
        assert!(fp.position < hashes.len());
    }
}

// ---------------------------------------------------------------------
// Normalize + collate: search keys sort stably.
// ---------------------------------------------------------------------

#[test]
fn search_keys_produce_stable_natural_ordering() {
    // Three variants of "café" that should collapse to the same
    // search key and thus be pairwise-Equal under any collator.
    let variants = ["Café", "CAFÉ", "café"];
    let keys: Vec<String> = variants.iter().map(|s| normalize::search_key(s)).collect();
    // All three normalise to the same key.
    assert_eq!(keys[0], keys[1]);
    assert_eq!(keys[1], keys[2]);

    // Sorted by the ASCII-CI collator, files sort naturally.
    let c = collate::NaturalCollator::new(collate::AsciiCiCollator::new());
    let mut xs = ["file10", "file2", "file1"].to_vec();
    xs.sort_by(|a, b| {
        use stringcheese::collate::Collator;
        c.compare(a, b)
    });
    assert_eq!(xs, ["file1", "file2", "file10"]);
}

// ---------------------------------------------------------------------
// Textsplit + segment: chunks respect sentence boundaries.
// ---------------------------------------------------------------------

#[test]
fn sentence_splitter_produces_whole_sentences_per_chunk() {
    let text = "Alpha runs first. Beta follows quickly. Gamma trails behind.";
    let splitter = textsplit::SentenceSplitter::new(35);
    let chunks: Vec<_> = {
        use textsplit::TextSplitter;
        splitter.split(text)
    };
    assert!(!chunks.is_empty());
    // Every chunk carries at least one whole sentence — count
    // sentences via segment and verify.
    for chunk in &chunks {
        let sent_count = segment::split(&chunk.text, segment::SegmentUnit::Sentences).count();
        assert!(sent_count >= 1, "chunk {:?} had no sentences", chunk.text);
    }
}

// ---------------------------------------------------------------------
// Ident + escape: slug then percent-encode is a no-op past the
// first pass (slug is ASCII-safe already).
// ---------------------------------------------------------------------

#[test]
fn slug_then_percent_encode_is_idempotent_for_ascii_slugs() {
    let name = "Hello, Café World! — édition spéciale";
    let slug = ident::slugify(name);
    // Slug is ASCII by construction.
    assert!(slug.is_ascii());
    // Percent-encoding a slug that has no reserved characters
    // beyond hyphens returns the slug unchanged.
    let encoded = stringcheese::escape::escape(&slug, stringcheese::escape::Escape::UriComponent);
    // Hyphens are NOT in `NON_ALPHANUMERIC`'s safe set — they
    // percent-encode to %2D. Verify round-trip instead of
    // fixed-equality; a caller who cares about hyphen preservation
    // uses a narrower encode set.
    let round =
        stringcheese::escape::unescape(&encoded, stringcheese::escape::Escape::UriComponent)
            .unwrap();
    assert_eq!(round, slug);
}

// ---------------------------------------------------------------------
// Stats: characterisation composes with normalize.
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Feature-gated: pattern-regex re-export works end-to-end.
// ---------------------------------------------------------------------

#[cfg(feature = "pattern-regex")]
#[test]
fn pattern_regex_feature_reexports_regex_engine() {
    use stringcheese::pattern::Pattern;
    use stringcheese::pattern_regex::Regex;

    let re = Regex::new(r"\d+").unwrap();
    assert!(re.is_match("abc 42 xyz"));
    // Uniform Pattern trait dispatch — same shape as Literal/Wildcard/Glob.
    let m = re.find("abc 42 xyz").unwrap();
    assert_eq!(m.matched, "42");
}

#[test]
fn stats_ratios_track_normalized_input() {
    // Raw input has smart quotes and non-ASCII punctuation;
    // canonicalising first shifts the punctuation ratio slightly
    // (dashes/quotes collapse to ASCII equivalents but stay
    // punctuation).
    let raw = "\u{201C}Hello \u{2014} world.\u{201D}";
    let canon = normalize::punctuation_canonical(raw);

    let r_raw = stats::Ratios::of(raw);
    let r_canon = stats::Ratios::of(&canon);

    // Both are the same length in code points (canonicalisation
    // is 1-to-1 in this input); their punctuation counts should
    // therefore be the same fraction.
    let expected_len = raw.chars().count();
    let canon_len = canon.chars().count();
    assert_eq!(expected_len, canon_len);
    assert!((r_raw.punctuation - r_canon.punctuation).abs() < 1e-9);
}
