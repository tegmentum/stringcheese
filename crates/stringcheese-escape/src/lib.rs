//! # Escape and quoting utilities
//!
//! Turn arbitrary text into something safe to interpolate into a
//! specific target grammar — URIs, HTML, JSON strings, POSIX shell
//! commands. One [`Escape`] enum names the target; [`escape`] and
//! [`unescape`] dispatch on it.
//!
//! ## Contents
//!
//! - [`Escape`] — grammar the input is being embedded into.
//! - [`escape`] — encode a string for `Escape::…`.
//! - [`unescape`] — decode a string in `Escape::…`. Returns
//!   [`UnescapeError`] when the target grammar reports a syntax
//!   error and cannot be reversed unambiguously.
//!
//! ## Design
//!
//! Every variant delegates to a mature ecosystem crate — this is
//! a "wrap when the underlying problem is well-understood and the
//! upstream is well-audited" case. Our value is the discipline:
//! explicit target grammar at the call site, uniform output type,
//! and errors that carry enough context for the caller to decide
//! what to do.
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/escape.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-escape
//! ```
//!
//! Four groups drive the four [`Escape`] targets ([`Escape::UriComponent`],
//! [`Escape::Html`], [`Escape::JsonString`], [`Escape::ShellWord`]).
//! Each group runs three input byte lengths (256 B / 1 KiB / 4 KiB)
//! crossed with two flavors (`plain` — ASCII prose with no
//! metacharacters — and `meta` — dense with characters every target
//! has to escape). 24 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`);
//! wall-clock samples vary ±10-20 %. Throughput reported over
//! *input bytes* — higher is better.
//!
//! ```text
//! target  / flavor  / 256 B         / 1 KiB        / 4 KiB
//! ----------------------------------------------------------
//! json    / plain   / ~1.35 GiB/s   / ~1.75 GiB/s  / ~1.82 GiB/s
//! json    / meta    / ~580 MiB/s    / ~635 MiB/s   / ~670 MiB/s
//! uri     / plain   / ~335 MiB/s    / ~388 MiB/s   / ~437 MiB/s
//! uri     / meta    / ~265 MiB/s    / ~312 MiB/s   / ~325 MiB/s
//! html    / plain   / ~1.01 GiB/s   / ~1.13 GiB/s  / ~1.19 GiB/s
//! html    / meta    / ~600 MiB/s    / ~730 MiB/s   / ~772 MiB/s
//! shell   / plain   / ~770 MiB/s    / ~820 MiB/s   / ~690 MiB/s
//! shell   / meta    / ~190 MiB/s    / ~322 MiB/s   / ~346 MiB/s
//! ```
//!
//! Read:
//!
//! * **`json` leads on plain input** — the in-house 128-entry ASCII
//!   table plus bulk-copy passthrough is the fastest arm at ~1.8
//!   GiB/s on 4 KiB inputs. This is the bench that drove the
//!   2026-08-09 in-house rewrite over the original per-scalar
//!   match; the current numbers stand as the regression trip-wire.
//! * **`html` on plain sits comfortably above 1 GiB/s** and drops
//!   only ~30-40 % on `meta` — the entity substitution set is
//!   small (`& < > " ' /`) so the fast path stays hot even when
//!   most bytes need to be inspected.
//! * **`uri` on the shared `plain` seed sits at ~340-440 MiB/s**
//!   — not a passthrough number despite the flavor name. The
//!   `NON_ALPHANUMERIC` encode set treats space (0x20) as
//!   needing `%20`, so the shared prose seed
//!   (`"the quick brown fox …"`, ~18 % spaces) breaks the URI
//!   passthrough fast path on every space. HTML/JSON/shell
//!   don't consider space a metachar, so their fast paths still
//!   fire on the same seed — which is why only URI looks like a
//!   regression against the earlier all-alphanumeric baseline
//!   (1.41 GiB/s, see the prior table below). The percent-encoding
//!   hot path itself is unchanged: a probe on all-alphanumeric
//!   input reproduces ~1.4-1.6 GiB/s on the current crate
//!   version. The URI numbers here are the honest cost of
//!   percent-encoding realistic prose, not a hot-path regression.
//! * **`shell` on `meta` drops 3-4×** — every embedded single quote
//!   forces a backslash-escape and the whole input is wrapped in
//!   quotes. `shlex`'s per-scalar quoting logic dominates. A
//!   follow-up round could measure an in-house shell-quote path
//!   against the wrap.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.
//!
//! ## Prior baseline (2026-08-09, `stringcheese-bench/benches/escape.rs`)
//!
//! Retained for context — earlier 1 KB-only rows against which the
//! current suite's numbers should be compared:
//!
//! | Target             | plain (safe input) | metachar-heavy |
//! |--------------------|--------------------|----------------|
//! | `JsonString`       | 1.78 GiB/s         | 1.06 GiB/s     |
//! | `UriComponent`     | 1.41 GiB/s         |   662 MiB/s    |
//! | `Html`             |   989 MiB/s        |   942 MiB/s    |
//! | `ShellWord`        |   709 MiB/s        |   471 MiB/s    |
//!
//! `JsonString` (the one **in-house** grammar) now leads on
//! plain input. The initial implementation was 3-4× slower than
//! the URI wrap — the bench flagged that regression, and the
//! fix was a 128-entry static ASCII lookup table plus a
//! bulk-copy path for runs of passthrough bytes (see
//! `src/json.rs`'s "Implementation" note). The redesign closed
//! the gap and then some, thanks to the fact that ASCII escapes
//! are the only work JSON needs to do (U+0080+ scalars pass
//! through unchanged).
//!
//! `UriComponent` (wraps `percent-encoding`) still wins on
//! metachar-heavy input at the smallest sizes because its
//! SIMD-accelerated table lookups amortise the per-metachar
//! substitution cost differently — the wrap decision remains
//! correct for URI: `percent-encoding` is well-audited and
//! carries its own optimisation budget.
//!
//! The wrap-vs-in-house tradeoff for JSON is now data-driven,
//! not speculation: in-house wins on throughput AND avoids
//! `serde_json`'s ~500 KB compile footprint. If a future JSON
//! variant needs backslash-N SIMD or per-nybble table lookups,
//! the in-house path leaves that door open in a way a wrap
//! wouldn't.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod html;
pub mod json;
pub mod shell;
pub mod uri;

use alloc::string::String;

/// The target grammar an escape/unescape operation is speaking.
///
/// Passed to [`escape`] and [`unescape`]. The enum stays small on
/// purpose — grammars with distinct rulesets (e.g. HTML attribute
/// vs. HTML text content) live under their per-module namespace
/// where the ruleset is explicit rather than smuggled into an
/// enum variant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Escape {
    /// **URI component** — percent-encode every reserved character.
    /// Matches the encoding used for path segments, query keys, and
    /// query values. Wraps [`percent_encoding`] with the
    /// `NON_ALPHANUMERIC` set.
    UriComponent,

    /// **HTML text content** — escapes `& < > " ' /`. Safe to embed
    /// inside a tag's text body. See [`html`] for attribute-context
    /// helpers.
    Html,

    /// **JSON string body** — escapes `" \ \b \f \n \r \t` and any
    /// C0 control code. Does NOT add the surrounding quotes;
    /// callers add them when embedding.
    JsonString,

    /// **POSIX shell word** — wraps the input in single quotes and
    /// backslash-escapes any embedded single quote. Safe against
    /// every shell metacharacter. Wraps [`shlex`].
    ShellWord,
}

/// Escape `input` for `target`.
///
/// See each variant's docs for the exact escape ruleset.
#[must_use]
pub fn escape(input: &str, target: Escape) -> String {
    match target {
        Escape::UriComponent => uri::encode(input),
        Escape::Html => html::escape_text(input),
        Escape::JsonString => json::escape(input),
        Escape::ShellWord => shell::quote(input),
    }
}

/// Reverse [`escape`] for grammars that support it.
///
/// # Errors
///
/// Returns [`UnescapeError`] when the input isn't valid under
/// `target`'s decode grammar (malformed percent-triplet, unknown
/// HTML entity in strict mode, bad JSON escape sequence).
pub fn unescape(input: &str, target: Escape) -> Result<String, UnescapeError> {
    match target {
        Escape::UriComponent => uri::decode(input).map_err(UnescapeError::Uri),
        Escape::Html => Ok(html::unescape(input)),
        Escape::JsonString => json::unescape(input).map_err(UnescapeError::Json),
        Escape::ShellWord => shell::unquote(input).ok_or(UnescapeError::Shell),
    }
}

/// One reason an [`unescape`] call couldn't reverse its input.
///
/// Carries per-grammar context so the caller can decide whether to
/// surface, retry with a different target, or hard-fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnescapeError {
    /// URI decoding failed — malformed percent-triplet or invalid
    /// UTF-8 after decoding.
    Uri(uri::DecodeError),
    /// JSON string decoding failed — bad `\` escape sequence.
    Json(json::UnescapeError),
    /// Shell word decoding failed — unmatched quote or dangling
    /// backslash.
    Shell,
}

impl core::fmt::Display for UnescapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Uri(e) => write!(f, "URI decode error: {e}"),
            Self::Json(e) => write!(f, "JSON string decode error: {e}"),
            Self::Shell => write!(f, "shell word decode error: unmatched quote or bad escape"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnescapeError {}
