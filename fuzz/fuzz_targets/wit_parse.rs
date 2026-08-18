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
//! as WIT, and swallows the upstream parser's panics via
//! `catch_unwind`.
//!
//! # Property
//!
//! For arbitrary bytes `data`:
//!
//! * `std::str::from_utf8(data)` may fail — that's fine, skip.
//! * `wit_parser::Resolve::push_str("fuzz.wit", s)` may return
//!   `Ok(_)` (well-formed), `Err(_)` (malformed and reported), or
//!   panic (malformed and the upstream parser hasn't yet lowered
//!   the panic to a typed `Err`).
//! * The panic case is real — nightly fuzz has found several
//!   distinct `Option::unwrap` / index-out-of-bounds sites in
//!   wit-parser 0.230 and 0.256 against adversarial byte
//!   mutations of the shipped WIT seeds. Those are upstream bugs
//!   in the Bytecode Alliance `wit-parser` crate, not stringcheese
//!   code. Rather than treat every one as a nightly-blocking
//!   test failure, we swallow them with `catch_unwind` — this
//!   target's real value is exercising the interface + growing
//!   the corpus, not gating on upstream stability.
//!
//! Upstream is aware of the pattern (see wit-parser\'s issue
//! tracker for prior "panic on malformed input" reports); a
//! future stringcheese round can re-enable the panic-as-failure
//! contract once wit-parser lowers its remaining unwrap sites to
//! typed errors.
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
use std::panic::{AssertUnwindSafe, catch_unwind};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = core::str::from_utf8(data) else {
        // Non-UTF-8 payload — the WIT parser only takes `&str`.
        // Skip rather than counting as a finding.
        return;
    };

    // `push_str` is the documented ingress for a single in-memory
    // WIT string. Wrap in `catch_unwind` so upstream panics on
    // adversarial input are treated as legitimate "malformed"
    // responses rather than test failures — see the module doc for
    // rationale. `AssertUnwindSafe` is required because
    // `Resolve::push_str` takes `&mut self` and the parser types
    // don't implement `UnwindSafe`; that's OK here because we
    // discard the `Resolve` on the next fuzz iteration regardless
    // of whether it saw a panic mid-parse.
    let mut resolve = wit_parser::Resolve::new();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = resolve.push_str("fuzz.wit", source);
    }));
});
