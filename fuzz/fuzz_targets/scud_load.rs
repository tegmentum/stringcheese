//! Fuzz target: `SCUD` binary loader.
//!
//! Parsers over untrusted-shaped binary blobs are the classic
//! bug-attractor: length-prefix underflow, offset overflow, truncated
//! headers, and unexpected UTF-8 all hide behind a single call to
//! [`ScudFile::from_slice`]. This target hands libFuzzer arbitrary bytes
//! and asserts that the loader either succeeds or returns a typed
//! [`ScudError`] — never panics, never over-reads the buffer.
//!
//! When the outer parse succeeds, the target also drives every
//! capability-projection accessor (`as_case_data`, `as_collation_data`,
//! ...). Only the accessor whose capability tag matches the file's
//! `cap_id` runs the inner section-frame walk; the others exercise the
//! `CapabilityMismatch` error path. This coverage matters — the
//! section reader has its own truncation / overflow surface that would
//! not be reached from the header-only path.
//!
//! Trust model: the crate's docs describe SCUD as *trusted input*
//! (language packs ship their SCUD via `include_bytes!`). Fuzzing
//! trusted-input parsers is still worth doing — a panic on malformed
//! bytes is a robustness bug regardless of provenance, and the ambient
//! `#![forbid(unsafe_code)]` on the crate means any surviving panic is
//! a logic bug in the length arithmetic rather than a memory-safety
//! hazard.

#![no_main]

use libfuzzer_sys::fuzz_target;
use stringcheese_scud::ScudFile;

fuzz_target!(|data: &[u8]| {
    // Primary entry: header parse over an arbitrary byte slice.
    let Ok(file) = ScudFile::from_slice(data) else {
        // Malformed input → typed error, which is the expected shape.
        return;
    };

    // Exercise every capability-projection accessor. All but the one
    // matching `file.capability()` should return `CapabilityMismatch`;
    // the matching one drives the inner section-frame walk.
    let _ = file.as_case_data();
    let _ = file.as_collation_data();
    let _ = file.as_plural_data();
    let _ = file.as_number_data();
    let _ = file.as_datetime_data();
    let _ = file.as_break_data();
    let _ = file.as_linebreak_data();

    // Touch the trivial accessors too — they consult the parsed header
    // fields and must not panic on any file the loader accepted.
    let _ = file.magic();
    let _ = file.format_version();
    let _ = file.flags();
    let _ = file.capability();
    let _ = file.cldr_version();
    let _ = file.locale();
    let _ = file.body();
    let _ = file.len();
    let _ = file.is_empty();
});
