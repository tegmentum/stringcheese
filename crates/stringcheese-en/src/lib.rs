//! English language pack for the StringCheese toolkit.
//!
//! This is the reference `stringcheese-<lang>` implementation: a
//! zero-sized [`English`] value that carries the English stopword list,
//! the [`Porter`] (1980) stemmer, the default whitespace-and-punctuation
//! [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer), and a
//! Soundex phonetic hookup drawn from
//! [`stringcheese_phonetic`]. Callers grab the singleton
//! [`ENGLISH`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
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
//! * **Porter (1980), not Porter2 (Snowball).** Porter's original
//!   five-step algorithm is the reference stemmer every subsequent
//!   effort documents against; Porter2's revisions add rules and
//!   change conditional structure. Porter (1980) is enough for v0.1;
//!   Porter2 is a follow-up wave (see below).
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
//! # Deferred to a follow-up wave
//!
//! * **Porter2 (Snowball) stemmer.** The revised algorithm — slightly
//!   more accurate on modern text, and the reference for the Snowball
//!   stemmer generator — is planned for a subsequent release.
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
//! use stringcheese_en::ENGLISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ENGLISH.code(), "en");
//! assert_eq!(ENGLISH.name(), "English");
//! assert!(ENGLISH.is_stopword("the"));
//! assert!(!ENGLISH.is_stopword("cheese"));
//! assert_eq!(ENGLISH.stem("caresses"), "caress");
//! assert_eq!(ENGLISH.stem("ponies"), "poni");
//! ```
//!
//! # Module map
//!
//! - [`porter`] — the [`Porter`] (1980) stemmer.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - The [`English`] type and the [`ENGLISH`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod porter;
pub mod stopwords;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use porter::Porter;
pub use stopwords::STOPWORDS;

// -----------------------------------------------------------------------
// The English language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{
        Language, LanguagePhoneticEncoder, SimpleTokenizer, phonetic::SoundexAdapter,
    };

    use crate::porter::Porter;
    use crate::stopwords::STOPWORDS;

    /// The English language pack.
    ///
    /// Zero-sized; construct as [`English`] and reuse the value freely
    /// across threads and calls, or grab the crate-level [`ENGLISH`](crate::ENGLISH)
    /// constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct English;

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
            Porter.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SimpleTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&SOUNDEX)
        }
    }

    /// The singleton [`English`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`English`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack will follow.
    pub const ENGLISH: English = English;
}

#[cfg(feature = "alloc")]
pub use pack::{ENGLISH, English};

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-en` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
