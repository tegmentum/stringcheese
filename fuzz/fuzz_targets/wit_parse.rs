//! Fuzz target: WIT parser robustness.
//!
//! The WebAssembly Component Model WIT syntax is parsed at build
//! time by [`wit_parser`] as part of the `stringcheese` component
//! bindings pipeline. The WIT files under `component/wit/` are the
//! load-bearing contract between the Rust host and any non-Rust
//! caller; a panic in the parser on adversarial input would break
//! that pipeline (or, worse, a hostile caller supplying an
//! unexpectedly-shaped WIT payload could take down a tool that
//! consumes external WIT).
//!
//! This target hands libFuzzer arbitrary bytes, tries to parse them
//! as WIT, and asserts the parser never panics — every malformed
//! input must surface as a typed error, not an abort.
//!
//! # Property
//!
//! For arbitrary bytes `data`:
//!
//! * `std::str::from_utf8(data)` may fail — that's fine, skip.
//! * `wit_parser::Resolve::push_str("fuzz.wit", s)` may return
//!   `Err(_)` — that's the documented shape for any syntactically
//!   invalid WIT.
//! * Neither call may panic on any input.
//!
//! # Input
//!
//! Raw bytes, interpreted as UTF-8 when possible. Non-UTF-8
//! payloads are skipped — WIT source is defined over Unicode
//! scalars and the parser only accepts `&str`. libFuzzer learns
//! to produce valid UTF-8 quickly from the checked-in seeds (all
//! seeds are valid UTF-8 by construction).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = core::str::from_utf8(data) else {
        // Non-UTF-8 payload — the WIT parser only takes `&str`.
        // Skip rather than counting as a finding.
        return;
    };

    // `push_str` is the documented ingress for a single in-memory
    // WIT string; a fresh `Resolve` starts empty, so no cross-input
    // state leaks between fuzz iterations. Any well-formed input
    // returns `Ok(_)`; any malformed input returns `Err(_)`; any
    // panic is a robustness bug.
    let mut resolve = wit_parser::Resolve::new();
    let _ = resolve.push_str("fuzz.wit", source);
});
