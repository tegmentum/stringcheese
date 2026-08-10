//! Precompiled charsmap parity fixtures.
//!
//! `SentencePiece`'s Precompiled normalizer is what every Llama /
//! Mistral / T5 / XLM-`RoBERTa` `tokenizer.json` ships. This file pins
//! the algorithm against hand-crafted charsmaps that exercise the
//! transformations a real checkpoint's charsmap performs on
//! representative inputs:
//!
//! * longest-prefix wins when two keys overlap (`ab` beats `a`);
//! * an empty replacement string deletes the matched key (`\r` drop);
//! * a multi-byte UTF-8 key maps to a shorter replacement
//!   (`U+00A0` NBSP → ASCII space, a very common `SentencePiece`
//!   fold);
//! * unmatched positions pass through byte-for-byte, including
//!   multi-byte scalars the trie does not touch.
//!
//! We cannot ship the multi-hundred-KB real Llama charsmap in-tree
//! (both for repo size and licensing hygiene), so parity here is
//! against synthesized charsmaps whose expected output is derived by
//! walking the wire-format spec by hand. The same algorithm shape
//! that produces the right answer for these fixtures also produces
//! the right answer for the real charsmap — the trie encoding does
//! not care about size.
//!
//! Fetching the real Llama `tokenizer.json` at test time is deferred
//! behind the (as-yet-uncreated) `parity-real-vocab` feature that
//! the tiktoken parity landing will add; when that feature exists,
//! this file's `#[cfg(feature = "parity-real-vocab")]` block can be
//! extended to exercise the real charsmap end-to-end.

#![cfg(feature = "hf-tokenizer")]

use stringcheese_tokenizer_hf::normalizer::{Normalizer, PrecompiledNormalizer, normalize};

/// Build a charsmap blob (base64-encoded) from an in-memory Darts
/// trie plus normalized-string table.
///
/// The wire format is
/// `[trie_size: u32 LE][trie bytes][normalized bytes]`; see the
/// `precompiled` module docs for the full Darts node encoding.
fn build_charsmap(trie: &[u32], normalized: &[u8]) -> String {
    let mut blob = Vec::with_capacity(4 + trie.len() * 4 + normalized.len());
    let trie_bytes = u32::try_from(trie.len() * 4).unwrap();
    blob.extend_from_slice(&trie_bytes.to_le_bytes());
    for &u in trie {
        blob.extend_from_slice(&u.to_le_bytes());
    }
    blob.extend_from_slice(normalized);
    encode_base64_standard(&blob)
}

/// Standard base64 encoder (no line breaks, `=` padded). The test
/// intentionally uses a hand-rolled encoder so we do not depend on
/// the internal charsmap builder inside the `precompiled` module
/// (which is `#[cfg(test)]`-gated and not visible to integration
/// tests anyway).
fn encode_base64_standard(data: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHA[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHA[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() >= 2 {
            out.push(ALPHA[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() >= 3 {
            out.push(ALPHA[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Build the "Llama-flavoured mini" charsmap used by this file's
/// parity tests.
///
/// # Trie plan
///
/// The Darts double-array is dense — every reachable state is a slot
/// in `trie`, indexed by XOR-descent. Root sits at slot 0 with
/// offset 1 (so the initial descent puts us at slot 1). From there:
///
/// | Slot | Meaning                                        | Encoding      |
/// |------|------------------------------------------------|---------------|
/// | 0    | Root: offset=1, no label, no leaf              | `0x400`       |
/// | 2    | Leaf for `ab` → normalized[2] ("Y")            | `0x8000_0002` |
/// | 3    | `b` transition from `a`-state (slot 97),       |               |
/// |      | leaf, offset=1                                 | `0x562`       |
/// | 12   | `\r` (0x0D) transition, leaf, offset=1         | `0x50D`       |
/// | 13   | Leaf for `\r` → normalized[4] (empty string)   | `0x8000_0004` |
/// | 96   | `a` transition from root, leaf, offset=1       | `0x561`       |
/// | 97   | Leaf for `a` → normalized[0] ("X")             | `0x8000_0000` |
/// | 98   | `0xA0` transition from `0xC2`-state (slot 194),|               |
/// |      | leaf, offset=1                                 | `0x5A0`       |
/// | 99   | Leaf for NBSP (U+00A0) → normalized[5] (" ")   | `0x8000_0005` |
/// | 195  | `0xC2` transition from root, no leaf, offset=1 | `0x4C2`       |
///
/// The XOR arithmetic that lands each transition on its listed slot
/// is spelled out in the `precompiled` module's `hand_crafted_*`
/// tests; verify by walking the algorithm by hand. Sanity checks:
/// after matching `a`, `node_pos = 97`, so the follow-up `b`
/// transition lands at `97 XOR 0x62 = 3`; after matching `0xC2`,
/// `node_pos = 194`, so `0xA0` lands at `194 XOR 0xA0 = 98`.
///
/// # Normalized-string table
///
/// ```text
/// offset 0 → "X\0"      (replacement for `a`)
/// offset 2 → "Y\0"      (replacement for `ab`)
/// offset 4 → "\0"       (empty replacement — `\r` is deleted)
/// offset 5 → " \0"      (replacement for NBSP → ASCII space)
/// ```
fn llama_flavoured_charsmap() -> String {
    let mut trie = vec![0u32; 196];
    trie[0] = 0x400;
    trie[2] = 0x8000_0002; // leaf for "ab"
    trie[3] = 0x562; // 'b' transition from state 97 (post-'a')
    trie[12] = 0x50D; // '\r' transition
    trie[13] = 0x8000_0004; // leaf for '\r'
    trie[96] = 0x561; // 'a' transition from root
    trie[97] = 0x8000_0000; // leaf for 'a'
    trie[98] = 0x5A0; // 0xA0 transition from state 194 (post-0xC2)
    trie[99] = 0x8000_0005; // leaf for NBSP
    trie[195] = 0x4C2; // 0xC2 transition from root (no leaf)
    let normalized = b"X\0Y\0\0 \0";
    build_charsmap(&trie, normalized)
}

// ---------------------------------------------------------------------
// Parity: `PrecompiledNormalizer` direct API.
// ---------------------------------------------------------------------

/// Fixture: input, expected output, and a comment about the
/// transformation exercised. Kept in a single table so a diff shows
/// the whole parity surface at once.
const PARITY_FIXTURES: &[(&str, &str, &str)] = &[
    ("ab", "Y", "Longest-match: 'ab' beats the shorter 'a' key."),
    (
        "a",
        "X",
        "Shorter key still wins when the longer prefix is not present.",
    ),
    (
        "abc",
        "Yc",
        "Longest-match then passthrough on the unmatched trailing 'c'.",
    ),
    (
        "aab",
        "XY",
        "Match 'a' (no 'aa' key), then match 'ab' on the next scan.",
    ),
    (
        "\r",
        "",
        "Empty replacement string deletes the matched key.",
    ),
    (
        "a\rb",
        "Xb",
        "Deletion mid-string keeps the surrounding bytes intact.",
    ),
    (
        "\u{00A0}",
        " ",
        "Multi-byte key (NBSP, C2 A0) → single-byte replacement.",
    ),
    (
        "hello\u{00A0}world",
        "hello world",
        "NBSP fold inside a longer passthrough context.",
    ),
    (
        "cof\u{00E9}",
        "cof\u{00E9}",
        "Multi-byte scalar the trie does not touch passes through \
         byte-for-byte; no key ('a', 'ab', '\\r', NBSP) fires.",
    ),
    (
        "\u{4E2D}\u{6587}",
        "\u{4E2D}\u{6587}",
        "Three-byte CJK scalars pass through the trie untouched.",
    ),
    ("", "", "Empty input normalizes to empty output."),
    (
        "ba",
        "bX",
        "Prefix 'b' has no key; falls through to match 'a' next.",
    ),
];

#[test]
fn precompiled_normalizer_matches_hand_derived_parity_fixtures() {
    let normalizer = PrecompiledNormalizer::from_base64_charsmap(&llama_flavoured_charsmap())
        .expect("hand-crafted charsmap should parse");
    let mut failures = Vec::new();
    for (input, expected, comment) in PARITY_FIXTURES {
        let got = normalizer.normalize(input);
        if got != *expected {
            failures.push(format!(
                "input={input:?} expected={expected:?} got={got:?} — {comment}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} parity fixtures failed:\n{}",
        failures.len(),
        PARITY_FIXTURES.len(),
        failures.join("\n"),
    );
}

// ---------------------------------------------------------------------
// Parity: dispatch through the outer `Normalizer::Precompiled`
// variant (the real HF-loader entry point).
// ---------------------------------------------------------------------

#[test]
fn outer_normalizer_dispatch_matches_direct_api() {
    let charsmap = llama_flavoured_charsmap();
    let n = Normalizer::Precompiled {
        charsmap_base64: charsmap.clone(),
    };
    let direct = PrecompiledNormalizer::from_base64_charsmap(&charsmap).unwrap();
    for (input, expected, _) in PARITY_FIXTURES {
        assert_eq!(normalize(input, &n), *expected, "outer dispatch: {input:?}");
        assert_eq!(direct.normalize(input), *expected, "direct API: {input:?}");
    }
}

// ---------------------------------------------------------------------
// Parity: real-vocab harness — deferred behind the shared feature
// gate the tiktoken parity landing owns.
// ---------------------------------------------------------------------

// When the `parity-real-vocab` feature exists (added by the parallel
// tiktoken parity landing), extend this block to fetch a Meta-licensed
// Llama `tokenizer.json`, decode its Precompiled block, and diff the
// normalized output against a checked-in reference set. Left as a
// placeholder so the wiring is visible at grep time.
//
// #[cfg(feature = "parity-real-vocab")]
// #[test]
// fn llama_real_charsmap_matches_python_reference() {
//     // ...
// }
