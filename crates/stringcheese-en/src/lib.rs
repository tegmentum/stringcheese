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
//! * **Dictionary-order collation.** The default pack ships an
//!   [`EnglishCollator`] with case folding, leading-article stripping
//!   (`a`, `an`, `the`), and digits-after-letters ordering. Reach it
//!   through [`Language::collator`](stringcheese_lang::Language::collator)
//!   or the exported [`ENGLISH_DICTIONARY_COLLATOR`] constant. This is
//!   an ASCII-common-case implementation, not a full CLDR tailoring.
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
//! # Optional collator and tokenizer
//!
//! * **[`EnglishCollator`]** — dictionary-order English collation
//!   (ignore leading articles `a`/`an`/`the`, ASCII case-fold, digits
//!   sort after letters). Exposed as [`ENGLISH_DICTIONARY_COLLATOR`]
//!   and wired into the default [`ENGLISH`] pack's
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   accessor.
//! * **[`ContractionTokenizer`]** — English contraction-aware
//!   tokenization (`"don't"` → `["do", "n't"]`, `"won't"` →
//!   `["will", "n't"]`, etc.). Two presets:
//!   [`STANDARD`](ContractionTokenizer::STANDARD) preserves
//!   contraction fragments; [`NORMALIZED`](ContractionTokenizer::NORMALIZED)
//!   expands them (`"n't"` → `"not"`, `"'ll"` → `"will"`). Wire into
//!   the pack via [`English::with_contraction_tokenizer`] or reach for
//!   the pre-configured [`ENGLISH_WITH_CONTRACTIONS`] singleton (which
//!   uses [`STANDARD`](ContractionTokenizer::STANDARD)).
//!
//! # Deferred to a follow-up wave
//!
//! * **Lemmatization.** Reducing a word to its dictionary form
//!   (`"better"` → `"good"`) rather than a suffix-stripped stem
//!   requires a lexicon and is out of scope for a stem-only pack.
//! * **CLDR-tailored English collation.** The dictionary-order collator
//!   shipped here handles the ASCII common case; a proper
//!   [Unicode Collation Algorithm][uca] tailoring for English would
//!   depend on ICU-backed data tables this pack deliberately does not
//!   ship. Callers who need CLDR-conformant English collation should
//!   reach for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own).
//!
//! [uca]: https://unicode.org/reports/tr10/
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
//! - [`collator`] — the [`EnglishCollator`] dictionary-order comparator.
//! - [`contraction`] — the [`ContractionTokenizer`] English
//!   contraction-aware tokenizer.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - The [`English`] type and the [`ENGLISH`] / [`ENGLISH_PORTER2`] /
//!   [`ENGLISH_WITH_CONTRACTIONS`] constants live in this crate's root.

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
pub mod contraction;
#[cfg(feature = "alloc")]
pub mod porter;
#[cfg(feature = "alloc")]
pub mod porter2;
pub mod stopwords;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use collator::EnglishCollator;
#[cfg(feature = "alloc")]
pub use contraction::{CONTRACTION_TOKENIZER, ContractionTokenizer};
#[cfg(feature = "alloc")]
pub use porter::Porter;
#[cfg(feature = "alloc")]
pub use porter2::Porter2;
pub use stopwords::STOPWORDS;

/// The dictionary-order [`EnglishCollator`] preset, exposed as an
/// ergonomic `const` re-export of [`EnglishCollator::DICTIONARY`].
///
/// This is what the default [`ENGLISH`] pack's
/// [`Language::collator`](stringcheese_lang::Language::collator)
/// accessor returns.
#[cfg(feature = "alloc")]
pub const ENGLISH_DICTIONARY_COLLATOR: EnglishCollator = EnglishCollator::DICTIONARY;

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
        Collator, Language, LanguagePhoneticEncoder, SimpleTokenizer, Stemmer,
        phonetic::SoundexAdapter,
    };

    use crate::stopwords::STOPWORDS;
    use crate::{
        CONTRACTION_TOKENIZER, ENGLISH_DICTIONARY_COLLATOR, PORTER_STEMMER, PORTER2_STEMMER,
    };

    /// Which tokenizer this [`English`] instance uses.
    ///
    /// Modeled as an `enum` (not as a `&'static dyn ...` reference like
    /// the [`Stemmer`] field) so the pack stays `Copy`, its constant
    /// constructors stay `const`-usable, and the two shipped tokenizers
    /// stay a closed set — the trade-off is that a caller who wants a
    /// hand-rolled tokenizer implements
    /// [`Language`](stringcheese_lang::Language) directly rather than
    /// slotting a `&'static dyn Tokenizer` in here.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum EnglishTokenizerChoice {
        /// The default whitespace-and-punctuation
        /// [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
        Simple,
        /// The [`ContractionTokenizer`](crate::ContractionTokenizer)
        /// in its [`STANDARD`](crate::ContractionTokenizer::STANDARD)
        /// configuration.
        Contraction,
    }

    /// The English language pack.
    ///
    /// Carries a chosen stemmer as a `&'static dyn Stemmer` plus a
    /// tokenizer choice. The default (used by the
    /// [`ENGLISH`](crate::ENGLISH) constant) is Porter (1980) with the
    /// baseline [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer);
    /// callers who want Porter2 or the contraction-aware tokenizer
    /// grab a different constant
    /// ([`ENGLISH_PORTER2`](crate::ENGLISH_PORTER2),
    /// [`ENGLISH_WITH_CONTRACTIONS`](crate::ENGLISH_WITH_CONTRACTIONS)),
    /// or compose their own via
    /// [`with_porter2`](English::with_porter2) /
    /// [`with_contraction_tokenizer`](English::with_contraction_tokenizer).
    ///
    /// Two `English` values with different stemmers or tokenizer
    /// choices are otherwise identical — same stopwords, same
    /// collator, same phonetic encoder, same code/name. Both
    /// distinguishing pieces are cheap (a `&'static` reference and a
    /// two-variant enum), so `English` remains cheap to copy and cheap
    /// to construct.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone)]
    pub struct English {
        stemmer: &'static (dyn Stemmer + 'static),
        tokenizer: EnglishTokenizerChoice,
    }

    impl English {
        /// Construct an `English` pack with the default stemmer
        /// ([`Porter`](crate::Porter), 1980) and the baseline
        /// [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
        #[must_use]
        pub const fn new() -> Self {
            Self {
                stemmer: &PORTER_STEMMER,
                tokenizer: EnglishTokenizerChoice::Simple,
            }
        }

        /// Return an `English` pack using the classic
        /// [`Porter`](crate::Porter) (1980) stemmer.
        ///
        /// The receiver is consumed rather than modified in place so
        /// the method is `const`-usable at compile time (the same
        /// pattern as [`with_porter2`](Self::with_porter2)).
        #[must_use]
        pub const fn with_porter(mut self) -> Self {
            self.stemmer = &PORTER_STEMMER;
            self
        }

        /// Return an `English` pack using the revised
        /// [`Porter2`](crate::Porter2) (Snowball, 2001) stemmer.
        ///
        /// See [the crate docs' *Which stemmer?* section](crate#which-stemmer)
        /// for guidance on when to choose Porter2 over Porter.
        #[must_use]
        pub const fn with_porter2(mut self) -> Self {
            self.stemmer = &PORTER2_STEMMER;
            self
        }

        /// Return an `English` pack using the baseline
        /// [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
        ///
        /// This is the default; the method exists so a caller who
        /// composed a
        /// [`with_contraction_tokenizer`](Self::with_contraction_tokenizer)
        /// pack can undo that choice.
        #[must_use]
        pub const fn with_simple_tokenizer(mut self) -> Self {
            self.tokenizer = EnglishTokenizerChoice::Simple;
            self
        }

        /// Return an `English` pack using the
        /// [`ContractionTokenizer`](crate::ContractionTokenizer)
        /// (`STANDARD` preset).
        ///
        /// Reach for the [`ENGLISH_WITH_CONTRACTIONS`](crate::ENGLISH_WITH_CONTRACTIONS)
        /// constant if you want the default pack with contractions;
        /// this builder method lets a caller compose contraction
        /// tokenization with a non-default stemmer, e.g.
        /// `English::new().with_porter2().with_contraction_tokenizer()`.
        #[must_use]
        pub const fn with_contraction_tokenizer(mut self) -> Self {
            self.tokenizer = EnglishTokenizerChoice::Contraction;
            self
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
            match self.tokenizer {
                EnglishTokenizerChoice::Simple => Box::new(SimpleTokenizer::new().tokenize(text)),
                EnglishTokenizerChoice::Contraction => {
                    CONTRACTION_TOKENIZER.tokenize_borrowed(text)
                }
            }
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&SOUNDEX)
        }

        fn collator(&self) -> Option<&dyn Collator> {
            Some(&ENGLISH_DICTIONARY_COLLATOR)
        }
    }

    /// The singleton [`English`] language pack (Porter 1980 stemmer,
    /// baseline [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer)).
    ///
    /// Callers reach for this constant rather than constructing
    /// [`English`] every time — the two forms are equivalent, but the
    /// constant is the intended entry point and matches the pattern
    /// every other `stringcheese-<lang>` pack will follow.
    ///
    /// See [`ENGLISH_PORTER2`](crate::ENGLISH_PORTER2) for the same
    /// pack backed by the Porter2 stemmer, and
    /// [`ENGLISH_WITH_CONTRACTIONS`](crate::ENGLISH_WITH_CONTRACTIONS)
    /// for the same pack with contraction-aware tokenization.
    pub const ENGLISH: English = English::new();

    /// The singleton [`English`] language pack backed by the
    /// [`Porter2`](crate::Porter2) (Snowball, 2001) stemmer.
    ///
    /// Identical to [`ENGLISH`](crate::ENGLISH) in every respect
    /// except its stemmer.
    pub const ENGLISH_PORTER2: English = English::new().with_porter2();

    /// The singleton [`English`] language pack with the
    /// [`ContractionTokenizer`](crate::ContractionTokenizer) wired
    /// into [`Language::tokenize`](stringcheese_lang::Language::tokenize).
    ///
    /// Identical to [`ENGLISH`](crate::ENGLISH) in every respect
    /// except its tokenizer. Uses the
    /// [`STANDARD`](crate::ContractionTokenizer::STANDARD) preset
    /// (contractions are split but fragments are preserved). Callers
    /// who want the [`NORMALIZED`](crate::ContractionTokenizer::NORMALIZED)
    /// expansion behaviour should reach for
    /// [`ContractionTokenizer::NORMALIZED`](crate::ContractionTokenizer::NORMALIZED)
    /// directly rather than going through the [`Language`] trait.
    pub const ENGLISH_WITH_CONTRACTIONS: English = English::new().with_contraction_tokenizer();
}

#[cfg(feature = "alloc")]
pub use pack::{ENGLISH, ENGLISH_PORTER2, ENGLISH_WITH_CONTRACTIONS, English};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("en")`) find English without
// naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs. The default-stemmer `ENGLISH` constant is
// what lands in the registry; callers who want Porter2 keep reaching
// for `ENGLISH_PORTER2` directly.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ENGLISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-en` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
