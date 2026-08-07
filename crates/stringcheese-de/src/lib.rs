//! German language pack for the StringCheese toolkit.
//!
//! A zero-sized [`German`] value that carries the German stopword list,
//! the [`SnowballDe`] stemmer (per the Snowball German algorithm), the
//! default whitespace-and-punctuation
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer), and the
//! [`KoelnerPhonetik`] encoder (Postel 1969) as its phonetic hookup.
//! Callers grab the singleton [`GERMAN`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-de` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants Levenshtein
//! and Rabin-Karp doesn't pay for the German stopword table or the
//! Snowball stemmer's code. Callers who need German add
//! `stringcheese-de = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately conservative:
//!
//! * **Snowball German stemmer.** Martin Porter's Snowball German
//!   algorithm is the reference stemmer every German IR system
//!   documents against. See
//!   <https://snowballstem.org/algorithms/german/stemmer.html>.
//! * **Kölner Phonetik, not Soundex.** Hans Joachim Postel's 1969
//!   German-friendly encoder is the honest choice for German —
//!   Soundex's English-first mapping mishandles `sch`, `ch`, umlauts,
//!   and the softened `c` before front vowels. The English pack ships
//!   Soundex for the same reasons of algorithmic honesty; the German
//!   pack ships Kölner Phonetik.
//! * **Modest stopword list.** ~200 entries drawn from the
//!   NLTK / Snowball German stopword tradition. No domain-specific
//!   jargon; no archaic forms; no Swiss `ss`-only spellings.
//! * **Default Unicode collation.** German requires a locale tailoring
//!   for proper alphabetical sort (DIN 5007-1 treats `ä` as `a`,
//!   DIN 5007-2 as `ae`; the Duden and phone-book conventions
//!   disagree). This crate declines to pick a convention for callers
//!   and returns `None` from
//!   [`Language::collator`](stringcheese_lang::Language::collator);
//!   callers who need dictionary-style ordering should implement their
//!   own [`Collator`](stringcheese_lang::Collator) with an explicit
//!   variant choice.
//!
//! # Deferred to a follow-up wave
//!
//! * **Compound-noun splitting.** German famously builds long
//!   compound nouns
//!   (`Donaudampfschifffahrtsgesellschaft` → `Donau` / `Dampf` /
//!   `Schiff` / `Fahrt` / `Gesellschaft`). Splitting them requires a
//!   compound-noun dictionary and is out of scope for a suffix-stripping
//!   stemmer; the default [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer)
//!   is enough for word-level segmentation but treats each compound as
//!   a single token. A dedicated `GermanTokenizer` with dictionary
//!   support is on the roadmap.
//! * **DIN 5007 collator.** The two DIN 5007 variants (umlauts as base
//!   vowels vs. as `ae`/`oe`/`ue` expansions) each require a small
//!   sort-key builder plus a caller-facing enum to select the variant.
//!   Deferred to a follow-up that adds all Latin-alphabet locale
//!   collators together.
//! * **Historical / regional stopword variants.** Middle High German,
//!   Bavarian, or Swiss spellings are absent; the list targets modern
//!   standard German only.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_de::GERMAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(GERMAN.code(), "de");
//! assert_eq!(GERMAN.name(), "German");
//! assert!(GERMAN.is_stopword("und"));
//! assert!(!GERMAN.is_stopword("Käse"));
//! assert_eq!(GERMAN.stem("Häuser"), "haus");
//! assert_eq!(GERMAN.stem("haben"), "hab");
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`SnowballDe`] stemmer.
//! - [`phonetic`] — the [`KoelnerPhonetik`] encoder.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - The [`German`] type and the [`GERMAN`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::KoelnerPhonetik;
#[cfg(feature = "alloc")]
pub use snowball::SnowballDe;
pub use stopwords::STOPWORDS;

// -----------------------------------------------------------------------
// The German language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder, SimpleTokenizer};

    use crate::phonetic::KoelnerPhonetik;
    use crate::snowball::SnowballDe;
    use crate::stopwords::STOPWORDS;

    /// The German language pack.
    ///
    /// Zero-sized; construct as [`German`] and reuse the value freely
    /// across threads and calls, or grab the crate-level [`GERMAN`](crate::GERMAN)
    /// constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct German;

    /// The static Kölner Phonetik adapter [`German`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static KOELNER: KoelnerPhonetik = KoelnerPhonetik;

    impl Language for German {
        fn code(&self) -> &'static str {
            "de"
        }

        fn name(&self) -> &'static str {
            "German"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            SnowballDe.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SimpleTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&KOELNER)
        }
    }

    /// The singleton [`German`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`German`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const GERMAN: German = German;
}

#[cfg(feature = "alloc")]
pub use pack::{GERMAN, German};

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-de` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
