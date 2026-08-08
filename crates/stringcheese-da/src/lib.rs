//! Danish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Danish`] value that carries the Danish stopword list,
//! the [`DanishSnowball`] stemmer, the whitespace-and-punctuation
//! [`DanishTokenizer`], and a [`DanishPhonex`] phonetic hookup. Callers
//! grab the singleton [`DANISH`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Danish-specific letters
//!
//! The Danish alphabet extends the 26-letter Latin base with three
//! letters at the end: `æ` (`ash`), `ø` (`o-slash`), and `å` (`a-ring`).
//! All three occur in high-frequency vocabulary (`være` "to be", `ønske`
//! "to wish", `får` "gets"). The Snowball vowel set includes all three,
//! and the PHONEX preprocessor folds them for phonetic key collapse
//! (see [`phonetic`]).
//!
//! Danish shares its extended letter set with Norwegian Bokmål; only
//! the vocabulary and morphology differ. Historically Danish used `aa`
//! for what is now `å`, and some proper names still preserve the
//! digraph (`Aalborg`, `Aabenraa`). The default tokenizer treats those
//! as ordinary letter sequences; a caller who wants `aa → å`
//! normalization should apply it before tokenizing.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-da` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Danish stopword table or
//! the Snowball stemmer's code. Callers who need Danish add
//! `stringcheese-da = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Danish stemmer.** Martin Porter's Snowball Danish
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/danish/stemmer.html>. The
//!   reference stemmer used across Danish IR pipelines; Lucene's
//!   `DanishAnalyzer` and Elasticsearch's `danish` analyzer both
//!   descend from the same `danish.sbl` source. Four-step cascade:
//!   (1) main-suffix (nominal / adjectival / verb inflections plus the
//!   valid-s-ending `-s` strip); (2) consonant-pair `-gd` / `-dt` /
//!   `-gt` / `-kt` trailing-letter strip in R1; (3) derivational
//!   suffix (`ig` / `lig` / `elig` / `els` delete plus `-igst → -ig`
//!   prelude and `løst → løs` rewrite); (4) undouble trailing repeated
//!   consonant. R1 region adjusted so it never begins before char 3.
//! * **~120-word stopword list.** Drawn from the Snowball project's
//!   `danish/stop.txt` — the intersection of the ranked most-frequent
//!   forms with the full paradigms of the copula `være` / `have` /
//!   `kunne` / `ville` / `skulle` / `måtte` / `burde`.
//! * **PHONEX-Danish phonetic encoder.** A Soundex-shaped 4-character
//!   encoder with Danish-tuned preprocessing (`sj → S`, `sk` before
//!   front vowels → `S`, `ch → S`, silent `h`, plus the `å → o` /
//!   `æ → e` / `ø → e` letter folds) and the standard PHONEX
//!   classification table. See [`phonetic`] for the algorithm.
//! * **Simple tokenizer.** Danish, like German / Swedish / Norwegian,
//!   is whitespace-and-punctuation delimited and requires no elision-
//!   splitting pass — [`DanishTokenizer`] is a transparent wrapper
//!   around [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Danish collation places `æ`, `ø`, `å` after `z`
//!   (in that order) — the standard Unicode root does not; callers who
//!   need a locale-tailored collator should reach for `icu_collator`
//!   via a [`stringcheese_lang::Collator`] impl of their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Danish.** A parallel encoder with a variable-length
//!   key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Compound-noun splitting.** Danish, like German and Norwegian,
//!   productively compounds nouns (`børne + have → børnehave`).
//!   Splitting them needs a compound-noun dictionary and is not part
//!   of the Snowball algorithm.
//! * **`aa → å` normalization.** Historical Danish spellings and some
//!   proper names preserve the `aa` digraph; a normalization pre-pass
//!   is deferred to a caller-supplied step.
//! * **Icelandic (`stringcheese-is`).** The other North Germanic
//!   Latin-script language; adds `þ ð` and a much richer inflectional
//!   morphology that would need its own algorithm.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_da::DANISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(DANISH.code(), "da");
//! assert_eq!(DANISH.name(), "Danish");
//! assert!(DANISH.is_stopword("og"));
//! assert!(DANISH.is_stopword("er"));
//! assert!(!DANISH.is_stopword("fisk"));
//!
//! let toks: Vec<&str> = DANISH
//!     .tokenize("Katten sover på måtten.")
//!     .collect();
//! assert_eq!(toks, ["Katten", "sover", "på", "måtten"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`DanishSnowball`] stemmer.
//! - [`phonetic`] — [`DanishPhonex`] plus the [`DanishPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`DanishTokenizer`] wrapper.
//! - The [`Danish`] type and the [`DANISH`] constant live in this
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
pub use phonetic::{DanishPhonex, DanishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::DanishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::DanishTokenizer;

// -----------------------------------------------------------------------
// The Danish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::DanishPhonexAdapter;
    use crate::snowball::DanishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::DanishTokenizer;

    /// The Danish language pack.
    ///
    /// Zero-sized; construct as [`Danish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`DANISH`](crate::DANISH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Danish;

    /// The static [`DanishPhonexAdapter`] [`Danish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: DanishPhonexAdapter = DanishPhonexAdapter;

    impl Language for Danish {
        fn code(&self) -> &'static str {
            "da"
        }

        fn name(&self) -> &'static str {
            "Danish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            DanishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(DanishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Danish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Danish`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const DANISH: Danish = Danish;
}

#[cfg(feature = "alloc")]
pub use pack::{DANISH, Danish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("da")`) find Danish without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(DANISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-da` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
