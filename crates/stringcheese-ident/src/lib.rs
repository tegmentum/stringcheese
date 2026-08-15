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
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/ident.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-ident
//! ```
//!
//! Four groups drive the four public surfaces: `to_case` (case
//! conversion, four target variants swept per cell), `detect`
//! (case classification), `slugify` (deunicode → filter →
//! collapse pipeline), `sanitize` (identifier char-policy filter).
//! Each group runs two input byte lengths (256 B / 4 KiB) crossed
//! with two flavors (`ascii` and `accented`). `to_case` adds a
//! 4-way sweep over [`Case`] targets. 20 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`);
//! wall-clock samples vary ±10-20 %. Throughput reported over
//! *input bytes* — higher is better.
//!
//! ```text
//! surface           / flavor    / 256 B         / 4 KiB
//! -----------------------------------------------------------
//! detect            / ascii     / ~630 MiB/s    / ~670 MiB/s
//! detect            / accented  / ~2.2 GiB/s    / ~12 GiB/s
//! slugify           / ascii     / ~530 MiB/s    / ~640 MiB/s
//! slugify           / accented  / ~256 MiB/s    / ~270 MiB/s
//! sanitize          / ascii     / ~445 MiB/s    / ~470 MiB/s
//! sanitize          / accented  / ~500 MiB/s    / ~540 MiB/s
//! to_case  ASCII    / any target/ ~180-390 MiB/s/ ~395-440 MiB/s
//! to_case  non-ASCII/ any target/ ~50-70 MiB/s  / ~50-70 MiB/s
//! ```
//!
//! Read:
//!
//! * **`detect` ASCII sits at ~650 MiB/s** — a full byte-level scan
//!   that walks the entire input to verify the `snake_case`
//!   convention. The `accented` flavor short-circuits early
//!   (mixed-case-plus-diacritics doesn't match any convention;
//!   `detect` returns `None` after checking a handful of scalars),
//!   which is why the accented column reports "throughput" numbers
//!   in the multi-GiB/s range — the load-bearing signal there is
//!   the ~100 ns fixed early-exit cost, not the per-byte scan
//!   rate.
//! * **`slugify` and `sanitize` sit in the 450-640 MiB/s band on
//!   ASCII**. `slugify` drops 2× on `accented` because `deunicode`'s
//!   per-scalar transliteration table walks the non-ASCII scalars;
//!   `sanitize` is actually *faster* on `accented` in this bench
//!   because the accented shape has more short words → more
//!   filtered characters → shorter output allocation per input
//!   byte.
//! * **`to_case` splits ASCII from non-ASCII.** On ASCII input a
//!   hand-rolled byte-level word-boundary scanner (see
//!   [`case`]'s `ASCII fast path` section) replaces heck's
//!   Unicode-category dispatch and lifts throughput from the
//!   ~65-80 MiB/s heck band to ~400-440 MiB/s at 4 KiB — a
//!   roughly 5-8× win, byte-for-byte identical to the heck path
//!   (differential-tested against every quirk in heck's own
//!   digit-boundary / all-caps corpus). Non-ASCII input still
//!   takes heck and sits in the same 50-70 MiB/s band because the
//!   Unicode boundary detection dominates over the emit-formatting.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.
//!
//! ## Prior baseline (2026-08-09, `stringcheese-bench/benches/ident.rs`)
//!
//! Retained for context — the earlier prose-only rows against
//! which the current suite's numbers should be compared:
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
