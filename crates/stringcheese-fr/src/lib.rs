//! French language pack for the StringCheese toolkit.
//!
//! A zero-sized [`French`] value that carries the French stopword
//! list, the [`FrenchSnowball`] stemmer, the elision-aware
//! [`FrenchTokenizer`], and a [`Phonex`] phonetic hookup. Callers grab
//! the singleton [`FRENCH`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-fr` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the French stopword table
//! or the Snowball stemmer's code. Callers who need French add
//! `stringcheese-fr = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball French stemmer.** Martin Porter's own French
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/french/stemmer.html>. The
//!   reference for French IR stemmers; Lucene's `FrenchAnalyzer`
//!   composes it (plus an elision filter that echoes what
//!   [`FrenchTokenizer`] does here).
//! * **~200 word stopword list.** The intersection of NLTK's `french`,
//!   the Snowball project's `french/stop.txt`, and Lucene's French
//!   analyzer. Includes both apostrophe-suffixed clitics (`l'`, `d'`,
//!   `qu'`) and stripped-clitic forms (`l`, `d`, `qu`) so both
//!   tokenizer conventions recognize them.
//! * **PHONEX phonetic encoder.** A Soundex-shaped encoder with
//!   French-tuned preprocessing (`PH → F`, `GN → N`, `CH → X`,
//!   `QU → K`, `Y → I`, `W → V`, plus accent folding). Chosen over
//!   Métaphone Français because Soundex's fixed-width 4-character key
//!   plays well with the phonetic subsystem's `String` return type
//!   and because the classic Soundex-shape output is easier to
//!   reference-test than Métaphone's variable-length key. See
//!   [`phonetic`] for the algorithm.
//! * **Elision-aware tokenizer.** Splits `l'homme` into `["l'",
//!   "homme"]` — keeping the apostrophe attached to the clitic so a
//!   downstream detokenizer can round-trip by simple concatenation.
//!   Keeps compounds like `aujourd'hui` together. See
//!   [`tokenizer`] for the rule.
//! * **Default Unicode collation.** French uses a specific
//!   accent-sensitive collation (last accent wins, in the standard
//!   Unicode CLDR French tailoring), but the shipped
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   accessor returns `None` — the pack does not carry the CLDR
//!   tailoring data. Callers who need French collation should reach
//!   for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Français.** A parallel encoder with a
//!   variable-length key; better for record-linkage precision, but
//!   heavier to reference-test and out of scope for the initial
//!   drop.
//! * **CLDR-tailored French collator.** Depends on an ICU-backed
//!   data table we deliberately don't ship in the language pack.
//! * **Verlan / familiar-register handling.** No pack today ships
//!   verlan detokenization or slang-lexicon extensions; these are a
//!   downstream concern.
//! * **Aspirated `h` lexicon.** The tokenizer is orthography-driven
//!   — it doesn't know that `le haricot` never elides to `l'haricot`.
//!   A future lexicon-backed override could reject invalid elisions.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_fr::FRENCH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(FRENCH.code(), "fr");
//! assert_eq!(FRENCH.name(), "French");
//! assert!(FRENCH.is_stopword("le"));
//! assert!(FRENCH.is_stopword("qu'"));
//! assert!(!FRENCH.is_stopword("fromage"));
//! assert_eq!(FRENCH.stem("continuer"), "continu");
//!
//! let toks: Vec<&str> = FRENCH
//!     .tokenize("L'homme qui aimait aujourd'hui.")
//!     .collect();
//! assert_eq!(toks, ["L'", "homme", "qui", "aimait", "aujourd'hui"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`FrenchSnowball`] stemmer.
//! - [`phonetic`] — [`Phonex`] plus the [`PhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`FrenchTokenizer`] elision-aware splitter.
//! - The [`French`] type and the [`FRENCH`] constant live in this
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
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{Phonex, PhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::FrenchSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::{FrenchTokenizer, FrenchTokens};

// -----------------------------------------------------------------------
// The French language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PhonexAdapter;
    use crate::snowball::FrenchSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::FrenchTokenizer;

    /// The French language pack.
    ///
    /// Zero-sized; construct as [`French`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`FRENCH`](crate::FRENCH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct French;

    /// The static [`PhonexAdapter`] [`French`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: PhonexAdapter = PhonexAdapter;

    impl Language for French {
        fn code(&self) -> &'static str {
            "fr"
        }

        fn name(&self) -> &'static str {
            "French"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            FrenchSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(FrenchTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`French`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`French`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const FRENCH: French = French;
}

#[cfg(feature = "alloc")]
pub use pack::{FRENCH, French};

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-fr` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
