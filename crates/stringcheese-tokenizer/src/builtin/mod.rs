//! Built-in [`Segmenter`](crate::Segmenter) implementations.
//!
//! None of the built-ins here need an external data table larger than
//! what [`stringcheese-unicode`](stringcheese_unicode) already carries —
//! they are the "reach for it in five seconds without pulling in a
//! model" set. Every implementation is `no_std` + `alloc` and forbids
//! unsafe code.
//!
//! See `docs/design/tokenizers.md` § 4 for the full built-in surface
//! planned for this crate. [`WordSegmenter`] and [`SentenceSegmenter`]
//! delegate to the UAX #29 iterators exposed by
//! [`stringcheese_unicode`] (features `word-segmentation` and
//! `sentence-segmentation`, both default-on).

pub mod delimiter;
pub mod grapheme;
pub mod identifier;
pub mod ngram;
pub mod sentence;
pub mod whitespace;
pub mod word;

pub use delimiter::DelimiterTokenizer;
pub use grapheme::GraphemeSegmenter;
pub use identifier::{IdentifierMode, IdentifierTokenizer};
pub use ngram::NgramSegmenter;
pub use sentence::SentenceSegmenter;
pub use whitespace::WhitespaceTokenizer;
pub use word::WordSegmenter;
