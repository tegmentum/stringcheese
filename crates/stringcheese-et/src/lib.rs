//! Estonian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Estonian`] value that carries the Estonian stopword
//! list, the [`EstonianStemmer`] suffix-stripping stemmer, the
//! whitespace-and-punctuation [`EstonianTokenizer`], and an
//! [`EstonianPhonex`] phonetic hookup. Callers grab the singleton
//! [`ESTONIAN`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language) trait.
//!
//! # Second Uralic (non-Indo-European) language pack
//!
//! Estonian is the workspace's second **Uralic** language pack after
//! [`stringcheese-fi`](../stringcheese_fi/index.html) (Finnish) — a
//! close sibling in the Finnic branch of Uralic. The two languages
//! share substantial structure but diverge in critical ways that any
//! pack-level implementation has to acknowledge:
//!
//! * **Agglutinative morphology.** Like Finnish, Estonian stacks case,
//!   number, and clitic suffixes on a stem, though the average
//!   Estonian orthographic word is shorter than the Finnish
//!   equivalent (Estonian has largely lost Finnish's stacked
//!   possessive suffixes — possession is expressed with a separate
//!   pronoun instead).
//! * **Fourteen grammatical cases** (Finnish has fifteen — Estonian
//!   dropped the instructive but retains almost the same inventory
//!   under different names). The Estonian case endings this pack
//!   strips: allative `-le`, ablative `-lt`, translative `-ks`,
//!   terminative `-ni`, essive `-na`, comitative `-ga`, abessive
//!   `-ta`, illative `-sse`, elative `-st`, inessive `-s`, plural
//!   genitive `-de`, plus the plural nominative marker `-d` and the
//!   plural partitive markers `-id` / `-te`.
//! * **No vowel harmony.** This is the biggest simplification versus
//!   Finnish. Estonian lost native vowel harmony centuries ago —
//!   modern Standard Estonian permits any vowel combination inside a
//!   word. The stemmer therefore does not enumerate back / front
//!   harmony variants of each suffix, and the phonetic encoder does
//!   not need harmony-aware preprocessing.
//! * **Estonian orthography is highly phonetic.** Like Finnish,
//!   Estonian was standardized to be a near-1:1 grapheme-to-phoneme
//!   mapping. A simple encoder mapping diacritics and digraphs to
//!   ASCII is sufficient — no digraph rewrites like English `ph → f`
//!   or complex silent-letter rules.
//! * **Diacritic set.** Estonian carries `ä` (U+00E4), `ö` (U+00F6),
//!   `ü` (U+00FC), `õ` (U+00F5), plus `š` (U+0161) and `ž` (U+017E)
//!   in loanwords. Notably: **no `å`** (unlike Finnish), and **`õ`
//!   is Estonian's signature letter** — a close-mid back unrounded
//!   vowel /ɤ/ that is unique to Estonian among European languages.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-et` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Estonian stopword table
//! or the stemmer's code. Callers who need Estonian add
//! `stringcheese-et = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Lightweight suffix-stripping stemmer.** There is **no official
//!   Snowball Estonian algorithm** (Snowball's catalogue lists no
//!   `estonian.sbl`). The shipped stemmer is a hand-audited
//!   longest-match suffix stripper inspired by academic references,
//!   covering the productive case endings, plural markers, common
//!   verb-conjugation suffixes, and the diminutive `-ke` / `-kene`.
//!   It runs a single pass over the suffix table with a 2-character
//!   min-stem floor. See [`stemmer`].
//! * **~90-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, conjunctions, the copular *olema* "to
//!   be" and its high-frequency conjugations, negation forms,
//!   quantifiers, and common adverbs. See [`stopwords`].
//! * **PHONEX-Estonian phonetic encoder.** A light 4-character
//!   Soundex-shape encoder with Estonian-tuned preprocessing (long-
//!   vowel and long-consonant collapse; `ä → a`, `ö → o`, `ü → u`,
//!   `õ → o`, `š → s`, `ž → z` folds). Chosen small because
//!   Estonian orthography is already almost phonetic. See
//!   [`phonetic`].
//! * **Simple tokenizer.** Estonian has no clitic elision like French
//!   nor compound morpheme boundaries a tokenizer could see. Its
//!   orthography is whitespace-and-punctuation delimited, and every
//!   Estonian special letter is alphabetic under Unicode's
//!   classification. [`EstonianTokenizer`] is a transparent wrapper
//!   around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer); the
//!   stemmer handles suffix stripping.
//! * **Default Unicode case-fold.** Estonian has *no* locale-specific
//!   case-fold quirks — every letter's case pair is a Unicode
//!   default. The pack's
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   implementation nonetheless overrides the default trait method
//!   (which uses [`str::eq_ignore_ascii_case`]) to apply Unicode
//!   case-fold, so inputs like `SINA`, `KÜLL`, `ÕNNETU` match the
//!   lowercase stopword entries.
//!
//! # Deferred to a follow-up wave
//!
//! * **Consonant-gradation reversal.** Estonian, like Finnish, has
//!   consonant gradation (`raamat` → `raamatu` "book", `laps` →
//!   `lapse` "child"). The full alternation lexicon is deferred.
//! * **Vowel-alternation reversal.** Estonian's stem-vowel changes
//!   (`kool` → `koolid` "school → schools" with no vowel change vs.
//!   `käsi` → `käed` "hand → hands" with vowel loss) require a
//!   lexicon.
//! * **Full-corpus reference cross-verification.** The shipped
//!   [`tests/stemmer_reference.rs`](../../tests/stemmer_reference.rs)
//!   test embeds a subset that exercises every suffix category's
//!   happy path.
//! * **Lemmatization.** Reducing inflected verb forms to their
//!   canonical *-ma* infinitive requires a lexicon.
//! * **Compound-word splitting.** Estonian forms noun–noun compounds
//!   productively (`raamatukogu` "library" = `raamatu` "book" +
//!   `kogu` "collection"). Splitting these needs a lexicon.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_et::ESTONIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ESTONIAN.code(), "et");
//! assert_eq!(ESTONIAN.name(), "Estonian");
//! assert!(ESTONIAN.is_stopword("ja"));
//! assert!(ESTONIAN.is_stopword("SEE"));  // Unicode case-fold.
//! assert!(!ESTONIAN.is_stopword("raamat"));
//! assert_eq!(ESTONIAN.stem("majas"), "maja");
//!
//! let toks: Vec<&str> = ESTONIAN
//!     .tokenize("Tere, maailm! Tallinn on ilus.")
//!     .collect();
//! assert_eq!(toks, ["Tere", "maailm", "Tallinn", "on", "ilus"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`EstonianStemmer`] suffix stripper.
//! - [`phonetic`] — [`EstonianPhonex`] plus the
//!   [`EstonianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`EstonianTokenizer`] wrapper.
//! - The [`Estonian`] type and the [`ESTONIAN`] constant live in this
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
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{EstonianPhonex, EstonianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::EstonianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::EstonianTokenizer;

// -----------------------------------------------------------------------
// The Estonian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::EstonianPhonexAdapter;
    use crate::stemmer::EstonianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::EstonianTokenizer;

    /// The Estonian language pack.
    ///
    /// Zero-sized; construct as [`Estonian`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`ESTONIAN`](crate::ESTONIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Estonian;

    /// The static [`EstonianPhonexAdapter`] [`Estonian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static PHONEX: EstonianPhonexAdapter = EstonianPhonexAdapter;

    impl Language for Estonian {
        fn code(&self) -> &'static str {
            "et"
        }

        fn name(&self) -> &'static str {
            "Estonian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Unicode-case-fold stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`]) so that stopwords carrying
        /// non-ASCII letters (`ä`, `ö`, `ü`, `õ`, `š`, `ž`) match
        /// their uppercase forms. Estonian has no locale-specific
        /// case-fold quirks — the default Unicode fold is correct for
        /// every Estonian letter.
        fn is_stopword(&self, word: &str) -> bool {
            let lowered: alloc::string::String = word
                .chars()
                .flat_map(|c| c.to_lowercase().collect::<alloc::vec::Vec<_>>())
                .collect();
            STOPWORDS.iter().any(|s| *s == lowered)
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            EstonianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(EstonianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Estonian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Estonian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const ESTONIAN: Estonian = Estonian;
}

#[cfg(feature = "alloc")]
pub use pack::{ESTONIAN, Estonian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("et")`) find Estonian
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ESTONIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-et` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
