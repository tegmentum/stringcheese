//! Italian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Italian`] value that carries the Italian stopword
//! list, an identity stemmer, and the whitespace-and-punctuation
//! [`ItalianTokenizer`]. Callers grab the singleton [`ITALIAN`]
//! `const` — no construction ceremony required — and delegate
//! through the [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade
//! does *not* re-export `stringcheese-it` — language packs are
//! per-crate, per-language dependencies, so a caller who only
//! wants English or only wants Levenshtein doesn't pay for the
//! Italian stopword table. Callers who need Italian add
//! `stringcheese-it = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Scope — minimum viable Italian pack
//!
//! This crate ships as the **Latin-adjacent trio's third member**
//! (alongside `stringcheese-es` and `stringcheese-pt`) — a
//! deliberately narrow base pack that unblocks the WIT-i18n
//! Phase 3 shipped-locale gap. The Italian rules the pack ships:
//!
//! * **~30-word stopword list.** A starter list covering articles,
//!   prepositions, coordinating conjunctions, the high-frequency
//!   personal pronouns, and the bare infinitives of the common
//!   auxiliary and modal verbs (`essere`, `avere`, `fare`, …).
//!   Documented as MVP; a full Snowball-derived ~280-word list
//!   ships in a follow-up alongside the stemmer. See [`stopwords`].
//! * **Identity stemmer.**
//!   [`Language::stem`](stringcheese_lang::Language::stem) returns
//!   the input verbatim (`Cow::Borrowed`). A Snowball Italian
//!   stemmer needs
//!   a Snowball binding not currently vendored into the workspace
//!   — the sibling `stringcheese-fr` / `stringcheese-es` /
//!   `stringcheese-pt` packs each hand-port their Snowball
//!   algorithm to pure Rust, and adding the Italian port is a
//!   documented follow-up.
//! * **Simple tokenizer.** [`ItalianTokenizer`] is a transparent
//!   wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer);
//!   Italian orthography is whitespace-and-punctuation delimited
//!   and does not need a bespoke splitter.
//! * **No phonetic encoder.**
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns `None`. Italian lacks a settled canonical phonetic
//!   algorithm (unlike English Soundex / German Kölner Phonetik);
//!   the sibling packs' PHONEX-family encoders are per-language
//!   ports of the same shape and the Italian variant is a
//!   documented follow-up.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Italian sorts under the standard Unicode
//!   Latin ordering with no locale-specific reorderings; callers
//!   who need a locale-tailored collator should reach for
//!   `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Phase 3 SCUD packs
//!
//! Behind opt-in `plural-scud` and `number-scud` features the
//! crate additionally ships:
//!
//! * [`plural_data`] — the CLDR 44.1 Italian plural-rule table
//!   (three-way cardinal `one`/`many`/`other` + ordinal `many`
//!   bucket for `n ∈ {8, 11, 80, 800}`).
//! * [`number_data`] — the CLDR 44.1 Italian number-formatting
//!   patterns (Italy conventions: group `.`, decimal `,`,
//!   currency after value with a space, percent `%` after value
//!   with a space).
//!
//! See the `docs/design/wit-i18n.md` § 8.3 progress notes for the
//! Phase 3 delivery contract.
//!
//! # Deferred to a follow-up wave
//!
//! * **Snowball Italian stemmer.** Martin Porter's Italian
//!   algorithm at
//!   <https://snowballstem.org/algorithms/italian/stemmer.html> —
//!   the reference for Italian IR stemmers. Needs a pure-Rust
//!   port matching the pattern the sibling Latin-script packs
//!   already follow.
//! * **Full ~280-word stopword list.** The Snowball project's
//!   `italian/stop.txt` distribution includes the full paradigms
//!   of `essere` / `avere` / `stare` / `fare` (`sono`, `sei`, `è`,
//!   `siamo`, `siete`, `sono`, `ho`, `hai`, `ha`, `abbiamo`,
//!   `avete`, `hanno`, …). Ships alongside the stemmer.
//! * **Italian phonetic encoder.** A PHONEX-Italian variant with
//!   the double-consonant collapse (`piatto → PIATO`), palatal
//!   digraph handling (`gn`, `gl`, `sc + i/e`), and vowel-length
//!   sensitivity Italian orthography carries. Follow-up.
//! * **Regional variants — `it-CH` (Italian-Switzerland).** Uses
//!   CHF as the primary currency; the shipped `number-it` pack
//!   already includes CHF for compatibility, but a separate
//!   `it-CH` SCUD would use `.` group / `.` decimal (Swiss
//!   conventions) instead of Italian `.` / `,`. Follow-up.
//! * **Sicilian / Sardinian / Neapolitan** — separate CLDR
//!   locales (`scn`, `sc`, `nap`) with their own vocabulary and
//!   plural rules (Sicilian shares Italian's `EsItPtCardinalMany`
//!   opcode; the base crate would fork the stopwords and
//!   register a distinct BCP-47 code).
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_it::ITALIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ITALIAN.code(), "it");
//! assert_eq!(ITALIAN.name(), "Italian");
//! assert!(ITALIAN.is_stopword("il"));
//! assert!(ITALIAN.is_stopword("e"));
//! assert!(!ITALIAN.is_stopword("formaggio"));
//! // Identity stemmer — MVP surface until the Snowball port lands.
//! assert_eq!(ITALIAN.stem("parlando"), "parlando");
//!
//! let toks: Vec<&str> = ITALIAN
//!     .tokenize("Il gatto dorme sul tappeto.")
//!     .collect();
//! assert_eq!(toks, ["Il", "gatto", "dorme", "sul", "tappeto"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`ItalianTokenizer`] wrapper.
//! - [`plural_data`] — the plural-it SCUD pack (feature
//!   `plural-scud`).
//! - [`number_data`] — the number-it SCUD pack (feature
//!   `number-scud`).
//! - The [`Italian`] type and the [`ITALIAN`] constant live in
//!   this crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "datetime-scud")]
pub mod datetime_data;
#[cfg(feature = "number-scud")]
pub mod number_data;
#[cfg(feature = "plural-scud")]
pub mod plural_data;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

pub use stopwords::STOPWORDS;
pub use tokenizer::ItalianTokenizer;

// -----------------------------------------------------------------------
// The Italian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::Language;

    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::ItalianTokenizer;

    /// The Italian language pack.
    ///
    /// Zero-sized; construct as [`Italian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`ITALIAN`](crate::ITALIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Italian;

    impl Language for Italian {
        fn code(&self) -> &'static str {
            "it"
        }

        fn name(&self) -> &'static str {
            "Italian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        // No `is_stopword` override — the MVP stopword list is ASCII
        // only (documented in `stopwords.rs`), so the default trait
        // method (which uses `str::eq_ignore_ascii_case`) is correct.

        /// Identity stemmer.
        ///
        /// Returns the input verbatim as [`Cow::Borrowed`]. A
        /// Snowball Italian stemmer is a documented follow-up (see
        /// the [crate-level docs](crate)); until it lands the pack
        /// ships without inflectional collapse — `parlando` and
        /// `parlare` remain distinct equivalence-class
        /// representatives.
        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Cow::Borrowed(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(ItalianTokenizer::new().tokenize(text))
        }

        // No `phonetic_encoder` override — Italian PHONEX is a
        // documented follow-up (see the crate-level docs). The
        // default trait method returns `None`.

        // No `collator` override — Italian sorts under the default
        // Unicode Latin ordering with no CLDR-specific tailorings.
    }

    /// The singleton [`Italian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Italian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended
    /// entry point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const ITALIAN: Italian = Italian;
}

#[cfg(feature = "alloc")]
pub use pack::{ITALIAN, Italian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("it")`) find Italian
// without naming the crate. See `stringcheese_lang::registry` for
// the design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ITALIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-it` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
