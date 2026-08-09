//! Per-script trigram profiles — hard-sharded.
//!
//! Each script's trigram tables live in its own file behind a Cargo
//! feature. When a feature is off the file isn't compiled in and the
//! constant falls back to an empty `LangProfileList`. Detection's
//! per-script match arms consult the empty list and produce no hits,
//! which is the correct behaviour for a build that hasn't opted into
//! that script — no runtime cost, no binary cost.

use super::super::Lang;
use super::Trigram;

/// One language's trigram profile — a static slice of the 300 most
/// frequent trigrams in that language, in descending order.
pub type LangProfile = &'static [Trigram];

/// The `(Lang, LangProfile)` pairs for every language in a script.
pub type LangProfileList = &'static [(Lang, LangProfile)];

// ---------------------------------------------------------------------
// Per-script trigram tables — feature-gated.
// ---------------------------------------------------------------------

#[cfg(feature = "latn")]
mod latn;
#[cfg(feature = "latn")]
pub use latn::LATIN_LANGS;
#[cfg(not(feature = "latn"))]
pub static LATIN_LANGS: LangProfileList = &[];

#[cfg(feature = "cyrl")]
mod cyrl;
#[cfg(feature = "cyrl")]
pub use cyrl::CYRILLIC_LANGS;
#[cfg(not(feature = "cyrl"))]
pub static CYRILLIC_LANGS: LangProfileList = &[];

#[cfg(feature = "arab")]
mod arab;
#[cfg(feature = "arab")]
pub use arab::ARABIC_LANGS;
#[cfg(not(feature = "arab"))]
pub static ARABIC_LANGS: LangProfileList = &[];

#[cfg(feature = "deva")]
mod deva;
#[cfg(feature = "deva")]
pub use deva::DEVANAGARI_LANGS;
#[cfg(not(feature = "deva"))]
pub static DEVANAGARI_LANGS: LangProfileList = &[];

#[cfg(feature = "hebr")]
mod hebr;
#[cfg(feature = "hebr")]
pub use hebr::HEBREW_LANGS;
#[cfg(not(feature = "hebr"))]
pub static HEBREW_LANGS: LangProfileList = &[];
