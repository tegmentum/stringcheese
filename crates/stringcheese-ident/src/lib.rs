//! # Identifier and slug utilities
//!
//! Case conversion, case detection, slug generation, and identifier
//! sanitization — the "convert this human string to something a
//! machine can index / URL-encode / use as a variable name" family.
//!
//! ## Contents
//!
//! - [`Case`] — the target/source case discipline (`Snake`, `Camel`,
//!   `Pascal`, `Kebab`, `ScreamingSnake`, `Train`). Explicit at every
//!   conversion call — no silent choice.
//! - [`to_case`] — dispatch a source string to its `Case` form.
//!   Wraps [`heck`] because case conversion is exactly the kind of
//!   small, mature engine we shouldn't reimplement.
//! - [`Case::detect`] — best-effort classification of an input's
//!   convention (for round-trip / preserve-caller-choice pipelines).
//! - [`slugify`] / [`Slugger`] — Unicode-to-ASCII slug generation
//!   with a configurable separator. Uses [`deunicode`] for the
//!   transliteration step.
//! - [`Sanitizer`] — replace non-identifier characters, enforce a
//!   valid start character, cap length.
//!
//! ## Baseline (2026-08-09)
//!
//! Numbers from `stringcheese-bench/benches/ident.rs`:
//!
//! | Surface                | throughput (4 KB input) |
//! |------------------------|-------------------------|
//! | `Case::detect`         | 10.5 GiB/s              |
//! | `slugify` (plain ASCII)| ~570 MiB/s              |
//! | `Sanitizer::sanitize`  | ~435 MiB/s              |
//! | `slugify` (accented)   | ~270 MiB/s              |
//! | `to_case` (any variant)|  ~67 MiB/s              |
//!
//! `Case::detect` is memory-bound — a byte-level scan with no
//! allocation. `slugify` and `Sanitizer` both sit in the
//! 400-600 MiB/s band on ASCII; `slugify` on accented input
//! drops 2× because `deunicode`'s per-scalar transliteration
//! table dominates. `to_case` is the slowest surface here — the
//! `heck` wrap costs ~10× more per byte than the in-house
//! scanners, reflecting heck's per-word word-boundary detection
//! and re-encoding. If a caller hits the case-conversion
//! ceiling on a hot path, the fix is a bespoke word-boundary
//! walker, not a heck upgrade — heck's algorithm is fine, it's
//! just doing more work than a simple filter.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod case;
pub mod sanitize;
pub mod slug;

pub use case::{Case, to_case};
pub use sanitize::Sanitizer;
pub use slug::{Slugger, slugify};
