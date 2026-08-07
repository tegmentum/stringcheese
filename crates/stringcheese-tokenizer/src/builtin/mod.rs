//! Built-in [`Segmenter`](crate::Segmenter) implementations.
//!
//! None of the built-ins here need an external data table larger than
//! what [`stringcheese-unicode`](stringcheese_unicode) already carries —
//! they are the "reach for it in five seconds without pulling in a
//! model" set. Every implementation is `no_std` + `alloc` and forbids
//! unsafe code.
//!
//! See `docs/design/tokenizers.md` § 4 for the full built-in surface
//! planned for this crate. `WordSegmenter` / `SentenceSegmenter` are
//! not yet included pending UAX #29 word- and sentence-boundary support
//! in [`stringcheese-unicode`](stringcheese_unicode); the stubs in
//! [`crates/stringcheese-manip/src/split/mod.rs`][deferred] point at
//! the same upstream gap.
//!
//! [deferred]: https://github.com/tegmentum/stringcheese/blob/main/crates/stringcheese-manip/src/split/mod.rs

pub mod delimiter;
pub mod grapheme;
pub mod identifier;
pub mod ngram;
pub mod whitespace;

pub use delimiter::DelimiterTokenizer;
pub use grapheme::GraphemeSegmenter;
pub use identifier::{IdentifierMode, IdentifierTokenizer};
pub use ngram::NgramSegmenter;
pub use whitespace::WhitespaceTokenizer;
