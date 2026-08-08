//! Romanian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Romanian`] value that carries the Romanian stopword
//! list, the [`RomanianSnowball`] stemmer, the whitespace-and-punctuation
//! [`RomanianTokenizer`], and a [`RomanianPhonex`] phonetic hookup.
//! Callers grab the singleton [`ROMANIAN`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ro` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Romanian stopword table
//! or the Snowball stemmer's code. Callers who need Romanian add
//! `stringcheese-ro = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Romanian — first Balkan Romance pack
//!
//! Romanian sits at a linguistic crossroads. Genealogically it is a
//! Romance language — a direct descendant of Vulgar Latin, sibling of
//! Spanish, French, Portuguese, and Italian — but geographically it
//! grew up inside the **Balkan Sprachbund**, sharing centuries of
//! contact with Bulgarian, Macedonian, Albanian, and Modern Greek.
//! That contact left signature Balkan features on Romanian that its
//! Romance cousins lack:
//!
//! * **Postposed definite article.** Where Spanish writes `el libro`
//!   ("the book" as two words) and French `le livre`, Romanian writes
//!   `cartea` — the definite article is a **suffix** on the noun, not
//!   a separate word. This is the same pattern Bulgarian
//!   (`книгата`) and Macedonian (`книгата`) exhibit. The Snowball
//!   Romanian stemmer's **Step 0** strips these postposed articles
//!   first (`-ul`/`-ului`/`-ei`/`-elor`/…), so `cartea` and `carte`
//!   both stem to the same root.
//! * **Case marking retained.** Every other Romance language lost the
//!   Latin case system; Romanian kept a reduced form of it —
//!   nominative/accusative merge, but genitive/dative are distinct
//!   (`unui băiat` "of/to a boy" vs. `un băiat` "a boy"). The stemmer
//!   handles this by stripping the genitive/dative singular suffixes
//!   (`-lui`, `-i`, `-ii`) and the genitive plural (`-lor`) in the
//!   article-stripping step.
//! * **Preserved Latin morphology on verbs.** The four-way conjugation
//!   distinction (`-a`, `-ea`, `-e`, `-i` infinitive classes) survives
//!   with all the endings the algorithm has to handle: `-ăm`/`-ați`
//!   present, `-am`/`-ai`/`-a` imperfect, `-esc`/`-ești`/`-ește`
//!   `-i`-class present, etc.
//!
//! # Diacritics: comma-below vs. cedilla
//!
//! Romanian uses two letters that render as consonant-plus-diacritic:
//! `ș` (s with comma below, U+0219) and `ț` (t with comma below,
//! U+021B). Historically these were represented with **cedilla**
//! (`ş` U+015F and `ţ` U+0163) because early Unicode versions and
//! most fonts lacked the comma-below glyphs. Modern Romanian
//! orthography (per the Romanian Academy and Unicode 3.0+) uses the
//! comma-below forms; the cedilla forms remain in circulation on
//! older documents, older fonts, and text authored with tooling that
//! defaults to Turkish/Latin-2 keyboards.
//!
//! **This pack folds cedilla to comma-below at every entry point.**
//! The stemmer, the phonetic encoder, and the (default) stopword
//! lookup all normalize `ş → ș` and `ţ → ț` before any downstream
//! work; a caller can index a corpus authored with cedilla forms and
//! query it with comma-below forms (or vice versa) and the two will
//! collide as expected.
//!
//! # Implementation choices
//!
//! * **Snowball Romanian stemmer.** Martin Porter's official Romanian
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/romanian/stemmer.html>. The
//!   reference stemmer used across Romanian IR pipelines (Lucene's
//!   `RomanianAnalyzer`, Elasticsearch's `romanian` analyzer,
//!   `snowballstemmer` (Python), NLTK's
//!   `SnowballStemmer("romanian")` — all descend from the same
//!   `romanian.sbl` source). Ported faithfully with cedilla / comma-
//!   below folding as a preprocessing pass.
//! * **~130-word stopword list.** Drawn from the intersection of the
//!   Snowball project's `romanian/stop.txt`, Lucene's Romanian
//!   analyzer, and a hand-audited head. Covers articles (postposed
//!   forms handled separately by the stemmer, so this list carries
//!   the free-standing forms only), prepositions, coordinating
//!   conjunctions, personal / possessive / demonstrative pronouns,
//!   and the high-frequency conjugations of `a fi` "to be", `a avea`
//!   "to have", `a face` "to do/make".
//! * **PHONEX-Romanian phonetic encoder.** A Soundex-shaped
//!   4-character key with Romanian-tuned preprocessing: fold
//!   diacritics (`ă`/`â`/`î` → `A`/`I`), fold `ș → S`, fold `ț → T`
//!   (both cedilla and comma-below forms); handle the Romance
//!   `ch`/`gh` digraph convention (Romanian writes `ch` before front
//!   vowel to spell hard `/k/`, `gh` before front vowel to spell
//!   hard `/g/`); silent `H` after `C`/`G`. Adapter name `"phonex-ro"`.
//! * **Simple tokenizer.** Romanian orthography is whitespace-and-
//!   punctuation delimited with no clitic elision (no French `l'`, no
//!   Italian `dell'`); the `SimpleTokenizer` wrapper suffices.
//!   Romanian's diacritic letters (`ă â î ș ț` and their cedilla
//!   variants) all satisfy `char::is_alphanumeric`, so they stay
//!   word-internal.
//! * **Default Unicode collation.** Romanian has a well-established
//!   sort order (`... a ă â b c d ... i î ... s ș t ț ... z`) that
//!   diverges from the code-point ordering, but that tailoring lives
//!   in `icu_collator` — this pack does not carry the CLDR tailoring
//!   data, and [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Român.** A variable-length parallel encoder; better
//!   for record-linkage precision, heavier to reference-test.
//! * **CLDR-tailored Romanian collator.** Depends on an ICU-backed
//!   data table this pack deliberately does not ship.
//! * **Moldovan sibling (`stringcheese-ro-MD` or `stringcheese-mo`).**
//!   Moldovan is essentially Romanian written in Cyrillic (pre-1989)
//!   or Latin (post-1989); the Latin variant is orthographically
//!   identical to Romanian and covered by this pack. Cyrillic
//!   Moldovan is deferred.
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   ships `voc.txt` / `output.txt` reference files with thousands of
//!   pairs; the shipped
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   embeds a subset.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ro::ROMANIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ROMANIAN.code(), "ro");
//! assert_eq!(ROMANIAN.name(), "Romanian");
//! assert!(ROMANIAN.is_stopword("și"));
//! assert!(ROMANIAN.is_stopword("în"));
//! assert!(!ROMANIAN.is_stopword("brânză"));
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`RomanianSnowball`] stemmer.
//! - [`phonetic`] — [`RomanianPhonex`] plus the
//!   [`RomanianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`RomanianTokenizer`] wrapper.
//! - The [`Romanian`] type and the [`ROMANIAN`] constant live in this
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
pub use phonetic::{RomanianPhonex, RomanianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::{RomanianSnowball, fold_cedilla_to_comma_below};
pub use stopwords::STOPWORDS;
pub use tokenizer::RomanianTokenizer;

// -----------------------------------------------------------------------
// The Romanian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::RomanianPhonexAdapter;
    use crate::snowball::RomanianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::RomanianTokenizer;

    /// The Romanian language pack.
    ///
    /// Zero-sized; construct as [`Romanian`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`ROMANIAN`](crate::ROMANIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Romanian;

    /// The static [`RomanianPhonexAdapter`] [`Romanian`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: RomanianPhonexAdapter = RomanianPhonexAdapter;

    impl Language for Romanian {
        fn code(&self) -> &'static str {
            "ro"
        }

        fn name(&self) -> &'static str {
            "Romanian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Overrides the default trait implementation to fold the
        /// legacy **cedilla** forms `ş`/`ţ` (U+015F / U+0163) to the
        /// modern **comma-below** forms `ș`/`ț` (U+0219 / U+021B)
        /// before comparing against the stopword list. The stopword
        /// list is stored in comma-below form.
        ///
        /// Downstream callers who tokenize text authored on older
        /// systems (which typically emit cedilla forms) can pass those
        /// tokens straight through to `is_stopword` without a manual
        /// normalization pass.
        fn is_stopword(&self, word: &str) -> bool {
            // Fast path: no cedilla in the input → fall through to the
            // default ASCII-case-insensitive scan.
            if !word
                .chars()
                .any(|c| c == 'ş' || c == 'Ş' || c == 'ţ' || c == 'Ţ')
            {
                return STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(word));
            }
            let folded: String = word.chars().map(fold_cedilla_char).collect();
            STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(&folded))
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            RomanianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(RomanianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Romanian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Romanian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const ROMANIAN: Romanian = Romanian;

    /// Fold a single scalar from the legacy Romanian cedilla form to
    /// the modern comma-below form. Every other scalar passes through
    /// unchanged.
    ///
    /// Kept as a helper here (rather than reaching into
    /// [`fold_cedilla_to_comma_below`]) so that the trait-level
    /// override can act on a `char` iterator without building an
    /// intermediate `String` unless a cedilla actually appears in the
    /// input.
    #[inline]
    fn fold_cedilla_char(c: char) -> char {
        match c {
            'ş' => 'ș',
            'Ş' => 'Ș',
            'ţ' => 'ț',
            'Ţ' => 'Ț',
            other => other,
        }
    }
}

#[cfg(feature = "alloc")]
pub use pack::{ROMANIAN, Romanian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("ro")`) find Romanian without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ROMANIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ro` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
