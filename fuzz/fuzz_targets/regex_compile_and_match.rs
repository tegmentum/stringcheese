//! Fuzz target: `stringcheese_pattern_regex::Regex` compile + `is_match`.
//!
//! Regex compilation of untrusted patterns is a classic DoS / panic
//! vector — malformed escapes, missing brackets, unbalanced groups,
//! adversarial repetition counts, and invalid Unicode property
//! references all live along that boundary. The wrapped `regex` crate
//! handles most of these gracefully (returning a typed error), but
//! `stringcheese-pattern-regex` is a *boundary layer* with its own
//! constructor pipeline (`Regex::new`, `bytes`, `case_insensitive`,
//! `literal`) and its own error-mapping shape. Any panic in that layer
//! is a robustness bug; this target hands libFuzzer arbitrary bytes,
//! drives one of the four constructors, and — when the compile
//! succeeds — also calls `is_match` on a second slot of the input.
//!
//! ## Input format
//!
//! Bytes are laid out as `mode || pattern || 0x00 || haystack`:
//!
//! * Byte 0 (if present) selects the constructor mode via `byte % 4`:
//!     - 0 → `Regex::new`             (Unicode default)
//!     - 1 → `Regex::bytes`           (`(?-u)` prepended by the wrapper)
//!     - 2 → `Regex::case_insensitive` (`(?i)` prepended by the wrapper)
//!     - 3 → `Regex::literal`         (auto-escaped via `regex::escape`)
//! * The remaining bytes are split on the first `0x00` byte: everything
//!   before the NUL is the pattern text; everything after is the
//!   haystack. Missing NUL → whole tail is the pattern, empty haystack.
//! * Pattern and haystack must both be valid UTF-8 — the wrapped
//!   `regex::Regex::new` only accepts `&str`. Non-UTF-8 slots are
//!   skipped rather than counted as findings; libFuzzer learns to
//!   produce valid UTF-8 quickly from the seed corpus (all seeds are
//!   valid UTF-8 by construction).
//!
//! ## Invariant
//!
//! For any input: the constructor returns either `Ok(Regex)` or a typed
//! `RegexError`. When it returns `Ok`, `is_match` on that compiled regex
//! must not panic on any UTF-8 haystack.

#![no_main]

use libfuzzer_sys::fuzz_target;
use stringcheese_pattern::Pattern;
use stringcheese_pattern_regex::Regex;

fuzz_target!(|data: &[u8]| {
    // Empty input is a valid libFuzzer seed shape — nothing to drive,
    // but not a bug either. Return without touching the constructors.
    let Some((&mode_byte, tail)) = data.split_first() else {
        return;
    };

    // Split the tail on the first NUL byte: pattern before, haystack
    // after. No NUL → whole tail is the pattern, empty haystack.
    let (pattern_bytes, haystack_bytes) = match tail.iter().position(|&b| b == 0) {
        Some(idx) => (&tail[..idx], &tail[idx + 1..]),
        None => (tail, &[][..]),
    };

    // Both slots must be valid UTF-8 — the wrapped `regex::Regex::new`
    // only takes `&str`. Skip non-UTF-8 inputs rather than tripping
    // the fuzzer on a shape it cannot exercise.
    let Ok(pattern) = core::str::from_utf8(pattern_bytes) else {
        return;
    };
    let Ok(haystack) = core::str::from_utf8(haystack_bytes) else {
        return;
    };

    // The constructor pipeline. Mode `% 4` keeps every byte value in
    // range without biasing toward any single constructor.
    let compiled = match mode_byte % 4 {
        0 => Regex::new(pattern),
        1 => Regex::bytes(pattern),
        2 => Regex::case_insensitive(pattern),
        3 => Regex::literal(pattern),
        _ => unreachable!("mode_byte % 4 is 0..=3"),
    };

    // Ok/Err are both fine; a typed `RegexError` is the wrapper's
    // documented shape for any syntactic problem. Only unwind panics
    // fail the target — libFuzzer catches them via `fuzz_target!`.
    let Ok(re) = compiled else {
        return;
    };

    // Drive the primary matching entry point on the compiled regex.
    // The `Pattern` trait's `is_match` should never panic on any
    // valid `&str` haystack.
    let _ = re.is_match(haystack);
});
