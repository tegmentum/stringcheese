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
//! ## Baseline (2026-08-09)
//!
//! Encode throughput per target on 1 KB inputs, from
//! `stringcheese-bench/benches/escape.rs`:
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
