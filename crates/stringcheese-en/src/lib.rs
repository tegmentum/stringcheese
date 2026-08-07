//! English language pack for the StringCheese toolkit.
//!
//! This is the reference `stringcheese-<lang>` implementation: an
//! [`English`] value that carries the English stopword list, a chosen
//! stemmer (Porter (1980) by default, Porter2 (Snowball, 2001)
//! optionally), the default whitespace-and-punctuation
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer), and a
//! Soundex phonetic hookup drawn from [`stringcheese_phonetic`].
//! Callers grab the singleton [`ENGLISH`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-en` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants Levenshtein
//! and Rabin-Karp doesn't pay for the English stopword table or the
//! Porter stemmer's code. Callers who need English add
//! `stringcheese-en = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately conservative:
//!
//! * **Two stemmers, both shipped.** The pack ships both the classic
//!   [`Porter`] (1980) stemmer and the revised [`Porter2`] (Snowball,
//!   2001). Porter is the default for the [`ENGLISH`] constant to
//!   preserve backwards compatibility; Porter2 is available through
//!   [`ENGLISH_PORTER2`] or [`English::with_porter2`]. See
//!   [*Which stemmer?*](#which-stemmer) below.
//! * **Modest stopword list.** ~150 entries drawn from the intersection
//!   of NLTK, scikit-learn, and van Rijsbergen's classic list. No
//!   domain-specific jargon, no archaic forms.
//! * **Soundex, not Double Metaphone.** Soundex is the 1918 encoder
//!   designed for English surnames; the phonetic crate ships both
//!   Soundex and Double Metaphone, but for the English pack's default
//!   the century-old, well-understood algorithm is the honest choice.
//!   Callers who want Double Metaphone construct the encoder
//!   themselves via `stringcheese_phonetic::DoubleMetaphone`.
//! * **Default Unicode collation.** English does not need a locale
//!   tailoring for basic sort order; the pack's
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   accessor returns `None`. Callers who need dictionary-style
//!   ordering (case-insensitive; ignoring leading `A`, `An`, `The`)
//!   should implement their own [`Collator`](stringcheese_lang::Collator).
//!
//! # Which stemmer?
//!
//! - **[`Porter`] (1980)** — the reference algorithm every subsequent
//!   effort documents against. Choose this when reproducing older
//!   published results, when your consumers already index against
//!   Porter stems, or when you want the smallest, most-audited rule
//!   table. This is what [`ENGLISH`] hands back from `.stem()`.
//! - **[`Porter2`] (Snowball, 2001)** — Porter's own revised algorithm.
//!   Corrects several Porter defects (`-ying` → `-ie` for
//!   `dying`/`lying`/`tying`; better handling of short words like
//!   `sky`, `news`; explicit exception table; R1/R2 region markers
//!   instead of the measure `m`; `us`/`ss` edge cases). Choose this
//!   for fresh IR pipelines with no backwards-compat constraint. Use
//!   [`ENGLISH_PORTER2`] or `English::default().with_porter2()`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Contraction-aware tokenization.** The shipped tokenizer treats
//!   the apostrophe in `"don't"` as a separator; a real English
//!   tokenizer would emit `["do", "n't"]` (or `["don't"]`, depending
//!   on the caller's preference). This is on the roadmap for a
//!   dedicated `EnglishTokenizer` type that overrides
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize).
//! * **English-specific collator.** Dictionary-order sorting (fold
//!   case, ignore leading articles, treat ligatures as expansions)
//!   is a natural follow-on; it needs its own design pass.
//! * **Lemmatization.** Reducing a word to its dictionary form
//!   (`"better"` → `"good"`) rather than a suffix-stripped stem
//!   requires a lexicon and is out of scope for a stem-only pack.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_en::{ENGLISH, ENGLISH_PORTER2};
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ENGLISH.code(), "en");
//! assert_eq!(ENGLISH.name(), "English");
//! assert!(ENGLISH.is_stopword("the"));
//! assert!(!ENGLISH.is_stopword("cheese"));
//!
//! // Porter (1980) — the default, preserved for backwards compat.
//! assert_eq!(ENGLISH.stem("caresses"), "caress");
//! assert_eq!(ENGLISH.stem("ponies"), "poni");
//!
//! // Porter2 (Snowball) — opt in with ENGLISH_PORTER2.
//! assert_eq!(ENGLISH_PORTER2.stem("dying"), "die");
//! assert_eq!(ENGLISH_PORTER2.stem("sky"), "sky");
//! ```
//!
//! # Module map
//!
//! - [`porter`] — the [`Porter`] (1980) stemmer.
//! - [`porter2`] — the [`Porter2`] (Snowball, 2001) stemmer.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - The [`English`] type and the [`ENGLISH`] / [`ENGLISH_PORTER2`]
//!   constants live in this crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod porter;
#[cfg(feature = "alloc")]
pub mod porter2;
pub mod stopwords;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use porter::Porter;
#[cfg(feature = "alloc")]
pub use porter2::Porter2;
pub use stopwords::STOPWORDS;

/// The [`Porter`] (1980) stemmer, exposed as a static so callers can
/// name a `&'static dyn Stemmer` without allocation.
///
/// This is what [`ENGLISH`]'s
/// [`Language::stem`](stringcheese_lang::Language::stem) method uses
/// by default (backwards compatibility with pre-Porter2 pack
/// releases).
#[cfg(feature = "alloc")]
pub static PORTER_STEMMER: Porter = Porter;

/// The [`Porter2`] (Snowball, 2001) stemmer, exposed as a static so
/// callers can name a `&'static dyn Stemmer` without allocation.
///
/// [`ENGLISH_PORTER2`] and [`English::with_porter2`] both use this
/// static; callers who want a Porter2-backed English pack should reach
/// for one of those rather than constructing `English` directly.
#[cfg(feature = "alloc")]
pub static PORTER2_STEMMER: Porter2 = Porter2;

// -----------------------------------------------------------------------
// The English language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{
        Language, LanguagePhoneticEncoder, SimpleTokenizer, Stemmer, phonetic::SoundexAdapter,
    };

    use crate::stopwords::STOPWORDS;
    use crate::{PORTER_STEMMER, PORTER2_STEMMER};

    /// The English language pack.
    ///
    /// Carries a chosen stemmer as a `&'static dyn Stemmer`. The
    /// default (used by the [`ENGLISH`](crate::ENGLISH) constant) is
    /// Porter (1980); callers wanting Porter2 (Snowball, 2001) should
    /// grab [`ENGLISH_PORTER2`](crate::ENGLISH_PORTER2) or call
    /// [`with_porter2`](English::with_porter2) on an existing
    /// `English`.
    ///
    /// Two `English` values with different stemmers are otherwise
    /// identical — same stopwords, same tokenizer, same phonetic
    /// encoder, same code/name. The `stemmer` field is the only
    /// distinguishing piece, and it's a `&'static` reference, so
    /// `English` remains cheap to copy and cheap to construct.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone)]
    pub struct English {
        stemmer: &'static (dyn Stemmer + 'static),
    }

    impl English {
        /// Construct an `English` pack with the default stemmer
        /// ([`Porter`](crate::Porter), 1980).
        #[must_use]
        pub const fn new() -> Self {
            Self {
                stemmer: &PORTER_STEMMER,
            }
        }

        /// Return an `English` pack using the classic
        /// [`Porter`](crate::Porter) (1980) stemmer.
        ///
        /// The receiver is consumed rather than modified in place so
        /// the method is `const`-usable at compile time (the same
        /// pattern as [`with_porter2`](Self::with_porter2)).
        #[must_use]
        pub const fn with_porter(self) -> Self {
            Self {
                stemmer: &PORTER_STEMMER,
            }
        }

        /// Return an `English` pack using the revised
        /// [`Porter2`](crate::Porter2) (Snowball, 2001) stemmer.
        ///
        /// See [the crate docs' *Which stemmer?* section](crate#which-stemmer)
        /// for guidance on when to choose Porter2 over Porter.
        #[must_use]
        pub const fn with_porter2(self) -> Self {
            Self {
                stemmer: &PORTER2_STEMMER,
            }
        }
    }

    impl Default for English {
        fn default() -> Self {
            Self::new()
        }
    }

    /// The static Soundex adapter [`English`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static SOUNDEX: SoundexAdapter = SoundexAdapter;

    impl Language for English {
        fn code(&self) -> &'static str {
            "en"
        }

        fn name(&self) -> &'static str {
            "English"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            self.stemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SimpleTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&SOUNDEX)
        }
    }

    /// The singleton [`English`] language pack (Porter 1980 stemmer).
    ///
    /// Callers reach for this constant rather than constructing
    /// [`English`] every time — the two forms are equivalent, but the
    /// constant is the intended entry point and matches the pattern
    /// every other `stringcheese-<lang>` pack will follow.
    ///
    /// See [`ENGLISH_PORTER2`](crate::ENGLISH_PORTER2) for the same
    /// pack backed by the Porter2 stemmer.
    pub const ENGLISH: English = English::new();

    /// The singleton [`English`] language pack backed by the
    /// [`Porter2`](crate::Porter2) (Snowball, 2001) stemmer.
    ///
    /// Identical to [`ENGLISH`](crate::ENGLISH) in every respect
    /// except its stemmer.
    pub const ENGLISH_PORTER2: English = English::new().with_porter2();
}

#[cfg(feature = "alloc")]
pub use pack::{ENGLISH, ENGLISH_PORTER2, English};

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-en` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
