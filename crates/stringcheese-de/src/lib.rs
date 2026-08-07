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
//! * **DIN 5007 collation (opt-in).** German dictionary-order collation
//!   is a locale-tailored problem: DIN 5007-1 treats `ä` as `a` and
//!   `ß` as `ss` (the Duden / dictionary ordering), while DIN 5007-2
//!   treats `ä` as `ae` and keeps `ß = ss` (the phone-book ordering).
//!   Callers who need one of the conventions reach for
//!   [`GERMAN_WITH_DIN5007_DICTIONARY`] or
//!   [`GERMAN_WITH_DIN5007_PHONEBOOK`]; the default [`GERMAN`] pack
//!   declines to pick a convention and returns `None` from
//!   [`Language::collator`](stringcheese_lang::Language::collator),
//!   preserving backwards compatibility with the pre-DIN-5007
//!   releases. See the [`collator`] module for the two variants and
//!   the identity [`GermanCollator::ASCII`] fallback.
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
//! * **CLDR-tailored German collation.** The DIN 5007 collator
//!   shipped here handles the two canonical German conventions
//!   (dictionary and phone-book) on ASCII + umlaut + `ß` input; a
//!   proper [Unicode Collation Algorithm][uca] tailoring for German
//!   would depend on ICU-backed data tables this pack deliberately
//!   does not ship. Callers who need CLDR-conformant German collation
//!   should reach for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own), the same
//!   trade-off `stringcheese-en` makes with its `icu-collator`
//!   Cargo feature.
//! * **Regional / Austrian and Swiss ordering variants.** Austrian
//!   telephone directories occasionally follow rules distinct from
//!   Germany's DIN 5007-2 (e.g. treating `ä`, `ö`, `ü` as extra
//!   letters after `z`), and Swiss German never uses `ß` at all
//!   (`ss` is written literally). Neither is shipped; callers who
//!   need those conventions build a [`Collator`](stringcheese_lang::Collator)
//!   of their own.
//!
//! [uca]: https://unicode.org/reports/tr10/
//!
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
//! - [`collator`] — the [`GermanCollator`] DIN 5007 comparator.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - The [`German`] type and the [`GERMAN`] / [`GERMAN_WITH_DIN5007_DICTIONARY`] /
//!   [`GERMAN_WITH_DIN5007_PHONEBOOK`] constants live in this crate's
//!   root.

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

#[cfg(feature = "alloc")]
pub mod collator;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use collator::{GermanCollator, GermanCollatorVariant};
#[cfg(feature = "alloc")]
pub use phonetic::KoelnerPhonetik;
#[cfg(feature = "alloc")]
pub use snowball::SnowballDe;
pub use stopwords::STOPWORDS;

/// The DIN 5007-1 (dictionary / Duden) [`GermanCollator`] preset,
/// exposed as an ergonomic `const` re-export of
/// [`GermanCollator::DIN_5007_DICTIONARY`].
///
/// This is what [`GERMAN_WITH_DIN5007_DICTIONARY`]'s
/// [`Language::collator`](stringcheese_lang::Language::collator)
/// accessor returns.
#[cfg(feature = "alloc")]
pub const GERMAN_DIN5007_DICTIONARY_COLLATOR: GermanCollator = GermanCollator::DIN_5007_DICTIONARY;

/// The DIN 5007-2 (phonebook) [`GermanCollator`] preset, exposed as
/// an ergonomic `const` re-export of
/// [`GermanCollator::DIN_5007_PHONEBOOK`].
///
/// This is what [`GERMAN_WITH_DIN5007_PHONEBOOK`]'s
/// [`Language::collator`](stringcheese_lang::Language::collator)
/// accessor returns.
#[cfg(feature = "alloc")]
pub const GERMAN_DIN5007_PHONEBOOK_COLLATOR: GermanCollator = GermanCollator::DIN_5007_PHONEBOOK;

// -----------------------------------------------------------------------
// The German language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Collator, Language, LanguagePhoneticEncoder, SimpleTokenizer};

    use crate::phonetic::KoelnerPhonetik;
    use crate::snowball::SnowballDe;
    use crate::stopwords::STOPWORDS;
    use crate::{GERMAN_DIN5007_DICTIONARY_COLLATOR, GERMAN_DIN5007_PHONEBOOK_COLLATOR};

    /// Which collator this [`German`] instance uses.
    ///
    /// Modeled as an `enum` (rather than a `&'static dyn Collator`
    /// reference) so the pack stays `Copy`, its constant constructors
    /// stay `const`-usable, and the shipped collators stay a closed
    /// set. The [`None`](Self::None) variant is the default — it
    /// preserves the pre-DIN-5007 behaviour where [`German`] declined
    /// to pick between the two conventions and returned `None` from
    /// [`Language::collator`](stringcheese_lang::Language::collator).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum GermanCollatorChoice {
        /// No collator wired — [`Language::collator`] returns `None`
        /// and callers fall back to Unicode code-point ordering. This
        /// is the default for [`GERMAN`](crate::GERMAN) so the pack
        /// stays backwards-compatible with the pre-DIN-5007 release.
        None,
        /// The DIN 5007-1 dictionary / Duden collator
        /// ([`GermanCollator::DIN_5007_DICTIONARY`](crate::GermanCollator::DIN_5007_DICTIONARY)).
        Din5007Variant1,
        /// The DIN 5007-2 phonebook collator
        /// ([`GermanCollator::DIN_5007_PHONEBOOK`](crate::GermanCollator::DIN_5007_PHONEBOOK)).
        Din5007Variant2,
        /// The identity fallback
        /// ([`GermanCollator::ASCII`](crate::GermanCollator::ASCII))
        /// — no umlaut expansion, no case folding.
        Ascii,
    }

    /// The German language pack.
    ///
    /// Carries a collator choice; the default (used by the
    /// [`GERMAN`](crate::GERMAN) constant) declines to pick a
    /// collation convention and returns `None` from
    /// [`Language::collator`](stringcheese_lang::Language::collator).
    /// Callers who want one of the DIN 5007 conventions grab
    /// [`GERMAN_WITH_DIN5007_DICTIONARY`](crate::GERMAN_WITH_DIN5007_DICTIONARY)
    /// or [`GERMAN_WITH_DIN5007_PHONEBOOK`](crate::GERMAN_WITH_DIN5007_PHONEBOOK),
    /// or compose their own via
    /// [`with_din5007_variant1`](German::with_din5007_variant1) /
    /// [`with_din5007_variant2`](German::with_din5007_variant2) /
    /// [`with_ascii_collator`](German::with_ascii_collator).
    ///
    /// Two `German` values with different collator choices are
    /// otherwise identical — same stopwords, same stemmer, same
    /// tokenizer, same phonetic encoder, same code/name.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct German {
        collator: GermanCollatorChoice,
    }

    impl German {
        /// Construct a `German` pack with the default no-collator
        /// choice (matches the pre-DIN-5007 release).
        #[must_use]
        pub const fn new() -> Self {
            Self {
                collator: GermanCollatorChoice::None,
            }
        }

        /// Return a `German` pack whose
        /// [`Language::collator`](stringcheese_lang::Language::collator)
        /// hands back the DIN 5007-1 dictionary / Duden collator
        /// ([`GermanCollator::DIN_5007_DICTIONARY`](crate::GermanCollator::DIN_5007_DICTIONARY)).
        ///
        /// Reach for the [`GERMAN_WITH_DIN5007_DICTIONARY`](crate::GERMAN_WITH_DIN5007_DICTIONARY)
        /// constant if you want the default pack with the DIN 5007-1
        /// collator; this builder method lets a caller compose the
        /// collator with a non-default state (currently there is none
        /// — the pack ships one stemmer and one tokenizer — but the
        /// builder shape matches every other `stringcheese-<lang>`
        /// pack).
        #[must_use]
        pub const fn with_din5007_variant1(mut self) -> Self {
            self.collator = GermanCollatorChoice::Din5007Variant1;
            self
        }

        /// Return a `German` pack whose
        /// [`Language::collator`](stringcheese_lang::Language::collator)
        /// hands back the DIN 5007-2 phonebook collator
        /// ([`GermanCollator::DIN_5007_PHONEBOOK`](crate::GermanCollator::DIN_5007_PHONEBOOK)).
        ///
        /// Reach for the [`GERMAN_WITH_DIN5007_PHONEBOOK`](crate::GERMAN_WITH_DIN5007_PHONEBOOK)
        /// constant if you want the default pack with the DIN 5007-2
        /// collator.
        #[must_use]
        pub const fn with_din5007_variant2(mut self) -> Self {
            self.collator = GermanCollatorChoice::Din5007Variant2;
            self
        }

        /// Return a `German` pack whose
        /// [`Language::collator`](stringcheese_lang::Language::collator)
        /// hands back the identity
        /// [`GermanCollator::ASCII`](crate::GermanCollator::ASCII)
        /// collator (equivalent to raw [`str::cmp`]).
        ///
        /// Useful as a baseline for tests or when a caller wants a
        /// locale-neutral comparator carried through the
        /// [`Language`](stringcheese_lang::Language) trait rather than
        /// plumbed separately.
        #[must_use]
        pub const fn with_ascii_collator(mut self) -> Self {
            self.collator = GermanCollatorChoice::Ascii;
            self
        }
    }

    impl Default for German {
        fn default() -> Self {
            Self::new()
        }
    }

    /// The static Kölner Phonetik adapter [`German`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static KOELNER: KoelnerPhonetik = KoelnerPhonetik;

    /// The static [`GermanCollator::ASCII`](crate::GermanCollator::ASCII)
    /// preset, kept as a `static` so
    /// [`Language::collator`](stringcheese_lang::Language::collator)
    /// can return a reference through a trait object.
    static ASCII_COLLATOR: crate::GermanCollator = crate::GermanCollator::ASCII;

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

        fn collator(&self) -> Option<&dyn Collator> {
            match self.collator {
                GermanCollatorChoice::None => None,
                GermanCollatorChoice::Din5007Variant1 => Some(&GERMAN_DIN5007_DICTIONARY_COLLATOR),
                GermanCollatorChoice::Din5007Variant2 => Some(&GERMAN_DIN5007_PHONEBOOK_COLLATOR),
                GermanCollatorChoice::Ascii => Some(&ASCII_COLLATOR),
            }
        }
    }

    /// The singleton [`German`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`German`] every time — the two forms are equivalent, but the
    /// constant is the intended entry point and matches the pattern
    /// every other `stringcheese-<lang>` pack follows.
    ///
    /// This is the no-collator default; see
    /// [`GERMAN_WITH_DIN5007_DICTIONARY`](crate::GERMAN_WITH_DIN5007_DICTIONARY)
    /// and [`GERMAN_WITH_DIN5007_PHONEBOOK`](crate::GERMAN_WITH_DIN5007_PHONEBOOK)
    /// for the two DIN 5007 conventions.
    pub const GERMAN: German = German::new();

    /// The singleton [`German`] language pack wired with the DIN
    /// 5007-1 dictionary / Duden collator
    /// ([`GermanCollator::DIN_5007_DICTIONARY`](crate::GermanCollator::DIN_5007_DICTIONARY)).
    ///
    /// Identical to [`GERMAN`](crate::GERMAN) in every respect except
    /// its collator.
    pub const GERMAN_WITH_DIN5007_DICTIONARY: German = German::new().with_din5007_variant1();

    /// The singleton [`German`] language pack wired with the DIN
    /// 5007-2 phonebook collator
    /// ([`GermanCollator::DIN_5007_PHONEBOOK`](crate::GermanCollator::DIN_5007_PHONEBOOK)).
    ///
    /// Identical to [`GERMAN`](crate::GERMAN) in every respect except
    /// its collator.
    pub const GERMAN_WITH_DIN5007_PHONEBOOK: German = German::new().with_din5007_variant2();
}

#[cfg(feature = "alloc")]
pub use pack::{GERMAN, GERMAN_WITH_DIN5007_DICTIONARY, GERMAN_WITH_DIN5007_PHONEBOOK, German};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("de")`) find German without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(GERMAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-de` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
