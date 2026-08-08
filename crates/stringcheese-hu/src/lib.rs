//! Hungarian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Hungarian`] value that carries the Hungarian stopword
//! list, the [`HungarianSnowball`] stemmer, the
//! whitespace-and-punctuation [`HungarianTokenizer`], and a
//! [`HungarianPhonex`] phonetic hookup. Callers grab the singleton
//! [`HUNGARIAN`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-hu` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Hungarian stopword table
//! or the Snowball Hungarian stemmer's code. Callers who need
//! Hungarian add `stringcheese-hu = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! # Hungarian orthography and morphology at a glance
//!
//! Hungarian is a **Uralic** language — non-Indo-European, related to
//! Finnish and Estonian — with rich **agglutinative** morphology and
//! strict **vowel harmony**. Its alphabet extends the Latin block with
//! long and short vowel pairs plus a handful of umlaut-marked vowels
//! with their long counterparts:
//!
//! | Short | Long |
//! |-------|------|
//! | `a`   | `á`  |
//! | `e`   | `é`  |
//! | `i`   | `í`  |
//! | `o`   | `ó`  |
//! | `ö`   | `ő`  |
//! | `u`   | `ú`  |
//! | `ü`   | `ű`  |
//!
//! Consonantal orthography further introduces digraphs `cs, dz, gy,
//! ly, ny, sz, ty, zs` and the single trigraph `dzs`. These are
//! single-letter graphemes for collation purposes but two/three ASCII
//! scalars in UTF-8.
//!
//! ## Vowel harmony
//!
//! Hungarian classifies vowels by **backness**:
//!
//! - **Front vowels**: `e, é, i, í, ö, ő, ü, ű`
//! - **Back vowels**:  `a, á, o, ó, u, ú`
//!
//! Suffixes come in **harmonizing pairs** (occasionally triplets when
//! rounded / unrounded splits): the inessive "in" is `-ban` after a
//! back-vowel stem (`ház → házban` "in the house") and `-ben` after a
//! front-vowel stem (`kert → kertben` "in the garden"). The allative
//! "to" is a **triplet** `-hoz / -hez / -höz`, splitting the front
//! series along roundedness (`kép → képhez`, `kör → körhöz`).
//!
//! This pack encodes vowel harmony **inside the suffix table** rather
//! than as a runtime predicate: every surface variant of each suffix
//! is listed as its own literal entry (`-ban`, `-ben`, `-hoz`, `-hez`,
//! `-höz`, …). The stemmer's runtime job is a longest-match
//! surface-form search — the harmony rule is baked into the table's
//! shape, not the algorithm.
//!
//! # Design choices
//!
//! * **Snowball Hungarian stemmer.** The algorithm documented at
//!   <https://snowballstem.org/algorithms/hungarian/stemmer.html>.
//!   Hungarian is agglutinative — a single orthographic word can carry
//!   case, possessive, plural, and verb suffixes stacked — so the
//!   algorithm strips iteratively across the case-ending, owner
//!   (possessive), plural, and verb-suffix categories. See [`snowball`]
//!   for the rules and the deferred-item list.
//! * **~180-word stopword list.** Drawn from the union of published
//!   Hungarian stopword collections. Covers personal, demonstrative,
//!   and interrogative pronouns, coordinating and subordinating
//!   conjunctions, common postpositions, the high-frequency
//!   conjugations of the copula *lenni* / *van*, common adverbs, and
//!   quantifiers. See [`stopwords`].
//! * **PHONEX-Hungarian phonetic encoder.** A 4-character Soundex-shape
//!   key with Hungarian-tuned preprocessing (digraph rewrites
//!   `cs → C`, `sz → S`, `zs → Z'`, `gy → G'`, `ny → N'`, `ty → T'`,
//!   `ly → J`, `dz → Z`, `dzs → J`; long-vowel fold to short vowels;
//!   umlaut-fold to base vowel for the interior) over a Hungarian-tuned
//!   classification table. Adapter name: `"phonex-hu"`. See
//!   [`phonetic`] for the mapping table and rationale.
//! * **Simple tokenizer.** Hungarian is whitespace-and-punctuation
//!   delimited; every letter of the Hungarian alphabet (including the
//!   long-vowel and umlaut-vowel variants) satisfies
//!   [`char::is_alphanumeric`]. [`HungarianTokenizer`] is a transparent
//!   wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Unicode case fold in `is_stopword`.** The default trait
//!   implementation uses [`str::eq_ignore_ascii_case`] which would
//!   miss uppercase Hungarian-specific vowels (`Á`, `É`, `Í`, `Ó`,
//!   `Ö`, `Ő`, `Ú`, `Ü`, `Ű`). Hungarian overrides
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   with a Unicode lowercase pass.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Hungarian collation is locale-tailored — the
//!   digraphs `cs`, `dz`, `dzs`, `gy`, `ly`, `ny`, `sz`, `ty`, `zs`
//!   sort as single letters at their own position in the alphabet
//!   (e.g. `cs` sorts after `c` and before `d`, `sz` sorts after `s`
//!   and before `t`) — a tailoring the standard CLDR Hungarian
//!   collation encodes. This pack does not carry the CLDR tailoring
//!   data; callers who need Hungarian collation should reach for
//!   `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Full Snowball parity on the vocab file.** The reference-pair
//!   test embeds a hand-traced set that exercises each family of
//!   suffixes and the front/back harmony contract; full-corpus
//!   cross-verification (Snowball ships `voc.txt` / `output.txt`) is a
//!   follow-up.
//! * **Verb-conjugation lemmatization.** Reducing `mentem → menni`,
//!   `voltam → lenni` needs a lexicon; the shipped stemmer is a
//!   suffix-stripping algorithm.
//! * **Compound-word segmentation.** Hungarian writes noun compounds
//!   as a single word (`asztalitenisz` "table tennis"); a compound
//!   splitter would need a lexicon. Out of scope.
//! * **Definite / indefinite verb-conjugation awareness.** Hungarian
//!   verbs conjugate for definiteness of their direct object; the
//!   shipped stemmer treats both series as surface-form suffix strips.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_hu::HUNGARIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(HUNGARIAN.code(), "hu");
//! assert_eq!(HUNGARIAN.name(), "Hungarian");
//! assert!(HUNGARIAN.is_stopword("és"));
//! assert!(HUNGARIAN.is_stopword("ÉS")); // Unicode fold: ÉS → és.
//! assert!(!HUNGARIAN.is_stopword("kutya"));
//! assert_eq!(HUNGARIAN.stem("házban"), "ház");
//! assert_eq!(HUNGARIAN.stem("kertben"), "kert");
//!
//! let toks: Vec<&str> = HUNGARIAN
//!     .tokenize("Szia, világ! Budapest szép város.")
//!     .collect();
//! assert_eq!(toks, ["Szia", "világ", "Budapest", "szép", "város"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`HungarianSnowball`] stemmer.
//! - [`phonetic`] — [`HungarianPhonex`] plus the
//!   [`HungarianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`HungarianTokenizer`] wrapper.
//! - The [`Hungarian`] type and the [`HUNGARIAN`] constant live in
//!   this crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`. Same pattern as
// the other language packs.
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
pub use phonetic::{HungarianPhonex, HungarianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::HungarianSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::HungarianTokenizer;

// -----------------------------------------------------------------------
// The Hungarian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::HungarianPhonexAdapter;
    use crate::snowball::HungarianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::HungarianTokenizer;

    /// The Hungarian language pack.
    ///
    /// Zero-sized; construct as [`Hungarian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`HUNGARIAN`](crate::HUNGARIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Hungarian;

    /// The static [`HungarianPhonexAdapter`] [`Hungarian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static PHONEX: HungarianPhonexAdapter = HungarianPhonexAdapter;

    /// Normalize a Hungarian string for stopword comparison: lowercase
    /// under default Unicode rules. Handles the Hungarian-specific
    /// vowels `Á É Í Ó Ö Ő Ú Ü Ű` correctly (each folds to its
    /// lowercase counterpart with no locale tailoring required).
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Hungarian {
        fn code(&self) -> &'static str {
            "hu"
        }

        fn name(&self) -> &'static str {
            "Hungarian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Hungarian-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Hungarian-specific vowel like `Á`, `É`, `Í`, `Ó`, `Ö`, `Ő`,
        /// `Ú`, `Ü`, `Ű`) with a Unicode lowercase pass — so `ÉS` and
        /// `és` both find `és` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            HungarianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(HungarianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Hungarian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Hungarian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const HUNGARIAN: Hungarian = Hungarian;
}

#[cfg(feature = "alloc")]
pub use pack::{HUNGARIAN, Hungarian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("hu")`) find Hungarian
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(HUNGARIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-hu` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
