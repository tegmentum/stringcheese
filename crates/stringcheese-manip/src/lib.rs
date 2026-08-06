//! StringCheese-manip — string manipulation for the StringCheese toolkit.
//!
//! This crate is the manipulation half of the StringCheese charter — the
//! companion to [`stringcheese-compare`](https://docs.rs/stringcheese-compare)
//! for computing distances/similarities. Where `stringcheese-compare` answers
//! *"how similar are these two strings?"*, this crate answers *"how do I
//! shape one string into another?"* with a coherent, Unicode-aware,
//! allocation-conscious API.
//!
//! # Status
//!
//! **First wave shipped in v0.1.** Three modules — [`inspect`], [`trim`],
//! and [`case`] — carry real implementations; every other module below is
//! still a placeholder whose doc comment records the intended scope. Items
//! land in follow-on releases, module-by-module.
//!
//! Depending on `stringcheese-manip` today is safe — the crate compiles
//! and re-releases will only *add* items, never remove them at this
//! pre-1.0 stage. Callers who need manipulation the shipped modules do not
//! yet cover should reach for the standard library and re-evaluate as more
//! modules populate.
//!
//! # Design commitments
//!
//! - **Unicode semantics are explicit at every boundary.** Every function
//!   that touches text names the boundary it works at — bytes, USVs
//!   (Unicode Scalar Values / `char`), extended grapheme clusters, or
//!   display width. No silent choices, no "just call `chars()` and hope".
//! - **Allocation-conscious layered API.** Where the operation permits,
//!   both a borrowed variant (returns `&str` / `Cow<str>`) and an owned
//!   variant (returns `String`) are exposed, plus an iterator form for
//!   streaming and an `into_output` form for reusing caller-owned
//!   buffers.
//! - **Four API levels** (from most casual to most explicit):
//!   1. Free functions on `&str` — the pleasant default: `manip::trim(s)`.
//!   2. Extension trait — dot-syntax for the ergonomic case:
//!      `s.stringcheese_trim()`.
//!   3. Configured operations — reusable, allocation-free value types:
//!      `Trim::new().with_edges(&['/', ' '])`.
//!   4. Declarative pipelines — `TextPipeline`, a transformation IR
//!      that stages multiple operations for one-pass application; see
//!      the [`pipeline`] module.
//!
//! # Module map
//!
//! Modules marked *shipped* below carry real implementations in v0.1;
//! the remainder are placeholders whose doc comment records the scope.
//! See `docs/DESIGN.md` for the full charter.
//!
//! - [`inspect`] — *shipped.* Read-only interrogation: is-empty, byte /
//!   scalar / grapheme count, first / last character, first / last
//!   grapheme. Every function names the boundary it works at and
//!   performs zero allocation.
//! - [`trim`] — *shipped.* Remove characters from the edges:
//!   whitespace-trim, char-set trim, predicate trim, both a family of
//!   zero-allocation free functions and a reusable [`trim::Trim`]
//!   configured operation.
//! - [`case`] — *shipped.* Case transformations that respect Unicode:
//!   `to_lowercase`, `to_uppercase`, `to_title_case`, `capitalize`, plus
//!   `_into` variants that append into a caller-owned buffer and ASCII
//!   fast paths for callers who know their input is ASCII. Delegates to
//!   `stringcheese-unicode` for grapheme iteration.
//! - [`split`] — divide a string into pieces: by scalar, by pattern,
//!   by predicate, by Unicode boundary (word / sentence / grapheme).
//! - [`join`] — combine pieces back: `Vec<&str>` join, iterator join,
//!   allocation-free join over a pre-sized buffer.
//! - [`replace`] — substitute matches: single-shot, all-instances,
//!   pattern-callback, budget-limited.
//! - [`normalize`] — canonicalize shape: collapse whitespace, normalize
//!   line endings, strip control characters, NFC/NFD/NFKC/NFKD (delegated
//!   to `stringcheese-unicode`).
//! - [`pad`] — pad to a target width: left, right, center; by scalar
//!   width or display width.
//! - `slice` — extract a substring at the right boundary: bytes, scalars,
//!   graphemes; safe against splitting a multi-scalar grapheme.
//!   (Module name shadows the primitive `slice` for rustdoc-linking
//!   purposes; use `stringcheese_manip::slice::…` explicitly.)
//! - [`find`] — locate matches: `find`, `find_all`, `contains`,
//!   `starts_with`, `ends_with` — thin ergonomic wrappers over the
//!   substring-search kernels in `stringcheese-compare::search`.
//! - [`escape`] — encode for a target syntax: HTML, JSON, shell,
//!   percent-encoding, C-string.
//! - [`quote`] — wrap in delimiters: single, double, backtick, custom;
//!   with escaping to make the result parseable.
//! - [`lines`] — line-oriented operations: iterate lines, non-empty
//!   lines, prefix / suffix lines, trim per line.
//! - [`template`] — placeholder substitution: `{name}` interpolation
//!   from a map, positional `{0}` args, escape rules for literal
//!   braces.
//! - [`pipeline`] — `TextPipeline`, a transformation IR that stages
//!   multiple operations for one-pass application. Operations expose
//!   their memory footprint and are combinable into a single fused
//!   transform.
//!
//! # Not in this crate
//!
//! - **Comparison, similarity, distance, alignment, phonetic keys** —
//!   see `stringcheese-compare` and the other comparison-family crates.
//! - **Regex engines** — [`find`] and [`replace`] accept `Pattern`s
//!   in the sense of `str::find` (chars, closures, `&str` needles);
//!   full regex is a separate library.
//! - **I/O, streaming from readers** — manipulation is over in-memory
//!   `&str` / `String`. Reader-driven manipulation is a downstream
//!   concern.
//! - **Locale-specific rules that need CLDR data** — those live in the
//!   opt-in `stringcheese-<language>` packs (e.g., `stringcheese-en`,
//!   `stringcheese-de`); the core `stringcheese-manip` API is
//!   language-agnostic.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

// `alloc` is brought into scope for the modules that need heap-allocating
// types: the owned-`String`-returning case transformations, the
// `Trim` configured operation, and any inspect helper that delegates to
// `stringcheese-unicode` (which itself requires alloc).
#[cfg(feature = "alloc")]
extern crate alloc;

pub mod case;
pub mod inspect;
pub mod trim;

/// Divide a string into pieces: by scalar, by pattern, by predicate,
/// by Unicode boundary (word / sentence / grapheme).
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod split {}

/// Combine pieces back: `Vec<&str>` join, iterator join,
/// allocation-free join over a pre-sized buffer.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod join {}

/// Substitute matches: single-shot, all-instances, pattern-callback,
/// budget-limited.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod replace {}

/// Canonicalize shape: collapse whitespace, normalize line endings,
/// strip control characters, NFC/NFD/NFKC/NFKD (delegated to
/// `stringcheese-unicode`).
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod normalize {}

/// Pad to a target width: left, right, center; by scalar width or
/// display width.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod pad {}

/// Extract a substring at the right boundary: bytes, scalars, graphemes;
/// safe against splitting a multi-scalar grapheme.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod slice {}

/// Locate matches: `find`, `find_all`, `contains`, `starts_with`,
/// `ends_with` — thin ergonomic wrappers over the substring-search
/// kernels in `stringcheese-compare::search`.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod find {}

/// Encode for a target syntax: HTML, JSON, shell, percent-encoding,
/// C-string.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod escape {}

/// Wrap in delimiters: single, double, backtick, custom; with escaping
/// to make the result parseable.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod quote {}

/// Line-oriented operations: iterate lines, non-empty lines, prefix /
/// suffix lines, trim per line.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod lines {}

/// Placeholder substitution: `{name}` interpolation from a map,
/// positional `{0}` args, escape rules for literal braces.
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod template {}

/// Declarative transformation pipeline — `TextPipeline` IR that
/// stages multiple operations for one-pass application.
///
/// Operations (`Trim`, `Normalize`, `CaseFold`, `CollapseWhitespace`,
/// `Remove`, `Replace`, ...) expose their memory footprint and are
/// combinable into a single fused transform. The IR is inspectable
/// (each stage's `Debug` names the operation) and re-orderable
/// (independent stages can be swapped without changing the result).
///
/// # Status
///
/// Scaffold only — no items shipped yet.
pub mod pipeline {}

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-manip` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
