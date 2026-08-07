//! Dutch language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Dutch`] value that carries the Dutch stopword list,
//! the [`DutchSnowball`] stemmer, the whitespace-and-punctuation
//! [`DutchTokenizer`], and a [`DutchPhonex`] phonetic hookup. Callers
//! grab the singleton [`DUTCH`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-nl` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Dutch stopword table or
//! the Snowball stemmer's code. Callers who need Dutch add
//! `stringcheese-nl = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Dutch stemmer.** Martin Porter's own Dutch algorithm,
//!   documented at
//!   <https://snowballstem.org/algorithms/dutch/stemmer.html>. The
//!   reference for Dutch IR stemmers; Lucene's `DutchAnalyzer` and
//!   Elasticsearch's `dutch` analyzer both compose it. Includes the
//!   full four-step cascade (standard suffix / e-ending / heid /
//!   derivational) plus the short-vowel-double `-aa/-ee/-oo/-uu`
//!   undouble step, the initial-`y` / `y`-after-vowel / `i`-between-
//!   vowels glide-marking prelude, and the `gem` guard on the `-en`
//!   strip.
//! * **~160-word stopword list.** Drawn from the Snowball project's
//!   `dutch/stop.txt` — the intersection of the ranked most-frequent
//!   forms with the full paradigms of `zijn` / `hebben` / `zullen` /
//!   `worden` / `kunnen` / `moeten` / `mogen` / `willen` / `doen`.
//! * **PHONEX-Dutch phonetic encoder.** A Soundex-shaped 4-character
//!   encoder with Dutch-tuned preprocessing (`ij → i`, `sch → sX`,
//!   `ch → g`, diaeresis / acute fold, silent `H`) and a Dutch-tuned
//!   classification table (labial `B/P/F/V/W`, velar-palatal
//!   `C/K/G/Q/J/X`, dental `D/T`, sibilant `S/Z`). See [`phonetic`]
//!   for the algorithm.
//! * **Simple tokenizer.** Dutch, like German, is whitespace-and-
//!   punctuation delimited and requires no elision-splitting pass —
//!   [`DutchTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Netherlands Dutch as default.** The Snowball algorithm is
//!   neutral between Netherlands Dutch (nl-NL) and Belgian Dutch
//!   (nl-BE); the shared standard vocabulary of the Snowball
//!   `dutch/stop.txt` distribution serves both varieties. The BCP-47
//!   code registered is the base `"nl"`; regional variants are
//!   deferred.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Dutch collation follows the standard Unicode
//!   root; callers who need a locale-tailored collator should reach
//!   for `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Belgian Dutch (nl-BE) / Afrikaans variants.** Regional-variant
//!   stopword extensions and Flemish-specific verb / spelling
//!   differences. Afrikaans is close enough to warrant its own
//!   language pack rather than a mode toggle.
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Dutch.** A parallel encoder with a variable-length
//!   key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Compound-noun splitting.** Dutch productively compounds nouns
//!   (`boek + winkel → boekwinkel`, `stad + huis → stadhuis`).
//!   Splitting them needs a compound-noun dictionary and is not part
//!   of the Snowball algorithm.
//! * **Verb-conjugation lemmatization.** Reducing `ben → zijn`,
//!   `beter → goed` requires a lexicon; the shipped stemmer is a
//!   suffix-stripper only.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_nl::DUTCH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(DUTCH.code(), "nl");
//! assert_eq!(DUTCH.name(), "Dutch");
//! assert!(DUTCH.is_stopword("de"));
//! assert!(DUTCH.is_stopword("en"));
//! assert!(!DUTCH.is_stopword("kaas"));
//! assert_eq!(DUTCH.stem("lopen"), "lop");
//! assert_eq!(DUTCH.stem("boeken"), "boek");
//!
//! let toks: Vec<&str> = DUTCH
//!     .tokenize("De kat slaapt op de mat.")
//!     .collect();
//! assert_eq!(toks, ["De", "kat", "slaapt", "op", "de", "mat"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`DutchSnowball`] stemmer.
//! - [`phonetic`] — [`DutchPhonex`] plus the [`DutchPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`DutchTokenizer`] wrapper.
//! - The [`Dutch`] type and the [`DUTCH`] constant live in this
//!   crate's root.

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
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{DutchPhonex, DutchPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::DutchSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::DutchTokenizer;

// -----------------------------------------------------------------------
// The Dutch language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::DutchPhonexAdapter;
    use crate::snowball::DutchSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::DutchTokenizer;

    /// The Dutch language pack.
    ///
    /// Zero-sized; construct as [`Dutch`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`DUTCH`](crate::DUTCH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Dutch;

    /// The static [`DutchPhonexAdapter`] [`Dutch`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: DutchPhonexAdapter = DutchPhonexAdapter;

    impl Language for Dutch {
        fn code(&self) -> &'static str {
            "nl"
        }

        fn name(&self) -> &'static str {
            "Dutch"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            DutchSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(DutchTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Dutch`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Dutch`] every time — the type is zero-sized, so the two forms
    /// are equivalent, but the constant is the intended entry point
    /// and matches the pattern every other `stringcheese-<lang>` pack
    /// follows.
    pub const DUTCH: Dutch = Dutch;
}

#[cfg(feature = "alloc")]
pub use pack::{DUTCH, Dutch};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("nl")`) find Dutch without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(DUTCH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-nl` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
