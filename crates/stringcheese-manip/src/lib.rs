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
//! **All 15 modules now ship real implementations in v0.1.** The full
//! roster — [`inspect`], [`trim`], [`case`], [`split`], [`join`],
//! [`replace`], [`normalize`], [`mod@slice`], [`find`], [`pad`], [`lines`],
//! [`escape`], [`quote`], [`template`], and [`pipeline`] — carries the
//! surface described below; the earlier scaffold placeholder for
//! [`pipeline`] was filled in the wave that closed out the v0.1 charter.
//!
//! Depending on `stringcheese-manip` today is safe — the crate compiles
//! and re-releases will only *add* items, never remove them at this
//! pre-1.0 stage.
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
//! Every module below carries a real implementation in v0.1; see
//! `docs/DESIGN.md` for the full charter.
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
//! - [`pad`] — *shipped.* Pad to a target width: left, right, center; by
//!   scalar count or byte count. Display-width variants are scaffolded
//!   pending upstream `stringcheese-unicode` display-width support.
//! - [`mod@slice`] — *shipped.* Extract a substring at the right boundary:
//!   bytes (returns `Option` on non-boundary), scalars, graphemes; plus
//!   `take` / `drop` first-`n` / after-`n` helpers at every level.
//!   (Module name shadows the primitive `slice` for rustdoc-linking
//!   purposes; use `stringcheese_manip::slice::…` explicitly.)
//! - [`find`] — *shipped.* Locate matches: `find`, `find_all`, `rfind`,
//!   `contains`, `starts_with`, `ends_with`, `count_matches`, plus the
//!   multi-pattern `find_any` — thin ergonomic wrappers that delegate to
//!   the substring-search kernels in `stringcheese-compare::search`.
//! - [`escape`] — *shipped.* Encode for a target syntax: HTML, JSON,
//!   POSIX / Windows shell, RFC 3986 percent-encoding, C-string, regex.
//!   `Cow<str>` returns keep the common no-escape-needed path
//!   allocation-free.
//! - [`quote`] — *shipped.* Wrap in delimiters: single, double,
//!   backtick, angle, custom, and typographic curly variants; a
//!   `quote_smart` picker that minimises embedded escapes; a zero-alloc
//!   [`quote::unquote`] that strips any standard quote pair.
//! - [`lines`] — *shipped.* Line-oriented operations: iterate lines
//!   (with or without terminators), non-empty lines, prefix / suffix
//!   lines, trim per line, plus `dedent` and `indent` for Python-style
//!   block-level whitespace shaping.
//! - [`template`] — *shipped.* Placeholder substitution — `str.format`-
//!   lite. Strict, permissive, and positional variants over a
//!   [`template::TemplateContext`] trait; a streaming iterator form for
//!   writing directly to a `Write`. Not a general templating engine (no
//!   conditionals, loops, or filters); reach for `askama` / `handlebars`
//!   / `tera` for that.
//! - [`wrap`] — *shipped* (feature `line-breaking`). Word-wrap and
//!   reflow at UAX #14 line-break opportunities via
//!   [`stringcheese_unicode::line_breaks()`]. Column width is measured
//!   per Unicode East Asian Width so CJK ideographs count as two
//!   columns and combining marks as zero. Offers greedy first-fit
//!   [`wrap::wrap_at_width`] and its borrowed
//!   [`wrap::wrap_at_width_borrowed`] variant, [`wrap::fill`],
//!   [`wrap::reflow`] (paragraph-aware), and a [`wrap::WrapOptions`]
//!   builder for indents and force-break control.
//! - [`pipeline`] — *shipped.* [`pipeline::TextPipeline`], a
//!   transformation IR that stages multiple [`pipeline::Operation`]s
//!   for one-pass application over a ping-pong buffer pair. Concrete
//!   operations wrap the shipping modules ([`pipeline::Trim`],
//!   [`pipeline::Normalize`], [`pipeline::CaseFold`],
//!   [`pipeline::CollapseWhitespace`], [`pipeline::Remove`],
//!   [`pipeline::Replace`], [`pipeline::Escape`], [`pipeline::Truncate`]);
//!   each carries a `name()` for introspection and budget-limited ops
//!   short-circuit the pipeline when their limit is reached.
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

// All 15 modules ship real implementations. The pipeline module is
// alloc-gated because it stores `Box<dyn Operation>` stages in a
// heap-allocated `Vec`.
pub mod case;
pub mod escape;
pub mod find;
pub mod inspect;
pub mod join;
pub mod lines;
pub mod normalize;
pub mod pad;
pub mod pipeline;
pub mod quote;
pub mod replace;
pub mod slice;
pub mod split;
pub mod template;
pub mod trim;
// UAX #14-based word-wrap and reflow. Feature-gated on
// `line-breaking`, which forwards to `stringcheese-unicode`'s
// `line-breaking` feature and pulls in `unicode-width` for East Asian
// Width-aware column measurement. Callers who do not need wrapping
// can build with `--no-default-features --features std` and skip both
// the UAX #14 property table and the East Asian Width lookup.
#[cfg(feature = "line-breaking")]
pub mod wrap;

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-manip` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
