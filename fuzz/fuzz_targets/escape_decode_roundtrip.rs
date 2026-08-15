//! Fuzz target: escape / unescape round-trip for the four target grammars.
//!
//! [`stringcheese_escape`] dispatches on an [`Escape`] enum: URI
//! component, HTML text, JSON string body, POSIX shell word. Three
//! of the four grammars support both `encode` and `decode`; the
//! fourth (`ShellWord`) is exercised in encode-only form because
//! shell-quote round-tripping is only defined for a subset of
//! inputs (NUL bytes cannot be represented, and empty-word encoding
//! rewrites the shape). This target asserts the invariant that
//! *is* always defined:
//!
//! # Property
//!
//! For arbitrary valid UTF-8 input `x` and grammar `g` ∈
//! {URI, HTML, JSON}:
//!
//! * `escape(x, g)` must not panic.
//! * `unescape(escape(x, g), g)` must return `Ok(y)` with `y == x`.
//!
//! For `g == Shell`:
//!
//! * `escape(x, g)` must not panic.
//!
//! The `encode(decode(y))` direction is deliberately not asserted:
//! decoding is a many-to-one map (`&#65;` and `&#x41;` both decode
//! to `A`; `%2F` and `%2f` both decode to `/`), so encoding after
//! decoding does not in general reproduce the input.
//!
//! # Input
//!
//! Bytes are laid out as `mode_byte || payload`:
//!
//! * Byte 0 (if present) selects the grammar via `byte % 4`:
//!     - 0 → `Escape::UriComponent`
//!     - 1 → `Escape::Html`
//!     - 2 → `Escape::JsonString`
//!     - 3 → `Escape::ShellWord` (encode-only)
//! * The remaining bytes are the payload. Non-UTF-8 payloads are
//!   skipped rather than counted as findings — the escape API only
//!   takes `&str`, and libFuzzer learns to produce valid UTF-8
//!   quickly from the checked-in seeds.
//!
//! # Invariant
//!
//! Neither `escape` nor `unescape` may panic on any input, including
//! empty payloads, all-metachar payloads, dense-control-code payloads,
//! or payloads containing already-encoded entity sequences. Any panic
//! is a robustness bug.

#![no_main]

use libfuzzer_sys::fuzz_target;
use stringcheese_escape::{Escape, escape, unescape};

fuzz_target!(|data: &[u8]| {
    // Empty input is a valid libFuzzer seed shape — nothing to
    // drive, but not a bug either. Return without touching the
    // escape API.
    let Some((&mode_byte, payload)) = data.split_first() else {
        return;
    };

    // Payload must be valid UTF-8 — the escape API only takes `&str`.
    // Skip non-UTF-8 payloads rather than tripping on a shape the
    // API cannot exercise.
    let Ok(input) = core::str::from_utf8(payload) else {
        return;
    };

    // Grammar dispatch. `% 4` keeps every byte value in range
    // without biasing toward any single grammar.
    let target = match mode_byte % 4 {
        0 => Escape::UriComponent,
        1 => Escape::Html,
        2 => Escape::JsonString,
        3 => Escape::ShellWord,
        _ => unreachable!("mode_byte % 4 is 0..=3"),
    };

    // Encode is required to not panic on every input under every
    // grammar. Shell is encode-only per the fuzz-target contract —
    // POSIX shells cannot represent embedded NUL bytes and shlex's
    // quote/unquote pair is not a bijection over arbitrary bytes.
    let encoded = escape(input, target);
    if matches!(target, Escape::ShellWord) {
        return;
    }

    // URI / HTML / JSON: decode must succeed and reproduce the input
    // verbatim. Any disagreement is the intentional fuzz signal —
    // either the encoder produced output the decoder rejects (a
    // typed-error path bug) or the decoder returned bytes that do
    // not match the encoder's input (a semantic bijection bug).
    let decoded = unescape(&encoded, target)
        .unwrap_or_else(|e| panic!("unescape rejected encode output for {target:?}: {e}"));
    assert_eq!(
        decoded, input,
        "round-trip mismatch for {target:?}: decode(encode({input:?})) == {decoded:?}"
    );
});
