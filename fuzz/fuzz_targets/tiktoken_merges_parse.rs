//! Fuzz target: tiktoken plaintext `mergeable_ranks` parser.
//!
//! The upstream OpenAI tiktoken format is a plaintext file with one
//! `<base64-encoded-bytes> <u32-rank>` per line. The workspace parses
//! this format into a `(vocab, merges)` pair via
//! [`build_scud_from_tiktoken`], which is a parser boundary over
//! attacker-shaped bytes: malformed base64, missing rank tokens, ranks
//! that overflow `u32`, tab-vs-space delimiters, CRLF vs LF endings,
//! embedded NULs, and non-UTF-8 sequences are all classic bug-attractor
//! inputs for a hand-rolled text parser. This target hands libFuzzer
//! arbitrary bytes and asserts that the parser returns either `Ok` or a
//! typed `Err(String)` — never panics, never over-reads its input.
//!
//! Non-UTF-8 inputs are *not* skipped early: the parser accepts a
//! `&[u8]` and does its own UTF-8 check, so libFuzzer's raw bytes
//! exercise both the UTF-8 rejection path and the successful-parse
//! path.
//!
//! Trust model: the plaintext format is only consumed by
//! contributors who bring their own `mergeable_ranks.tiktoken` file
//! into `data/<variant>.tiktoken` for local pack regeneration. Even so,
//! a panic on malformed bytes is a robustness bug — the crate's
//! `#![forbid(unsafe_code)]` makes any surviving panic a logic bug in
//! the parser's length or arithmetic accounting rather than a memory
//! hazard.

#![no_main]

use libfuzzer_sys::fuzz_target;
use stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken;

fuzz_target!(|data: &[u8]| {
    // The invariant: for any input, the parser returns Ok or a typed
    // Err(String). Any panic — from base64 arithmetic, UTF-8 handling,
    // rank overflow, or the merge-synthesis walk — is a bug.
    let _ = build_scud_from_tiktoken(data);
});
